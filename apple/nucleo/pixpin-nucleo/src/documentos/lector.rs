//! **Lo del lector que no es pintar** (tanda B7): cambiar el nombre con F2
//! (`renombrar_doc.rs` de Windows), juntar la tinta con lo que cambio en el
//! disco mientras se anotaba (`lector_pdf/al_dia.rs`) y la pagina que sube
//! sola (`lector.rs`, las cuentas de `pixpin_docs::auto_desplazar`).
//!
//! # El nombre
//!
//! Como en Windows y en el movil, el nombre que cambia es **el del mensaje
//! del chat** del que sale el documento, no el del fichero: el fichero de un
//! chat es compartido con los otros aparatos por la sincronizacion (su
//! `ruta` viaja en el mensaje) y moverlo lo dejaria huerfano alli. Un
//! documento **suelto**, de fuera del almacen, si se renombra en el disco, y
//! con el **sus hermanos** (`<nombre>.pixpin-lectura`, la carpeta
//! `<nombre>.pixpin-anotado` con la tinta), que cuelgan de su nombre. Lo que
//! esta dentro del almacen sin ser de ningun mensaje no se toca.
//!
//! # Juntar con el disco
//!
//! El lector guarda la tinta de cada hoja en memoria. Si otro la escribe con
//! el lector abierto (la sincronizacion con el movil, el asistente) y aqui
//! hay algo a medias, guardar lo pisaria: se junta con la regla de la
//! sincronizacion (`fusion::json`, figura por figura), con lo que se leyo
//! como base, igual que `plano_cad::anotar::juntar_con_el_disco` de Windows.

use std::path::{Path, PathBuf};

use pixpin_docs::auto_desplazar as auto;
use pixpin_proyecto::almacen;
use pixpin_proyecto::cuaderno::{self, Cuaderno, Mensaje};
use pixpin_proyecto::vista;

use crate::chats::{self, CambioMensaje};
use crate::{ErrorNucleo, Nucleo};

/// Cuantas letras admite el nombre, como el campo del movil.
const LARGO_MAXIMO: usize = 80;

/// Lo que paso al cambiar el nombre de un documento desde el lector.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum DocumentoRenombrado {
    /// Lo escrito esta vacio o es el mismo nombre: no se hizo nada.
    SinCambios,
    /// Cambio el nombre del mensaje del chat; el fichero sigue donde estaba.
    EnElChat { nombre: String },
    /// Se movio el fichero suelto (y sus hermanos) a `ruta`.
    EnDisco { nombre: String, ruta: String },
    /// Ya hay un fichero con ese nombre al lado: no se pisa.
    YaExiste { nombre: String },
    /// Es del almacen pero de ningun mensaje que se sepa: no se toca.
    NoSePuede,
}

/// **El nombre nuevo**, a partir de lo escrito y del de antes, o `None` si
/// no hay que cambiar nada (vacio, igual que antes o un nombre que Windows
/// no admite). Se limpia de lo que Windows no admite en un nombre de
/// fichero (el documento viaja alli) y lleva la extension de antes
/// (`conSuExtension` del movil).
pub(crate) fn nombre_nuevo(escrito: &str, antes: &str) -> Option<String> {
    let limpio: String = escrito
        .chars()
        .filter(|c| *c != '\n' && *c != '\r')
        .take(LARGO_MAXIMO)
        .map(|c| {
            if "\\/:*?\"<>|".contains(c) || c < ' ' {
                '-'
            } else {
                c
            }
        })
        .collect();
    // Windows no admite un nombre que acabe en punto o en espacio.
    let limpio = limpio.trim().trim_end_matches(['.', ' ']).trim();
    if limpio.is_empty() {
        return None;
    }
    let base = limpio
        .split('.')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_uppercase();
    let reservado = matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((base.starts_with("COM") || base.starts_with("LPT"))
            && base.len() == 4
            && base.as_bytes()[3].is_ascii_digit());
    if reservado {
        return None;
    }
    let nuevo = con_su_extension(limpio, antes);
    (nuevo != antes).then_some(nuevo)
}

/// La extension de antes, si la tenia y es de las de verdad (cinco letras o
/// menos), y si lo escrito no la lleva ya.
fn con_su_extension(nuevo: &str, antes: &str) -> String {
    let ext = antes.rsplit_once('.').map_or("", |(_, e)| e);
    if ext.is_empty() || ext.chars().count() > 5 {
        return nuevo.to_string();
    }
    if nuevo
        .to_lowercase()
        .ends_with(&format!(".{}", ext.to_lowercase()))
    {
        nuevo.to_string()
    } else {
        format!("{nuevo}.{ext}")
    }
}

