//! **Las claves de las API, cifradas con DPAPI.**
//!
//! Pedido del usuario: que solo se le pida la clave de cada proveedor. Una
//! clave es dinero: no puede quedar en claro en ningun sitio. Por eso:
//!
//! - Se cifra con `CryptProtectData` en el **ambito del usuario** de Windows
//!   (solo esta cuenta en este equipo la descifra; otra cuenta, otro PC o
//!   una copia del disco no), con una entropia propia de PixPin para que otro
//!   programa que use DPAPI sin ella no la saque por descuido.
//! - Vive en su **fichero propio** en `%LOCALAPPDATA%\PixPinMax\claves`:
//!   nunca en el TOML de ajustes (que el usuario abre, copia y comparte),
//!   nunca en la carpeta de datos que se sincroniza o se exporta, y
//!   `LOCALAPPDATA` no viaja con el perfil movil de Windows.
//! - Ni la clave ni un trozo de ella pasan por `tracing`: para ensenarla se
//!   usa [`enmascarar`], que deja ver solo el final.
//!
//! Lo de DPAPI habla con el sistema operativo: cada `unsafe` lleva su
//! `// SAFETY:`.
//!
//! ## Fuera de Windows: un [`Llavero`] que pone la app
//!
//! DPAPI solo existe en Windows, y en Apple el sitio de una clave es el
//! **Llavero** (Keychain, que ademas puede viajar por iCloud Keychain).
//! El nucleo no lo llama: define el trait [`Llavero`] y la app de Swift
//! instala el suyo al arrancar con [`instalar_llavero`] (por el puente de
//! UniFFI, fase 1). Sin llavero instalado, [`guardar`] da un error y
//! [`leer`] da `None`: **nunca se escribe una clave en claro** como apano.
//! En Windows, si nadie instala otro, sigue DPAPI como siempre.

#[cfg(windows)]
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[cfg(windows)]
use windows::Win32::Foundation::{HLOCAL, LocalFree};
#[cfg(windows)]
use windows::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
};
#[cfg(windows)]
use windows::core::w;

use super::Proveedor;

/// **Donde viven las claves**, cifradas por el sistema. En Apple lo
/// implementa la app con el Llavero (`SecItemAdd`/`SecItemCopyMatching`,
/// servicio `PixPinMax.asistente`, cuenta = [`Proveedor::id`]). Las claves
/// que llegan ya vienen recortadas y no vacias.
pub trait Llavero: Send + Sync {
    fn guardar(&self, p: Proveedor, clave: &str) -> Result<(), String>;
    /// `None` si no hay o no se puede leer: se pide otra vez.
    fn leer(&self, p: Proveedor) -> Option<String>;
    /// Borrar lo que no esta no es un error.
    fn borrar(&self, p: Proveedor) -> Result<(), String>;
    /// ¿Hay clave guardada? (Sin descifrarla, si el sistema lo permite.)
    fn hay(&self, p: Proveedor) -> bool {
        self.leer(p).is_some()
    }
    /// Un secreto con nombre que no es una clave de API (los tokens de las
    /// conexiones del asistente, `conexion-google-drive`). Fuera de Windows
    /// van al Llavero como las claves; sin soporte, error.
    fn guardar_secreto(&self, _nombre: &str, _valor: &str) -> Result<(), String> {
        Err("este llavero no guarda secretos con nombre".into())
    }
    fn leer_secreto(&self, _nombre: &str) -> Option<String> {
        None
    }
    fn borrar_secreto(&self, _nombre: &str) -> Result<(), String> {
        Ok(())
    }
}

/// Guarda un secreto con nombre en el llavero instalado (fuera de Windows,
/// el de la app). Sin llavero, error: nunca en claro.
pub fn guardar_secreto(nombre: &str, valor: &str) -> Result<(), String> {
    llavero()
        .ok_or_else(|| "no hay llavero donde guardar".to_string())?
        .guardar_secreto(nombre, valor)
}

/// El secreto con nombre, o `None`.
pub fn leer_secreto(nombre: &str) -> Option<String> {
    llavero()?.leer_secreto(nombre)
}

