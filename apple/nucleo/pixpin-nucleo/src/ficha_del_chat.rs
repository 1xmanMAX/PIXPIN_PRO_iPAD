//! **La ficha del chat**: lo que cuenta y ensena la isla que sale al pulsar
//! el nombre del chat (Windows `ventana_chat/ficha_del_chat.rs`, 25-oct-2026,
//! el usuario: «Mira Telegram: una isla flotante con fotos, videos,
//! archivos, enlaces…»).
//!
//! Aqui solo lo que no es pintar: que pestanas salen y en que orden, cuantas
//! cosas tiene cada una, con cual se abre, que hay en cada una (con lo
//! buscado ya filtrado) y, de cada cosa, si es un video, su enlace y su
//! posicion en el chat (para «Ver en el chat»). Que cuenta cada pestana es
//! `pixpin_proyecto::cuaderno::Seccion` (`indices_de_seccion`, con
//! `es_video` y `tiene_enlace`), el mismo que en Windows y el movil; como
//! se ve, la rejilla y la previa, es de Swift.
//!
//! Como en Windows (`indices_de_la_seccion` de `ventana_chat.rs`): las
//! lecciones no salen (ni su mensaje ni sus fotos y audios), y la galeria
//! son las hojas del `proyecto.json` (por su codigo unico), no una clase.

use pixpin_proyecto::almacen::{self, Indice};
use pixpin_proyecto::cuaderno::{self, Mensaje, Seccion};

use crate::chats::{MensajeChat, validar_chat};
use crate::{ErrorNucleo, Nucleo};

/// Una pestana de la ficha (`cuaderno::Seccion`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum SeccionFicha {
    Galeria,
    Todo,
    Fotos,
    Videos,
    Archivos,
    Enlaces,
    Voz,
    Dibujos,
    Fijados,
    Buzon,
}

/// En que orden salen las pestanas (`ORDEN` de Windows): primero lo que se
/// mira, luego lo que se lee, y «Todo» y el buzon al final.
pub const ORDEN: [SeccionFicha; 10] = [
    SeccionFicha::Galeria,
    SeccionFicha::Fotos,
    SeccionFicha::Videos,
    SeccionFicha::Archivos,
    SeccionFicha::Enlaces,
    SeccionFicha::Voz,
    SeccionFicha::Dibujos,
    SeccionFicha::Fijados,
    SeccionFicha::Todo,
    SeccionFicha::Buzon,
];

impl SeccionFicha {
    fn del_cuaderno(self) -> Seccion {
        match self {
            SeccionFicha::Galeria => Seccion::Galeria,
            SeccionFicha::Todo => Seccion::Todo,
            SeccionFicha::Fotos => Seccion::Fotos,
            SeccionFicha::Videos => Seccion::Videos,
            SeccionFicha::Archivos => Seccion::Archivos,
            SeccionFicha::Enlaces => Seccion::Enlaces,
            SeccionFicha::Voz => Seccion::Voz,
            SeccionFicha::Dibujos => Seccion::Dibujos,
            SeccionFicha::Fijados => Seccion::Fijados,
            SeccionFicha::Buzon => Seccion::Buzon,
        }
    }
}

/// Una pestana con lo que tiene.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PestanaFicha {
    pub seccion: SeccionFicha,
    pub cuenta: u32,
    /// Lo que se mira (fotos, videos, dibujos, hojas) va en rejilla; lo que
    /// se lee, en filas (`Seccion::es_cuadricula`).
    pub cuadricula: bool,
}

/// La cabecera y las pestanas de la ficha.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FichaDelChatLeida {
    /// Las que tienen algo (y la puesta, aunque se haya vaciado), en `ORDEN`.
    pub pestanas: Vec<PestanaFicha>,
    /// Con la que se abre: la primera que tenga algo; en un chat vacio,
    /// «Todo», que dice que no hay nada.
    pub primera: SeccionFicha,
    /// Cuantas cosas tiene el chat («24 cosas»).
    pub cosas: u32,
    /// El codigo unico del chat: el que hay que mirar para saber si lo de
    /// aqui y lo del movil son lo mismo (en el movil sale igual).
    pub codigo: String,
}

/// Un enlace como se ensena en su fila (`tarjetas::Enlace` de Windows).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct EnlaceFicha {
    /// La direccion completa, la que se abre.
    pub url: String,
    /// De donde es: `github.com`.
    pub donde: String,
    /// De que va: el camino con las barras aireadas (`forge / pixpin`).
    pub de_que: String,
}

