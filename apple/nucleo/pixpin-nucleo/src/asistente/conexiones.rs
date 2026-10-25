//! **Las conexiones del asistente en el Mac** (Google Drive; Windows,
//! 17-oct, `asistente/conexiones.rs`): las cuatro herramientas (buscar,
//! leer, bajar, subir) con las piezas del Mac, y conectar, desconectar y el
//! estado para Ajustes. Los tokens van al Llavero (`claves`), nunca en claro.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use pixpin_ia::acciones::AccionApp;
use pixpin_ia::conexiones::{self, acciones::Chat, guardado};
use serde_json::json;

use super::documentos_acciones::{self as docs, Contexto, Sirve};
use crate::ErrorNucleo;

struct DelChat<'a, 'b> {
    cx: &'a Contexto<'b>,
}

impl Chat for DelChat<'_, '_> {
    fn archivo(&self, descripcion: Option<&str>) -> Result<(String, PathBuf), String> {
        docs::elegir(self.cx, descripcion, Sirve::Cualquiera).map(|c| (c.nombre, c.ruta))
    }

    fn texto_de(&self, ruta: &Path) -> Result<String, String> {
        let nombre = pixpin_docs::nombre(ruta);
        if docs::es_office_de_paginas(&nombre) || docs::es_pdf(ruta) {
            let p = docs::paginas(self.cx.app, ruta, self.cx.parar)?;
            return Ok(p.textos.iter().map(|t| t.texto.as_str()).collect::<Vec<_>>().join("\n\n"));
        }
        if docs::es_imagen(ruta) {
            let app = self.cx.app()?;
            return app.leer_texto(ruta.display().to_string(), Vec::new()).map_err(|e| e.to_string());
        }
        std::fs::read_to_string(ruta).map_err(|_| format!("«{nombre}» no se puede leer como texto"))
    }

    fn al_chat(&self, ruta: &Path, frase: &str) -> Result<(), String> {
        let mut p = json!({"pixpin": 1, "accion": "chat", "texto": frase, "archivos": [ruta]});
        if !self.cx.proyecto.trim().is_empty() {
            p["proyecto"] = json!(self.cx.proyecto);
        }
        docs::mandar(self.cx, p)
    }

    fn confirmar(&self, pregunta: &str) -> bool {
        self.cx.app.is_some_and(|a| a.confirmar(pregunta.to_string()))
    }

    fn carpeta(&self) -> PathBuf {
        std::env::temp_dir().join("pixpin-asistente").join("conexiones")
    }
}

/// **Hace una herramienta de las conexiones** (en el hilo del turno).
pub fn hacer(a: &AccionApp, cx: &Contexto) -> Result<String, String> {
    // «¿Que tengo hoy?»: de lo ya leido de Google Calendar (sin red).
    if a.herramienta == conexiones::acciones::VER_CALENDARIO {
        return Ok(crate::calendario_google::ver_calendario(cx.nucleo.ruta(), &a.pedido, cx.desfase_ms));
    }
    conexiones::acciones::hacer(a, &|s| conexiones::Conexion::abrir(s), &DelChat { cx })
}

// ---------------------------------------------------------------------------
// Ajustes → Asistente → Conexiones

/// Un servicio que se puede conectar, como se ve en Ajustes.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ConexionIa {
    pub id: String,
    pub nombre: String,
    /// Donde se crea el ID de cliente.
    pub consola: String,
    /// El ID de cliente guardado (no es secreto).
    pub cliente: Option<String>,
    pub conectado: bool,
    pub cuenta: Option<String>,
}

static ESPERA: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);

fn servicio(id: &str) -> Result<&'static conexiones::Servicio, ErrorNucleo> {
    conexiones::por_id(id).ok_or_else(|| ErrorNucleo::Argumento { detalle: format!("servicio desconocido: {id}") })
}

fn carpeta() -> PathBuf {
    guardado::carpeta().unwrap_or_default()
}

fn mal(detalle: String) -> ErrorNucleo {
    ErrorNucleo::Asistente { detalle }
}

/// Los servicios y como estan.
#[uniffi::export]
pub fn ia_conexiones() -> Vec<ConexionIa> {
    conexiones::SERVICIOS
        .iter()
        .map(|s| {
            let g = guardado::leer_de(&carpeta(), s);
            ConexionIa {
                id: s.id.into(),
                nombre: s.nombre.into(),
                consola: s.consola.into(),
                cliente: g.cliente.as_ref().map(|c| c.id.clone()),
                conectado: g.conectado(),
                cuenta: g.cuenta.clone(),
            }
        })
        .collect()
}

