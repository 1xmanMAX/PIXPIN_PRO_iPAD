//! **`carpeta` en «Buscar en PixPin»**: las carpetas de paginas de todos los
//! chats, para abrirlas, crear una o anadirles una pagina sin abrir el chat
//! (Windows 22c3ce2 y 211a1cd, 25-oct-2026: `p carpeta` de Flow,
//! `apps/pixpin-lanzador/src/cuadernos.rs`, y el pedido `carpeta_paginas`).
//!
//! Sale cada carpeta (la mas reciente primero, como mucho [`TOPE`]) y, si no
//! es la de una clase grabada, su «Nueva página en …». Si lo escrito no es
//! el nombre de ninguna, «Nueva carpeta «texto»». Lo de abrir ventanas lo
//! hace Swift (`Buscar/`, `Chat/Carpetas.swift`); aqui se lee y se crea.

use pixpin_proyecto::almacen::{self, Indice};
use pixpin_proyecto::carpeta_de_paginas::{self as carpetas, Carpeta};
use pixpin_proyecto::cuaderno::{self, Mensaje};

use crate::carpetas::{NOMBRE_DE_SERIE, carpeta_por_nombre, nombre_libre, nombre_visible};
use crate::chats::validar_chat;
use crate::{ErrorNucleo, MensajeChat, Nucleo};

/// Cuantas carpetas se ensenan, con su «Nueva página» cada una.
pub const TOPE: usize = 6;

/// Una carpeta de paginas de algun chat, para la lista de Buscar.
#[derive(Debug, Clone, PartialEq, Eq, Hash, uniffi::Record)]
pub struct CarpetaDeFlow {
    pub chat: String,
    pub nombre_chat: String,
    /// El `id` de su mensaje.
    pub id: String,
    pub nombre: String,
    /// Cuantas paginas lleva (las borradas del chat aun cuentan: se saltan
    /// al abrirla), como el plugin de Windows.
    pub paginas: u32,
    /// La de una clase o reunion grabada: se abre, pero no se le anaden
    /// paginas.
    pub grabacion: bool,
    pub cuando: i64,
}

/// Lo que sale para `carpeta <texto>`.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CarpetasDeFlow {
    pub carpetas: Vec<CarpetaDeFlow>,
    /// Si alguna se llama justo asi: entonces no se ofrece crearla.
    pub exacta: bool,
}

/// Minusculas y sin tildes, como `normalizar` del plugin.
fn plegar(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    s.nfd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .collect::<String>()
        .to_lowercase()
}

/// El nombre que se ensena de una carpeta («Cuaderno» si no tiene).
fn nombre_de(m: &Mensaje) -> String {
    match m.nombre.trim() {
        "" => NOMBRE_DE_SERIE.to_string(),
        _ => nombre_visible(m),
    }
}

/// El subtitulo de la fila de una carpeta, como el plugin de Windows.
#[uniffi::export]
pub fn detalle_de_carpeta_de_flow(c: CarpetaDeFlow) -> String {
    if c.grabacion {
        return format!("Clase o reunión grabada · {} · Intro: abrirla", c.nombre_chat);
    }
    let cuantas = match c.paginas {
        0 => "Vacía".to_string(),
        1 => "1 página".to_string(),
        n => format!("{n} páginas"),
    };
    format!("Carpeta de páginas · {cuantas} · {} · Intro: abrirla", c.nombre_chat)
}

#[uniffi::export]
impl Nucleo {
    /// **Las carpetas que se llaman como `texto`** (todas sin texto) de
    /// todos los chats, la mas reciente primero. Bloqueante.
    pub fn carpetas_para_flow(&self, texto: String) -> Result<CarpetasDeFlow, ErrorNucleo> {
        let buscado = plegar(texto.trim());
        let indice = Indice::leer_si_esta(self.ruta())?;
        let mut todas = Vec::new();
        for ficha in &indice.proyectos {
            if validar_chat(&ficha.id).is_err() {
                continue;
            }
            let dir = almacen::carpeta(self.ruta(), &ficha.id);
            let Ok(c) = cuaderno::Cuaderno::leer_de(&dir) else {
                continue;
            };
            for m in c.mensajes.iter().filter(|m| carpetas::es_carpeta(m)) {
                let nombre = nombre_de(m);
                if !buscado.is_empty() && !plegar(&nombre).contains(&buscado) {
                    continue;
                }
                todas.push(CarpetaDeFlow {
                    chat: ficha.id.clone(),
                    nombre_chat: ficha.nombre.clone(),
                    id: m.id.clone(),
                    nombre,
                    paginas: Carpeta::de_mensaje(m).map_or(0, |c| c.len() as u32),
                    grabacion: carpetas::es_grabacion(m),
                    cuando: m.cuando,
                });
            }
        }
        todas.sort_by_key(|c| std::cmp::Reverse(c.cuando));
        let exacta = todas.iter().any(|c| plegar(&c.nombre) == buscado);
        todas.truncate(TOPE);
        Ok(CarpetasDeFlow { carpetas: todas, exacta })
    }

