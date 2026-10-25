//! **`pixpin-puente`: la extension del navegador habla con PixPin** (el de
//! Windows en el Mac: el pedido va por el enlace firmado, `mensajero`).
//!
//! Chrome y Edge no dejan que una extension mande `WM_COPYDATA`; si dejan
//! que arranque un programa registrado («host» de mensajeria nativa) y le
//! pase JSON por la entrada estandar. Este es ese programa: el navegador lo
//! arranca en cada clic del menu de la extension (`chrome.runtime.
//! sendNativeMessage`), el puente lee UN mensaje ([`protocolo`]), lo
//! traduce a un pedido de PixPin ([`traducir`]), deja en la cache la imagen
//! si la trae, manda el pedido a la PixPin abierta por el mismo camino que el
//! plugin de Flow (`crate::mensajero`), contesta y se va.
//!
//! **Rapido**: no carga nada de la app ni la espera mas que a su ventana de
//! mensajes; lo que tarda de verdad (OCR, la voz, mandar al movil) lo hace
//! PixPin en sus hilos despues de contestar. Si PixPin no esta abierta, se
//! arranca y se espera a su ventana, como hace el lanzador.
//!
//! Las imagenes se quedan en `<raiz>\cache\navegador\` (un pin las necesita
//! en el disco mientras esta abierto; el chat las copia a su carpeta) y se
//! borran a los [`DIAS_EN_CACHE`] dias, en la siguiente vuelta del puente.

pub mod base64;
pub mod mensajero;
pub mod protocolo;
pub mod traducir;

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use serde_json::json;

use protocolo::ErrorLectura;

/// Cuanto viven los ficheros del puente en la cache.
pub const DIAS_EN_CACHE: u64 = 7;

/// La carpeta de la cache del puente dentro de la raiz de datos.
pub const CARPETA_CACHE: &str = "navegador";

/// Cuanto se espera a que aparezca PixPin tras arrancarla.
pub const ESPERA_ARRANQUE: Duration = Duration::from_secs(8);

/// Quien entrega el pedido: PixPin de verdad o, en las pruebas, uno que solo
/// apunta.
pub trait Cartero {
    /// Manda el JSON y devuelve la respuesta de la ventana (`1` aceptado,
    /// `2` version, `3` no se entiende, `0` nadie escucha).
    fn mandar(&mut self, json: &str) -> isize;
    /// Arranca PixPin. `Err` si no se pudo.
    fn arrancar(&mut self) -> Result<(), String>;
    /// Espera un poco antes de volver a probar.
    fn esperar(&mut self, cuanto: Duration) {
        std::thread::sleep(cuanto);
    }
}

/// El cartero de verdad: `WM_COPYDATA` a la PixPin abierta.
pub struct AlaApp {
    /// `pixpinmax.exe`, para arrancarla si no esta.
    pub exe: Option<PathBuf>,
}

impl Cartero for AlaApp {
    fn mandar(&mut self, json: &str) -> isize {
        crate::mensajero::enviar_pedido(json)
    }

    fn arrancar(&mut self) -> Result<(), String> {
        let app = self.exe.as_ref().filter(|e| e.exists()).ok_or("No se encuentra PixPin.app")?;
        std::process::Command::new("open").arg("-g").arg(app).status().map(|_| ()).map_err(|e| e.to_string())
    }
}

/// Un cartero que no manda nada (modo «en seco» de las pruebas: el pedido
/// sale en la respuesta).
pub struct EnSeco;

impl Cartero for EnSeco {
    fn mandar(&mut self, _json: &str) -> isize {
        crate::mensajero::respuesta::ACEPTADO
    }
    fn arrancar(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Manda el pedido; si nadie escucha, arranca PixPin y reintenta hasta que
/// su ventana conteste o pasen [`ESPERA_ARRANQUE`].
pub fn entregar(cartero: &mut dyn Cartero, json: &str) -> Result<(), String> {
    let mut r = cartero.mandar(json);
    if r == 0 {
        cartero.arrancar()?;
        let inicio = Instant::now();
        while r == 0 {
            if inicio.elapsed() > ESPERA_ARRANQUE {
                return Err("PixPin no se abrió a tiempo".into());
            }
            cartero.esperar(Duration::from_millis(100));
            r = cartero.mandar(json);
        }
    }
    use crate::mensajero::respuesta;
    match r {
        respuesta::ACEPTADO => Ok(()),
        respuesta::VERSION_NO => {
            Err("Esta PixPin es más vieja que la extensión: actualízala".into())
        }
        respuesta::NO_SE_ENTIENDE => Err("Esta PixPin no entiende el pedido: actualízala".into()),
        otra => Err(format!("PixPin no contestó (respuesta {otra})")),
    }
}

/// Donde vive cada cosa.
#[derive(Debug, Clone)]
pub struct Entorno {
    /// La raiz de datos de PixPin.
    pub raiz: PathBuf,
    /// Un nombre unico para los ficheros de esta vuelta.
    pub sello: String,
    /// Devolver el pedido en la respuesta (pruebas).
    pub ensenar_pedido: bool,
}

impl Entorno {
    /// El de verdad: la regla de `pixpin_store::rutas::resolver` (portable si
    /// junto al exe hay un `pixpinmax.toml`, si no `%APPDATA%\PixPinMax`), o
    /// la carpeta de `PIXPIN_PUENTE_EN_SECO` en las pruebas.
    pub fn del_equipo(dir_exe: Option<&Path>, en_seco: Option<PathBuf>) -> Option<Entorno> {
        let sello = format!(
            "{}-{}",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0),
            std::process::id()
        );
        let raiz = match en_seco {
            Some(r) => r,
            // La del Mac: Application Support/PixPinMax (`carpetaDeDatos`).
            None => {
                let _ = dir_exe;
                PathBuf::from(std::env::var_os("HOME")?).join("Library/Application Support/PixPinMax")
            }
        };
        Some(Entorno {
            raiz,
            sello,
            ensenar_pedido: false,
        })
    }

    pub fn carpeta(&self) -> PathBuf {
        self.raiz.join("cache").join(CARPETA_CACHE)
    }
}

/// `pixpinmax.exe`: junto al puente (asi se instala) o en el sitio fijo de
/// `herramientas/instalar.ps1`.
pub fn exe_de_pixpin(dir_exe: Option<&Path>) -> Option<PathBuf> {
    // Dentro de la app (`PixPin.app/Contents/MacOS/pixpin-puente`) o en
    // /Applications.
    let dentro = dir_exe.and_then(|d| d.parent()).and_then(|c| c.parent()).filter(|a| a.extension().is_some_and(|e| e == "app"));
    Some(dentro.map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("/Applications/PixPin.app")))
}

