//! **Lo que se reconoce al escribir una tarea**, del lado de la app (17 y
//! 18-oct-2026).
//!
//! El parser es [`pixpin_frase::tarea`] (el mismo del pedido `anadir_tarea`
//! y del plugin de Flow Launcher): #etiquetas en cualquier sitio, urgencia
//! normal / urgente / muy urgente y la hora de recordar escrita como se dice.
//! Aqui solo lo que es de la app:
//!
//! - las palabras de cada urgencia, las de Ajustes → Tareas ([`fijar`], como
//!   `tema_cosmos::fijar`: las mira el hilo de la ventana de Tareas);
//! - el reloj ([`ahora`]);
//! - lo que ensena una tarjeta ([`a_la_vista`]): el titulo limpio, la hora
//!   legible y la nota, los emoticonos y las chapitas de la urgencia y las
//!   etiquetas;
//! - meter y sacar la nota del texto ([`con_nota`]), sin tocar las imagenes
//!   ni la fecha de creacion `➕`, que son de `pixpin_proyecto::mini`.

#![forbid(unsafe_code)]
// La copia de Windows: lo que el Mac aun no usa se queda igual.
#![allow(dead_code)]

use std::sync::RwLock;

use pixpin_ia::fechas::{self, Momento};
use pixpin_proyecto::mini;

pub use pixpin_frase::tarea::{Frase, NOTA, Palabras, Urgencia};

/// Las palabras de cada urgencia que valen ahora. `None`: las de fabrica.
static PALABRAS: RwLock<Option<Palabras>> = RwLock::new(None);

/// Las pone quien lee los ajustes (al arrancar y al cambiarlos).
pub fn fijar(p: Palabras) {
    if let Ok(mut g) = PALABRAS.write() {
        *g = Some(p);
    }
}

/// Las palabras de cada urgencia de ahora.
pub fn palabras() -> Palabras {
    PALABRAS
        .read()
        .ok()
        .and_then(|g| g.clone())
        .unwrap_or_default()
}

/// **Entiende una tarea escrita** con las palabras de Ajustes.
pub fn leer(texto: &str, ahora: Momento) -> Frase {
    pixpin_frase::tarea::leer(texto, ahora, &palabras())
}

/// El texto que se guarda de lo escrito en una caja: lo entendido o, si no
/// queda titulo ni hay imagenes (solo «#casa»), lo escrito tal cual.
#[allow(dead_code)]
pub fn a_guardar(escrito: &str, con_imagenes: bool, ahora: Momento) -> String {
    let f = leer(escrito, ahora);
    if f.titulo.is_empty() && !con_imagenes {
        escrito.trim().to_string()
    } else {
        f.texto()
    }
}

/// El momento local de ahora. En el Mac la hora local la da Swift
/// (`desfase_ms`: local menos UTC), como en el resto del nucleo.
pub fn ahora(desfase_ms: i64) -> Momento {
    Momento::de_ms_locales(crate::ahora_ms() + desfase_ms)
}

/// Los milisegundos locales de un momento.
pub fn ms_de(m: Momento) -> i64 {
    (m.dia * 1440 + i64::from(m.minuto)) * 60_000
}

/// Las marcas de un texto de tarea (lo que se ve, sin `➕` ni imagenes; o
/// el crudo entero: la fecha y las imagenes no estorban).
pub fn marcas(texto: &str) -> Frase {
    let (visible, _) = mini::partir(texto);
    let (visible, _) = mini::imagenes_de(visible);
    pixpin_frase::tarea::marcas(&visible)
}

/// Una chapita de una tarea: su urgencia o una etiqueta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Chapa {
    Urgencia(Urgencia),
    Etiqueta(String),
}

impl Chapa {
    /// Lo que se busca al pulsarla: la etiqueta con su `#`, o el emoticono
    /// de la urgencia (que es lo que lleva el texto guardado).
    pub fn busqueda(&self) -> String {
        match self {
            Chapa::Urgencia(u) => u.marca().unwrap_or_default().to_string(),
            Chapa::Etiqueta(e) => format!("#{e}"),
        }
    }
}

/// Lo que ensena la tarjeta de una tarea.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Vista {
    /// El titulo limpio y, si tiene, la hora legible («· ⏰ hoy 17:00») y
    /// la nota («· 💬 …»).
    pub texto: String,
    /// Sus emoticonos (`emoticonos_de`), en circulos.
    pub emos: Vec<String>,
    /// La urgencia (si no es normal) y las etiquetas, en chapitas.
    pub chapas: Vec<Chapa>,
}

