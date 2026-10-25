//! **Las claves del asistente en el Llavero del Mac** (Keychain).
//!
//! `pixpin_ia::nube::claves` no guarda una clave en claro nunca: en Windows
//! usa DPAPI y fuera de Windows espera a que la app instale su
//! [`claves::Llavero`]. Aqui ese llavero es un **interfaz que implementa
//! Swift** ([`LlaveroApp`], un trait de UniFFI con implementacion de fuera)
//! con `SecItemAdd`/`SecItemCopyMatching`/`SecItemDelete`. Swift lo instala
//! una vez al arrancar con [`instalar_llavero`].
//!
//! El servicio es [`SERVICIO`] y la cuenta el id del proveedor
//! (`claude`, `deepseek`), lo que dice el trait de `pixpin-ia`.
//!
//! Ni la clave ni un trozo pasan por el registro: para ensenarla,
//! [`ia_clave_enmascarada`].

use std::sync::Arc;

use pixpin_ia::nube::Proveedor;
use pixpin_ia::nube::claves;

use crate::ErrorNucleo;

/// El servicio del Llavero donde viven las claves del asistente.
pub const SERVICIO: &str = "PixPinMax.asistente";

/// El de las conexiones del asistente (Google Drive): su ID de cliente y sus
/// tokens de OAuth, en JSON (en Windows, `conexion-<servicio>.cifrado`).
pub const SERVICIO_CONEXIONES: &str = "PixPinMax.conexiones";

/// **El Llavero de la app**, implementado en Swift. Cada metodo es una
/// llamada al sistema (rapida); se llama solo al guardar o al usar una
/// clave, nunca en bucle.
#[uniffi::export(foreign)]
pub trait LlaveroApp: Send + Sync {
    /// Guarda (o sustituye) la clave de `cuenta` en `servicio`.
    fn guardar(&self, servicio: String, cuenta: String, clave: String) -> Result<(), ErrorNucleo>;
    /// La clave guardada, o `None` si no hay o no se puede leer.
    fn leer(&self, servicio: String, cuenta: String) -> Option<String>;
    /// Borra la clave. Borrar lo que no esta no es un error.
    fn borrar(&self, servicio: String, cuenta: String) -> Result<(), ErrorNucleo>;
}

impl From<uniffi::UnexpectedUniFFICallbackError> for ErrorNucleo {
    fn from(e: uniffi::UnexpectedUniFFICallbackError) -> Self {
        ErrorNucleo::Disco { detalle: e.reason }
    }
}

/// El adaptador al trait de `pixpin-ia`.
struct Adaptador {
    app: Arc<dyn LlaveroApp>,
}

impl claves::Llavero for Adaptador {
    fn guardar(&self, p: Proveedor, clave: &str) -> Result<(), String> {
        self.app
            .guardar(SERVICIO.into(), p.id().into(), clave.into())
            .map_err(|e| e.to_string())
    }
    fn leer(&self, p: Proveedor) -> Option<String> {
        self.app.leer(SERVICIO.into(), p.id().into())
    }
    fn borrar(&self, p: Proveedor) -> Result<(), String> {
        self.app
            .borrar(SERVICIO.into(), p.id().into())
            .map_err(|e| e.to_string())
    }
    fn guardar_secreto(&self, nombre: &str, valor: &str) -> Result<(), String> {
        self.app
            .guardar(SERVICIO_CONEXIONES.into(), nombre.into(), valor.into())
            .map_err(|e| e.to_string())
    }
    fn leer_secreto(&self, nombre: &str) -> Option<String> {
        self.app.leer(SERVICIO_CONEXIONES.into(), nombre.into())
    }
    fn borrar_secreto(&self, nombre: &str) -> Result<(), String> {
        self.app
            .borrar(SERVICIO_CONEXIONES.into(), nombre.into())
            .map_err(|e| e.to_string())
    }
}

fn proveedor(id: &str) -> Result<Proveedor, ErrorNucleo> {
    Proveedor::por_id(id).ok_or_else(|| ErrorNucleo::Argumento {
        detalle: format!("proveedor desconocido: {id:?}"),
    })
}

/// **Instala el Llavero de la app** (una vez, al arrancar). Instalarlo otra
/// vez da error de argumento y se queda el primero.
#[uniffi::export]
pub fn instalar_llavero(llavero: Arc<dyn LlaveroApp>) -> Result<(), ErrorNucleo> {
    claves::instalar_llavero(Box::new(Adaptador { app: llavero }))
        .map_err(|detalle| ErrorNucleo::Argumento { detalle })
}