/// Borra el secreto con nombre (lo que no esta no es un error).
pub fn borrar_secreto(nombre: &str) -> Result<(), String> {
    llavero().map_or(Ok(()), |l| l.borrar_secreto(nombre))
}

/// Fuera de Windows no hay carpeta de claves: todo va al Llavero. Se da
/// una ruta simbolica para que quien la pide siga (las conexiones la
/// ignoran fuera de las pruebas).
#[cfg(not(windows))]
pub fn carpeta() -> Option<std::path::PathBuf> {
    Some(std::path::PathBuf::from("llavero"))
}

/// En las pruebas fuera de Windows no hay DPAPI: un «cifrado» de juguete
/// (marca y XOR), que no deja nada legible, para probar el mismo camino de
/// ficheros que en Windows. Fuera de las pruebas no existe: todo va al
/// Llavero.
#[cfg(all(not(windows), test))]
const MARCA_PRUEBA: &[u8] = b"PXPRUEBA";

#[cfg(all(not(windows), test))]
pub fn cifrar(claro: &[u8]) -> Result<Vec<u8>, String> {
    Ok(MARCA_PRUEBA
        .iter()
        .copied()
        .chain(claro.iter().map(|b| b ^ 0xA5))
        .collect())
}

#[cfg(all(not(windows), test))]
pub fn descifrar(cifrado: &[u8]) -> Result<Vec<u8>, String> {
    let resto = cifrado
        .strip_prefix(MARCA_PRUEBA)
        .ok_or("no esta cifrado")?;
    Ok(resto.iter().map(|b| b ^ 0xA5).collect())
}

static LLAVERO: OnceLock<Box<dyn Llavero>> = OnceLock::new();

/// **Instala el llavero de la app** (una vez, al arrancar). `Err` si ya
/// habia uno.
pub fn instalar_llavero(l: Box<dyn Llavero>) -> Result<(), String> {
    LLAVERO
        .set(l)
        .map_err(|_| "ya habia un llavero instalado".to_string())
}

/// El llavero en uso: el instalado o, en Windows, DPAPI.
fn llavero() -> Option<&'static dyn Llavero> {
    if let Some(l) = LLAVERO.get() {
        return Some(l.as_ref());
    }
    #[cfg(windows)]
    {
        Some(&Dpapi)
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Guardar con un llavero dado: recorta, y una clave vacia la borra.
pub fn guardar_con(l: &dyn Llavero, p: Proveedor, clave: &str) -> Result<(), String> {
    let clave = clave.trim();
    if clave.is_empty() {
        l.borrar(p)
    } else {
        l.guardar(p, clave)
    }
}

/// Leer con un llavero dado: una clave en blanco cuenta como ninguna.
pub fn leer_con(l: &dyn Llavero, p: Proveedor) -> Option<String> {
    l.leer(p).filter(|c| !c.trim().is_empty())
}

/// Mezclada con la clave al cifrar: sin ella no se descifra.
#[cfg(windows)]
const ENTROPIA: &[u8] = b"PixPinMax/asistente/nube/v1";

/// El llavero de Windows: DPAPI del usuario, un fichero por proveedor en
/// [`carpeta`].
#[cfg(windows)]
struct Dpapi;

#[cfg(windows)]
impl Llavero for Dpapi {
    fn guardar(&self, p: Proveedor, clave: &str) -> Result<(), String> {
        guardar_en(&carpeta().ok_or("no se encuentra LOCALAPPDATA")?, p, clave)
    }
    fn leer(&self, p: Proveedor) -> Option<String> {
        leer_de(&carpeta()?, p)
    }
    fn borrar(&self, p: Proveedor) -> Result<(), String> {
        borrar_en(&carpeta().ok_or("no se encuentra LOCALAPPDATA")?, p)
    }
    fn hay(&self, p: Proveedor) -> bool {
        carpeta().is_some_and(|c| fichero(&c, p).is_file())
    }
}

/// `%LOCALAPPDATA%\PixPinMax\claves`.
#[cfg(windows)]
pub fn carpeta() -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA")?;
    Some(PathBuf::from(local).join("PixPinMax").join("claves"))
}