/// **Lo que ensena la tarjeta** de una tarea: el titulo sin las marcas, la
/// hora y la nota en el renglon, los emoticonos en sus circulos y la
/// urgencia y las `#etiquetas` en chapitas aparte (18-oct-2026: «que se
/// quite del titulo y se ensene aparte»).
pub fn a_la_vista(texto: &str, ahora: Option<Momento>) -> Vista {
    let m = marcas(texto);
    let (mut limpio, emos) = super::emoticonos::emoticonos_de(&m.titulo);
    if let Some(h) = m.hora {
        let cuando = match ahora {
            Some(a) => h.legible(a),
            None => {
                let (y, mo, d) = fechas::fecha_de(h.dia);
                format!("{d}/{mo}/{y} {:02}:{:02}", h.minuto / 60, h.minuto % 60)
            }
        };
        limpio.push_str(&format!(" · ⏰ {cuando}"));
    }
    if let Some(n) = &m.nota {
        limpio.push_str(&format!(" · {NOTA} {n}"));
    }
    let mut chapas = Vec::new();
    if m.urgencia != Urgencia::Normal {
        chapas.push(Chapa::Urgencia(m.urgencia));
    }
    chapas.extend(m.etiquetas.into_iter().map(Chapa::Etiqueta));
    Vista {
        texto: limpio.trim().trim_start_matches('·').trim().to_string(),
        emos,
        chapas,
    }
}

