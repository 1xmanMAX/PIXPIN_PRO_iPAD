//! **El mensajero del Mac** (el `pixpin_shell::mensajero` de Windows): en
//! vez de `WM_COPYDATA`, el pedido va por el enlace `pixpinmax://pedido`,
//! firmado con la clave de la app (`enlaces.clave`) y abierto con
//! `open -g` (arranca PixPin si esta cerrada).

use std::path::PathBuf;

use sha2::{Digest, Sha256};

/// Lo mas largo de un pedido.
pub const TOPE_PEDIDO: usize = 64 * 1024;

/// La version del protocolo (`"pixpin": 1`).
pub const VERSION_PEDIDO: u64 = 1;

pub mod respuesta {
    pub const ACEPTADO: isize = 1;
    pub const VERSION_NO: isize = 2;
    pub const NO_SE_ENTIENDE: isize = 3;
}

pub const ACCIONES: [&str; 37] = [
    "abrir",
    "chat",
    "nota_nueva",
    "lienzo_nuevo",
    "grabar",
    "lista_nueva",
    "anadir_tarea",
    "marcar_tarea",
    "ventana_principal",
    "pinear",
    "iconos",
    "soltar",
    "reproducir",
    "leccion_nueva",
    "lecciones",
    "mover_tarea",
    "ventana",
    "capturar",
    "pinear_ultima",
    "conservar_captura",
    "borrar_captura",
    "copiar_imagen",
    "quitar_tarea",
    "corregir_tarea",
    "borrar_lista",
    "enviar_al_movil",
    "sincronizar",
    // 13-oct: el asistente sin abrir el chat (`p ia …`) y sus
    // automatizaciones.
    "asistente",
    "automatizacion",
    // 14-oct: una tabla del chat ya rellena (el asistente, tabla escaneada).
    "tabla_nueva",
    // 15-oct: «+10 min» de la notificacion de un recordatorio
    // (`notificacion`).
    "posponer",
    // 15-oct: el asistente maneja los pines, el timeline y la voz.
    "pines",
    "timeline",
    // 15-oct: la extension del navegador (`apps/pixpin-puente`): leer en voz
    // alta un texto suelto y pasar el texto de una imagen al portapapeles.
    "leer_en_voz",
    "ocr",
    // 15-oct: una herramienta del asistente con sus argumentos y el
    // resultado por un fichero (el servidor MCP, `apps/pixpin-mcp`).
    "herramienta",
    // 17-oct: fotos y texto a un lienzo por su nombre, apilados.
    "al_lienzo",
];

/// El vistazo de la ventana de Windows: JSON, version y accion conocida.
pub fn validar_pedido(json: &str) -> isize {
    let Ok(serde_json::Value::Object(pedido)) = serde_json::from_str::<serde_json::Value>(json) else {
        return respuesta::NO_SE_ENTIENDE;
    };
    match pedido.get("pixpin").and_then(|v| v.as_u64()) {
        Some(VERSION_PEDIDO) => {}
        Some(_) => return respuesta::VERSION_NO,
        None => return respuesta::NO_SE_ENTIENDE,
    }
    match pedido.get("accion").and_then(|v| v.as_str()) {
        Some(a) if ACCIONES.contains(&a) => respuesta::ACEPTADO,
        _ => respuesta::NO_SE_ENTIENDE,
    }
}

/// Los bytes que no se tocan al codificar un campo (los «no reservados»).
fn codificar(t: &str) -> String {
    let mut s = String::with_capacity(t.len() * 3);
    for b in t.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            s.push(b as char);
        } else {
            s.push_str(&format!("%{b:02X}"));
        }
    }
    s
}

/// HMAC-SHA256 (RFC 2104), en hexadecimal.
pub fn hmac(clave: &[u8], texto: &[u8]) -> String {
    let mut k = [0u8; 64];
    if clave.len() > 64 {
        k[..32].copy_from_slice(&Sha256::digest(clave));
    } else {
        k[..clave.len()].copy_from_slice(clave);
    }
    let mut dentro = Sha256::new();
    dentro.update(k.map(|b| b ^ 0x36));
    dentro.update(texto);
    let mut fuera = Sha256::new();
    fuera.update(k.map(|b| b ^ 0x5c));
    fuera.update(dentro.finalize());
    fuera.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// La clave de firmar de la app, si ya firmo algo alguna vez.
fn clave() -> Option<Vec<u8>> {
    let casa = std::env::var_os("HOME").map(PathBuf::from)?;
    let d = std::fs::read(casa.join("Library/Application Support/PixPinMax/enlaces.clave")).ok()?;
    (d.len() == 32).then_some(d)
}

/// **El enlace del pedido**, firmado si hay clave (lo canonico es el verbo y
/// los campos ya descodificados, uno por renglon: `pedido\njson=…`).
pub fn enlace(json: &str, clave: Option<&[u8]>) -> String {
    let mut u = format!("pixpinmax://pedido?json={}", codificar(json));
    if let Some(k) = clave {
        u.push_str("&firma=");
        u.push_str(&hmac(k, format!("pedido\njson={json}").as_bytes()));
    }
    u
}

/// **Manda el pedido a PixPin**: `1` si se abrio el enlace, `3` si no se
/// entiende, `0` si no se pudo abrir.
pub fn enviar_pedido(json: &str) -> isize {
    let v = validar_pedido(json);
    if v != respuesta::ACEPTADO {
        return v;
    }
    let u = enlace(json, clave().as_deref());
    let ok = std::process::Command::new("open").arg("-g").arg(&u).status().map(|s| s.success()).unwrap_or(false);
    if ok { respuesta::ACEPTADO } else { 0 }
}