/// **Una vuelta del puente**: lee un mensaje, lo hace y contesta. Devuelve
/// el codigo de salida (0 salvo que no se pudiera ni contestar).
pub fn correr(
    entrada: &mut impl Read,
    salida: &mut impl Write,
    entorno: &Entorno,
    cartero: &mut dyn Cartero,
) -> i32 {
    let respuesta = match atender(entrada, entorno, cartero) {
        Ok(None) => return 0,
        Ok(Some(r)) => r,
        Err(e) => json!({"ok": false, "error": e}),
    };
    match protocolo::escribir_mensaje(salida, respuesta.to_string().as_bytes()) {
        Ok(()) => {
            // Despues de contestar: el navegador ya no espera por esto.
            limpiar_viejos(&entorno.carpeta(), DIAS_EN_CACHE);
            0
        }
        Err(_) => 1,
    }
}

/// Lo de [`correr`] sin escribir: la respuesta, o `None` si no llego nada.
fn atender(
    entrada: &mut impl Read,
    entorno: &Entorno,
    cartero: &mut dyn Cartero,
) -> Result<Option<serde_json::Value>, String> {
    let bytes = match protocolo::leer_mensaje(entrada, protocolo::TOPE_ENTRADA) {
        Ok(b) => b,
        Err(ErrorLectura::Fin) => return Ok(None),
        Err(ErrorLectura::Enorme(n)) => {
            return Err(format!(
                "Demasiado grande para PixPin ({} MB; el tope es {} MB)",
                n.div_ceil(1024 * 1024),
                protocolo::TOPE_ENTRADA / (1024 * 1024)
            ));
        }
        Err(e) => return Err(format!("Mensaje cortado o roto: {e:?}")),
    };
    let mensaje: traducir::Mensaje =
        serde_json::from_slice(&bytes).map_err(|e| format!("Mensaje roto (JSON): {e}"))?;
    let plan = traducir::traducir(&mensaje, &entorno.carpeta(), &entorno.sello)?;
    for (ruta, datos) in &plan.ficheros {
        if let Some(d) = ruta.parent() {
            std::fs::create_dir_all(d).map_err(|e| format!("No se pudo guardar: {e}"))?;
        }
        std::fs::write(ruta, datos).map_err(|e| format!("No se pudo guardar: {e}"))?;
    }
    if let Err(e) = entregar(cartero, &plan.pedido) {
        // Lo escrito no le sirve a nadie si el pedido no llego.
        for (ruta, _) in &plan.ficheros {
            let _ = std::fs::remove_file(ruta);
        }
        return Err(e);
    }
    let mut r = json!({"ok": true, "aviso": plan.hecho});
    if plan.recortado {
        r["aviso"] = json!(format!(
            "{} (el texto era muy largo y se recortó)",
            plan.hecho
        ));
    }
    if entorno.ensenar_pedido {
        r["pedido"] = json!(plan.pedido);
    }
    Ok(Some(r))
}

/// Borra lo de la cache del puente que tenga mas de `dias` dias. Solo lo
/// suyo (`nav-…`): la carpeta es del puente, pero por si acaso.
pub fn limpiar_viejos(carpeta: &Path, dias: u64) {
    let Ok(entradas) = std::fs::read_dir(carpeta) else {
        return;
    };
    let limite = Duration::from_secs(dias * 24 * 3600);
    for e in entradas.flatten() {
        if !e.file_name().to_string_lossy().starts_with("nav-") {
            continue;
        }
        let viejo = e
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|edad| edad > limite);
        if viejo {
            let ruta = e.path();
            let _ = if ruta.is_dir() {
                std::fs::remove_dir_all(&ruta)
            } else {
                std::fs::remove_file(&ruta)
            };
        }
    }
}

#[cfg(test)]
mod pruebas;
