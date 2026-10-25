//! **OAuth 2.0 para aplicaciones de escritorio**, con PKCE y la vuelta a
//! `http://127.0.0.1:<puerto>` (RFC 8252), lo que Google recomienda para
//! una app instalada:
//!
//! 1. se abre un puerto libre en 127.0.0.1 (solo este equipo lo ve);
//! 2. se abre el navegador del usuario en la pagina de permiso del servicio
//!    con el reto PKCE (S256) y un `state` aleatorio;
//! 3. el servicio vuelve a `http://127.0.0.1:<puerto>/?code=...&state=...`;
//!    se comprueba el `state` y se contesta una pagina de «ya puedes volver
//!    a PixPin»;
//! 4. el codigo se canjea por los tokens con el verificador PKCE.
//!
//! Todo con las variables del [`Servicio`] y sus [`Extremos`]: lo mismo
//! vale para Dropbox o OneDrive, y las pruebas lo apuntan a un servidor
//! falso en 127.0.0.1.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde_json::Value;
use sha2::{Digest, Sha256};

use super::guardado::{Cliente, Tokens};
use super::url::{consulta, escapar, formulario, valor};
use super::{Extremos, Servicio};
use crate::nube::http::{self, Fallo};

/// Lo mas que se espera al usuario en el navegador.
pub const ESPERA: Duration = Duration::from_secs(5 * 60);

/// Ms desde 1970.
pub fn ahora_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// `n` bytes del generador del sistema (BCrypt).
#[cfg(windows)]
pub fn aleatorio(n: usize) -> Vec<u8> {
    use windows::Win32::Security::Cryptography::{
        BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom,
    };
    let mut b = vec![0u8; n];
    // SAFETY: el bufer vive durante la llamada y se pasa entero; sin asa de
    // algoritmo, con el generador preferido del sistema.
    let r = unsafe { BCryptGenRandom(None, &mut b, BCRYPT_USE_SYSTEM_PREFERRED_RNG) };
    assert!(r.is_ok(), "el generador aleatorio del sistema fallo");
    b
}

/// `n` bytes del generador del sistema (fuera de Windows, `/dev/urandom`,
/// el mismo que da `SecRandomCopyBytes` en Apple).
#[cfg(not(windows))]
pub fn aleatorio(n: usize) -> Vec<u8> {
    use std::io::Read;
    let mut b = vec![0u8; n];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut b))
        .expect("el generador aleatorio del sistema fallo");
    b
}

/// Base64 «para URL» sin relleno (RFC 4648 §5), la de PKCE.
pub fn base64_url(datos: &[u8]) -> String {
    crate::base64::codificar(datos)
        .trim_end_matches('=')
        .replace('+', "-")
        .replace('/', "_")
}

/// El par de PKCE: el verificador (secreto, se manda al canjear) y su reto
/// S256 (va en la URL del navegador).
#[derive(Clone)]
pub struct Pkce {
    pub verificador: String,
    pub reto: String,
}

pub fn reto_de(verificador: &str) -> String {
    base64_url(&Sha256::digest(verificador.as_bytes()))
}

pub fn pkce() -> Pkce {
    // 32 bytes → 43 letras, el minimo del RFC 7636 y de sobra.
    let verificador = base64_url(&aleatorio(32));
    Pkce {
        reto: reto_de(&verificador),
        verificador,
    }
}

/// **La URL de la pagina de permiso.**
pub fn url_de_autorizar(
    s: &Servicio,
    ex: &Extremos,
    cliente_id: &str,
    redireccion: &str,
    reto: &str,
    estado: &str,
) -> String {
    let alcances = s.alcances.join(" ");
    let mut pares = vec![
        ("client_id", cliente_id),
        ("redirect_uri", redireccion),
        ("response_type", "code"),
        ("scope", alcances.as_str()),
        ("code_challenge", reto),
        ("code_challenge_method", "S256"),
        ("state", estado),
    ];
    pares.extend(s.extra_autorizar.iter().copied());
    let sep = if ex.autorizar.contains('?') { '&' } else { '?' };
    format!("{}{sep}{}", ex.autorizar, formulario(&pares))
}

