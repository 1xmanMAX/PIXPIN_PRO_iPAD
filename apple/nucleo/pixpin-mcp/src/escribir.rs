//! **Las herramientas de la app**, hechas por la PixPin que corre.
//!
//! El protocolo de pedidos solo contesta «aceptado» (`1`): el trabajo lo
//! hace luego el bucle de la app. Pero el modelo necesita saber que paso
//! («Apuntada «pan» para mañana 10:00», «No encuentro esa tarea»). Por eso
//! hay un pedido propio, `herramienta` (`docs/protocolo-pedidos.md`):
//!
//! ```json
//! {"pixpin":1, "accion":"herramienta", "nombre":"anadir_tarea",
//!  "args":{"texto":"pan","fecha":"mañana"},
//!  "respuesta":"C:\\Users\\…\\Temp\\pixpin-mcp\\1234-1.json",
//!  "preguntar":"¿…?", "origen":"Claude"}
//! ```
//!
//! La app valida la fila con la misma tabla, la hace como el asistente
//! (`asistente::acciones::hacer` o `documentos_acciones::hacer`, con el
//! permiso en su aviso flotante) y escribe `{"ok":bool,"texto":"…"}` en
//! `respuesta`, de una vez (escribe al lado y renombra). Aqui se espera a que
//! aparezca ese fichero.
//!
//! **Por que un fichero** y no una ventana o una tuberia de vuelta: no hace
//! falta que este exe tenga ventana ni bucle de mensajes, sobrevive a que
//! cualquiera de los dos procesos se caiga (lo peor es esperar hasta el
//! plazo). La app solo escribe dentro de la carpeta temporal `pixpin-mcp`.
//!
//! **En el Mac** el pedido va por el enlace `pixpinmax://pedido?json=…`,
//! abierto con `open -g` (sin traer PixPin delante; la arranca si esta
//! cerrada) y **firmado** con la clave de la app
//! (`~/Library/Application Support/PixPinMax/enlaces.clave`, la de
//! `FirmaEnlaces` de Swift): asi la app no pregunta por el enlace, solo lo
//! que tiene que preguntar (lo que borra o cambia, con `preguntar`).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use pixpin_ia::acciones::{self, Accion};

/// La carpeta de las respuestas, dentro de la temporal.
pub const CARPETA: &str = "pixpin-mcp";
/// Cuanto se espera a la respuesta de algo que no pregunta.
pub const PLAZO: Duration = Duration::from_secs(150);
/// Y de algo que pregunta: lo que tarda la pregunta en caducar en la app
/// (`asistente::chat::CADUCA_LA_PREGUNTA`, 5 min) y un margen.
pub const PLAZO_CON_PREGUNTA: Duration = Duration::from_secs(5 * 60 + 30);
/// Otro que abra el enlace (pruebas: guardarlo en vez de abrirlo).
pub const VAR_ABRIR: &str = "PIXPIN_MCP_ABRIR";

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

/// Lo manda a PixPin (la arranca si hace falta).
fn mandar(json: &str) -> Result<(), String> {
    let u = enlace(json, clave().as_deref());
    if let Some(f) = std::env::var_os(VAR_ABRIR).filter(|v| !v.is_empty()) {
        return std::fs::write(f, u).map_err(|e| e.to_string());
    }
    let ok = std::process::Command::new("open").arg("-g").arg(&u).status().map(|s| s.success()).unwrap_or(false);
    if ok { Ok(()) } else { Err("no se pudo abrir PixPin (¿esta instalada en Aplicaciones?)".into()) }
}

static CONTADOR: AtomicU64 = AtomicU64::new(0);

/// Un fichero de respuesta nuevo (aun no existe).
fn fichero_de_respuesta() -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join(CARPETA);
    std::fs::create_dir_all(&dir).map_err(|e| format!("no se pudo preparar la respuesta: {e}"))?;
    let n = CONTADOR.fetch_add(1, Ordering::Relaxed);
    let ms = crate::hora::ahora_utc_ms();
    Ok(dir.join(format!("{}-{ms}-{n}.json", std::process::id())))
}

/// La pregunta que hace PixPin para lo que cambia algo que ya existe
/// ([`crate::herramientas::MODIFICA`]); lo que ya pregunta en la tabla lleva
/// su propia pregunta (con los nombres de verdad) y no necesita esta.
pub fn pregunta_de(a: &Accion, args: &Map<String, Value>) -> Option<String> {
    if a.confirmar.is_some() || !crate::herramientas::MODIFICA.contains(&a.herramienta) {
        return None;
    }
    let mut datos: Vec<String> = a
        .params
        .iter()
        .filter_map(|p| {
            let v = args.get(p.nombre)?;
            let t = acciones::leible(v);
            (!t.trim().is_empty()).then(|| format!("{}: «{}»", p.nombre, recortar(&t, 80)))
        })
        .collect();
    datos.truncate(6);
    Some(format!("Claude pide: {} ({}). ¿Lo hago?", a.descripcion.trim_end_matches('.'), datos.join(", ")))
}

fn recortar(t: &str, n: usize) -> String {
    if t.chars().count() <= n {
        t.to_string()
    } else {
        format!("{}…", t.chars().take(n).collect::<String>())
    }
}

/// El pedido `herramienta` para esa fila.
pub fn pedido(a: &Accion, args: &Map<String, Value>, respuesta: &Path) -> Value {
    let mut p = json!({
        "pixpin": 1,
        "accion": "herramienta",
        "nombre": a.herramienta,
        "args": args,
        "respuesta": respuesta.display().to_string(),
        "origen": "Claude",
    });
    if let Some(q) = pregunta_de(a, args) {
        p["preguntar"] = json!(q);
    }
    p
}