/// Una cosa de una pestana.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CosaDeLaFicha {
    pub mensaje: MensajeChat,
    /// Su posicion en el chat (0 el mas antiguo), la de `Nucleo::mensajes`:
    /// «Ver en el chat» salta a ella.
    pub posicion: u64,
    pub es_video: bool,
    /// El primer enlace de su texto, si lo lleva.
    pub enlace: Option<EnlaceFicha>,
}

/// Si un mensaje es de una leccion (`lecciones::almacen::es_de_leccion` de
/// Windows): la leccion misma o una foto o un audio suyos (`respondeA:
/// lec-<id>`).
fn es_de_leccion(m: &Mensaje) -> bool {
    crate::lecciones::es_leccion(m)
        || m.responde_a
            .as_deref()
            .is_some_and(|r| r.starts_with(pixpin_lecciones::leccion::PREFIJO))
}

/// **Que mensajes son de una seccion**, por su sitio en `mensajes`: el
/// `indices_de_la_seccion` de Windows. `hojas` son los codigos unicos de las
/// hojas del proyecto (para la galeria).
pub fn de_la_seccion(
    mensajes: &[Mensaje],
    seccion: SeccionFicha,
    hojas: &std::collections::BTreeSet<String>,
) -> Vec<usize> {
    if seccion != SeccionFicha::Galeria {
        let mut v = cuaderno::indices_de_seccion(mensajes, seccion.del_cuaderno());
        v.retain(|&n| !es_de_leccion(&mensajes[n]));
        return v;
    }
    mensajes
        .iter()
        .enumerate()
        .filter(|(_, m)| !m.en_buzon && m.uid.as_deref().is_some_and(|u| hojas.contains(u)))
        .map(|(n, _)| n)
        .collect()
}

/// Las pestanas que salen, con su cuenta: las que tienen algo y, aunque
/// este vacia, la `actual`.
pub fn pestanas(
    cuentas: &[(SeccionFicha, usize)],
    actual: Option<SeccionFicha>,
) -> Vec<PestanaFicha> {
    ORDEN
        .iter()
        .filter_map(|s| {
            let n = cuentas.iter().find(|(x, _)| x == s).map_or(0, |(_, n)| *n);
            (n > 0 || actual == Some(*s)).then(|| PestanaFicha {
                seccion: *s,
                cuenta: n as u32,
                cuadricula: s.del_cuaderno().es_cuadricula(),
            })
        })
        .collect()
}

/// La pestana con la que se abre la ficha.
pub fn primera(cuentas: &[(SeccionFicha, usize)]) -> SeccionFicha {
    ORDEN
        .iter()
        .copied()
        .find(|s| cuentas.iter().any(|(x, n)| x == s && *n > 0))
        .unwrap_or(SeccionFicha::Todo)
}

/// Si lo buscado con la lupa de la ficha esta en su texto o en su nombre,
/// sin mirar mayusculas ni tildes (`hay_coincidencia` de Windows).
pub fn casa_con(m: &Mensaje, buscado: &str) -> bool {
    let aguja = crate::buscar::puntos::normalizar(buscado.trim());
    aguja.is_empty()
        || crate::buscar::puntos::normalizar(&m.resumen()).contains(&aguja)
        || crate::buscar::puntos::normalizar(&m.nombre).contains(&aguja)
}

/// Lo que la puntuacion suele dejar pegado al final sin ser de la direccion.
const PEGADOS_AL_FINAL: &str = ".,;:!?)]}\"'»";

/// **El primer enlace del texto**, como `tarjetas::primer_enlace` de
/// Windows (y `primerEnlace` del movil): sin salir a la red, con lo que la
/// propia direccion dice.
pub fn primer_enlace(texto: &str) -> Option<EnlaceFicha> {
    let crudo = buscar(texto, &["https://", "http://"]).or_else(|| buscar(texto, &["www."]))?;
    let crudo = recortar_lo_pegado(crudo);
    if crudo.is_empty() {
        return None;
    }
    let sin_protocolo = crudo.split_once("://").map_or(crudo, |(_, r)| r);
    let hasta = sin_protocolo
        .find(['/', '?'])
        .unwrap_or(sin_protocolo.len());
    let servidor = sin_protocolo[..hasta].to_lowercase();
    // `https://localhost` no es un sitio que se reconozca por su nombre.
    if !servidor.contains('.') {
        return None;
    }
    // Sin el «www.»: no distingue nada. La direccion que se abre lo conserva.
    let donde = servidor
        .strip_prefix("www.")
        .unwrap_or(&servidor)
        .to_string();
    let camino = sin_protocolo[hasta..].trim_matches('/');
    let camino = camino.split('?').next().unwrap_or("");
    let de_que = camino
        .split('/')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" / ");
    let url = if crudo.contains("://") {
        crudo.to_string()
    } else {
        format!("https://{crudo}")
    };
    Some(EnlaceFicha { url, donde, de_que })
}