    /// **Una carpeta nueva** en `chat` llamada `nombre` (o «Cuaderno»), sin
    /// repetir el de otra («Cuaderno 2»), como `crear_por_nombre` de
    /// Windows. Devuelve su mensaje. Bloqueante.
    pub fn crear_carpeta_por_nombre(
        &self,
        chat: String,
        nombre: String,
    ) -> Result<MensajeChat, ErrorNucleo> {
        validar_chat(&chat)?;
        let dir = almacen::carpeta(self.ruta(), &chat);
        let mensajes = match cuaderno::Cuaderno::leer_de(&dir) {
            Ok(c) => c.mensajes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e.into()),
        };
        let base = match nombre.trim() {
            "" => NOMBRE_DE_SERIE,
            n => n,
        };
        let libre = nombre_libre(base, &mensajes);
        // Un chat recien creado aun no tiene su cuaderno en el disco (y
        // `carpeta_nueva` lo lee): vacio, como lo deja el primer mensaje.
        let fichero = dir.join("guardados.jsonl");
        if !fichero.exists() {
            std::fs::create_dir_all(&dir)?;
            std::fs::write(&fichero, "")?;
        }
        self.carpeta_nueva(&chat, Some(&libre), None).map(MensajeChat::from)
    }

    /// **La carpeta de `chat`** con el `id` dado o, si ya no esta, la que se
    /// llama `nombre` (el pedido `carpeta_paginas`). `None` si no hay.
    pub fn buscar_carpeta_de_paginas(
        &self,
        chat: String,
        id: Option<String>,
        nombre: String,
    ) -> Result<Option<MensajeChat>, ErrorNucleo> {
        validar_chat(&chat)?;
        let dir = almacen::carpeta(self.ruta(), &chat);
        let mensajes = match cuaderno::Cuaderno::leer_de(&dir) {
            Ok(c) => c.mensajes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let por_id = id
            .as_deref()
            .and_then(|id| mensajes.iter().position(|m| m.id == id && carpetas::es_carpeta(m)));
        Ok(por_id
            .or_else(|| carpeta_por_nombre(&mensajes, &nombre))
            .map(|i| MensajeChat::from(mensajes[i].clone())))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::sync::Arc;

    fn nucleo(etiqueta: &str) -> (Arc<Nucleo>, std::path::PathBuf, String) {
        let d = std::env::temp_dir()
            .join(format!("pixpin-carpetas-flow-{etiqueta}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let n = Nucleo::abrir(d.display().to_string(), "Mac".into()).unwrap();
        let chat = n.crear_chat("Clases".into()).unwrap();
        (n, d, chat.id)
    }

    #[test]
    fn las_carpetas_de_todos_los_chats_por_su_nombre_y_sin_repetir_al_crear() {
        let (n, d, chat) = nucleo("lista");
        let a = n.crear_carpeta_por_nombre(chat.clone(), "  ".into()).unwrap();
        assert_eq!(a.nombre, "Cuaderno");
        let b = n.crear_carpeta_por_nombre(chat.clone(), String::new()).unwrap();
        assert_eq!(b.nombre, "Cuaderno 2", "no repite el nombre");
        let otro = n.crear_chat("Obra".into()).unwrap().id;
        let diario = n.crear_carpeta_por_nombre(otro.clone(), "Diario de obra".into()).unwrap();

        let todas = n.carpetas_para_flow(String::new()).unwrap();
        assert_eq!(todas.carpetas.len(), 3);
        assert_eq!(todas.carpetas[0].id, diario.id, "la mas reciente primero");
        assert_eq!(todas.carpetas[0].nombre_chat, "Obra");
        assert!(!todas.exacta, "sin texto no hay exacta");

        let r = n.carpetas_para_flow("DIARIO".into()).unwrap();
        assert_eq!(r.carpetas.len(), 1);
        assert!(!r.exacta);
        assert!(n.carpetas_para_flow("diario de obra".into()).unwrap().exacta);
        assert_eq!(
            detalle_de_carpeta_de_flow(r.carpetas[0].clone()),
            "Carpeta de páginas · Vacía · Obra · Intro: abrirla"
        );

        // Por id; si ya no esta, por su nombre.
        let por_id = n.buscar_carpeta_de_paginas(chat.clone(), Some(b.id.clone()), String::new());
        assert_eq!(por_id.unwrap().map(|m| m.id), Some(b.id.clone()));
        let por_nombre = n.buscar_carpeta_de_paginas(otro.clone(), Some("999".into()), "diario".into());
        assert_eq!(por_nombre.unwrap().map(|m| m.id), Some(diario.id));
        // Casos negativos: lo que no esta, ni por id ni por nombre.
        assert_eq!(n.buscar_carpeta_de_paginas(chat.clone(), None, "No existe".into()).unwrap(), None);
        assert!(n.carpetas_para_flow("nada parecido".into()).unwrap().carpetas.is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn la_carpeta_de_una_grabacion_se_dice_asi_y_no_cuenta_paginas() {
        let c = CarpetaDeFlow {
            chat: "1".into(),
            nombre_chat: "Clases".into(),
            id: "2".into(),
            nombre: "Geometría".into(),
            paginas: 4,
            grabacion: true,
            cuando: 0,
        };
        assert_eq!(detalle_de_carpeta_de_flow(c.clone()), "Clase o reunión grabada · Clases · Intro: abrirla");
        assert_eq!(
            detalle_de_carpeta_de_flow(CarpetaDeFlow { grabacion: false, paginas: 1, ..c }),
            "Carpeta de páginas · 1 página · Clases · Intro: abrirla"
        );
    }
}