fn canonica(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// De quien es un documento.
enum Dueno {
    /// Un mensaje del chat cuya carpeta es `carpeta`.
    Chat { carpeta: PathBuf, mensaje: Mensaje },
    /// Un fichero de fuera del almacen.
    Suelto,
    /// Dentro del almacen, pero de ningun mensaje que se sepa.
    DelAlmacen,
}

/// **De quien es `doc`** (`renombrar_doc::dueno` de Windows): primero si
/// esta en la carpeta de un proyecto (subiendo hasta su `guardados.jsonl`,
/// tambien uno guardado fuera del almacen); si no, si lo senala algun
/// mensaje de los chats del almacen (lo que llego del movil vive aparte).
fn dueno(raiz: &Path, doc: &Path) -> Dueno {
    let doc = canonica(doc);
    let es_este = |carpeta: &Path, ficha: Option<&str>, m: &Mensaje| {
        let Some(r) = m.ruta.as_deref() else {
            return false;
        };
        let real = match ficha {
            Some(f) => vista::ruta_real(raiz, f, r),
            None => Some(carpeta.join(r)),
        };
        real.is_some_and(|p| canonica(&p) == doc)
    };
    for carpeta in doc.ancestors().skip(1).take(4) {
        if carpeta.join("guardados.jsonl").is_file() {
            let Ok(c) = Cuaderno::leer_de(carpeta) else {
                return Dueno::DelAlmacen;
            };
            return match c.mensajes.into_iter().find(|m| es_este(carpeta, None, m)) {
                Some(mensaje) => Dueno::Chat {
                    carpeta: carpeta.to_path_buf(),
                    mensaje,
                },
                None => Dueno::DelAlmacen,
            };
        }
    }
    if !doc.starts_with(canonica(raiz)) {
        return Dueno::Suelto;
    }
    for f in almacen::Indice::leer(raiz).proyectos {
        let carpeta = almacen::carpeta(raiz, &f.id);
        let Ok(c) = Cuaderno::leer_de(&carpeta) else {
            continue;
        };
        if let Some(mensaje) = c
            .mensajes
            .into_iter()
            .find(|m| es_este(&carpeta, Some(&f.id), m))
        {
            return Dueno::Chat { carpeta, mensaje };
        }
    }
    Dueno::DelAlmacen
}

/// Mueve el fichero y todo lo que cuelga de su nombre (`<nombre>.pixpin-*`).
/// El fichero primero: si falla no se ha tocado nada. Un hermano que no se
/// deja mover no deshace lo demas: lo peor es perder por donde se iba.
fn mover_con_hermanos(ruta: &Path, destino: &Path) -> std::io::Result<()> {
    std::fs::rename(ruta, destino)?;
    let (Some(carpeta), Some(viejo), Some(nuevo)) = (
        ruta.parent(),
        ruta.file_name().map(|n| n.to_string_lossy().to_string()),
        destino.file_name().map(|n| n.to_string_lossy().to_string()),
    ) else {
        return Ok(());
    };
    let prefijo = format!("{viejo}.pixpin-");
    let Ok(entradas) = std::fs::read_dir(carpeta) else {
        return Ok(());
    };
    for e in entradas.flatten() {
        let nombre = e.file_name().to_string_lossy().to_string();
        if let Some(resto) = nombre.strip_prefix(&prefijo) {
            let _ = std::fs::rename(e.path(), carpeta.join(format!("{nuevo}.pixpin-{resto}")));
        }
    }
    Ok(())
}

/// **Junta la tinta de una hoja** (`mio`, lo de aqui) con la que hay ahora
/// en el disco (`suyo`), siendo `base` la que se leyo al abrirla (`None`
/// si no se sabe: entonces se junta todo, que perder algo es peor que tener
/// de mas). `None` si alguna no se entiende: entonces vale lo de aqui.
#[uniffi::export]
pub fn juntar_tinta_con_el_disco(
    base: Option<String>,
    mio: String,
    suyo: String,
) -> Option<String> {
    use pixpin_sincro::canonico::Json;
    use pixpin_sincro::fusion;
    // Los tres por el mismo camino: un mismo numero escrito de dos maneras
    // (`1.0E-7`, `1e-7`) seria un cambio que no hizo nadie.
    let json = |t: &str| {
        serde_json::from_str::<serde_json::Value>(t)
            .ok()
            .map(|v| Json::de_valor(&v))
    };
    let (mio, suyo) = (json(&mio)?, json(&suyo)?);
    let base = base.as_deref().and_then(json);
    let junto = fusion::json(
        base.as_ref(),
        Some(&mio),
        Some(&suyo),
        &fusion::Criterio::default(),
        &mut fusion::Cuenta::default(),
    )?;
    Some(junto.a_texto())
}

/// Las palabras de un texto: lo que hay entre blancos (para la pagina que
/// sube sola).
#[uniffi::export]
pub fn contar_palabras(texto: String) -> u64 {
    auto::contar_palabras(&texto) as u64
}

/// Un paso mas rapido (`sentido` +1) o mas lento (−1) de la pagina que sube
/// sola, en palabras por minuto, redondeando al paso y entre el minimo y el
/// maximo.
#[uniffi::export]
pub fn otra_velocidad_de_leer(ppm: u32, sentido: i32) -> u32 {
    auto::otra(ppm, sentido)
}

/// **Cuanto hay que bajar por segundo**, en las unidades de `alto`, para
/// leer `ppm` palabras por minuto un documento de `palabras` palabras.
#[uniffi::export]
pub fn velocidad_de_bajar(ppm: u32, palabras: u64, alto: f32) -> f32 {
    auto::pixeles_por_segundo(ppm, palabras as usize, alto)
}

/// **«Terminas en 12 min · a las 18:40»** o «Terminado» (`lector-auto-*` de
/// Windows): `progreso` de 0 a 1 y `ahora_en_minutos` los de hoy desde las
/// 00:00 en la hora de este equipo.
#[uniffi::export]
pub fn rotulo_de_lo_que_queda(
    palabras: u64,
    progreso: f32,
    ppm: u32,
    ahora_en_minutos: i64,
) -> String {
    let minutos = auto::minutos_que_quedan(palabras as usize, progreso, ppm);
    if minutos <= 0.0 {
        return "Terminado".into();
    }
    let cuanto = auto::rotulo_con(minutos, "menos de 1 min", "");
    let hora = auto::hora_de_terminar(ahora_en_minutos, minutos);
    format!("Terminas en {cuanto} · a las {hora}")
}

/// **Un color de la tinta sobre el papel** (`contraste::adaptar` del motor,
/// lo que hace Windows al pintar la tinta del lector en negativo o en
/// oscuro): si ya se lee (3:1) se queda; si no, se le cambia la claridad
/// conservando el tono. Devuelve `[r, g, b, a]` de 0 a 1.
#[uniffi::export]
pub fn tinta_sobre_papel(color: Vec<f32>, papel: Vec<f32>) -> Vec<f32> {
    use pixpin_motor2d::contraste::adaptar;
    use pixpin_motor2d::elemento::ColorRgba;
    let c = |v: &[f32], k: usize, por_defecto: f32| v.get(k).copied().unwrap_or(por_defecto);
    let a = adaptar(
        ColorRgba {
            r: c(&color, 0, 0.0),
            g: c(&color, 1, 0.0),
            b: c(&color, 2, 0.0),
            a: c(&color, 3, 1.0),
        },
        ColorRgba {
            r: c(&papel, 0, 1.0),
            g: c(&papel, 1, 1.0),
            b: c(&papel, 2, 1.0),
            a: 1.0,
        },
    );
    vec![a.r, a.g, a.b, a.a]
}

#[uniffi::export]
impl Nucleo {
    /// **La velocidad de la pagina que sube sola** recordada en este equipo
    /// (`palabras_por_minuto` del estado, como Windows; 230 la de fabrica).
    pub fn palabras_por_minuto(&self) -> u32 {
        auto::valida(
            pixpin_store::estado::cargar(&self.ubicacion())
                .palabras_por_minuto
                .unwrap_or(auto::POR_DEFECTO),
        )
    }

    /// Guarda la velocidad sin tocar lo demas que se recuerda.
    pub fn guardar_palabras_por_minuto(&self, ppm: u32) -> Result<(), ErrorNucleo> {
        let u = self.ubicacion();
        let mut e = pixpin_store::estado::cargar(&u);
        e.palabras_por_minuto = Some(auto::valida(ppm));
        pixpin_store::estado::guardar(&u, &e)?;
        Ok(())
    }

    /// **El modo oscuro de los visores** (luna/sol de la barra): uno para
    /// todos, como en Windows (`lectores_oscuros` del estado). `None` es
    /// «nunca se toco»: entonces manda el aspecto del Mac.
    pub fn lectores_oscuros(&self) -> Option<bool> {
        pixpin_store::estado::cargar(&self.ubicacion()).lectores_oscuros
    }

    pub fn guardar_lectores_oscuros(&self, oscuros: bool) -> Result<(), ErrorNucleo> {
        let u = self.ubicacion();
        let mut e = pixpin_store::estado::cargar(&u);
        e.lectores_oscuros = Some(oscuros);
        pixpin_store::estado::guardar(&u, &e)?;
        Ok(())
    }

    /// **El nombre que se ensena** de un documento en el lector: el del
    /// mensaje si es de un chat (el que el usuario le puso alli), el del
    /// fichero si no.
    pub fn nombre_de_documento(&self, documento: String) -> String {
        let doc = PathBuf::from(&documento);
        match dueno(self.ruta(), &doc) {
            Dueno::Chat { mensaje, .. } if !mensaje.nombre.trim().is_empty() => mensaje.nombre,
            _ => doc
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or(documento),
        }
    }

    /// **Cambia el nombre de un documento desde el lector** (F2): el del
    /// mensaje si es de un chat, el del fichero (con sus hermanos) si es
    /// suelto. `escrito` es lo tecleado; la extension de antes se conserva.
    /// Bloqueante.
    pub fn renombrar_documento(
        &self,
        documento: String,
        escrito: String,
    ) -> Result<DocumentoRenombrado, ErrorNucleo> {
        let doc = PathBuf::from(&documento);
        let raiz = self.ruta();
        match dueno(raiz, &doc) {
            Dueno::Chat { carpeta, mensaje } => {
                let Some(nuevo) = nombre_nuevo(&escrito, &mensaje.nombre) else {
                    return Ok(DocumentoRenombrado::SinCambios);
                };
                let cambio = CambioMensaje::Nombre {
                    nombre: nuevo.clone(),
                };
                let mut cache = self.cache_tomada();
                let hecho =
                    cuaderno::cambiar(&carpeta, &mensaje.id, |m| chats::aplicar(m, &cambio))?;
                cache.quitar(&carpeta.join("guardados.jsonl"));
                Ok(match hecho {
                    Some(m) if m.nombre != mensaje.nombre => {
                        DocumentoRenombrado::EnElChat { nombre: m.nombre }
                    }
                    _ => DocumentoRenombrado::SinCambios,
                })
            }
            Dueno::DelAlmacen => Ok(DocumentoRenombrado::NoSePuede),
            Dueno::Suelto => {
                let antes = doc
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let Some(nuevo) = nombre_nuevo(&escrito, &antes) else {
                    return Ok(DocumentoRenombrado::SinCambios);
                };
                let destino = doc.with_file_name(&nuevo);
                // Solo cambian mayusculas: en un disco que no las distingue
                // es el mismo fichero y `exists` diria que si.
                let mismo = destino.to_string_lossy().to_lowercase()
                    == doc.to_string_lossy().to_lowercase();
                if !mismo && destino.exists() {
                    return Ok(DocumentoRenombrado::YaExiste { nombre: nuevo });
                }
                mover_con_hermanos(&doc, &destino)?;
                Ok(DocumentoRenombrado::EnDisco {
                    nombre: nuevo,
                    ruta: destino.display().to_string(),
                })
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::sync::Arc;

    use pixpin_proyecto::almacen::{Ficha, Indice};
    use pixpin_proyecto::cuaderno::{Clase, Sello};

    fn raiz(etiqueta: &str) -> PathBuf {
        let r = std::env::temp_dir().join(format!(
            "pixpin-nucleo-lector-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(&r).unwrap();
        r
    }

    fn nucleo(r: &Path) -> Arc<Nucleo> {
        Nucleo::abrir(r.display().to_string(), "Mac".into()).unwrap()
    }

    #[test]
    fn el_nombre_nuevo_lleva_su_extension_y_se_limpia_como_en_windows() {
        assert_eq!(
            nombre_nuevo("Memoria", "informe.pdf").as_deref(),
            Some("Memoria.pdf")
        );
        assert_eq!(
            nombre_nuevo("Memoria.PDF", "informe.pdf").as_deref(),
            Some("Memoria.PDF")
        );
        assert_eq!(
            nombre_nuevo("a/b:c", "x.docx").as_deref(),
            Some("a-b-c.docx")
        );
        assert_eq!(nombre_nuevo("fin. ", "x.pdf").as_deref(), Some("fin.pdf"));
        // Casos negativos: vacio, igual, reservado de Windows.
        assert_eq!(nombre_nuevo("   ", "x.pdf"), None);
        assert_eq!(nombre_nuevo("x", "x.pdf"), None);
        assert_eq!(nombre_nuevo("CON", "x.pdf"), None);
        assert_eq!(nombre_nuevo("com1.txt", "x.pdf"), None);
        // Una extension larga no es extension.
        assert_eq!(nombre_nuevo("b", "a.excalidraw").as_deref(), Some("b"));
    }

    #[test]
    fn un_documento_suelto_se_renombra_con_sus_hermanos() {
        let r = raiz("suelto");
        let fuera = raiz("suelto-doc");
        let pdf = fuera.join("tema.pdf");
        std::fs::write(&pdf, b"%PDF-1.4").unwrap();
        std::fs::write(fuera.join("tema.pdf.pixpin-lectura"), "zoom=1").unwrap();
        std::fs::create_dir_all(fuera.join("tema.pdf.pixpin-anotado")).unwrap();
        std::fs::write(
            fuera.join("tema.pdf.pixpin-anotado/hoja-1.excalidraw"),
            "{}",
        )
        .unwrap();
        std::fs::write(fuera.join("otro.pdf"), b"%PDF").unwrap();
        let n = nucleo(&r);
        assert_eq!(n.nombre_de_documento(pdf.display().to_string()), "tema.pdf");
        let hecho = n
            .renombrar_documento(pdf.display().to_string(), "Tema 3".into())
            .unwrap();
        let nuevo = fuera.join("Tema 3.pdf");
        assert_eq!(
            hecho,
            DocumentoRenombrado::EnDisco {
                nombre: "Tema 3.pdf".into(),
                ruta: nuevo.display().to_string()
            }
        );
        assert!(nuevo.is_file() && !pdf.exists());
        assert!(fuera.join("Tema 3.pdf.pixpin-lectura").is_file());
        assert!(
            fuera
                .join("Tema 3.pdf.pixpin-anotado/hoja-1.excalidraw")
                .is_file()
        );
        // Caso negativo: no se pisa otro fichero.
        let otra_vez = n
            .renombrar_documento(nuevo.display().to_string(), "otro".into())
            .unwrap();
        assert_eq!(
            otra_vez,
            DocumentoRenombrado::YaExiste {
                nombre: "otro.pdf".into()
            }
        );
        assert!(nuevo.is_file());
        let _ = std::fs::remove_dir_all(&fuera);
    }

    #[test]
    fn un_adjunto_del_chat_cambia_el_nombre_del_mensaje_y_no_el_fichero() {
        let r = raiz("chat");
        let ficha = Ficha::nueva("Tesis", 5, "PC01");
        let mut i = Indice::default();
        i.proyectos.push(ficha.clone());
        i.guardar(&r).unwrap();
        let ruta = almacen::guardar_adjunto(&r, &ficha.id, "informe.pdf", b"%PDF-1.4").unwrap();
        let sello = Sello {
            cuando: 10,
            numero: 1,
            aparato: "PC01".into(),
            proyecto: ficha.id.clone(),
        };
        let carpeta = almacen::carpeta(&r, &ficha.id);
        let m = Mensaje::adjunto(Clase::Archivo, "informe.pdf", &ruta, 2, &sello);
        cuaderno::anadir(&carpeta, &m).unwrap();
        let pdf = carpeta.join(&ruta);
        let n = nucleo(&r);
        let doc = pdf.display().to_string();
        let hecho = n
            .renombrar_documento(doc.clone(), "Memoria".into())
            .unwrap();
        assert_eq!(
            hecho,
            DocumentoRenombrado::EnElChat {
                nombre: "Memoria.pdf".into()
            }
        );
        assert!(pdf.is_file(), "el fichero sigue donde estaba");
        assert_eq!(n.nombre_de_documento(doc.clone()), "Memoria.pdf");
        let c = Cuaderno::leer_de(&carpeta).unwrap();
        assert_eq!(c.mensajes[0].nombre, "Memoria.pdf");
        // Caso negativo: el mismo nombre no reescribe nada.
        assert_eq!(
            n.renombrar_documento(doc, "Memoria".into()).unwrap(),
            DocumentoRenombrado::SinCambios
        );
        // Y lo del almacen que no es de ningun mensaje no se toca.
        let huerfano = carpeta.join("suelto.pdf");
        std::fs::write(&huerfano, b"%PDF").unwrap();
        assert_eq!(
            n.renombrar_documento(huerfano.display().to_string(), "x".into())
                .unwrap(),
            DocumentoRenombrado::NoSePuede
        );
        assert!(huerfano.is_file());
    }

    fn trazo(id: &str, x: f64) -> serde_json::Value {
        serde_json::json!({
            "id": id, "type": "freedraw", "x": x, "y": 100, "width": 50, "height": 0,
            "strokeWidth": 2, "version": 1, "versionNonce": 7, "updated": 1,
            "isDeleted": false, "points": [[0.0, 0.0], [50.0, 0.0]]
        })
    }

    fn lienzo(elementos: Vec<serde_json::Value>) -> String {
        serde_json::json!({"type": "excalidraw", "version": 2, "elements": elementos}).to_string()
    }

    #[test]
    fn la_tinta_de_aqui_y_la_que_llego_al_disco_se_juntan_sin_perder_nada() {
        let base = lienzo(vec![trazo("comun", 0.0)]);
        let mio = lienzo(vec![trazo("comun", 0.0), trazo("mio", 10.0)]);
        let suyo = lienzo(vec![trazo("comun", 0.0), trazo("del-movil", 20.0)]);
        let junto = juntar_tinta_con_el_disco(Some(base), mio.clone(), suyo.clone()).unwrap();
        for id in ["comun", "mio", "del-movil"] {
            assert!(junto.contains(&format!("\"{id}\"")), "falta {id}: {junto}");
        }
        // Sin base tambien junta (se queda con todo).
        let sin_base = juntar_tinta_con_el_disco(None, mio.clone(), suyo).unwrap();
        assert!(sin_base.contains("\"mio\"") && sin_base.contains("\"del-movil\""));
        // Caso negativo: lo del disco no se entiende y no se junta nada.
        assert_eq!(juntar_tinta_con_el_disco(None, mio, "{roto".into()), None);
    }

    #[test]
    fn la_velocidad_de_leer_se_recuerda_y_las_cuentas_son_las_de_windows() {
        let r = raiz("ppm");
        let n = nucleo(&r);
        assert_eq!(n.palabras_por_minuto(), auto::POR_DEFECTO);
        n.guardar_palabras_por_minuto(260).unwrap();
        assert_eq!(nucleo(&r).palabras_por_minuto(), 260);
        // Fuera de los limites se queda en ellos.
        n.guardar_palabras_por_minuto(5).unwrap();
        assert_eq!(n.palabras_por_minuto(), auto::MINIMO);
        assert_eq!(otra_velocidad_de_leer(237, 1), 240);
        assert_eq!(otra_velocidad_de_leer(237, -1), 220);
        assert_eq!(contar_palabras("  hola  mundo\nde nuevo ".into()), 4);
        assert!((velocidad_de_bajar(240, 1200, 6000.0) - 20.0).abs() < 1e-3);
        assert_eq!(velocidad_de_bajar(240, 0, 6000.0), 0.0);
        assert_eq!(
            rotulo_de_lo_que_queda(1200, 0.5, 240, 18 * 60 + 30),
            "Terminas en 3 min · a las 18:33"
        );
        assert_eq!(rotulo_de_lo_que_queda(1200, 1.0, 240, 0), "Terminado");
    }

    #[test]
    fn la_tinta_negra_se_aclara_sobre_el_papel_oscuro_y_el_modo_oscuro_se_recuerda() {
        let negro = vec![0.1, 0.1, 0.1, 1.0];
        let sobre_negro = tinta_sobre_papel(negro.clone(), vec![0.0, 0.0, 0.0]);
        assert!(sobre_negro[0] > 0.5, "se aclara: {sobre_negro:?}");
        assert_eq!(sobre_negro[3], 1.0);
        // Caso negativo: sobre papel blanco se lee y no se toca.
        assert_eq!(tinta_sobre_papel(negro.clone(), vec![1.0, 1.0, 1.0]), negro);
        let r = raiz("oscuro");
        let n = nucleo(&r);
        n.guardar_lectores_oscuros(false).unwrap();
        assert_eq!(nucleo(&r).lectores_oscuros(), Some(false));
        n.guardar_lectores_oscuros(true).unwrap();
        assert_eq!(n.lectores_oscuros(), Some(true));
    }
}