/// El fichero de la clave de un proveedor.
#[cfg(windows)]
pub fn fichero(carpeta: &Path, p: Proveedor) -> PathBuf {
    carpeta.join(format!("{}.clave", p.id()))
}

/// Lo que se ensena de una clave: el principio fijo del proveedor y los
/// cuatro ultimos caracteres. `sk-ant-api03-…AbCd`.
pub fn enmascarar(clave: &str) -> String {
    let clave = clave.trim();
    let n = clave.chars().count();
    if n <= 8 {
        return "•".repeat(n.max(4));
    }
    let fin: String = clave.chars().skip(n - 4).collect();
    let prefijo: String = clave.chars().take_while(|c| *c != '-').take(6).collect();
    if clave.starts_with("sk-") {
        let cabeza: String = clave
            .chars()
            .take(if clave.starts_with("sk-ant-") { 7 } else { 3 })
            .collect();
        format!("{cabeza}…{fin}")
    } else {
        format!("{prefijo}…{fin}")
    }
}

/// Si lo pegado tiene pinta de clave (sin espacios dentro, largo razonable).
/// No se comprueba el prefijo: los proveedores lo cambian.
pub fn parece_clave(clave: &str) -> bool {
    let c = clave.trim();
    (16..=400).contains(&c.len()) && c.chars().all(|ch| ch.is_ascii_graphic())
}

/// **Cifra** con DPAPI del usuario.
#[cfg(windows)]
pub fn cifrar(claro: &[u8]) -> Result<Vec<u8>, String> {
    let entrada = blob(claro);
    let entropia = blob(ENTROPIA);
    let mut salida = CRYPT_INTEGER_BLOB::default();
    // SAFETY: los blobs de entrada apuntan a bufferes vivos durante la
    // llamada; la salida la reserva Windows con `LocalAlloc` y se libera
    // abajo con `LocalFree` tras copiarla.
    unsafe {
        CryptProtectData(
            &entrada,
            w!("PixPin Max"),
            Some(&entropia),
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut salida,
        )
        .map_err(|e| format!("no se pudo cifrar la clave: {}", e.message()))?;
        Ok(copiar_y_soltar(salida))
    }
}

/// **Descifra** lo cifrado por [`cifrar`] en esta misma cuenta.
#[cfg(windows)]
pub fn descifrar(cifrado: &[u8]) -> Result<Vec<u8>, String> {
    let entrada = blob(cifrado);
    let entropia = blob(ENTROPIA);
    let mut salida = CRYPT_INTEGER_BLOB::default();
    // SAFETY: como en `cifrar`.
    unsafe {
        CryptUnprotectData(
            &entrada,
            None,
            Some(&entropia),
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut salida,
        )
        .map_err(|e| format!("no se pudo descifrar la clave: {}", e.message()))?;
        Ok(copiar_y_soltar(salida))
    }
}

#[cfg(windows)]
fn blob(b: &[u8]) -> CRYPT_INTEGER_BLOB {
    CRYPT_INTEGER_BLOB {
        cbData: b.len() as u32,
        pbData: b.as_ptr() as *mut u8,
    }
}

/// # Safety
/// `b` lo tiene que haber llenado DPAPI (memoria de `LocalAlloc`).
#[cfg(windows)]
unsafe fn copiar_y_soltar(b: CRYPT_INTEGER_BLOB) -> Vec<u8> {
    if b.pbData.is_null() {
        return Vec::new();
    }
    // SAFETY: DPAPI devolvio `cbData` bytes validos en `pbData`; se copian
    // y despues se borran y liberan una sola vez.
    unsafe {
        let v = std::slice::from_raw_parts(b.pbData, b.cbData as usize).to_vec();
        std::ptr::write_bytes(b.pbData, 0, b.cbData as usize);
        let _ = LocalFree(Some(HLOCAL(b.pbData.cast())));
        v
    }
}

