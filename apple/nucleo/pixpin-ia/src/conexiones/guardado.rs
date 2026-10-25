//! **Lo guardado de cada conexion, cifrado con DPAPI**: el ID de cliente
//! que creo el usuario en la consola del servicio, los tokens de OAuth y la
//! cuenta conectada.
//!
//! Mismo trato que las claves de las API (`nube::claves`): un token de
//! Drive abre los archivos del usuario, asi que
//!
//! - va cifrado con `CryptProtectData` del usuario y la entropia de PixPin
//!   (se reutiliza [`claves::cifrar`], no hay un segundo cifrado);
//! - vive en su fichero en `%LOCALAPPDATA%\PixPinMax\claves`
//!   (`conexion-<servicio>.cifrado`): nunca en el TOML de ajustes, nunca en
//!   la carpeta de datos que se sincroniza, nunca en un registro;
//! - el `Debug` de estos tipos no ensena ni el token ni el secreto.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::Servicio;
use crate::nube::claves;

/// El ID de cliente de OAuth que el usuario creo una vez (y su «secreto»,
/// que Google da tambien a las apps de escritorio y pide al canjear el
/// codigo; en una app de escritorio no es secreto de verdad, pero se guarda
/// igual de cifrado).
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Cliente {
    pub id: String,
    pub secreto: String,
}

impl std::fmt::Debug for Cliente {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cliente")
            .field("id", &self.id)
            .field("secreto", &if self.secreto.is_empty() { "" } else { "•••" })
            .finish()
    }
}

/// Los tokens de una conexion.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Tokens {
    pub acceso: String,
    /// Para pedir otro de acceso sin volver al navegador. Vacio si el
    /// servicio no dio ninguno.
    pub refresco: String,
    /// Cuando caduca el de acceso (ms desde 1970).
    pub caduca_ms: i64,
}

impl std::fmt::Debug for Tokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tokens")
            .field("acceso", &"•••")
            .field("refresco", &if self.refresco.is_empty() { "" } else { "•••" })
            .field("caduca_ms", &self.caduca_ms)
            .finish()
    }
}

/// Todo lo de un servicio.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Guardado {
    pub cliente: Option<Cliente>,
    pub tokens: Option<Tokens>,
    /// El correo de la cuenta conectada, para ensenarlo en Ajustes.
    pub cuenta: Option<String>,
}

impl Guardado {
    pub fn conectado(&self) -> bool {
        self.tokens.as_ref().is_some_and(|t| !t.acceso.is_empty() || !t.refresco.is_empty())
    }
}

/// `%LOCALAPPDATA%\PixPinMax\claves`, la misma de las claves de las API.
pub fn carpeta() -> Option<PathBuf> {
    claves::carpeta()
}

pub fn fichero(carpeta: &Path, s: &Servicio) -> PathBuf {
    carpeta.join(format!("conexion-{}.cifrado", s.id))
}

fn a_json(g: &Guardado) -> Value {
    let mut v = json!({});
    if let Some(c) = &g.cliente {
        v["cliente"] = json!({"id": c.id, "secreto": c.secreto});
    }
    if let Some(t) = &g.tokens {
        v["tokens"] = json!({"acceso": t.acceso, "refresco": t.refresco, "caduca_ms": t.caduca_ms});
    }
    if let Some(c) = &g.cuenta {
        v["cuenta"] = json!(c);
    }
    v
}

fn de_json(v: &Value) -> Guardado {
    let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    Guardado {
        cliente: v.get("cliente").map(|c| Cliente { id: s(c, "id"), secreto: s(c, "secreto") }).filter(|c| !c.id.is_empty()),
        tokens: v.get("tokens").map(|t| Tokens {
            acceso: s(t, "acceso"),
            refresco: s(t, "refresco"),
            caduca_ms: t.get("caduca_ms").and_then(Value::as_i64).unwrap_or(0),
        }),
        cuenta: v.get("cuenta").and_then(Value::as_str).map(str::to_string),
    }
}