/// **Conecta**: abre el navegador con `abrir` y espera la vuelta. Bloquea
/// (hasta [`ESPERA`] o hasta que `parar` se encienda): llamarlo desde un
/// hilo de fondo.
pub fn autorizar(
    s: &Servicio,
    ex: &Extremos,
    cliente: &Cliente,
    abrir: &dyn Fn(&str) -> Result<(), String>,
    parar: &AtomicBool,
    espera: Duration,
) -> Result<Tokens, String> {
    let oyente = TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("no se pudo abrir un puerto local: {e}"))?;
    let puerto = oyente.local_addr().map_err(|e| e.to_string())?.port();
    let redireccion = format!("http://127.0.0.1:{puerto}/");
    let p = pkce();
    let estado = base64_url(&aleatorio(16));
    abrir(&url_de_autorizar(
        s,
        ex,
        &cliente.id,
        &redireccion,
        &p.reto,
        &estado,
    ))?;
    let codigo = esperar_codigo(&oyente, &estado, s, parar, espera)?;
    canjear(ex, cliente, &codigo, &p.verificador, &redireccion)
}

/// Espera la vuelta del navegador al puerto local y saca el `code`.
fn esperar_codigo(
    oyente: &TcpListener,
    estado: &str,
    s: &Servicio,
    parar: &AtomicBool,
    espera: Duration,
) -> Result<String, String> {
    oyente.set_nonblocking(true).map_err(|e| e.to_string())?;
    let limite = Instant::now() + espera;
    loop {
        if parar.load(Ordering::Relaxed) {
            return Err("cancelado".into());
        }
        if Instant::now() > limite {
            return Err(format!(
                "no llego el permiso de {} a tiempo; pulsa Conectar otra vez",
                s.nombre
            ));
        }
        match oyente.accept() {
            Ok((flujo, _)) => {
                if let Some(r) = atender(flujo, estado, s) {
                    return r;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50))
            }
            Err(e) => return Err(format!("el puerto local fallo: {e}")),
        }
    }
}