/// Guarda el ID de cliente pegado (solo, con su secreto, o el JSON de Google).
#[uniffi::export]
pub fn ia_guardar_cliente(servicio_id: String, pegado: String) -> Result<String, ErrorNucleo> {
    let s = servicio(&servicio_id)?;
    conexiones::guardar_cliente_en(s, &carpeta(), &pegado).map(|c| c.id).map_err(mal)
}

/// **Conecta**: abre el navegador y espera la vuelta (hasta 5 min). Bloquea:
/// Swift lo llama fuera del hilo principal. Devuelve la cuenta.
#[uniffi::export]
pub fn ia_conectar(servicio_id: String) -> Result<String, ErrorNucleo> {
    let abrir = |url: &str| -> Result<(), String> {
        std::process::Command::new("open").arg(url).status().map(|_| ()).map_err(|e| format!("no se pudo abrir el navegador: {e}"))
    };
    conectar_abriendo(&servicio_id, &abrir)
}

/// **Quien abre el navegador** para dar permiso (lo implementa Swift): en
/// iPadOS no hay `open`, y la pagina la abre `ASWebAuthenticationSession`
/// o `UIApplication.open`. Devuelve si se pudo abrir.
#[uniffi::export(callback_interface)]
pub trait AbrirNavegador: Send + Sync {
    fn abrir(&self, url: String) -> bool;
}

/// **Conecta** como [`ia_conectar`], pero la pagina de permiso la abre
/// Swift con `abrir` (iPad e iPhone). Bloquea hasta la vuelta (hasta 5 min).
#[uniffi::export]
pub fn ia_conectar_con(
    servicio_id: String,
    abrir: Box<dyn AbrirNavegador>,
) -> Result<String, ErrorNucleo> {
    let abrir = |url: &str| -> Result<(), String> {
        if abrir.abrir(url.to_string()) {
            Ok(())
        } else {
            Err("no se pudo abrir el navegador".to_string())
        }
    };
    conectar_abriendo(&servicio_id, &abrir)
}

/// Lo comun de conectar: corta la espera de antes y espera la vuelta.
fn conectar_abriendo(
    servicio_id: &str,
    abrir: &dyn Fn(&str) -> Result<(), String>,
) -> Result<String, ErrorNucleo> {
    let s = servicio(servicio_id)?;
    let parar = Arc::new(AtomicBool::new(false));
    if let Some(antes) = ESPERA.lock().unwrap_or_else(|e| e.into_inner()).replace(parar.clone()) {
        antes.store(true, Ordering::SeqCst);
    }
    conexiones::conectar_en(s, &s.extremos(), &carpeta(), abrir, &parar, conexiones::oauth::ESPERA).map_err(mal)
}

/// Corta la espera al navegador.
#[uniffi::export]
pub fn ia_cancelar_conexion() {
    if let Some(p) = ESPERA.lock().unwrap_or_else(|e| e.into_inner()).take() {
        p.store(true, Ordering::SeqCst);
    }
}

/// Desconecta (borra los tokens y retira el permiso).
#[uniffi::export]
pub fn ia_desconectar(servicio_id: String) -> Result<(), ErrorNucleo> {
    ia_cancelar_conexion();
    let s = servicio(&servicio_id)?;
    conexiones::desconectar_en(s, &s.extremos(), &carpeta()).map_err(mal)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn drive_sale_en_ajustes_y_sin_conectar_lo_dice_al_modelo() {
        let c = ia_conexiones();
        let d = c.iter().find(|c| c.id == "google_drive").expect("Drive en la lista");
        assert_eq!(d.nombre, "Google Drive");
        assert!(!d.conectado);
        // Caso negativo: un servicio que no existe, o algo que no es un ID.
        assert!(ia_guardar_cliente("dropbox".into(), "x".into()).is_err());
        assert!(ia_guardar_cliente("google_drive".into(), "no es un id".into()).is_err());
        assert!(ia_desconectar("nada".into()).is_err());
    }

    struct Abridor(std::sync::Arc<std::sync::atomic::AtomicU32>);

    impl AbrirNavegador for Abridor {
        fn abrir(&self, _url: String) -> bool {
            self.0.fetch_add(1, Ordering::SeqCst);
            false
        }
    }

    #[test]
    fn conectar_con_un_servicio_que_no_existe_no_abre_nada() {
        let veces = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        // Caso negativo: el servicio no esta, y el navegador ni se abre.
        assert!(ia_conectar_con("dropbox".into(), Box::new(Abridor(veces.clone()))).is_err());
        assert_eq!(veces.load(Ordering::SeqCst), 0);
    }
}