/// **Lee** lo guardado: fuera de Windows (y fuera de las pruebas), del
/// Llavero de la app, con el mismo JSON que el fichero cifrado de Windows.
#[cfg(all(not(windows), not(test)))]
pub fn leer_de(_carpeta: &Path, s: &Servicio) -> Guardado {
    claves::leer_secreto(&nombre_secreto(s))
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .map(|v| de_json(&v))
        .unwrap_or_default()
}

/// **Guarda** en el Llavero (fuera de Windows). Si no queda nada, lo borra.
#[cfg(all(not(windows), not(test)))]
pub fn guardar_en(_carpeta: &Path, s: &Servicio, g: &Guardado) -> Result<(), String> {
    if g.cliente.is_none() && g.tokens.is_none() {
        return claves::borrar_secreto(&nombre_secreto(s));
    }
    claves::guardar_secreto(&nombre_secreto(s), &a_json(g).to_string())
}

#[cfg(all(not(windows), not(test)))]
fn nombre_secreto(s: &Servicio) -> String {
    format!("conexion-{}", s.id)
}

/// **Lee** lo guardado. Sin fichero, o si no se puede descifrar (otra
/// cuenta de Windows, fichero danado): vacio, y se pide otra vez.
#[cfg(any(windows, test))]
pub fn leer_de(carpeta: &Path, s: &Servicio) -> Guardado {
    let Ok(cifrado) = std::fs::read(fichero(carpeta, s)) else { return Guardado::default() };
    let Ok(claro) = claves::descifrar(&cifrado) else { return Guardado::default() };
    serde_json::from_slice::<Value>(&claro).map(|v| de_json(&v)).unwrap_or_default()
}

/// **Guarda** (cifrado). Si no queda nada, borra el fichero.
#[cfg(any(windows, test))]
pub fn guardar_en(carpeta: &Path, s: &Servicio, g: &Guardado) -> Result<(), String> {
    let ruta = fichero(carpeta, s);
    if g.cliente.is_none() && g.tokens.is_none() {
        return match std::fs::remove_file(&ruta) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("no se pudo borrar la conexion: {e}")),
            _ => Ok(()),
        };
    }
    let cifrado = claves::cifrar(a_json(g).to_string().as_bytes())?;
    std::fs::create_dir_all(carpeta).map_err(|e| format!("no se pudo crear la carpeta de claves: {e}"))?;
    // Primero aparte y luego se cambia de nombre: un corte a medias no deja
    // los tokens buenos rotos.
    let tmp = ruta.with_extension("cifrado.parte");
    std::fs::write(&tmp, &cifrado).map_err(|e| format!("no se pudo guardar la conexion: {e}"))?;
    std::fs::rename(&tmp, &ruta).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("no se pudo guardar la conexion: {e}")
    })
}

/// **Lo que pega el usuario en Ajustes**, sea lo que sea de esto:
///
/// - el ID de cliente solo (`1234-abc.apps.googleusercontent.com`);
/// - el ID y el secreto separados por un espacio;
/// - el JSON entero que da Google al descargar el cliente
///   (`{"installed":{"client_id":...,"client_secret":...}}`).
///
/// `None` si no parece nada de eso.
pub fn leer_cliente(texto: &str) -> Option<Cliente> {
    let t = texto.trim();
    if t.starts_with('{') {
        let v: Value = serde_json::from_str(t).ok()?;
        let dentro = v.get("installed").or_else(|| v.get("web")).unwrap_or(&v);
        let id = dentro.get("client_id")?.as_str()?.trim().to_string();
        let secreto = dentro.get("client_secret").and_then(Value::as_str).unwrap_or_default().trim().to_string();
        return valido(&id).then_some(Cliente { id, secreto });
    }
    let mut partes = t.split_whitespace();
    let id = partes.next()?.to_string();
    let secreto = partes.next().unwrap_or_default().to_string();
    if partes.next().is_some() || !valido(&id) {
        return None;
    }
    Some(Cliente { id, secreto })
}

