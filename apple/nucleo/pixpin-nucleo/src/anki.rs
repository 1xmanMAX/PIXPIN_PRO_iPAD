//! **«Enviar a Anki»** del menu de un mensaje (Windows 2561be0,
//! `ventana_chat/anki.rs`). Las tarjetas salen de `pixpin_anki::tarjetas`:
//!
//! - **Una clase grabada** (nota de voz con capturas, [`crate::clase`]): una
//!   tarjeta por captura, con la imagen delante y lo dicho en ese tramo
//!   detras.
//! - **Una foto**: la imagen delante y su descripcion detras.
//! - **Una nota**: sus «pregunta :: respuesta», «P: / R:» y huecos
//!   `{{c1::…}}`.
//!
//! Van al mazo `PixPin::<proyecto>` con las etiquetas `pixpin` y la del
//! proyecto. Con Anki abierto (y AnkiConnect, `127.0.0.1:8765`), directas;
//! si no, un fichero para «Archivo → Importar» en `<raiz>/anki/`, y las
//! imagenes en la carpeta de medios del perfil de Anki si se encuentra (en
//! el Mac, `~/Library/Application Support/Anki2/<perfil>/collection.media`).
//!
//! Lo que se le dice al usuario lo pone Swift con los textos de Windows
//! (`anki-hecho`, `anki-fichero`...), a partir de [`EnvioAnki`].

use std::path::{Path, PathBuf};

use pixpin_anki::{Imagen, Tarjeta, tarjetas};
use pixpin_proyecto::almacen::{self, Indice};
use pixpin_proyecto::cuaderno::{self, Clase, Mensaje};

use crate::chats::validar_chat;
use crate::{ErrorNucleo, Nucleo};

/// Como acabo «Enviar a Anki».
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum EnvioAnki {
    /// El mensaje no da ninguna tarjeta (`anki-nada`).
    Nada,
    /// `n` tarjetas nuevas en el mazo (`anki-hecho`).
    Hecho { n: u32, mazo: String },
    /// Ya estaban todas (`anki-ya-estaban`).
    YaEstaban { n: u32, mazo: String },
    /// Sin Anki abierto: el fichero para importar (`anki-fichero`).
    Fichero { n: u32, mazo: String, ruta: String },
    /// Anki no las admitio (`anki-fallo`).
    Fallo { motivo: String },
}

/// Si una nota da alguna tarjeta (para ofrecer «Enviar a Anki» en su menu).
#[uniffi::export]
pub fn anki_hay_tarjetas_en_nota(texto: String) -> bool {
    !tarjetas::de_la_nota(&texto).is_empty()
}

/// El mazo de un proyecto: `::` es el separador de submazos de Anki, dentro
/// del nombre no.
pub fn mazo_de(nombre: &str) -> String {
    format!("PixPin::{}", nombre.trim().replace("::", ":"))
}

/// El titulo de un audio: su nombre sin la extension
/// (`biblioteca_audio::titulo_de_audio` de Windows).
pub fn titulo_de_audio(m: &Mensaje) -> String {
    let nombre = m.nombre.trim();
    match nombre.rsplit_once('.') {
        Some((base, _)) if !base.is_empty() => base.to_string(),
        _ => nombre.to_string(),
    }
}

/// El milisegundo de un `[1:23] texto` y el texto sin la marca
/// (`pixpin_voz::tiempo_de`, copiado al caracter).
pub fn tiempo_de(linea: &str) -> Option<(i64, &str)> {
    let recortada = linea.trim_start();
    let resto = recortada.strip_prefix('[')?;
    let (dentro, despues) = resto.split_once(']')?;
    let mut partes = dentro.split(':');
    let a = partes.next()?;
    let b = partes.next()?;
    let c = partes.next();
    if partes.next().is_some() {
        return None;
    }
    let numero = |t: &str| -> Option<i64> {
        if t.is_empty() || !t.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        t.parse().ok()
    };
    let dos = |t: &str| if t.len() == 2 { numero(t) } else { None };
    let (h, m, s) = match c {
        Some(c) => (numero(a)?, dos(b)?, dos(c)?),
        None => (0, numero(a)?, dos(b)?),
    };
    if m > 59 || s > 59 {
        return None;
    }
    Some((
        (h * 3600 + m * 60 + s) * 1000,
        despues.strip_prefix(' ').unwrap_or(despues),
    ))
}