/// Una nota en un renglon y sin las marcas que confundirian al leerla.
fn nota_limpia(nota: &str) -> String {
    nota.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .filter(|c| *c != super::RELOJ && *c != mini::MARCA_DE_CREADA && *c != NOTA)
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// **El texto crudo de una tarea con otra nota** (vacia: sin nota). La nota
/// va tras `💬`, delante de las imagenes, la hora y la fecha, que se quedan
/// como estaban.
pub fn con_nota(crudo: &str, nota: &str) -> String {
    let (visible, creada) = mini::partir(crudo);
    let (cabeza, cola) = match super::hora_de_tarea(visible) {
        Some(t) => (super::sin_hora(visible), Some(super::marca_de_hora(t))),
        None => (visible.to_string(), None),
    };
    let (texto, imagenes) = mini::imagenes_de(&cabeza);
    let sin_nota = match texto.find(NOTA) {
        Some(i) => texto[..i].trim_end().to_string(),
        None => texto,
    };
    let nota = nota_limpia(nota);
    let mut s = sin_nota;
    if !nota.is_empty() {
        s = format!("{} {NOTA} {nota}", s.trim_end());
    }
    let mut s = mini::con_imagenes(&s, &imagenes);
    if let Some(c) = cola {
        s = format!("{} {c}", s.trim_end());
    }
    match creada {
        Some(f) => mini::con_fecha(&s, f),
        None => s.trim().to_string(),
    }
}

/// **La hora que se esta eligiendo** en la caja de una tarea (el selector
/// de dia y hora): la marcada en lo escrito o la que se entiende de ello.
pub fn hora_escrita(escrito: &str, ahora: Momento) -> Option<Momento> {
    leer(escrito, ahora).hora
}

/// Lo escrito con otra hora de recordar (`None`: sin hora). Si lo escrito
/// decia una fecha con palabras («mañana a las 10»), se quita: manda la
/// elegida. Lo demas se queda tal cual; se reconoce al guardar.
pub fn con_hora_escrita(escrito: &str, hora: Option<Momento>, ahora: Momento) -> String {
    let mut base = match pixpin_frase::tarea::marca_de_hora(escrito) {
        Some((i, _)) => escrito[..i].trim_end().to_string(),
        None => escrito.trim_end().to_string(),
    };
    // La fecha dicha con palabras sale: la que vale es la del selector.
    let sin_fecha = pixpin_frase::tarea::sin_fecha_dicha(&base, ahora, &palabras());
    if sin_fecha != base {
        base = sin_fecha;
    }
    match hora {
        Some(h) if base.is_empty() => h.marca(),
        Some(h) => format!("{base} {}", h.marca()),
        None => base,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Miercoles 14 de octubre de 2026, 10:30 (el de las pruebas de fechas).
    fn ahora() -> Momento {
        Momento {
            dia: fechas::dias_de(2026, 10, 14),
            minuto: 10 * 60 + 30,
        }
    }

    fn a(dia: u32, h: u32, m: u32) -> Option<Momento> {
        Some(Momento {
            dia: fechas::dias_de(2026, 10, i64::from(dia)),
            minuto: h * 60 + m,
        })
    }

    #[test]
    fn la_tarjeta_ensena_el_titulo_limpio_y_las_chapitas_aparte() {
        let crudo = "llamar al banco ⏫ #casa #obra 💬 cierran a las 3 ⏰ 2026-10-14 17:00 ➕ 2026-10-14";
        let v = a_la_vista(crudo, Some(ahora()));
        assert_eq!(v.texto, "llamar al banco · ⏰ hoy 17:00 · 💬 cierran a las 3");
        assert_eq!(
            v.chapas,
            [
                Chapa::Urgencia(Urgencia::MuyUrgente),
                Chapa::Etiqueta("casa".into()),
                Chapa::Etiqueta("obra".into())
            ]
        );
        assert!(v.emos.is_empty(), "la urgencia ya no es un emoticono suelto");
        assert_eq!(v.chapas[1].busqueda(), "#casa");
        assert_eq!(v.chapas[0].busqueda(), "⏫");
        // Una tarea de antes, sin marcas: igual que con `emoticonos_de`.
        let v = a_la_vista("pan 🍞", None);
        assert_eq!((v.texto.as_str(), v.emos.as_slice()), ("pan", &["🍞".to_string()][..]));
        assert!(v.chapas.is_empty(), "normal: sin chapita");
        // La «baja» de antes ya no se ensena.
        assert!(a_la_vista("pan 🔽", None).chapas.is_empty());
    }

    #[test]
    fn guardar_lo_escrito_reconoce_todo_y_sin_titulo_lo_deja_tal_cual() {
        assert_eq!(
            a_guardar("#casa llamar al banco urgente 18/10 17:00", false, ahora()),
            "llamar al banco 🔼 #casa ⏰ 2026-10-18 17:00"
        );
        assert_eq!(a_guardar("#casa", false, ahora()), "#casa");
        // Corregir una tarea guardada la deja igual.
        let crudo = "yeso ⏫ #obra 💬 falta camión ⏰ 2026-10-14 17:00";
        assert_eq!(a_guardar(crudo, false, ahora()), crudo);
    }

    #[test]
    fn el_selector_cambia_la_hora_de_lo_escrito() {
        assert_eq!(hora_escrita("pan mañana a las 10", ahora()), a(15, 10, 0));
        assert_eq!(hora_escrita("pan ⏰ 2026-10-20 08:00", ahora()), a(20, 8, 0));
        assert_eq!(hora_escrita("pan", ahora()), None);
        let con = |t: &str, h| con_hora_escrita(t, h, ahora());
        assert_eq!(con("pan #casa", a(20, 8, 0)), "pan #casa ⏰ 2026-10-20 08:00");
        assert_eq!(con("pan ⏰ 2026-10-20 08:00", a(21, 9, 30)), "pan ⏰ 2026-10-21 09:30");
        assert_eq!(con("pan ⏰ 2026-10-20 08:00", None), "pan");
        assert_eq!(con("", a(20, 8, 0)), "⏰ 2026-10-20 08:00");
        // Una fecha dicha con palabras la sustituye la elegida.
        assert_eq!(con("pan mañana a las 10 urgente", a(20, 8, 0)), "pan urgente ⏰ 2026-10-20 08:00");
        // Y al guardar queda la elegida.
        assert_eq!(a_guardar(&con("pan", a(20, 8, 0)), false, ahora()), "pan ⏰ 2026-10-20 08:00");
    }

    #[test]
    fn la_nota_entra_y_sale_sin_tocar_la_hora_la_fecha_ni_las_imagenes() {
        let crudo = "yeso ⏫ ![img 01](pixpin:files/a.png) ⏰ 2026-10-14 17:00 ➕ 2026-10-10";
        let con = con_nota(crudo, "falta\nel camión ⏰");
        assert_eq!(
            con,
            "yeso ⏫ 💬 falta el camión ![img 01](pixpin:files/a.png) ⏰ 2026-10-14 17:00 ➕ 2026-10-10"
        );
        assert_eq!(super::super::hora_de_tarea(&con), a(14, 17, 0).map(ms_de));
        assert_eq!(mini::partir(&con).1, mini::partir(crudo).1, "la fecha sigue");
        assert_eq!(mini::imagenes_de(mini::partir(&con).0).1.len(), 1);
        // Otra nota cambia la de antes; vacia la quita.
        let otra = con_nota(&con, "mañana");
        assert_eq!(marcas(&otra).nota.as_deref(), Some("mañana"));
        assert_eq!(otra.matches(NOTA).count(), 1);
        let sin = con_nota(&otra, "  ");
        assert_eq!(sin, crudo);
        // Sin hora ni fecha.
        assert_eq!(con_nota("pan", "no hay"), "pan 💬 no hay");
    }
}
