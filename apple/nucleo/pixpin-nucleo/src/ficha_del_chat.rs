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
//!
//! La segunda pasada de Windows (99f918a, 2439c5c, 8c92ba4: la cara y el
//! teclado del panel de medios de Telegram):
//!
//! - **Lo mas nuevo primero**, como Telegram (los rotulos de mes los pone
//!   Swift, con la hora de este Mac).
//! - **Los proyectos viven aqui**, en la primera pestana, con la que se
//!   abre la isla: cada proyecto de la lista de chats, en su orden
//!   ([`Nucleo::proyectos_de_la_ficha`]). No son mensajes: no se guarda ni
//!   viaja nada.
//! - **Las fotos de las notas ricas** (`![…](pixpin:files/…)`) salen en
//!   «Fotos» cada una como una cosa mas, en el sitio de su nota
//!   (`pixpin_ui::info::con_las_fotos_de_las_notas`).
//! - El foco del teclado se mueve con las mismas cuentas que en Windows
//!   ([`vecina_en_la_ficha`], `pixpin_ui::info::Lista::vecina`).

use pixpin_proyecto::almacen::{self, Indice};
use pixpin_proyecto::cuaderno::{self, Clase, Mensaje, Seccion};
use pixpin_ui::info as ui;
use pixpin_ui::resaltado;

use crate::chats::{FichaChat, MensajeChat, validar_chat};
use crate::{ErrorNucleo, Nucleo};

/// Una pestana de la ficha (`cuaderno::Seccion`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum SeccionFicha {
    /// Los proyectos de la lista de chats: solo de la interfaz.
    Proyectos,
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

/// En que orden salen las pestanas (`ORDEN` de Windows): los proyectos
/// delante (es con la que se abre la isla), luego lo que se mira, lo que se
/// lee, y «Todo» y el buzon al final.
pub const ORDEN: [SeccionFicha; 11] = [
    SeccionFicha::Proyectos,
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
            SeccionFicha::Proyectos => Seccion::Proyectos,
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
    /// Con la que se abre: la primera que tenga algo (los proyectos, casi
    /// siempre); sin nada, «Todo», que dice que no hay nada.
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

/// Una cosa de una pestana: un mensaje entero o, en «Fotos», una foto de
/// una nota rica (`pixpin_ui::info::Cosa`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CosaDeLaFicha {
    pub mensaje: MensajeChat,
    /// Su posicion en el chat (0 el mas antiguo), la de `Nucleo::mensajes`:
    /// «Ver en el chat» salta a ella.
    pub posicion: u64,
    pub es_video: bool,
    /// El primer enlace de su texto, si lo lleva y es un mensaje entero: la
    /// foto de una nota es una foto aunque su nota lleve un enlace.
    pub enlace: Option<EnlaceFicha>,
    /// En una nota rica, que foto suya es (en el orden de su burbuja);
    /// `None`, el mensaje entero.
    pub foto: Option<u32>,
    /// El fichero de esa foto en este aparato, solo si esta.
    pub ruta_foto: Option<String>,
    /// Lo que dice de esa foto su alt, sin el ancho del editor
    /// (`planta|300`): su nombre en la previa. Vacio en un mensaje entero.
    pub alt: String,
}

/// Un proyecto de la pestana «Proyectos».
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ProyectoDeLaFicha {
    pub ficha: FichaChat,
    /// El de este chat: va marcado («Este chat») y pulsarlo no sale de la
    /// isla, lleva a su «Galeria».
    pub este: bool,
    /// Guardado en un disco que ahora no esta: no se puede abrir.
    pub disponible: bool,
}

/// Un trozo de lo buscado dentro de un nombre, en unidades UTF-16 (las de
/// `NSString`): va en negrita.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct TramoDeLaFicha {
    pub inicio: u32,
    pub largo: u32,
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
/// hojas del proyecto (para la galeria). Los proyectos no son mensajes.
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

/// Si la burbuja de `m` es una nota (`nota_rica::es_nota` de Windows): ni
/// una mini-app ni lo que escribe el asistente.
fn es_nota(m: &Mensaje) -> bool {
    matches!(m.clase, None | Some(Clase::Nota))
        && m.miniapp.is_none()
        && !crate::asistente::chat::es_del_asistente(m)
}