/// Los trozos de una transcripcion con su minuto (`voz::trozos` de
/// Windows): parrafos separados por una linea en blanco.
pub fn trozos(texto: &str) -> Vec<(Option<i64>, String)> {
    let texto = texto.replace("\r\n", "\n");
    let mut v = Vec::new();
    let mut bloque = String::new();
    let cerrar = |bloque: &mut String, v: &mut Vec<(Option<i64>, String)>| {
        let limpio = bloque.trim();
        if !limpio.is_empty() {
            v.push(match tiempo_de(limpio) {
                Some((ms, resto)) => (Some(ms), resto.trim().to_string()),
                None => (None, limpio.to_string()),
            });
        }
        bloque.clear();
    };
    for linea in texto.split('\n') {
        if linea.trim().is_empty() {
            cerrar(&mut bloque, &mut v);
        } else {
            if !bloque.is_empty() {
                bloque.push('\n');
            }
            bloque.push_str(linea);
        }
    }
    cerrar(&mut bloque, &mut v);
    v
}

/// Un trozo sin las marcas de Markdown que no aportan (`#`, `**`, `__`) ni
/// las imagenes (`voz::sin_marcas` de Windows).
pub fn sin_marcas(texto: &str) -> String {
    texto
        .lines()
        .filter(|l| !l.trim_start().starts_with("!["))
        .map(|l| {
            l.trim_start_matches(['#', ' '])
                .replace("**", "")
                .replace("__", "")
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Las tarjetas del mensaje `m` de un chat llamado `proyecto`. `imagen`
/// lee la foto de un mensaje (`None` si no esta en este aparato).
pub fn tarjetas_de(
    m: &Mensaje,
    mensajes: &[Mensaje],
    proyecto: &str,
    pregunta_de_la_clase: &str,
    imagen: impl Fn(&Mensaje) -> Option<Imagen>,
) -> Vec<Tarjeta> {
    let mut v = match m.clase {
        Some(Clase::Voz) => {
            let dichos: Vec<(i64, String)> = m
                .transcripcion
                .as_deref()
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(trozos)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|(ms, t)| Some((ms?, sin_marcas(&t))))
                .collect();
            let capturas: Vec<(i64, Imagen)> = crate::clase::capturas_de(m)
                .into_iter()
                .filter_map(|(ms, id)| {
                    let c = mensajes.iter().find(|c| c.id == id)?;
                    Some((ms, imagen(c)?))
                })
                .collect();
            tarjetas::de_la_clase(&titulo_de_audio(m), pregunta_de_la_clase, &dichos, capturas)
        }
        Some(Clase::Imagen) => imagen(m)
            .map(|i| vec![tarjetas::de_la_foto(i, &m.texto, &m.nombre)])
            .unwrap_or_default(),
        _ => tarjetas::de_la_nota(&m.texto),
    };
    let etiquetas = vec!["pixpin".to_string(), tarjetas::etiqueta(proyecto)];
    for t in &mut v {
        t.etiquetas = etiquetas.clone();
    }
    v
}

/// Sin Anki abierto: el fichero para importar en `<raiz>/anki/`, y las
/// imagenes en `medios` (la carpeta de medios de Anki) si la hay.
pub fn a_fichero(
    raiz: &Path,
    mazo: &str,
    tarjetas: &[Tarjeta],
    ahora_ms: i64,
    medios: Option<&Path>,
) -> std::io::Result<PathBuf> {
    let carpeta = raiz.join("anki");
    std::fs::create_dir_all(&carpeta)?;
    let nombre: String = mazo
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let ruta = carpeta.join(format!("{nombre}-{ahora_ms}.txt"));
    std::fs::write(&ruta, pixpin_anki::fichero::texto(mazo, tarjetas))?;
    if let Some(medios) = medios {
        for i in tarjetas.iter().flat_map(|t| &t.imagenes) {
            let _ = std::fs::write(medios.join(&i.nombre), &i.bytes);
        }
    }
    Ok(ruta)
}

/// La carpeta de medios del perfil de Anki tocado por ultima vez:
/// `<anki2>/<perfil>/collection.media`.
pub fn medios_en(anki2: &Path) -> Option<PathBuf> {
    std::fs::read_dir(anki2)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path().join("collection.media"))
        .filter(|p| p.is_dir())
        .max_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
}

/// La de este Mac (`~/Library/Application Support/Anki2`), si Anki esta.
fn medios_de_anki() -> Option<PathBuf> {
    let casa = PathBuf::from(std::env::var_os("HOME")?);
    medios_en(&casa.join("Library").join("Application Support").join("Anki2"))
}

#[uniffi::export]
impl Nucleo {
    /// **Manda a Anki** las tarjetas del mensaje `mensaje` de `chat`, como
    /// `anki::enviar` de Windows. `pregunta_de_la_clase` es el texto de
    /// delante de cada captura (`anki-pregunta-clase`). Bloqueante: lee el
    /// cuaderno y habla con Anki por `127.0.0.1`.
    pub fn enviar_a_anki(
        &self,
        chat: String,
        mensaje: String,
        pregunta_de_la_clase: String,
    ) -> Result<EnvioAnki, ErrorNucleo> {
        validar_chat(&chat)?;
        let raiz = self.ruta();
        let proyecto = Indice::leer_si_esta(raiz)?
            .buscar(&chat)
            .map(|f| f.nombre.clone())
            .ok_or_else(|| ErrorNucleo::NoExiste { que: chat.clone() })?;
        let c = cuaderno::Cuaderno::leer_de(&almacen::carpeta(raiz, &chat))?;
        let m = c
            .mensajes
            .iter()
            .find(|m| m.id == mensaje)
            .ok_or_else(|| ErrorNucleo::NoExiste { que: mensaje.clone() })?;
        let leer = |c: &Mensaje| -> Option<Imagen> {
            let ruta = self.resolver(&chat, c.ruta.as_deref()?)?;
            let bytes = std::fs::read(&ruta).ok()?;
            let ext = ruta
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("png")
                .to_ascii_lowercase();
            Some(Imagen {
                // Unico en la carpeta de medios de Anki y estable: mandar
                // dos veces la misma clase no llena Anki de copias.
                nombre: format!("pixpin-{}.{ext}", c.id),
                bytes,
            })
        };
        let tarjetas = tarjetas_de(m, &c.mensajes, &proyecto, &pregunta_de_la_clase, leer);
        if tarjetas.is_empty() {
            return Ok(EnvioAnki::Nada);
        }
        let mazo = mazo_de(&proyecto);
        let n = tarjetas.len() as u32;
        Ok(match pixpin_anki::conectar::anadir(&mazo, &tarjetas) {
            Ok(0) => EnvioAnki::YaEstaban { n, mazo },
            Ok(hechas) => EnvioAnki::Hecho {
                n: hechas as u32,
                mazo,
            },
            Err(pixpin_anki::conectar::Fallo::SinAnki) => {
                let medios = medios_de_anki();
                let ruta = a_fichero(raiz, &mazo, &tarjetas, crate::ahora_ms(), medios.as_deref())?;
                EnvioAnki::Fichero {
                    n,
                    mazo,
                    ruta: ruta.display().to_string(),
                }
            }
            Err(e) => EnvioAnki::Fallo {
                motivo: e.to_string(),
            },
        })
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_mazo_cuelga_de_pixpin_sin_romper_los_submazos() {
        assert_eq!(mazo_de(" Física "), "PixPin::Física");
        assert_eq!(mazo_de("A::B"), "PixPin::A:B");
    }

    #[test]
    fn la_transcripcion_se_parte_en_trozos_con_su_minuto() {
        let t = trozos("[0:00] llamar al **aparejador**\n\n[1:02:09] y pedir\n\nsuelto");
        assert_eq!(
            t,
            vec![
                (Some(0), "llamar al **aparejador**".to_string()),
                (Some(3_729_000), "y pedir".to_string()),
                (None, "suelto".to_string()),
            ]
        );
        assert_eq!(sin_marcas("# Tema\n![audio](x)\n**negrita**"), "Tema\nnegrita");
        // Casos negativos: lo que no es una marca valida no da minuto.
        assert_eq!(tiempo_de("[1:5] no"), None);
        assert_eq!(tiempo_de("[0:75] no"), None);
        assert_eq!(tiempo_de("sin marca"), None);
    }

    #[test]
    fn una_clase_da_una_tarjeta_por_captura_con_su_tramo() {
        let mut voz = Mensaje {
            id: "v".into(),
            nombre: "Clase 24-10 18h05.m4a".into(),
            clase: Some(Clase::Voz),
            transcripcion: Some("[0:00] uno\n\n[0:10] dos\n\n[0:30] tres".into()),
            ..Mensaje::default()
        };
        voz.resto.insert(
            crate::clase::CAMPO_CAPTURAS.into(),
            crate::clase::capturas_en_json(&[(9_000, "a".into()), (25_000, "b".into())]),
        );
        let a = Mensaje {
            id: "a".into(),
            clase: Some(Clase::Imagen),
            ..Mensaje::default()
        };
        let b = Mensaje {
            id: "b".into(),
            clase: Some(Clase::Imagen),
            ..Mensaje::default()
        };
        let todos = vec![voz.clone(), a, b];
        let imagen = |c: &Mensaje| {
            Some(Imagen {
                nombre: format!("pixpin-{}.png", c.id),
                bytes: vec![1],
            })
        };
        let t = tarjetas_de(&voz, &todos, "Física 1", "¿Qué se explicó aquí?", imagen);
        assert_eq!(t.len(), 2);
        assert!(t[0].frente.contains("pixpin-a.png"));
        assert!(t[0].frente.contains("Clase 24-10 18h05 · ⏱ 0:09"));
        assert!(t[0].reverso.starts_with("uno dos"));
        assert_eq!(t[0].etiquetas, vec!["pixpin".to_string(), "Física_1".to_string()]);
        // Caso negativo: una nota de voz sin capturas no da tarjetas.
        let sola = Mensaje {
            clase: Some(Clase::Voz),
            ..Mensaje::default()
        };
        assert!(tarjetas_de(&sola, &[], "x", "?", imagen).is_empty());
        // Y una nota sin preguntas tampoco.
        assert!(!anki_hay_tarjetas_en_nota("un parrafo suelto".into()));
        assert!(anki_hay_tarjetas_en_nota("capital :: Lima".into()));
    }

    #[test]
    fn sin_anki_queda_el_fichero_y_las_imagenes_en_sus_medios() {
        let dir = std::env::temp_dir().join(format!("pixpin-anki-nucleo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let medios = dir.join("Anki2").join("Usuario 1").join("collection.media");
        std::fs::create_dir_all(&medios).unwrap();
        assert_eq!(medios_en(&dir.join("Anki2")), Some(medios.clone()));
        let t = vec![Tarjeta {
            frente: "<img src=\"pixpin-a.png\">".into(),
            reverso: "r".into(),
            etiquetas: vec!["pixpin".into()],
            imagenes: vec![Imagen {
                nombre: "pixpin-a.png".into(),
                bytes: vec![7],
            }],
        }];
        let ruta = a_fichero(&dir, "PixPin::Física", &t, 42, Some(&medios)).unwrap();
        assert_eq!(ruta, dir.join("anki").join("PixPin--Física-42.txt"));
        assert!(std::fs::read_to_string(&ruta).unwrap().contains("#deck:PixPin::Física"));
        assert_eq!(std::fs::read(medios.join("pixpin-a.png")).unwrap(), vec![7]);
        // Caso negativo: sin Anki instalado no hay carpeta de medios.
        assert_eq!(medios_en(&dir.join("no-esta")), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
