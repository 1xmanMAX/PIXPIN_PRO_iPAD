//! **Ver la pantalla** en el Mac: `screencapture` del sistema (pide el
//! permiso de grabar la pantalla la primera vez, al terminal o al cliente
//! que arranco el MCP). Las ventanas sueltas no: en el Mac eso pide el
//! permiso de accesibilidad.

use serde_json::{Map, Value};

use crate::herramientas::{Contenido, Resultado};

pub fn ver_pantalla(_args: &Map<String, Value>) -> Resultado {
    let ruta = std::env::temp_dir().join(format!("pixpin-mcp-pantalla-{}.png", std::process::id()));
    let ok = std::process::Command::new("screencapture")
        .args(["-x", "-t", "png"])
        .arg(&ruta)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    let datos = std::fs::read(&ruta).ok().filter(|d| ok && !d.is_empty());
    let _ = std::fs::remove_file(&ruta);
    match datos {
        Some(datos) => Ok(vec![Contenido::Imagen { datos, mime: "image/png" }]),
        None => Err("no se pudo capturar la pantalla (¿falta el permiso de grabar la pantalla?)".into()),
    }
}

pub fn listar_ventanas(_args: &Map<String, Value>) -> Resultado {
    Err("en el Mac, listar ventanas aun no: usa ver_pantalla".into())
}