/// Lee una respuesta ya escrita: `Ok` con su texto o `Err` con el motivo.
pub fn leer_respuesta(texto: &str) -> Result<String, String> {
    let v: Value = serde_json::from_str(texto).map_err(|_| "PixPin contesto algo que no se entiende".to_string())?;
    let t = v.get("texto").and_then(Value::as_str).unwrap_or("").to_string();
    if v.get("ok").and_then(Value::as_bool) == Some(true) {
        Ok(if t.trim().is_empty() { "Hecho.".into() } else { t })
    } else {
        Err(if t.trim().is_empty() { "PixPin no pudo hacerlo".into() } else { t })
    }
}

/// **Hace una fila de la tabla en PixPin** y devuelve lo que contesto.
pub fn hacer(a: &'static Accion, args: &Map<String, Value>) -> Result<String, String> {
    // Lo mal puesto se dice ya, con el motivo de la tabla, sin ir a la app.
    acciones::validar(a, args).map_err(|e| e.to_string())?;
    let fichero = fichero_de_respuesta()?;
    let json = pedido(a, args, &fichero).to_string();
    mandar(&json)?;
    let plazo = if crate::herramientas::pregunta(a) { PLAZO_CON_PREGUNTA } else { PLAZO };
    esperar(&fichero, plazo)
}

/// Espera a que aparezca el fichero de respuesta (la app lo crea de una vez,
/// renombrando), lo lee y lo borra.
pub fn esperar(fichero: &Path, plazo: Duration) -> Result<String, String> {
    let hasta = Instant::now() + plazo;
    let mut paso = Duration::from_millis(5);
    loop {
        if let Ok(texto) = std::fs::read_to_string(fichero) {
            let _ = std::fs::remove_file(fichero);
            return leer_respuesta(&texto);
        }
        if Instant::now() >= hasta {
            return Err(format!("PixPin no contesto en {} s", plazo.as_secs()));
        }
        std::thread::sleep(paso);
        // Rapido al principio (casi todo tarda milisegundos), luego sin
        // gastar: una pregunta al usuario puede tardar minutos.
        paso = (paso * 2).min(Duration::from_millis(100));
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn args(s: &str) -> Map<String, Value> {
        serde_json::from_str(s).unwrap()
    }

    #[test]
    fn el_pedido_lleva_la_fila_los_argumentos_y_la_respuesta() {
        let a = acciones::por_nombre("anadir_tarea").unwrap();
        let p = pedido(a, &args(r#"{"texto":"pan","fecha":"mañana"}"#), Path::new(r"C:\t\r.json"));
        assert_eq!(p["accion"], "herramienta");
        assert_eq!(p["nombre"], "anadir_tarea");
        assert_eq!(p["args"]["fecha"], "mañana");
        assert_eq!(p["respuesta"], r"C:\t\r.json");
        assert!(p.get("preguntar").is_none(), "crear no pregunta");
    }

    #[test]
    fn el_enlace_va_codificado_y_firmado_como_lo_comprueba_la_app() {
        // El HMAC de la RFC 4231 (caso 2).
        assert_eq!(
            hmac(b"Jefe", b"what do ya want for nothing?"),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
        let u = enlace(r#"{"a":"ñ b"}"#, None);
        assert_eq!(u, "pixpinmax://pedido?json=%7B%22a%22%3A%22%C3%B1%20b%22%7D");
        let f = enlace("{}", Some(&[7u8; 32]));
        assert!(f.contains("&firma=") && f.len() == "pixpinmax://pedido?json=%7B%7D&firma=".len() + 64, "{f}");
    }

    #[test]
    fn lo_que_cambia_algo_lleva_su_pregunta_y_lo_que_borra_la_suya() {
        let e = acciones::por_nombre("editar_tarea").unwrap();
        let q = pregunta_de(e, &args(r#"{"tarea":"pan","texto":"pan integral"}"#)).unwrap();
        assert!(q.starts_with("Claude pide: Cambia el texto") && q.contains("«pan integral»"), "{q}");
        assert!(q.contains("tarea: «pan»"), "{q}");
        // La de borrar ya pregunta en la app con su frase: no se duplica.
        assert!(pregunta_de(acciones::por_nombre("borrar_lista").unwrap(), &args(r#"{"lista":"x"}"#)).is_none());
        assert!(pregunta_de(acciones::por_nombre("nota_nueva").unwrap(), &Map::new()).is_none());
    }

    #[test]
    fn la_respuesta_se_lee_bien_o_mal() {
        assert_eq!(leer_respuesta(r#"{"ok":true,"texto":"Apuntada «pan»."}"#), Ok("Apuntada «pan».".into()));
        assert_eq!(leer_respuesta(r#"{"ok":true}"#), Ok("Hecho.".into()));
        assert_eq!(leer_respuesta(r#"{"ok":false,"texto":"No encuentro «x»"}"#), Err("No encuentro «x»".into()));
        assert!(leer_respuesta("basura").is_err());
    }

    #[test]
    fn sin_respuesta_se_rinde_al_plazo() {
        let f = std::env::temp_dir().join(format!("pixpin-mcp-nada-{}.json", std::process::id()));
        let t0 = Instant::now();
        assert!(esperar(&f, Duration::from_millis(150)).unwrap_err().contains("no contesto"));
        assert!(t0.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn lo_mal_puesto_no_sale_de_aqui() {
        // Sin «texto» no hay tarea: el error de la tabla, sin buscar PixPin.
        let e = hacer(acciones::por_nombre("anadir_tarea").unwrap(), &Map::new()).unwrap_err();
        assert!(e.contains("«texto»"), "{e}");
    }
}