/// El primer trozo sin espacios que empieza por alguno de `inicios` al
/// principio de una palabra, sin mirar mayusculas.
fn buscar<'a>(texto: &'a str, inicios: &[&str]) -> Option<&'a str> {
    let minusculas = texto.to_ascii_lowercase();
    let mut mejor: Option<usize> = None;
    for inicio in inicios {
        let mut desde = 0;
        while let Some(n) = minusculas[desde..].find(inicio) {
            let i = desde + n;
            let antes = texto[..i].chars().next_back();
            if antes.is_none_or(|c| !(c.is_alphanumeric() || c == '_')) {
                mejor = Some(mejor.map_or(i, |m| m.min(i)));
                break;
            }
            desde = i + inicio.len();
        }
    }
    let i = mejor?;
    let resto = &texto[i..];
    let fin = resto.find(char::is_whitespace).unwrap_or(resto.len());
    Some(&resto[..fin])
}

/// Quita la puntuacion pegada al final; un parentesis que cierra uno abierto
/// dentro de la direccion es suyo.
fn recortar_lo_pegado(crudo: &str) -> &str {
    let mut fin = crudo.len();
    while let Some(c) = crudo[..fin].chars().next_back() {
        if !PEGADOS_AL_FINAL.contains(c) {
            break;
        }
        let hasta = &crudo[..fin];
        let abiertos = hasta.matches('(').count() as i64;
        let cerrados = hasta.matches(')').count() as i64;
        if c == ')' && abiertos > cerrados - 1 {
            break;
        }
        fin -= c.len_utf8();
    }
    &crudo[..fin]
}

impl Nucleo {
    /// Los mensajes del chat con su posicion, y los codigos de sus hojas.
    fn leer_para_la_ficha(
        &self,
        chat: &str,
    ) -> Result<(Vec<Mensaje>, Vec<u64>, std::collections::BTreeSet<String>), ErrorNucleo> {
        validar_chat(chat)?;
        let (mut mensajes, mut posiciones) = (Vec::new(), Vec::new());
        for (n, m) in self.mensajes_en_orden(chat)?.into_iter().enumerate() {
            if let Some(m) = m {
                mensajes.push(m);
                posiciones.push(n as u64);
            }
        }
        let hojas = almacen::uids_de_hojas(self.ruta(), chat);
        Ok((mensajes, posiciones, hojas))
    }
}

#[uniffi::export]
impl Nucleo {
    /// **La cabecera y las pestanas de la ficha** del chat: cuantas cosas
    /// tiene, su codigo y las pestanas que salen (con `actual` aunque se haya
    /// quedado vacia).
    pub fn ficha_del_chat(
        &self,
        chat: String,
        actual: Option<SeccionFicha>,
    ) -> Result<FichaDelChatLeida, ErrorNucleo> {
        let (mensajes, _, hojas) = self.leer_para_la_ficha(&chat)?;
        let cuentas: Vec<(SeccionFicha, usize)> = ORDEN
            .iter()
            .map(|s| (*s, de_la_seccion(&mensajes, *s, &hojas).len()))
            .collect();
        let cosas = cuentas
            .iter()
            .find(|(s, _)| *s == SeccionFicha::Todo)
            .map_or(0, |(_, n)| *n) as u32;
        let codigo = Indice::leer_si_esta(self.ruta())?
            .buscar(&chat)
            .map(|f| f.codigo_unico())
            .unwrap_or_default();
        Ok(FichaDelChatLeida {
            pestanas: pestanas(&cuentas, actual),
            primera: primera(&cuentas),
            cosas,
            codigo,
        })
    }