/// **Guarda** la clave de un proveedor (cifrada). Una clave vacia la borra.
#[cfg(windows)]
pub fn guardar_en(carpeta: &Path, p: Proveedor, clave: &str) -> Result<(), String> {
    let clave = clave.trim();
    let ruta = fichero(carpeta, p);
    if clave.is_empty() {
        return borrar_en(carpeta, p);
    }
    let cifrado = cifrar(clave.as_bytes())?;
    std::fs::create_dir_all(carpeta)
        .map_err(|e| format!("no se pudo crear la carpeta de claves: {e}"))?;
    // Primero a un fichero aparte y luego se cambia de nombre: un corte a
    // medias no deja una clave rota en lugar de la buena.
    let tmp = ruta.with_extension("clave.parte");
    std::fs::write(&tmp, &cifrado).map_err(|e| format!("no se pudo guardar la clave: {e}"))?;
    std::fs::rename(&tmp, &ruta).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("no se pudo guardar la clave: {e}")
    })
}

/// **Lee** la clave guardada. `None` si no hay o si no se puede descifrar
/// (otra cuenta, fichero danado): se pide otra vez.
#[cfg(windows)]
pub fn leer_de(carpeta: &Path, p: Proveedor) -> Option<String> {
    let cifrado = std::fs::read(fichero(carpeta, p)).ok()?;
    let claro = descifrar(&cifrado).ok()?;
    String::from_utf8(claro)
        .ok()
        .filter(|c| !c.trim().is_empty())
}

#[cfg(windows)]
pub fn borrar_en(carpeta: &Path, p: Proveedor) -> Result<(), String> {
    match std::fs::remove_file(fichero(carpeta, p)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("no se pudo borrar la clave: {e}")),
    }
}

/// Lo que dice [`guardar`] si la app no instalo su llavero.
pub const SIN_LLAVERO: &str =
    "no hay donde guardar la clave cifrada en este sistema (la app tiene que instalar su llavero)";

/// **Guarda** la clave en el llavero en uso (vacia: la borra).
pub fn guardar(p: Proveedor, clave: &str) -> Result<(), String> {
    guardar_con(llavero().ok_or(SIN_LLAVERO)?, p, clave)
}

/// **Lee** la clave del llavero en uso. `None` sin llavero o sin clave.
pub fn leer(p: Proveedor) -> Option<String> {
    leer_con(llavero()?, p)
}