/// Guarda la clave de un proveedor (`claude`, `deepseek`) en el llavero
/// instalado. Se recorta; una clave vacia borra la que hubiera. Sin
/// llavero instalado es un error: nunca se guarda en claro.
#[uniffi::export]
pub fn ia_guardar_clave(proveedor_id: String, clave: String) -> Result<(), ErrorNucleo> {
    let p = proveedor(&proveedor_id)?;
    claves::guardar(p, &clave).map_err(|detalle| ErrorNucleo::Asistente { detalle })
}

/// Si hay clave guardada para el proveedor.
#[uniffi::export]
pub fn ia_hay_clave(proveedor_id: String) -> bool {
    proveedor(&proveedor_id).is_ok_and(claves::hay)
}

/// Lo que se ensena de la clave guardada (`sk-ant-…AbCd`), o `None`.
#[uniffi::export]
pub fn ia_clave_enmascarada(proveedor_id: String) -> Option<String> {
    let p = proveedor(&proveedor_id).ok()?;
    claves::leer(p).map(|c| claves::enmascarar(&c))
}

/// Borra la clave del proveedor.
#[uniffi::export]
pub fn ia_borrar_clave(proveedor_id: String) -> Result<(), ErrorNucleo> {
    let p = proveedor(&proveedor_id)?;
    claves::guardar(p, "").map_err(|detalle| ErrorNucleo::Asistente { detalle })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// Un llavero en memoria (el papel del Llavero de Swift).
    #[derive(Default)]
    struct EnMemoria {
        claves: Mutex<HashMap<(String, String), String>>,
    }

    impl LlaveroApp for EnMemoria {
        fn guardar(&self, s: String, c: String, clave: String) -> Result<(), ErrorNucleo> {
            self.claves.lock().unwrap().insert((s, c), clave);
            Ok(())
        }
        fn leer(&self, s: String, c: String) -> Option<String> {
            self.claves.lock().unwrap().get(&(s, c)).cloned()
        }
        fn borrar(&self, s: String, c: String) -> Result<(), ErrorNucleo> {
            self.claves.lock().unwrap().remove(&(s, c));
            Ok(())
        }
    }

    #[test]
    fn el_adaptador_usa_el_servicio_y_la_cuenta_del_proveedor() {
        let mem = Arc::new(EnMemoria::default());
        let a = Adaptador { app: mem.clone() };
        claves::guardar_con(&a, Proveedor::Claude, "  sk-ant-api03-ABCDEFGHIJKLMNOP  ").unwrap();
        assert_eq!(
            mem.leer(SERVICIO.into(), "claude".into()).as_deref(),
            Some("sk-ant-api03-ABCDEFGHIJKLMNOP"),
            "recortada, en su servicio y cuenta"
        );
        assert!(claves::Llavero::hay(&a, Proveedor::Claude));
        // Caso negativo: la de otro proveedor no esta.
        assert!(!claves::Llavero::hay(&a, Proveedor::DeepSeek));
        // Vacia es borrar.
        claves::guardar_con(&a, Proveedor::Claude, "   ").unwrap();
        assert!(claves::leer_con(&a, Proveedor::Claude).is_none());
    }

    #[test]
    fn instalado_guarda_lee_y_borra_por_el_puente() {
        // Una sola vez por proceso: esta es la unica prueba que instala.
        instalar_llavero(Arc::new(EnMemoria::default())).unwrap();
        assert!(matches!(
            instalar_llavero(Arc::new(EnMemoria::default())),
            Err(ErrorNucleo::Argumento { .. })
        ));
        ia_guardar_clave("deepseek".into(), "sk-1234567890abcdefWXYZ".into()).unwrap();
        assert!(ia_hay_clave("deepseek".into()));
        assert_eq!(
            ia_clave_enmascarada("deepseek".into()).as_deref(),
            Some("sk-…WXYZ")
        );
        ia_borrar_clave("deepseek".into()).unwrap();
        assert!(!ia_hay_clave("deepseek".into()));
        // Caso negativo: un proveedor que no existe.
        assert!(matches!(
            ia_guardar_clave("openai".into(), "x".into()),
            Err(ErrorNucleo::Argumento { .. })
        ));
        assert!(!ia_hay_clave("openai".into()));
    }
}