/// Una peticion del navegador. `None`: no era la vuelta (el favicon, otra
/// ruta): se sigue esperando.
fn atender(mut flujo: TcpStream, estado: &str, s: &Servicio) -> Option<Result<String, String>> {
    let _ = flujo.set_nonblocking(false);
    let _ = flujo.set_read_timeout(Some(Duration::from_secs(5)));
    let mut buf = vec![0u8; 8192];
    let mut n = 0;
    while n < buf.len() {
        match flujo.read(&mut buf[n..]) {
            Ok(0) => break,
            Ok(k) => {
                n += k;
                if buf[..n].windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    let pedido = String::from_utf8_lossy(&buf[..n]);
    let linea = pedido.lines().next().unwrap_or_default();
    let ruta = linea.split_whitespace().nth(1).unwrap_or("/");
    let (camino, q) = ruta.split_once('?').unwrap_or((ruta, ""));
    let pares = consulta(q);
    if camino != "/" || (valor(&pares, "code").is_none() && valor(&pares, "error").is_none()) {
        let _ = flujo
            .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        return None;
    }
    let r = if valor(&pares, "state") != Some(estado) {
        Err("la vuelta del navegador no es la de esta conexion (state distinto); pulsa Conectar otra vez".to_string())
    } else if let Some(e) = valor(&pares, "error") {
        Err(if e == "access_denied" {
            format!("no se dio permiso en {}", s.nombre)
        } else {
            format!("{} no dio permiso: {e}", s.nombre)
        })
    } else {
        Ok(valor(&pares, "code").unwrap_or_default().to_string())
    };
    let (titulo, texto) = match &r {
        Ok(_) => (
            format!("{} conectado", s.nombre),
            "Listo. Ya puedes cerrar esta pestaña y volver a PixPin.".to_string(),
        ),
        Err(e) => (
            "No se conecto".to_string(),
            format!("{e}. Vuelve a PixPin para intentarlo otra vez."),
        ),
    };
    let html = format!(
        "<!doctype html><meta charset=utf-8><title>PixPin</title>\
         <body style=\"font-family:Segoe UI,sans-serif;background:#1e1f22;color:#e8e8e8;display:grid;place-items:center;height:90vh\">\
         <div style=\"text-align:center\"><h2>{}</h2><p>{}</p></div>",
        html_escapar(&titulo),
        html_escapar(&texto)
    );
    let _ = flujo.write_all(
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{html}",
            html.len()
        )
        .as_bytes(),
    );
    let _ = flujo.flush();
    Some(r)
}

fn html_escapar(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Por que no salio una peticion al servidor de tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FalloToken {
    /// El permiso ya no vale (`invalid_grant`: revocado, caducado, otra
    /// contrasena): hay que volver a conectar.
    Revocado(String),
    Otro(String),
}

impl FalloToken {
    pub fn mensaje(&self) -> &str {
        match self {
            FalloToken::Revocado(m) | FalloToken::Otro(m) => m,
        }
    }
}

fn pedir_token(ex: &Extremos, pares: &[(&str, &str)]) -> Result<Value, FalloToken> {
    let cuerpo = formulario(pares);
    let r = http::pedir(
        "POST",
        &ex.token,
        &[
            ("Content-Type", "application/x-www-form-urlencoded".into()),
            ("Accept", "application/json".into()),
        ],
        cuerpo.as_bytes(),
        1024 * 1024,
    );
    match r {
        Ok(r) => serde_json::from_slice(&r.cuerpo)
            .map_err(|_| FalloToken::Otro("el servidor de permisos contesto algo raro".into())),
        Err(Fallo::Http { codigo, cuerpo, .. }) => {
            let v: Value = serde_json::from_str(&cuerpo).unwrap_or(Value::Null);
            let error = v.get("error").and_then(Value::as_str).unwrap_or_default();
            let detalle = v
                .get("error_description")
                .and_then(Value::as_str)
                .unwrap_or(error);
            if error == "invalid_grant" {
                Err(FalloToken::Revocado(format!(
                    "el permiso caduco o se retiro ({detalle}); hay que volver a conectar"
                )))
            } else if error == "invalid_client" || error == "unauthorized_client" {
                Err(FalloToken::Otro(format!(
                    "el ID de cliente no vale ({detalle}); revisa el que pegaste en Ajustes → Asistente → Conexiones"
                )))
            } else {
                Err(FalloToken::Otro(format!(
                    "el servidor de permisos dijo {codigo}: {detalle}"
                )))
            }
        }
        Err(Fallo::Red(e)) => Err(FalloToken::Otro(format!("sin conexion: {e}"))),
    }
}

/// Los tokens de una respuesta del servidor. `refresco_anterior`: el que se
/// conserva si la respuesta no trae otro (al refrescar, Google no lo
/// repite).
pub fn tokens_de(v: &Value, refresco_anterior: &str, ahora: i64) -> Result<Tokens, FalloToken> {
    let acceso = v
        .get("access_token")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if acceso.is_empty() {
        return Err(FalloToken::Otro(
            "el servidor de permisos no dio el token".into(),
        ));
    }
    let dura = v
        .get("expires_in")
        .and_then(Value::as_i64)
        .unwrap_or(3600)
        .clamp(30, 30 * 24 * 3600);
    Ok(Tokens {
        acceso: acceso.to_string(),
        refresco: v
            .get("refresh_token")
            .and_then(Value::as_str)
            .filter(|r| !r.is_empty())
            .unwrap_or(refresco_anterior)
            .to_string(),
        caduca_ms: ahora + dura * 1000,
    })
}

fn con_cliente<'a>(
    cliente: &'a Cliente,
    mut pares: Vec<(&'a str, &'a str)>,
) -> Vec<(&'a str, &'a str)> {
    pares.push(("client_id", &cliente.id));
    if !cliente.secreto.is_empty() {
        pares.push(("client_secret", &cliente.secreto));
    }
    pares
}

/// **Canjea** el codigo de la vuelta por los tokens (con el verificador).
pub fn canjear(
    ex: &Extremos,
    cliente: &Cliente,
    codigo: &str,
    verificador: &str,
    redireccion: &str,
) -> Result<Tokens, String> {
    let pares = con_cliente(
        cliente,
        vec![
            ("grant_type", "authorization_code"),
            ("code", codigo),
            ("code_verifier", verificador),
            ("redirect_uri", redireccion),
        ],
    );
    let v = pedir_token(ex, &pares).map_err(|e| e.mensaje().to_string())?;
    tokens_de(&v, "", ahora_ms()).map_err(|e| e.mensaje().to_string())
}

/// **Pide otro token de acceso** con el de refresco.
pub fn refrescar(ex: &Extremos, cliente: &Cliente, refresco: &str) -> Result<Tokens, FalloToken> {
    if refresco.is_empty() {
        return Err(FalloToken::Revocado(
            "no hay permiso guardado para renovar; hay que volver a conectar".into(),
        ));
    }
    let pares = con_cliente(
        cliente,
        vec![("grant_type", "refresh_token"), ("refresh_token", refresco)],
    );
    let v = pedir_token(ex, &pares)?;
    tokens_de(&v, refresco, ahora_ms())
}

/// **Retira el permiso** en el servicio (al desconectar). Si falla (sin
/// red) no pasa nada grave: los tokens ya se borraron de este PC.
pub fn revocar(ex: &Extremos, token: &str) -> Result<(), String> {
    if ex.revocar.is_empty() || token.is_empty() {
        return Ok(());
    }
    let cuerpo = format!("token={}", escapar(token));
    http::pedir(
        "POST",
        &ex.revocar,
        &[("Content-Type", "application/x-www-form-urlencoded".into())],
        cuerpo.as_bytes(),
        64 * 1024,
    )
    .map(|_| ())
    .map_err(|e| match e {
        Fallo::Red(m) => format!("sin conexion: {m}"),
        Fallo::Http { codigo, .. } => format!("el servicio contesto {codigo}"),
    })
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::conexiones::drive::GOOGLE_DRIVE;

    #[test]
    fn el_reto_es_el_del_rfc_7636() {
        // El ejemplo del apendice B del RFC 7636.
        assert_eq!(
            reto_de("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        let p = pkce();
        assert_eq!(p.verificador.len(), 43);
        assert!(
            p.verificador
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        );
        assert_eq!(p.reto, reto_de(&p.verificador));
        assert_ne!(pkce().verificador, p.verificador, "aleatorio de verdad");
    }

    #[test]
    fn la_url_de_permiso_lleva_lo_que_pide_google() {
        let ex = GOOGLE_DRIVE.extremos();
        let u = url_de_autorizar(
            &GOOGLE_DRIVE,
            &ex,
            "id.apps",
            "http://127.0.0.1:5000/",
            "RETO",
            "EST",
        );
        assert!(
            u.starts_with("https://accounts.google.com/o/oauth2/v2/auth?"),
            "{u}"
        );
        let q = consulta(u.split_once('?').unwrap().1);
        assert_eq!(valor(&q, "redirect_uri"), Some("http://127.0.0.1:5000/"));
        assert_eq!(valor(&q, "code_challenge_method"), Some("S256"));
        assert_eq!(valor(&q, "code_challenge"), Some("RETO"));
        assert_eq!(valor(&q, "state"), Some("EST"));
        assert_eq!(valor(&q, "access_type"), Some("offline"));
        assert!(
            valor(&q, "scope")
                .unwrap()
                .contains("https://www.googleapis.com/auth/drive.file")
        );
    }

    #[test]
    fn el_refresco_se_conserva_si_no_viene_otro() {
        let v: Value = serde_json::from_str(r#"{"access_token":"a2","expires_in":3599}"#).unwrap();
        let t = tokens_de(&v, "r1", 1000).unwrap();
        assert_eq!(
            (t.acceso.as_str(), t.refresco.as_str(), t.caduca_ms),
            ("a2", "r1", 1000 + 3_599_000)
        );
        let v: Value =
            serde_json::from_str(r#"{"access_token":"a3","refresh_token":"r2"}"#).unwrap();
        assert_eq!(tokens_de(&v, "r1", 0).unwrap().refresco, "r2");
        assert!(tokens_de(&serde_json::json!({}), "r1", 0).is_err());
    }
}