fn valido(id: &str) -> bool {
    (8..=300).contains(&id.len()) && id.chars().all(|c| c.is_ascii_alphanumeric() || "-._".contains(c))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::conexiones::drive::GOOGLE_DRIVE;

    fn carpeta_de_prueba(nombre: &str) -> PathBuf {
        let c = std::env::temp_dir().join(format!("pixpin-conexiones-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&c);
        c
    }

    #[test]
    fn se_guarda_cifrado_y_nada_va_en_claro() {
        let c = carpeta_de_prueba("guardado");
        let g = Guardado {
            cliente: Some(Cliente { id: "1234-abcd.apps.googleusercontent.com".into(), secreto: "GOCSPX-secretoXYZ".into() }),
            tokens: Some(Tokens { acceso: "ya29.ACCESOprueba".into(), refresco: "1//REFRESCOprueba".into(), caduca_ms: 42 }),
            cuenta: Some("ana@example.com".into()),
        };
        guardar_en(&c, &GOOGLE_DRIVE, &g).unwrap();
        assert_eq!(leer_de(&c, &GOOGLE_DRIVE), g);
        let mut n = 0;
        for e in std::fs::read_dir(&c).unwrap() {
            n += 1;
            let bytes = std::fs::read(e.unwrap().path()).unwrap();
            for buscado in [&b"ACCESOprueba"[..], b"REFRESCOprueba", b"secretoXYZ", b"ana@example"] {
                assert!(!bytes.windows(buscado.len()).any(|w| w == buscado), "en claro en el disco");
            }
        }
        assert_eq!(n, 1, "sin temporales olvidados");
        // El Debug no ensena los secretos (por si alguien lo manda a tracing).
        let d = format!("{g:?}");
        assert!(!d.contains("ACCESO") && !d.contains("REFRESCO") && !d.contains("secretoXYZ"), "{d}");
        // Sin nada que guardar, se borra.
        guardar_en(&c, &GOOGLE_DRIVE, &Guardado::default()).unwrap();
        assert!(!fichero(&c, &GOOGLE_DRIVE).exists());
        assert_eq!(leer_de(&c, &GOOGLE_DRIVE), Guardado::default());
        let _ = std::fs::remove_dir_all(&c);
    }

    #[test]
    fn un_fichero_danado_es_como_no_tener_nada() {
        let c = carpeta_de_prueba("danado");
        std::fs::create_dir_all(&c).unwrap();
        std::fs::write(fichero(&c, &GOOGLE_DRIVE), b"no es dpapi").unwrap();
        assert_eq!(leer_de(&c, &GOOGLE_DRIVE), Guardado::default());
        let _ = std::fs::remove_dir_all(&c);
    }

    #[test]
    fn el_cliente_se_entiende_pegado_de_tres_maneras() {
        let id = "123456789-abcdef.apps.googleusercontent.com";
        assert_eq!(leer_cliente(id), Some(Cliente { id: id.into(), secreto: String::new() }));
        assert_eq!(leer_cliente(&format!("  {id}   GOCSPX-abc ")), Some(Cliente { id: id.into(), secreto: "GOCSPX-abc".into() }));
        let json = format!(r#"{{"installed":{{"client_id":"{id}","project_id":"p","client_secret":"GOCSPX-x","redirect_uris":["http://localhost"]}}}}"#);
        assert_eq!(leer_cliente(&json), Some(Cliente { id: id.into(), secreto: "GOCSPX-x".into() }));
        // Casos negativos.
        assert_eq!(leer_cliente(""), None);
        assert_eq!(leer_cliente("hola que tal amigo"), None);
        assert_eq!(leer_cliente("corto"), None);
        assert_eq!(leer_cliente("{\"installed\":{}}"), None);
        assert_eq!(leer_cliente("id/con/barras/que/no/vale"), None);
    }
}