    /// **Lo que ensena una pestana** de la ficha, del mas antiguo al mas
    /// nuevo, con lo `buscado` con su lupa ya filtrado (vacio: todo).
    pub fn cosas_de_la_ficha(
        &self,
        chat: String,
        seccion: SeccionFicha,
        buscado: String,
    ) -> Result<Vec<CosaDeLaFicha>, ErrorNucleo> {
        let (mensajes, posiciones, hojas) = self.leer_para_la_ficha(&chat)?;
        Ok(de_la_seccion(&mensajes, seccion, &hojas)
            .into_iter()
            .filter(|&n| casa_con(&mensajes[n], &buscado))
            .map(|n| {
                let m = &mensajes[n];
                CosaDeLaFicha {
                    posicion: posiciones[n],
                    es_video: cuaderno::es_video(m),
                    enlace: primer_enlace(&m.texto),
                    mensaje: MensajeChat::from(m.clone()),
                }
            })
            .collect())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_proyecto::cuaderno::{Clase, Sello};
    use std::path::PathBuf;
    use std::sync::Arc;

    fn nucleo(etiqueta: &str) -> (Arc<Nucleo>, PathBuf, String) {
        let d =
            std::env::temp_dir().join(format!("pixpin-ficha-{etiqueta}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let n = Nucleo::abrir(d.display().to_string(), "Mac".into()).unwrap();
        let chat = n.crear_chat("Casa Lima".into()).unwrap();
        (n, d, chat.id)
    }

    fn sello(chat: &str, numero: i64) -> Sello {
        Sello {
            cuando: 1_000 + numero,
            numero,
            aparato: "MAC1".into(),
            proyecto: chat.into(),
        }
    }

    fn adjunto(chat: &str, numero: i64, clase: Clase, nombre: &str) -> Mensaje {
        Mensaje::adjunto(
            clase,
            nombre,
            &format!("adjuntos/{nombre}"),
            10,
            &sello(chat, numero),
        )
    }

    #[test]
    fn el_orden_lleva_cada_seccion_una_vez_y_el_buzon_al_final() {
        for s in ORDEN {
            assert_eq!(ORDEN.iter().filter(|x| **x == s).count(), 1, "{s:?}");
        }
        let sitio = |s: SeccionFicha| ORDEN.iter().position(|x| *x == s).unwrap();
        assert!(sitio(SeccionFicha::Fotos) < sitio(SeccionFicha::Archivos));
        assert!(sitio(SeccionFicha::Videos) < sitio(SeccionFicha::Enlaces));
        assert_eq!(ORDEN[ORDEN.len() - 1], SeccionFicha::Buzon);
        // Cada una de la ficha es una del cuaderno distinta.
        for s in cuaderno::Seccion::TODAS {
            assert_eq!(ORDEN.iter().filter(|x| x.del_cuaderno() == s).count(), 1);
        }
    }

    #[test]
    fn solo_salen_las_pestanas_con_algo_y_la_puesta() {
        let cuentas = [
            (SeccionFicha::Fotos, 3),
            (SeccionFicha::Videos, 0),
            (SeccionFicha::Todo, 5),
            (SeccionFicha::Enlaces, 2),
        ];
        let p = pestanas(&cuentas, None);
        let s: Vec<_> = p.iter().map(|p| p.seccion).collect();
        assert_eq!(
            s,
            [
                SeccionFicha::Fotos,
                SeccionFicha::Enlaces,
                SeccionFicha::Todo
            ]
        );
        assert!(p[0].cuadricula && !p[1].cuadricula);
        assert_eq!(p[0].cuenta, 3);
        // La puesta sale aunque se haya quedado vacia.
        let p = pestanas(&cuentas, Some(SeccionFicha::Videos));
        assert_eq!(p[1].seccion, SeccionFicha::Videos);
        assert_eq!(p[1].cuenta, 0);
        assert_eq!(primera(&cuentas), SeccionFicha::Fotos);
        assert_eq!(
            primera(&[]),
            SeccionFicha::Todo,
            "un chat vacio abre «Todo»"
        );
    }

    #[test]
    fn el_primer_enlace_dice_de_donde_es_y_de_que_va() {
        let e = primer_enlace("mira (https://www.GitHub.com/forge/pixpin?x=1).").unwrap();
        assert_eq!(e.donde, "github.com");
        assert_eq!(e.de_que, "forge / pixpin");
        assert_eq!(e.url, "https://www.GitHub.com/forge/pixpin?x=1");
        let e = primer_enlace("www.ejemplo.com/cosa.").unwrap();
        assert_eq!(e.url, "https://www.ejemplo.com/cosa");
        assert!(primer_enlace("https://localhost:8080").is_none());
        assert!(primer_enlace("sin enlaces").is_none());
    }

    #[test]
    fn cada_pestana_cuenta_lo_suyo_sin_lecciones_y_con_su_posicion() {
        let (n, d, chat) = nucleo("cuentas");
        let carpeta = almacen::carpeta(&d, &chat);
        let mut leccion = adjunto(&chat, 5, Clase::Imagen, "de-la-leccion.png");
        leccion.responde_a = Some("lec-abc".into());
        let mut fijada = cuaderno::Mensaje::nota("Mira www.ejemplo.com/cosa", &sello(&chat, 6));
        fijada.fijado = true;
        let mut del_buzon = adjunto(&chat, 7, Clase::Imagen, "llego.jpg");
        del_buzon.en_buzon = true;
        for m in [
            cuaderno::Mensaje::nota("hola", &sello(&chat, 1)),
            adjunto(&chat, 2, Clase::Imagen, "foto.jpg"),
            adjunto(&chat, 3, Clase::Imagen, "paseo.MOV"),
            adjunto(&chat, 4, Clase::Archivo, "clase.mp4"),
            leccion,
            fijada,
            del_buzon,
            adjunto(&chat, 8, Clase::Archivo, "plano.pdf"),
        ] {
            cuaderno::anadir(&carpeta, &m).unwrap();
        }
        let f = n.ficha_del_chat(chat.clone(), None).unwrap();
        let cuenta = |s| {
            f.pestanas
                .iter()
                .find(|p| p.seccion == s)
                .map_or(0, |p| p.cuenta)
        };
        assert_eq!(
            cuenta(SeccionFicha::Fotos),
            1,
            "los videos no son fotos, ni lo de la leccion"
        );
        assert_eq!(
            cuenta(SeccionFicha::Videos),
            2,
            "foto o archivo con extension de video"
        );
        assert_eq!(
            cuenta(SeccionFicha::Archivos),
            1,
            "el mp4 no cuenta como archivo"
        );
        assert_eq!(cuenta(SeccionFicha::Enlaces), 1);
        assert_eq!(cuenta(SeccionFicha::Fijados), 1);
        assert_eq!(cuenta(SeccionFicha::Buzon), 1);
        assert_eq!(
            cuenta(SeccionFicha::Galeria),
            0,
            "sin proyecto.json no hay hojas"
        );
        assert_eq!(f.cosas, 6, "todo menos el buzon y la leccion");
        assert_eq!(f.primera, SeccionFicha::Fotos);
        assert!(!f.codigo.is_empty());

        let videos = n
            .cosas_de_la_ficha(chat.clone(), SeccionFicha::Videos, String::new())
            .unwrap();
        assert!(videos.iter().all(|c| c.es_video));
        assert_eq!(videos[0].mensaje.nombre, "paseo.MOV");
        // La posicion es la de las paginas del chat: lleva al mismo mensaje.
        let todo = n.mensajes(chat.clone(), 0, 100).unwrap().mensajes;
        for c in &videos {
            assert_eq!(todo[c.posicion as usize].id, c.mensaje.id);
        }
        let enlaces = n
            .cosas_de_la_ficha(chat.clone(), SeccionFicha::Enlaces, String::new())
            .unwrap();
        assert_eq!(enlaces[0].enlace.as_ref().unwrap().donde, "ejemplo.com");

        // La lupa, sin mirar mayusculas ni tildes, en el nombre o el texto.
        let hallado = n
            .cosas_de_la_ficha(chat.clone(), SeccionFicha::Todo, "PLANO".into())
            .unwrap();
        assert_eq!(hallado.len(), 1);
        assert_eq!(hallado[0].mensaje.nombre, "plano.pdf");
        let nada = n
            .cosas_de_la_ficha(chat.clone(), SeccionFicha::Todo, "zzz".into())
            .unwrap();
        assert!(nada.is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn un_chat_sin_nada_abre_todo_y_no_falla() {
        let (n, d, chat) = nucleo("vacio");
        let f = n.ficha_del_chat(chat.clone(), None).unwrap();
        assert!(f.pestanas.is_empty());
        assert_eq!(f.primera, SeccionFicha::Todo);
        let f = n
            .ficha_del_chat(chat.clone(), Some(SeccionFicha::Todo))
            .unwrap();
        assert_eq!(f.pestanas.len(), 1);
        assert!(
            n.cosas_de_la_ficha(chat, SeccionFicha::Fotos, String::new())
                .unwrap()
                .is_empty()
        );
        assert!(n.ficha_del_chat("../fuera".into(), None).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}