/// **Todo lo de la seccion, lo mas nuevo primero** (`cosas_de` de Windows):
/// sus mensajes y, en «Fotos», tambien cada foto de las notas ricas, en el
/// sitio de su nota y en el orden de su burbuja.
pub fn cosas_de(
    mensajes: &[Mensaje],
    seccion: SeccionFicha,
    hojas: &std::collections::BTreeSet<String>,
) -> Vec<ui::Cosa> {
    let suyos = de_la_seccion(mensajes, seccion, hojas);
    let mut v = if seccion != SeccionFicha::Fotos {
        suyos.into_iter().map(ui::Cosa::entera).collect()
    } else {
        // Las notas, de las de «Todo» (sin buzon ni lecciones), y solo las
        // que la burbuja pinta como nota rica.
        let notas: Vec<usize> = de_la_seccion(mensajes, SeccionFicha::Todo, hojas)
            .into_iter()
            .filter(|n| {
                let m = &mensajes[*n];
                m.texto.contains("![") && es_nota(m)
            })
            .collect();
        ui::con_las_fotos_de_las_notas(&suyos, &notas, |n| {
            ui::fotos_de_la_nota(&mensajes[n].texto).len()
        })
    };
    v.sort_by_key(|c| (std::cmp::Reverse(c.mensaje), c.foto));
    v
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
    buscado.trim().is_empty()
        || resaltado::hay_coincidencia(&m.resumen(), buscado)
        || resaltado::hay_coincidencia(&m.nombre, buscado)
}

/// El nombre de una foto de nota: lo que dice su alt, sin el ancho del
/// editor (`planta|300`).
fn alt_sin_ancho(alt: &str) -> String {
    alt.split('|').next().unwrap_or_default().trim().to_string()
}

/// **Lo buscado en negrita** dentro de `texto` (`negritas` de Windows): los
/// trozos que casan, en UTF-16. Sin nada buscado, ninguno.
#[uniffi::export]
pub fn negritas_de_la_ficha(texto: String, buscado: String) -> Vec<TramoDeLaFicha> {
    if buscado.trim().is_empty() {
        return Vec::new();
    }
    resaltado::coincidencias(&texto, &buscado)
        .into_iter()
        .filter_map(|t| {
            let (inicio, largo) = t.en_utf16(&texto);
            (largo > 0).then_some(TramoDeLaFicha { inicio, largo })
        })
        .collect()
}

/// Cuantas columnas tiene la rejilla de la isla en `ancho` puntos: de tres a
/// cinco, las que quepan con su celda minima (`columnas_isla`).
#[uniffi::export]
pub fn columnas_de_la_ficha(ancho: u32) -> u32 {
    ui::columnas_isla(ancho, 100).0
}

/// **A que cosa va el foco con las flechas** desde la `actual`
/// (`Lista::vecina` de Windows): a los lados, la de antes o la de despues;
/// arriba y abajo, la de la fila de encima o de debajo que caiga mas cerca
/// en horizontal, saltando los rotulos de mes. `meses` lleva la clave del
/// mes de cada cosa, en el orden en que se ensenan (en los proyectos, todas
/// iguales: no van por meses); `ancho`, el del contenido.
#[uniffi::export]
pub fn vecina_en_la_ficha(
    meses: Vec<i64>,
    cuadricula: bool,
    ancho: u32,
    actual: u32,
    dx: i32,
    dy: i32,
) -> Option<u32> {
    let area = pixpin_geom::Rect {
        x: 0,
        y: 0,
        ancho,
        alto: 1,
    };
    let lista = ui::lista_isla(area, &meses, cuadricula, ui::ISLA_FILA, 100);
    lista.vecina(actual as usize, dx, dy).map(|n| n as u32)
}