pub fn hay(p: Proveedor) -> bool {
    llavero().is_some_and(|l| l.hay(p))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// Un llavero en memoria, como el que pondra Swift con el Llavero.
    #[derive(Default)]
    struct EnMemoria(Mutex<HashMap<&'static str, String>>);

    impl Llavero for EnMemoria {
        fn guardar(&self, p: Proveedor, clave: &str) -> Result<(), String> {
            self.0.lock().unwrap().insert(p.id(), clave.to_string());
            Ok(())
        }
        fn leer(&self, p: Proveedor) -> Option<String> {
            self.0.lock().unwrap().get(p.id()).cloned()
        }
        fn borrar(&self, p: Proveedor) -> Result<(), String> {
            self.0.lock().unwrap().remove(p.id());
            Ok(())
        }
    }

    #[test]
    fn un_llavero_de_la_app_guarda_recortado_y_vacia_borra() {
        let l = EnMemoria::default();
        guardar_con(&l, Proveedor::Claude, "  sk-ant-api03-abc0123456789  ").unwrap();
        assert_eq!(
            leer_con(&l, Proveedor::Claude).as_deref(),
            Some("sk-ant-api03-abc0123456789")
        );
        assert!(l.hay(Proveedor::Claude));
        assert!(leer_con(&l, Proveedor::DeepSeek).is_none());
        guardar_con(&l, Proveedor::Claude, "   ").unwrap();
        assert!(leer_con(&l, Proveedor::Claude).is_none());
    }

    #[cfg(not(windows))]
    #[test]
    fn sin_llavero_no_se_guarda_nada_en_claro() {
        // Nadie instalo llavero en las pruebas: ni se guarda ni se inventa.
        assert_eq!(
            guardar(Proveedor::DeepSeek, "clave-de-prueba-0123456789").unwrap_err(),
            SIN_LLAVERO
        );
        assert!(leer(Proveedor::DeepSeek).is_none());
        assert!(!hay(Proveedor::DeepSeek));
    }

    #[cfg(windows)]
    fn carpeta_de_prueba(nombre: &str) -> PathBuf {
        let c = std::env::temp_dir().join(format!("pixpin-claves-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&c);
        c
    }

    const CLAVE: &str = "sk-ant-api03-PRUEBAxyzzy0123456789abcdefghijklmnopqrstuvwxyzQWERTY";

    #[cfg(windows)]
    #[test]
    fn se_guarda_cifrada_y_se_lee_igual() {
        let c = carpeta_de_prueba("ida");
        guardar_en(&c, Proveedor::Claude, CLAVE).unwrap();
        assert_eq!(leer_de(&c, Proveedor::Claude).as_deref(), Some(CLAVE));
        // La de otro proveedor no existe.
        assert!(leer_de(&c, Proveedor::DeepSeek).is_none());
        let _ = std::fs::remove_dir_all(&c);
    }

    #[cfg(windows)]
    #[test]
    fn la_clave_no_aparece_en_claro_en_ningun_fichero() {
        let c = carpeta_de_prueba("claro");
        guardar_en(&c, Proveedor::Claude, CLAVE).unwrap();
        guardar_en(
            &c,
            Proveedor::DeepSeek,
            "clave-de-prueba-deepseek-0123456789abcdef",
        )
        .unwrap();
        let mut ficheros = 0;
        for e in std::fs::read_dir(&c).unwrap() {
            let bytes = std::fs::read(e.unwrap().path()).unwrap();
            ficheros += 1;
            for buscada in [CLAVE.as_bytes(), b"PRUEBAxyzzy", b"0123456789abcdef"] {
                assert!(
                    !bytes.windows(buscada.len()).any(|w| w == buscada),
                    "clave en claro en el disco"
                );
            }
            // Ni en UTF-16.
            let ancha: Vec<u8> = "PRUEBAxyzzy"
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect();
            assert!(!bytes.windows(ancha.len()).any(|w| w == ancha.as_slice()));
        }
        assert_eq!(ficheros, 2, "sin ficheros temporales olvidados");
        let _ = std::fs::remove_dir_all(&c);
    }

    #[cfg(windows)]
    #[test]
    fn un_fichero_danado_no_da_clave_y_vacia_borra() {
        let c = carpeta_de_prueba("danado");
        std::fs::create_dir_all(&c).unwrap();
        std::fs::write(fichero(&c, Proveedor::DeepSeek), b"esto no es dpapi").unwrap();
        assert!(leer_de(&c, Proveedor::DeepSeek).is_none());
        guardar_en(&c, Proveedor::DeepSeek, "   ").unwrap();
        assert!(!fichero(&c, Proveedor::DeepSeek).exists());
        // Borrar lo que no esta no es un error.
        borrar_en(&c, Proveedor::DeepSeek).unwrap();
        let _ = std::fs::remove_dir_all(&c);
    }

    #[cfg(windows)]
    #[test]
    fn sin_la_entropia_de_pixpin_no_se_descifra() {
        let cifrado = cifrar(b"secreto").unwrap();
        let entrada = blob(&cifrado);
        let mut salida = CRYPT_INTEGER_BLOB::default();
        // SAFETY: igual que en `descifrar`, pero sin entropia.
        let r = unsafe {
            CryptUnprotectData(
                &entrada,
                None,
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut salida,
            )
        };
        assert!(r.is_err());
        assert_eq!(descifrar(&cifrado).unwrap(), b"secreto");
    }

    #[test]
    fn enmascarar_deja_ver_solo_el_final() {
        let m = enmascarar(CLAVE);
        assert_eq!(m, "sk-ant-…RTY".replace("RTY", "ERTY"));
        assert!(!m.contains("PRUEBA"));
        assert_eq!(enmascarar("sk-0123456789abcdef"), "sk-…cdef");
        assert_eq!(enmascarar("abc"), "••••");
        assert!(parece_clave(CLAVE));
        assert!(!parece_clave("hola mundo con espacios dentro"));
        assert!(!parece_clave("corta"));
    }
}