/// Lo que dura un video o una nota de voz en su insignia: «0:07», «12:30»,
/// «1:02:03»; sin duracion, nada (`duracion_corta`).
#[uniffi::export]
pub fn duracion_en_la_ficha(ms: i64) -> String {
    ui::duracion_corta(ms)
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

/// Los mensajes de un chat, su posicion en el y los codigos de sus hojas.
type ParaLaFicha = (Vec<Mensaje>, Vec<u64>, std::collections::BTreeSet<String>);

impl Nucleo {
    /// Los mensajes del chat con su posicion, y los codigos de sus hojas.
    fn leer_para_la_ficha(&self, chat: &str) -> Result<ParaLaFicha, ErrorNucleo> {
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
        let indice = Indice::leer_si_esta(self.ruta())?;
        let proyectos = indice.ordenadas().len();
        let cuentas: Vec<(SeccionFicha, usize)> = ORDEN
            .iter()
            .map(|s| {
                let n = match s {
                    SeccionFicha::Proyectos => proyectos,
                    _ => cosas_de(&mensajes, *s, &hojas).len(),
                };
                (*s, n)
            })
            .collect();
        let cosas = cuentas
            .iter()
            .find(|(s, _)| *s == SeccionFicha::Todo)
            .map_or(0, |(_, n)| *n) as u32;
        let codigo = indice
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

    /// **Lo que ensena una pestana** de la ficha, lo mas nuevo primero, con
    /// lo `buscado` con su lupa ya filtrado (vacio: todo). Los proyectos van
    /// aparte ([`Nucleo::proyectos_de_la_ficha`]).
    pub fn cosas_de_la_ficha(
        &self,
        chat: String,
        seccion: SeccionFicha,
        buscado: String,
    ) -> Result<Vec<CosaDeLaFicha>, ErrorNucleo> {
        let (mensajes, posiciones, hojas) = self.leer_para_la_ficha(&chat)?;
        Ok(cosas_de(&mensajes, seccion, &hojas)
            .into_iter()
            .filter(|c| casa_con(&mensajes[c.mensaje], &buscado))
            .map(|c| {
                let m = &mensajes[c.mensaje];
                let (ruta_foto, alt) = match c.foto {
                    Some(k) => ui::fotos_de_la_nota(&m.texto)
                        .into_iter()
                        .nth(k)
                        .map(|f| {
                            let ruta = self
                                .ruta_de_foto(&chat, &f.enlace)
                                .map(|r| r.display().to_string());
                            (ruta, alt_sin_ancho(&f.alt))
                        })
                        .unwrap_or_default(),
                    None => (None, String::new()),
                };
                CosaDeLaFicha {
                    posicion: posiciones[c.mensaje],
                    es_video: c.foto.is_none() && cuaderno::es_video(m),
                    enlace: c.foto.is_none().then(|| primer_enlace(&m.texto)).flatten(),
                    foto: c.foto.map(|k| k as u32),
                    ruta_foto,
                    alt,
                    mensaje: MensajeChat::from(m.clone()),
                }
            })
            .collect())
    }

    /// **Los proyectos de la pestana «Proyectos»**: los de la lista de
    /// chats, en su orden («Mensajes guardados» y el asistente delante, aunque
    /// sean viejos), con lo `buscado` en su nombre (vacio: todos).
    pub fn proyectos_de_la_ficha(
        &self,
        chat: String,
        buscado: String,
    ) -> Result<Vec<ProyectoDeLaFicha>, ErrorNucleo> {
        let indice = Indice::leer_si_esta(self.ruta())?;
        Ok(indice
            .ordenadas()
            .into_iter()
            .filter(|f| {
                buscado.trim().is_empty() || resaltado::hay_coincidencia(&f.nombre, &buscado)
            })
            .map(|f| ProyectoDeLaFicha {
                este: f.id == chat,
                disponible: pixpin_proyecto::ubicacion::disponible(f),
                ficha: FichaChat::from(f),
            })
            .collect())
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pixpin_proyecto::cuaderno::Sello;
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
        // Los proyectos, delante: la isla se abre en ellos (2439c5c).
        assert_eq!(ORDEN[0], SeccionFicha::Proyectos);
        assert_eq!(ORDEN[1], SeccionFicha::Galeria);
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
            "sin nada de nada se abre «Todo»"
        );
        // Con proyectos, se abre en ellos.
        let con = [(SeccionFicha::Proyectos, 2), (SeccionFicha::Fotos, 3)];
        assert_eq!(primera(&con), SeccionFicha::Proyectos);
        assert!(!pestanas(&con, None)[0].cuadricula);
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
        assert!(cuenta(SeccionFicha::Proyectos) >= 1, "al menos este chat");
        assert_eq!(f.cosas, 6, "todo menos el buzon y la leccion");
        assert_eq!(
            f.primera,
            SeccionFicha::Proyectos,
            "se abre en los proyectos"
        );
        assert!(!f.codigo.is_empty());

        let videos = n
            .cosas_de_la_ficha(chat.clone(), SeccionFicha::Videos, String::new())
            .unwrap();
        assert!(videos.iter().all(|c| c.es_video));
        // Lo mas nuevo primero, como Telegram.
        assert_eq!(videos[0].mensaje.nombre, "clase.mp4");
        assert_eq!(videos[1].mensaje.nombre, "paseo.MOV");
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
    fn las_fotos_de_las_notas_salen_en_fotos_en_el_sitio_de_su_nota() {
        let (n, d, chat) = nucleo("notas");
        let carpeta = almacen::carpeta(&d, &chat);
        let nota = cuaderno::Mensaje::nota(
            "# Obra\n\n![planta|300](pixpin:files/a.png)\n\nver https://ejemplo.com\n\n![](pixpin:files/b.png)",
            &sello(&chat, 2),
        );
        for m in [
            adjunto(&chat, 1, Clase::Imagen, "vieja.jpg"),
            nota,
            adjunto(&chat, 3, Clase::Imagen, "nueva.jpg"),
        ] {
            cuaderno::anadir(&carpeta, &m).unwrap();
        }
        let fotos = n
            .cosas_de_la_ficha(chat.clone(), SeccionFicha::Fotos, String::new())
            .unwrap();
        let que: Vec<_> = fotos
            .iter()
            .map(|c| (c.mensaje.nombre.as_str(), c.foto))
            .collect();
        assert_eq!(
            que,
            [
                ("nueva.jpg", None),
                ("", Some(0)),
                ("", Some(1)),
                ("vieja.jpg", None)
            ]
        );
        assert_eq!(fotos[1].alt, "planta", "sin el ancho del editor");
        assert!(fotos[1].enlace.is_none(), "la foto es una foto");
        assert!(fotos[1].ruta_foto.is_none(), "no esta en el aparato");
        let f = n.ficha_del_chat(chat.clone(), None).unwrap();
        let fotos_n = f
            .pestanas
            .iter()
            .find(|p| p.seccion == SeccionFicha::Fotos)
            .unwrap()
            .cuenta;
        assert_eq!(fotos_n, 4);
        assert_eq!(f.cosas, 3, "«Todo» cuenta mensajes");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn los_proyectos_son_los_de_la_lista_y_el_de_este_chat_va_marcado() {
        let (n, d, chat) = nucleo("proyectos");
        let otro = n.crear_chat("Puente Sur".into()).unwrap();
        let p = n
            .proyectos_de_la_ficha(chat.clone(), String::new())
            .unwrap();
        let lista: Vec<_> = n.chats().unwrap().into_iter().map(|f| f.id).collect();
        assert_eq!(
            p.iter().map(|x| x.ficha.id.clone()).collect::<Vec<_>>(),
            lista,
            "en el orden de la lista de chats"
        );
        assert!(p.iter().find(|x| x.ficha.id == chat).unwrap().este);
        assert!(!p.iter().find(|x| x.ficha.id == otro.id).unwrap().este);
        assert!(p.iter().all(|x| x.disponible));
        let p = n
            .proyectos_de_la_ficha(chat.clone(), "puente".into())
            .unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].ficha.id, otro.id);
        // Ningun mensaje es de los proyectos.
        assert!(
            n.cosas_de_la_ficha(chat, SeccionFicha::Proyectos, String::new())
                .unwrap()
                .is_empty()
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn el_foco_salta_los_meses_como_en_windows() {
        // 400 de ancho: tres columnas.
        assert_eq!(columnas_de_la_ficha(400), 3);
        assert_eq!(columnas_de_la_ficha(2000), 5);
        // Dos fotos de un mes y cuatro del siguiente.
        let meses = vec![2, 2, 1, 1, 1, 1];
        assert_eq!(
            vecina_en_la_ficha(meses.clone(), true, 400, 0, 1, 0),
            Some(1)
        );
        assert_eq!(vecina_en_la_ficha(meses.clone(), true, 400, 0, -1, 0), None);
        // Abajo, la de la fila de debajo mas cerca: el mes siguiente.
        assert_eq!(
            vecina_en_la_ficha(meses.clone(), true, 400, 1, 0, 1),
            Some(3)
        );
        assert_eq!(
            vecina_en_la_ficha(meses.clone(), true, 400, 2, 0, 1),
            Some(5)
        );
        assert_eq!(
            vecina_en_la_ficha(meses.clone(), true, 400, 2, 0, -1),
            Some(0)
        );
        assert_eq!(vecina_en_la_ficha(meses, true, 400, 0, 0, -1), None);
        // En filas, arriba y abajo de una en una.
        assert_eq!(vecina_en_la_ficha(vec![0; 3], false, 400, 1, 0, 1), Some(2));
        assert_eq!(duracion_en_la_ficha(7_000), "0:07");
        assert_eq!(duracion_en_la_ficha(0), "");
        let t = negritas_de_la_ficha("Casa Lima".into(), "lima".into());
        assert_eq!(
            t,
            [TramoDeLaFicha {
                inicio: 5,
                largo: 4
            }]
        );
        assert!(negritas_de_la_ficha("Casa".into(), " ".into()).is_empty());
    }

    #[test]
    fn un_chat_sin_nada_abre_en_los_proyectos_y_no_falla() {
        let (n, d, chat) = nucleo("vacio");
        let f = n.ficha_del_chat(chat.clone(), None).unwrap();
        let s: Vec<_> = f.pestanas.iter().map(|p| p.seccion).collect();
        assert_eq!(s, [SeccionFicha::Proyectos], "solo los proyectos");
        assert_eq!(f.primera, SeccionFicha::Proyectos);
        let f = n
            .ficha_del_chat(chat.clone(), Some(SeccionFicha::Todo))
            .unwrap();
        assert_eq!(f.pestanas.len(), 2);
        assert!(
            n.cosas_de_la_ficha(chat, SeccionFicha::Fotos, String::new())
                .unwrap()
                .is_empty()
        );
        assert!(n.ficha_del_chat("../fuera".into(), None).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}
