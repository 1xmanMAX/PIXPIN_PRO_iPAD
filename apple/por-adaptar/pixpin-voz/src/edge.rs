//! **Las voces de Microsoft Edge** («Leer en voz alta» de Edge): mas de 300
//! voces neuronales, gratis y sin cuenta. Puerto de `motor/EdgeVoz.kt` y de
//! `ui/VozDeEdge.kt` del movil (v0.90, `6e97552`, 22/23-sep-2026).
//!
//! **No es un servicio oficial.** Se le habla como lo hace Edge, copiando al
//! cliente de referencia `rany2/edge-tts` igual que el movil: el `Sec-MS-GEC`
//! (la hora redondeada a cinco minutos con el token, en SHA-256), un mensaje
//! de ajustes, el texto en SSML y, de vuelta, tramas binarias con el MP3.
//! Microsoft puede cambiarlo o cerrarlo cuando quiera: por eso quien lo usa
//! (`apps/pixpin/src/leer_en_voz.rs`) vuelve a la voz de Windows en cuanto
//! falla. **El texto se manda a Microsoft**, y el engranaje del lector lo
//! dice, como el del movil.
//!
//! La primera mitad son las cuentas, sin red y sin Windows (con las pruebas
//! de `EdgeVozTest.kt`). La segunda habla con Microsoft con **WinHTTP**, que
//! ya trae Windows y ya usa este crate para bajar el modelo de Whisper
//! (`whisper::bajar`). El documento de la tanda proponia el `MessageWebSocket`
//! de WinRT, pero ese no deja ver la respuesta de un 403: sin su cabecera
//! `Date` no se puede ajustar el reloj y reintentar, que es lo que hace el
//! movil cuando la hora del equipo no cuadra con la de Microsoft. WinHTTP
//! hace el WebSocket (`WinHttpWebSocketCompleteUpgrade`) y deja leer las
//! cabeceras; tampoco anade ningun crate.
//!
//! Lo leido se guarda en la cache, **un MP3 por voz y texto** (150 MB como
//! mucho; se van los que hace mas que no se oyen): volver a escuchar un
//! parrafo no gasta red ni depende de Microsoft. La lista de voces se guarda
//! una semana.

use sha2::{Digest, Sha256};

/// El token de cliente de Edge (`TRUSTED_CLIENT_TOKEN` de edge-tts).
pub const TOKEN: &str = "6A5AA1D4EAFF4E9FB37E23D68491D6F4";
/// La version de Chromium con la que se presenta (la del movil).
pub const CHROMIUM: &str = "143.0.3650.75";
/// Donde esta la lista de voces (el de siempre).
const SERVIDOR: &str = "speech.platform.bing.com";
const RUTA: &str = "/consumer/speech/synthesize/readaloud";
/// **Donde se habla, desde octubre de 2026.** El de siempre
/// (`speech.platform.bing.com/…/edge/v1`, el del movil) contesta 403 al
/// WebSocket de hablar desde el 12-oct (medido aqui el 13-oct, con la hora
/// bien y el mismo `Sec-MS-GEC`; `rany2/edge-tts` #490). Edge 155 habla por
/// este otro, que acepta el mismo token como `Ocp-Apim-Subscription-Key` y el
/// mismo `Sec-MS-GEC` (edge-tts #491, comprobado aqui: 101). La lista de
/// voces sigue en el de siempre.
const SERVIDOR_DE_HABLAR: &str = "api.msedgeservices.com";
const RUTA_DE_HABLAR: &str = "/tts/cognitiveservices/websocket/v1";

/// Lo mas largo que se manda de una vez. El servicio corta en 4096 bytes ya
/// escapados (`EdgeVoz.TOPE`).
pub const TOPE: usize = 1000;

/// **Las voces de IA de Microsoft**, las «multilingues» (las de Copilot):
/// leen cualquier idioma con la misma voz. Las seis que mejor suenan, en este
/// orden (`EdgeVoz.MULTILINGUES`).
pub const MULTILINGUES: [&str; 6] = [
    "en-US-AvaMultilingualNeural",
    "en-US-AndrewMultilingualNeural",
    "en-US-EmmaMultilingualNeural",
    "en-US-BrianMultilingualNeural",
    "fr-FR-VivienneMultilingualNeural",
    "de-DE-SeraphinaMultilingualNeural",
];

/// Cuantas del propio idioma se ofrecen, ademas de las multilingues.
pub const NATIVAS: usize = 4;

/// Cuanto vale la lista de voces guardada: una semana.
pub const SEGUNDOS_DE_LA_LISTA: u64 = 7 * 24 * 3600;

/// Lo que ocupa como mucho la cache de lo leido.
pub const TOPE_DE_CACHE: u64 = 150 * 1024 * 1024;

/// `https://…/voices/list?trustedclienttoken=…`
pub fn url_de_voces() -> String {
    format!("https://{SERVIDOR}{RUTA}/voices/list?trustedclienttoken={TOKEN}")
}

/// La ruta (con su consulta) del WebSocket de hablar.
pub fn ruta_de_hablar(conexion: &str, gec: &str) -> String {
    format!(
        "{RUTA_DE_HABLAR}?Ocp-Apim-Subscription-Key={TOKEN}&ConnectionId={conexion}&Sec-MS-GEC={gec}&Sec-MS-GEC-Version=1-{CHROMIUM}"
    )
}

/// `wss://…` (`EdgeVoz.urlDeHablar`, con el servidor nuevo).
pub fn url_de_hablar(conexion: &str, gec: &str) -> String {
    format!("wss://{SERVIDOR_DE_HABLAR}{}", ruta_de_hablar(conexion, gec))
}

fn mayor() -> &'static str {
    CHROMIUM.split('.').next().unwrap_or(CHROMIUM)
}

/// El `User-Agent` de Edge.
pub fn agente() -> String {
    let m = mayor();
    format!(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{m}.0.0.0 Safari/537.36 Edg/{m}.0.0.0"
    )
}

/// **Las cabeceras con las que se presenta Edge** (`EdgeVoz.cabeceras`).
pub fn cabeceras(muid: &str) -> Vec<(&'static str, String)> {
    vec![
        ("User-Agent", agente()),
        ("Accept-Language", "en-US,en;q=0.9".into()),
        ("Pragma", "no-cache".into()),
        ("Cache-Control", "no-cache".into()),
        (
            "Origin",
            "chrome-extension://jdiccldimpdaibmpdkjnbmckianbfold".into(),
        ),
        ("Cookie", format!("muid={muid};")),
    ]
}

/// **El `Sec-MS-GEC`**: la hora de Windows (centenas de nanosegundos desde
/// 1601) redondeada a cinco minutos hacia abajo, seguida del token, en
/// SHA-256 y en mayusculas (`EdgeVoz.gec`).
pub fn gec(unix_segundos: f64) -> String {
    let t = (unix_segundos + 11_644_473_600.0).max(0.0).floor() as u128;
    let t = t - t % 300;
    let centenas = t * 10_000_000;
    let hash = Sha256::digest(format!("{centenas}{TOKEN}").as_bytes());
    hash.iter().map(|b| format!("{b:02X}")).collect()
}

// ------------------------------------------------------------------ fechas

/// Dias desde el 1-1-1970 de una fecha del calendario (H. Hinnant).
fn dias_desde_1970(ano: i64, mes: u32, dia: u32) -> i64 {
    let y = if mes <= 2 { ano - 1 } else { ano };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = mes as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + dia as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Al reves: (año, mes, dia) de un dia desde 1970.
fn fecha_de_dias(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

const DIAS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
const MESES: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// **La fecha como la escribe JavaScript**, en UTC (`EdgeVoz.fecha`):
/// `Tue Oct 13 2026 10:00:00 GMT+0000 (Coordinated Universal Time)`.
pub fn fecha(ms: i64) -> String {
    let s = ms.div_euclid(1000);
    let dias = s.div_euclid(86_400);
    let resto = s.rem_euclid(86_400);
    let (a, m, d) = fecha_de_dias(dias);
    format!(
        "{} {} {d:02} {a:04} {:02}:{:02}:{:02} GMT+0000 (Coordinated Universal Time)",
        DIAS[dias.rem_euclid(7) as usize],
        MESES[(m - 1) as usize],
        resto / 3600,
        resto % 3600 / 60,
        resto % 60
    )
}

/// **La hora de Microsoft**, de la cabecera `Date` de su respuesta
/// (`Tue, 13 Oct 2026 10:00:00 GMT`), en segundos desde 1970. `None` si no
/// se entiende.
pub fn segundos_de_fecha_http(texto: &str) -> Option<f64> {
    let partes: Vec<&str> = texto.split_whitespace().collect();
    // [dia,] numero mes año hora GMT
    let p = if partes.first()?.ends_with(',') {
        &partes[1..]
    } else {
        &partes[..]
    };
    if p.len() < 4 {
        return None;
    }
    let dia: u32 = p[0].parse().ok().filter(|d| (1..=31).contains(d))?;
    let mes = MESES
        .iter()
        .position(|m| m.eq_ignore_ascii_case(p[1]))? as u32
        + 1;
    let ano: i64 = p[2].parse().ok()?;
    let hora: Vec<i64> = p[3].split(':').map(|x| x.parse().ok()).collect::<Option<_>>()?;
    if hora.len() != 3 || hora[0] > 23 || hora[1] > 59 || hora[2] > 60 {
        return None;
    }
    let s = dias_desde_1970(ano, mes, dia) * 86_400 + hora[0] * 3600 + hora[1] * 60 + hora[2];
    Some(s as f64)
}

/// **El desfase nuevo del reloj** tras un 403: lo que iba mas lo que la hora
/// de Microsoft le saca a la de aqui (`desfase += servidor - ahora`).
pub fn desfase_tras_403(desfase: f64, servidor: f64, ahora_aqui: f64) -> f64 {
    desfase + (servidor - ahora_aqui)
}

// ---------------------------------------------------------------- mensajes

/// El mensaje de ajustes (`speech.config`): MP3 de 24 kHz, sin marcas de
/// frase ni de palabra.
pub fn mensaje_de_ajustes(fecha: &str) -> String {
    format!(
        "X-Timestamp:{fecha}\r\nContent-Type:application/json; charset=utf-8\r\nPath:speech.config\r\n\r\n\
         {{\"context\":{{\"synthesis\":{{\"audio\":{{\"metadataoptions\":{{\"sentenceBoundaryEnabled\":\"false\",\"wordBoundaryEnabled\":\"false\"}},\
         \"outputFormat\":\"audio-24khz-48kbitrate-mono-mp3\"}}}}}}}}\r\n"
    )
}

/// **El texto que se lee**, con la voz. La «Z» detras de la hora es un fallo
/// de Edge que hay que copiar (`EdgeVoz.mensajeDeTexto`).
pub fn mensaje_de_texto(pedido: &str, fecha: &str, voz: &str, texto: &str) -> String {
    format!(
        "X-RequestId:{pedido}\r\nContent-Type:application/ssml+xml\r\nX-Timestamp:{fecha}Z\r\nPath:ssml\r\n\r\n\
         <speak version='1.0' xmlns='http://www.w3.org/2001/10/synthesis' xml:lang='en-US'>\
         <voice name='{}'><prosody pitch='+0Hz' rate='+0%' volume='+0%'>{}</prosody></voice></speak>",
        escapar(voz),
        escapar(&limpiar(texto))
    )
}

/// Fuera los caracteres de control que el servicio no admite (un tabulador
/// vertical de un OCR lo tumba).
pub fn limpiar(texto: &str) -> String {
    texto
        .chars()
        .map(|c| match c as u32 {
            0..=8 | 11..=12 | 14..=31 => ' ',
            _ => c,
        })
        .collect()
}

pub fn escapar(texto: &str) -> String {
    texto
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\'', "&apos;")
        .replace('"', "&quot;")
}

/// Lo que dice un mensaje de texto del servicio: su `Path` («turn.start»,
/// «turn.end»…).
pub fn ruta_de(mensaje: &str) -> Option<&str> {
    let cabeza = mensaje.split("\r\n\r\n").next()?;
    cabeza
        .split("\r\n")
        .find_map(|l| l.strip_prefix("Path:"))
}

/// **El audio de un mensaje binario**: dos bytes con lo que miden las
/// cabeceras, las cabeceras y detras el trozo de MP3. `None` si no es audio
/// o viene vacio (el ultimo, que cierra, lo esta).
pub fn audio_de(trama: &[u8]) -> Option<&[u8]> {
    if trama.len() < 2 {
        return None;
    }
    let largo = ((trama[0] as usize) << 8) | trama[1] as usize;
    if largo + 2 > trama.len() {
        return None;
    }
    let cabeceras = String::from_utf8_lossy(&trama[2..2 + largo]);
    if !cabeceras.split("\r\n").any(|l| l == "Path:audio") {
        return None;
    }
    let audio = &trama[2 + largo..];
    (!audio.is_empty()).then_some(audio)
}

// ------------------------------------------------------------------ voces

/// Una voz de la lista: «es-ES-ElviraNeural», «es-ES», «Female».
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Voz {
    pub nombre: String,
    pub idioma: String,
    pub genero: String,
}

impl Voz {
    /// «Elvira», sin el idioma delante ni «Neural» detras.
    pub fn corto(&self) -> String {
        let n = self
            .nombre
            .strip_prefix(&format!("{}-", self.idioma))
            .unwrap_or(&self.nombre);
        let n = n.strip_suffix("Neural").unwrap_or(n);
        n.replace("Multilingual", " multilingüe").trim().to_string()
    }
}

/// «es_ES», «es-es» → («es», «ES»), como `VozAlta.partes`.
pub fn partes(idioma: &str) -> (String, String) {
    let p: Vec<&str> = idioma
        .trim()
        .split(['-', '_'])
        .filter(|s| !s.is_empty())
        .collect();
    let lengua = p.first().map(|s| s.to_lowercase()).unwrap_or_default();
    let pais = p
        .get(1)
        .map(|s| s.to_uppercase())
        .filter(|s| s.len() == 2 || s.len() == 3)
        .unwrap_or_default();
    (lengua, pais)
}

/// La lista de voces que devuelve Microsoft (JSON); vacia si no se entiende.
pub fn de_lista(json: &str) -> Vec<Voz> {
    let Ok(serde_json::Value::Array(lista)) = serde_json::from_str::<serde_json::Value>(json)
    else {
        return Vec::new();
    };
    lista
        .iter()
        .filter_map(|v| {
            let texto = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string);
            Some(Voz {
                nombre: texto("ShortName")?,
                idioma: texto("Locale").unwrap_or_default(),
                genero: texto("Gender").unwrap_or_default(),
            })
        })
        .collect()
}

/// Las voces de una lengua, las del pais primero (`EdgeVoz.deLaLengua`).
pub fn de_la_lengua(voces: &[Voz], idioma: &str) -> Vec<Voz> {
    let (lengua, pais) = partes(idioma);
    let mut v: Vec<Voz> = voces
        .iter()
        .filter(|x| partes(&x.idioma).0 == lengua)
        .cloned()
        .collect();
    // Estable, como `sortedWith` de Kotlin.
    v.sort_by(|a, b| {
        let del_pais = |x: &Voz| !pais.is_empty() && partes(&x.idioma).1 == pais;
        del_pais(b)
            .cmp(&del_pais(a))
            .then_with(|| a.idioma.cmp(&b.idioma))
            .then_with(|| a.nombre.cmp(&b.nombre))
    });
    v
}

pub fn es_multilingue(v: &Voz) -> bool {
    v.nombre.contains("Multilingual")
}

/// **Solo las mejores** (`EdgeVoz.lasMejores`, «el listado es absurdamente
/// grande»): hasta [`NATIVAS`] del idioma —las del pais primero— y las
/// [`MULTILINGUES`]. De 322 a unas diez.
pub fn las_mejores(voces: &[Voz], idioma: &str) -> Vec<Voz> {
    let sin_ia: Vec<Voz> = voces.iter().filter(|v| !es_multilingue(v)).cloned().collect();
    let mut salida: Vec<Voz> = de_la_lengua(&sin_ia, idioma)
        .into_iter()
        .take(NATIVAS)
        .collect();
    salida.extend(
        MULTILINGUES
            .iter()
            .filter_map(|n| voces.iter().find(|v| v.nombre == *n).cloned()),
    );
    salida
}

/// **La voz que se usa**: la que eligio el usuario si esta entre
/// [`las_mejores`]; si no, la primera del pais, y si el idioma no tiene
/// ninguna, la primera multilingue (`EdgeVoz.elegir`).
pub fn elegir(voces: &[Voz], idioma: &str, preferida: Option<&str>) -> Option<Voz> {
    let suyas = las_mejores(voces, idioma);
    suyas
        .iter()
        .find(|v| Some(v.nombre.as_str()) == preferida)
        .or_else(|| suyas.first())
        .cloned()
}

// ------------------------------------------------------------------ la cache

/// El nombre del MP3 de un texto con una voz en la cache.
pub fn nombre_en_cache(voz: &str, texto: &str) -> String {
    let h = Sha256::digest(format!("{voz}\u{0}{texto}").as_bytes());
    let hex: String = h.iter().take(20).map(|b| format!("{b:02x}")).collect();
    format!("{hex}.mp3")
}

/// **Que se quita de la cache**: dados los ficheros (nombre, bytes, ultima
/// vez que se oyeron), los que sobran pasado `tope`, empezando por los que
/// hace mas que no se oyen (`VozDeEdge.podar`).
pub fn sobran(mut ficheros: Vec<(String, u64, u64)>, tope: u64) -> Vec<String> {
    ficheros.sort_by(|a, b| b.2.cmp(&a.2));
    let mut suma = 0u64;
    let mut fuera = Vec::new();
    for (n, bytes, _) in ficheros {
        suma += bytes;
        if suma > tope {
            fuera.push(n);
        }
    }
    fuera
}

/// Si la lista de voces guardada aun vale (de menos de una semana).
pub fn lista_fresca(edad_segundos: Option<u64>) -> bool {
    edad_segundos.is_some_and(|e| e < SEGUNDOS_DE_LA_LISTA)
}

#[cfg(windows)]
pub use red::{ErrorEdge, archivo, hablar, voces, voces_guardadas};

/// **La conexion con Microsoft** (`VozDeEdge`), por WinHTTP.
#[cfg(windows)]
mod red {
    use std::ffi::c_void;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    use sha2::{Digest, Sha256};
    use windows::Win32::Networking::WinHttp::{
        INTERNET_DEFAULT_HTTPS_PORT, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
        WINHTTP_ACCESS_TYPE_DEFAULT_PROXY, WINHTTP_ADDREQ_FLAG_ADD, WINHTTP_ADDREQ_FLAG_REPLACE,
        WINHTTP_FLAG_SECURE, WINHTTP_OPEN_REQUEST_FLAGS, WINHTTP_OPTION_UPGRADE_TO_WEB_SOCKET,
        WINHTTP_QUERY_DATE, WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE,
        WINHTTP_WEB_SOCKET_BINARY_FRAGMENT_BUFFER_TYPE, WINHTTP_WEB_SOCKET_BINARY_MESSAGE_BUFFER_TYPE,
        WINHTTP_WEB_SOCKET_BUFFER_TYPE, WINHTTP_WEB_SOCKET_CLOSE_BUFFER_TYPE,
        WINHTTP_WEB_SOCKET_UTF8_FRAGMENT_BUFFER_TYPE, WINHTTP_WEB_SOCKET_UTF8_MESSAGE_BUFFER_TYPE,
        WinHttpAddRequestHeaders, WinHttpCloseHandle, WinHttpConnect, WinHttpOpen,
        WinHttpOpenRequest, WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse,
        WinHttpSendRequest, WinHttpSetOption, WinHttpSetTimeouts, WinHttpWebSocketClose,
        WinHttpWebSocketCompleteUpgrade, WinHttpWebSocketReceive, WinHttpWebSocketSend,
    };
    use windows::core::{HSTRING, PCWSTR, w};

    use super::*;

    /// Por que no llego la voz de Microsoft.
    #[derive(Debug, thiserror::Error)]
    pub enum ErrorEdge {
        #[error("Microsoft no contesto: {0}")]
        Red(String),
        #[error("Microsoft contesto {0}")]
        Estado(u32),
        #[error("Microsoft no devolvio audio")]
        SinAudio,
        #[error("no se pudo guardar el audio: {0}")]
        Disco(#[from] std::io::Error),
    }

    /// Un fallo de una sola vez: si fue la hora, la de Microsoft.
    enum Fallo {
        Hora(f64),
        Otro(ErrorEdge),
    }

    /// Lo que va desfasado el reloj de aqui con el de Microsoft, en
    /// milisegundos (con signo, en un `u64` de bits): el `Sec-MS-GEC` va
    /// por la hora.
    static DESFASE_MS: AtomicU64 = AtomicU64::new(0);

    fn desfase() -> f64 {
        DESFASE_MS.load(Ordering::Relaxed) as i64 as f64 / 1000.0
    }

    fn ahora() -> f64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0)
    }

    /// 32 cifras hexadecimales que no se repiten: el `ConnectionId`, el
    /// `X-RequestId` y el `muid`.
    fn azar_hex() -> String {
        static CUENTA: AtomicU64 = AtomicU64::new(0);
        let n = CUENTA.fetch_add(1, Ordering::Relaxed);
        let t = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let h = Sha256::digest(format!("{t}:{n}:{}", std::process::id()).as_bytes());
        h.iter().take(16).map(|b| format!("{b:02X}")).collect()
    }

    fn muid() -> &'static str {
        static MUID: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        MUID.get_or_init(azar_hex)
    }

    /// Un asa de WinHTTP que se cierra sola.
    struct Asa(*mut c_void);

    impl Drop for Asa {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: el asa la devolvio WinHTTP, es de este `Asa` y no
                // se ha cerrado antes.
                let _ = unsafe { WinHttpCloseHandle(self.0) };
            }
        }
    }

    fn red(e: impl std::fmt::Display) -> Fallo {
        Fallo::Otro(ErrorEdge::Red(e.to_string()))
    }

    /// La peticion abierta y enviada (sesion, conexion, peticion), con su
    /// estado. `websocket` le pide a WinHTTP que suba a WebSocket.
    fn pedir(servidor: &str, ruta: &str, websocket: bool) -> Result<(Asa, Asa, Asa, u32), Fallo> {
        let ua = HSTRING::from(agente());
        // SAFETY: las cadenas son `HSTRING` o literales vivos durante cada
        // llamada; cada asa se envuelve en `Asa` en cuanto existe, y se
        // devuelven en el orden en que hay que soltarlas al reves.
        unsafe {
            let mut sesion = Asa(WinHttpOpen(
                &ua,
                WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
                PCWSTR::null(),
                PCWSTR::null(),
                0,
            ));
            if sesion.0.is_null() {
                sesion = Asa(WinHttpOpen(
                    &ua,
                    WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
                    PCWSTR::null(),
                    PCWSTR::null(),
                    0,
                ));
            }
            if sesion.0.is_null() {
                return Err(red(windows::core::Error::from_thread()));
            }
            // Conectar en 10 s y esperar cada trama 20 s, como el movil.
            let _ = WinHttpSetTimeouts(sesion.0, 10_000, 10_000, 10_000, 20_000);
            let conexion = Asa(WinHttpConnect(
                sesion.0,
                &HSTRING::from(servidor),
                INTERNET_DEFAULT_HTTPS_PORT,
                0,
            ));
            if conexion.0.is_null() {
                return Err(red(windows::core::Error::from_thread()));
            }
            let peticion = Asa(WinHttpOpenRequest(
                conexion.0,
                w!("GET"),
                &HSTRING::from(ruta),
                PCWSTR::null(),
                PCWSTR::null(),
                std::ptr::null(),
                WINHTTP_OPEN_REQUEST_FLAGS(WINHTTP_FLAG_SECURE.0),
            ));
            if peticion.0.is_null() {
                return Err(red(windows::core::Error::from_thread()));
            }
            if websocket {
                WinHttpSetOption(Some(peticion.0), WINHTTP_OPTION_UPGRADE_TO_WEB_SOCKET, None)
                    .map_err(red)?;
            }
            let lineas: String = cabeceras(muid())
                .into_iter()
                .filter(|(k, _)| *k != "User-Agent")
                .map(|(k, v)| format!("{k}: {v}\r\n"))
                .collect();
            let ancho: Vec<u16> = lineas.encode_utf16().collect();
            WinHttpAddRequestHeaders(
                peticion.0,
                &ancho,
                WINHTTP_ADDREQ_FLAG_ADD | WINHTTP_ADDREQ_FLAG_REPLACE,
            )
            .map_err(red)?;
            WinHttpSendRequest(peticion.0, None, None, 0, 0, 0).map_err(red)?;
            WinHttpReceiveResponse(peticion.0, std::ptr::null_mut()).map_err(red)?;
            let mut n: u32 = 0;
            let mut largo = std::mem::size_of::<u32>() as u32;
            let estado = WinHttpQueryHeaders(
                peticion.0,
                WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                PCWSTR::null(),
                Some((&mut n as *mut u32).cast()),
                &mut largo,
                std::ptr::null_mut(),
            )
            .map(|_| n)
            .unwrap_or(0);
            Ok((sesion, conexion, peticion, estado))
        }
    }

    /// La cabecera `Date` de la respuesta, como texto.
    fn fecha_de(peticion: &Asa) -> Option<String> {
        let mut buf = vec![0u16; 128];
        let mut largo = (buf.len() * 2) as u32;
        // SAFETY: el bufer es nuestro y `largo` dice lo que mide en bytes.
        unsafe {
            WinHttpQueryHeaders(
                peticion.0,
                WINHTTP_QUERY_DATE,
                PCWSTR::null(),
                Some(buf.as_mut_ptr().cast()),
                &mut largo,
                std::ptr::null_mut(),
            )
            .ok()?;
        }
        Some(String::from_utf16_lossy(&buf[..largo as usize / 2]))
    }

    /// **Un trozo, por una conexion** (`hablarUnaVez`): se abre, se mandan
    /// los ajustes y el texto, llegan los trozos de MP3 y al «turn.end» se
    /// cierra, como hace `edge-tts`.
    fn hablar_una_vez(texto: &str, voz: &str) -> Result<Vec<u8>, Fallo> {
        let gec = gec(ahora() + desfase());
        let (_sesion, _conexion, peticion, estado) =
            pedir(SERVIDOR_DE_HABLAR, &ruta_de_hablar(&azar_hex(), &gec), true)?;
        if estado == 403
            && let Some(servidor) = fecha_de(&peticion).and_then(|f| segundos_de_fecha_http(&f))
        {
            return Err(Fallo::Hora(servidor));
        }
        if estado != 101 {
            return Err(Fallo::Otro(ErrorEdge::Estado(estado)));
        }
        // SAFETY: la peticion acaba de contestar 101; el asa del WebSocket
        // se envuelve enseguida y la de la peticion se suelta despues (la
        // documentacion de WinHTTP lo pide asi).
        let ws = Asa(unsafe { WinHttpWebSocketCompleteUpgrade(peticion.0, None) });
        drop(peticion);
        if ws.0.is_null() {
            return Err(red(windows::core::Error::from_thread()));
        }
        let mandar = |texto: &str| -> Result<(), Fallo> {
            // SAFETY: el WebSocket esta vivo y el bufer es nuestro durante
            // la llamada.
            let r = unsafe {
                WinHttpWebSocketSend(
                    ws.0,
                    WINHTTP_WEB_SOCKET_UTF8_MESSAGE_BUFFER_TYPE,
                    Some(texto.as_bytes()),
                )
            };
            if r == 0 { Ok(()) } else { Err(red(format!("enviar: {r}"))) }
        };
        let f = fecha(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0),
        );
        mandar(&mensaje_de_ajustes(&f))?;
        mandar(&mensaje_de_texto(&azar_hex(), &f, voz, texto))?;

        let empezo = Instant::now();
        let mut audio = Vec::new();
        let mut mensaje: Vec<u8> = Vec::new();
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            if empezo.elapsed() > Duration::from_secs(30) {
                return Err(red("treinta segundos sin acabar"));
            }
            let mut leidos: u32 = 0;
            let mut tipo = WINHTTP_WEB_SOCKET_BUFFER_TYPE(0);
            // SAFETY: el WebSocket esta vivo; el bufer es nuestro y se le
            // dice lo que mide.
            let r = unsafe {
                WinHttpWebSocketReceive(
                    ws.0,
                    buf.as_mut_ptr().cast(),
                    buf.len() as u32,
                    &mut leidos,
                    &mut tipo,
                )
            };
            if r != 0 {
                return Err(red(format!("recibir: {r}")));
            }
            mensaje.extend_from_slice(&buf[..leidos as usize]);
            match tipo {
                WINHTTP_WEB_SOCKET_BINARY_FRAGMENT_BUFFER_TYPE
                | WINHTTP_WEB_SOCKET_UTF8_FRAGMENT_BUFFER_TYPE => continue,
                WINHTTP_WEB_SOCKET_BINARY_MESSAGE_BUFFER_TYPE => {
                    if let Some(a) = audio_de(&mensaje) {
                        audio.extend_from_slice(a);
                    }
                }
                WINHTTP_WEB_SOCKET_UTF8_MESSAGE_BUFFER_TYPE => {
                    let t = String::from_utf8_lossy(&mensaje);
                    if ruta_de(&t) == Some("turn.end") {
                        // SAFETY: WebSocket vivo; cierre normal sin motivo.
                        let _ = unsafe { WinHttpWebSocketClose(ws.0, 1000, None, 0) };
                        return if audio.is_empty() {
                            Err(Fallo::Otro(ErrorEdge::SinAudio))
                        } else {
                            Ok(audio)
                        };
                    }
                }
                WINHTTP_WEB_SOCKET_CLOSE_BUFFER_TYPE => {
                    return Err(red("Microsoft cerro la conexion"));
                }
                _ => {}
            }
            mensaje.clear();
        }
    }

    /// **Un trozo de texto en MP3** (`hablar`). Si Microsoft dice que la hora
    /// no cuadra (403), se ajusta el reloj y se reintenta una vez.
    pub fn hablar(texto: &str, voz: &str) -> Result<Vec<u8>, ErrorEdge> {
        match hablar_una_vez(texto, voz) {
            Ok(a) => Ok(a),
            Err(Fallo::Otro(e)) => Err(e),
            Err(Fallo::Hora(servidor)) => {
                let nuevo = desfase_tras_403(desfase(), servidor, ahora());
                DESFASE_MS.store((nuevo * 1000.0).round() as i64 as u64, Ordering::Relaxed);
                tracing::info!(desfase = nuevo, "la hora no cuadraba con la de Microsoft");
                hablar_una_vez(texto, voz).map_err(|f| match f {
                    Fallo::Otro(e) => e,
                    Fallo::Hora(_) => ErrorEdge::Estado(403),
                })
            }
        }
    }

    /// Un solo hilo a la vez poda la cache.
    static PODANDO: Mutex<()> = Mutex::new(());

    /// **El MP3 de `texto` con la voz `voz`** (`VozDeEdge.archivo`): de la
    /// cache si ya se leyo; si no, pedido a Microsoft en los trozos que da
    /// `trocear` (el de la voz, `voz_alta::trozos` con [`TOPE`]).
    pub fn archivo(
        carpeta: &Path,
        texto: &str,
        voz: &str,
        trocear: impl Fn(&str) -> Vec<String>,
    ) -> Result<PathBuf, ErrorEdge> {
        std::fs::create_dir_all(carpeta)?;
        let destino = carpeta.join(nombre_en_cache(voz, texto));
        if std::fs::metadata(&destino).is_ok_and(|m| m.len() > 0) {
            // Se oye otra vez: es de lo ultimo que se quita.
            if let Ok(f) = std::fs::File::options().append(true).open(&destino) {
                let _ = f.set_modified(SystemTime::now());
            }
            return Ok(destino);
        }
        let mut sale = Vec::new();
        for t in trocear(texto) {
            sale.extend(hablar(&t, voz)?);
        }
        if sale.is_empty() {
            return Err(ErrorEdge::SinAudio);
        }
        let tmp = destino.with_extension("mp3.tmp");
        std::fs::write(&tmp, &sale)?;
        let _ = std::fs::remove_file(&destino);
        std::fs::rename(&tmp, &destino)?;
        podar(carpeta);
        Ok(destino)
    }

    fn podar(carpeta: &Path) {
        let Ok(_uno) = PODANDO.try_lock() else {
            return;
        };
        let Ok(dir) = std::fs::read_dir(carpeta) else {
            return;
        };
        let ficheros: Vec<(String, u64, u64)> = dir
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "mp3"))
            .filter_map(|e| {
                let m = e.metadata().ok()?;
                let t = m
                    .modified()
                    .ok()?
                    .duration_since(UNIX_EPOCH)
                    .ok()?
                    .as_millis() as u64;
                Some((e.file_name().to_string_lossy().into_owned(), m.len(), t))
            })
            .collect();
        for n in sobran(ficheros, TOPE_DE_CACHE) {
            let _ = std::fs::remove_file(carpeta.join(n));
        }
    }

    const LISTA: &str = "voces-edge.json";

    /// **Las voces guardadas**, sin red (aunque sean de hace mas de una
    /// semana). Vacia si nunca se pidieron.
    pub fn voces_guardadas(carpeta: &Path) -> Vec<Voz> {
        std::fs::read_to_string(carpeta.join(LISTA))
            .map(|t| de_lista(&t))
            .unwrap_or_default()
    }

    /// **Las voces** (`VozDeEdge.voces`), de la lista de Microsoft; guardada
    /// una semana en `carpeta` para no pedirla cada vez. Sin red y sin copia,
    /// vacia.
    pub fn voces(carpeta: &Path) -> Vec<Voz> {
        let copia = carpeta.join(LISTA);
        let edad = std::fs::metadata(&copia)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| SystemTime::now().duration_since(t).ok())
            .map(|d| d.as_secs());
        if !lista_fresca(edad)
            && let Some(cuerpo) = bajar_la_lista()
            && !de_lista(&cuerpo).is_empty()
        {
            let _ = std::fs::create_dir_all(carpeta);
            let _ = std::fs::write(&copia, cuerpo);
        }
        voces_guardadas(carpeta)
    }

    fn bajar_la_lista() -> Option<String> {
        let url = url_de_voces();
        let ruta = url.strip_prefix(&format!("https://{SERVIDOR}"))?;
        let (_s, _c, peticion, estado) = pedir(SERVIDOR, ruta, false).ok()?;
        if estado != 200 {
            tracing::info!(estado, "Microsoft no dio la lista de voces");
            return None;
        }
        let mut cuerpo = Vec::new();
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let mut n: u32 = 0;
            // SAFETY: peticion viva; bufer nuestro de ese largo.
            unsafe {
                WinHttpReadData(peticion.0, buf.as_mut_ptr().cast(), buf.len() as u32, &mut n)
                    .ok()?;
            }
            if n == 0 {
                break;
            }
            cuerpo.extend_from_slice(&buf[..n as usize]);
        }
        String::from_utf8(cuerpo).ok()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_sec_ms_gec_redondea_a_cinco_minutos_y_es_el_sha256_en_mayusculas() {
        let a = gec(1_790_000_000.0);
        // Dentro de los mismos cinco minutos, el mismo; al pasar al
        // siguiente tramo, otro.
        assert_eq!(a, gec(1_790_000_000.0 + 50.0));
        assert_ne!(a, gec(1_790_000_000.0 + 400.0));
        assert!(a.len() == 64 && a.chars().all(|c| c.is_ascii_digit() || ('A'..='F').contains(&c)));
        // Cuenta a mano: (1790000000 + 11644473600) redondeado a 300, en
        // centenas de nanosegundos.
        let t = 1_790_000_000u64 + 11_644_473_600;
        let t = t - t % 300;
        let esperado: String = Sha256::digest(format!("{t}0000000{TOKEN}").as_bytes())
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect();
        assert_eq!(esperado, a);
    }

    #[test]
    fn el_texto_va_escapado_y_sin_caracteres_de_control() {
        let m = mensaje_de_texto("id", "fecha", "es-ES-ElviraNeural", "a < b & 'c'\u{0B}d");
        assert!(m.contains("a &lt; b &amp; &apos;c&apos; d"), "{m}");
        assert!(m.contains("X-Timestamp:fechaZ\r\nPath:ssml\r\n\r\n<speak"));
        assert!(m.contains("<voice name='es-ES-ElviraNeural'>"));
        // Lo de los ajustes: MP3 de 24 kHz y su ruta.
        let a = mensaje_de_ajustes("f");
        assert!(a.starts_with("X-Timestamp:f\r\n"));
        assert!(a.contains("Path:speech.config\r\n\r\n{\"context\""));
        assert!(a.contains("\"outputFormat\":\"audio-24khz-48kbitrate-mono-mp3\"}}}}\r\n"));
        assert!(serde_json::from_str::<serde_json::Value>(a.split("\r\n\r\n").nth(1).unwrap().trim()).is_ok());
    }

    #[test]
    fn el_audio_se_saca_de_la_trama_binaria() {
        fn trama(cab: &str, datos: &[u8]) -> Vec<u8> {
            let c = cab.as_bytes();
            let mut v = vec![(c.len() >> 8) as u8, c.len() as u8];
            v.extend_from_slice(c);
            v.extend_from_slice(datos);
            v
        }
        let mp3 = [1u8, 2, 3];
        assert_eq!(
            audio_de(&trama("X-RequestId:x\r\nContent-Type:audio/mpeg\r\nPath:audio\r\n", &mp3)),
            Some(&mp3[..])
        );
        assert_eq!(audio_de(&trama("Path:audio\r\n", &[])), None);
        assert_eq!(audio_de(&trama("Path:otra\r\n", &mp3)), None);
        assert_eq!(audio_de(&[0]), None);
        // Cabeceras que dicen medir mas de lo que llega: nada.
        assert_eq!(audio_de(&[0, 200, 1, 2]), None);
        assert_eq!(ruta_de("X-RequestId:x\r\nPath:turn.end\r\n\r\n{}"), Some("turn.end"));
        assert_eq!(ruta_de("X-RequestId:x\r\n\r\nPath:turn.end"), None);
    }

    #[test]
    fn solo_las_mejores_las_del_idioma_primero_y_las_multilingues_detras() {
        let mut muchas: Vec<Voz> = (1..=30)
            .map(|i| Voz {
                nombre: format!("es-ES-Voz{i}Neural"),
                idioma: "es-ES".into(),
                genero: "Female".into(),
            })
            .collect();
        muchas.extend(MULTILINGUES.iter().rev().map(|n| Voz {
            nombre: n.to_string(),
            idioma: n[..5].to_string(),
            genero: "Female".into(),
        }));
        muchas.push(Voz {
            nombre: "fr-FR-DeniseNeural".into(),
            idioma: "fr-FR".into(),
            genero: "Female".into(),
        });
        let lista = las_mejores(&muchas, "es-ES");
        assert_eq!(lista.len(), NATIVAS + MULTILINGUES.len());
        assert!(lista[..NATIVAS].iter().all(|v| v.idioma == "es-ES"));
        assert_eq!(
            lista[NATIVAS..].iter().map(|v| v.nombre.as_str()).collect::<Vec<_>>(),
            MULTILINGUES.to_vec()
        );
        // Una preferida que ya no esta entre las mejores no se usa.
        assert_eq!(
            elegir(&muchas, "es-ES", Some("es-ES-Voz29Neural")).map(|v| v.nombre),
            Some(lista[0].nombre.clone())
        );
    }

    #[test]
    fn se_elige_la_voz_preferida_y_si_no_la_del_pais() {
        let voces = de_lista(
            r#"[{"ShortName":"es-MX-DaliaNeural","Locale":"es-MX","Gender":"Female"},
                {"ShortName":"es-ES-ElviraNeural","Locale":"es-ES","Gender":"Female"},
                {"ShortName":"es-ES-AlvaroNeural","Locale":"es-ES","Gender":"Male"},
                {"ShortName":"en-US-EmmaMultilingualNeural","Locale":"en-US","Gender":"Female"}]"#,
        );
        assert_eq!(voces.len(), 4);
        let nombre = |v: Option<Voz>| v.map(|v| v.nombre).unwrap_or_default();
        assert_eq!(nombre(elegir(&voces, "es-ES", None)), "es-ES-AlvaroNeural");
        assert_eq!(nombre(elegir(&voces, "es-MX", None)), "es-MX-DaliaNeural");
        assert_eq!(
            nombre(elegir(&voces, "es-PE", Some("es-ES-ElviraNeural"))),
            "es-ES-ElviraNeural"
        );
        // En frances no hay nativa en esta lista: la multilingue, que lee
        // cualquier idioma.
        assert_eq!(nombre(elegir(&voces, "fr-FR", None)), "en-US-EmmaMultilingualNeural");
        assert_eq!(voces[3].corto(), "Emma multilingüe");
        assert_eq!(voces[1].corto(), "Elvira");
        assert!(de_lista("no es json").is_empty());
        assert!(elegir(&[], "es", None).is_none());
    }

    #[test]
    fn la_fecha_va_como_la_de_javascript_y_la_de_microsoft_se_lee() {
        // 13-oct-2026 10:00:00 UTC, un martes.
        let s = dias_desde_1970(2026, 10, 13) * 86_400 + 10 * 3600;
        assert_eq!(
            fecha(s * 1000 + 250),
            "Tue Oct 13 2026 10:00:00 GMT+0000 (Coordinated Universal Time)"
        );
        assert_eq!(fecha(0), "Thu Jan 01 1970 00:00:00 GMT+0000 (Coordinated Universal Time)");
        assert_eq!(fecha_de_dias(dias_desde_1970(2024, 2, 29)), (2024, 2, 29));
        assert_eq!(segundos_de_fecha_http("Tue, 13 Oct 2026 10:00:00 GMT"), Some(s as f64));
        assert_eq!(segundos_de_fecha_http("13 Oct 2026 10:00:00 GMT"), Some(s as f64));
        assert_eq!(segundos_de_fecha_http("Tue, 13 Foo 2026 10:00:00 GMT"), None);
        assert_eq!(segundos_de_fecha_http("Tue, 13 Oct 2026 25:00:00 GMT"), None);
        assert_eq!(segundos_de_fecha_http(""), None);
    }

    #[test]
    fn el_reloj_del_reintento_se_corre_lo_que_va_de_una_hora_a_otra() {
        // El equipo va diez minutos atrasado: el siguiente GEC lleva la hora de Microsoft.
        let aqui = 1_790_000_000.0;
        let servidor = aqui + 600.0;
        let d = desfase_tras_403(0.0, servidor, aqui);
        assert_eq!(d, 600.0);
        assert_eq!(gec(aqui + d), gec(servidor));
        assert_ne!(gec(aqui), gec(servidor), "sin ajustar, otro GEC: el 403");
        // Un segundo 403 suma sobre lo que ya habia.
        assert_eq!(desfase_tras_403(600.0, aqui + 650.0, aqui + 10.0), 1240.0);
    }

    #[test]
    fn la_cache_quita_lo_que_hace_mas_que_no_se_oye_y_la_lista_vale_una_semana() {
        let f = vec![
            ("viejo.mp3".to_string(), 60, 1),
            ("nuevo.mp3".to_string(), 60, 3),
            ("medio.mp3".to_string(), 60, 2),
        ];
        assert_eq!(sobran(f.clone(), 130), vec!["viejo.mp3"]);
        assert!(sobran(f, 1000).is_empty());
        assert_ne!(nombre_en_cache("a", "b"), nombre_en_cache("a", "c"));
        assert_ne!(nombre_en_cache("ab", "c"), nombre_en_cache("a", "bc"));
        assert!(nombre_en_cache("a", "b").ends_with(".mp3"));
        assert!(lista_fresca(Some(3600)));
        assert!(!lista_fresca(Some(SEGUNDOS_DE_LA_LISTA + 1)));
        assert!(!lista_fresca(None));
    }

    #[test]
    fn las_cabeceras_y_la_url_son_las_de_edge() {
        let c = cabeceras("ABC");
        assert!(c.iter().any(|(k, v)| *k == "Cookie" && v == "muid=ABC;"));
        assert!(c.iter().any(|(k, v)| *k == "User-Agent" && v.contains("Edg/143.0.0.0")));
        let u = url_de_hablar("id", "GEC");
        assert!(u.starts_with(
            "wss://api.msedgeservices.com/tts/cognitiveservices/websocket/v1?Ocp-Apim-Subscription-Key=6A5AA1D4EAFF4E9FB37E23D68491D6F4&"
        ));
        assert!(url_de_voces().starts_with("https://speech.platform.bing.com/"));
        assert!(u.ends_with("&ConnectionId=id&Sec-MS-GEC=GEC&Sec-MS-GEC-Version=1-143.0.3650.75"));
    }

    /// **Habla de verdad con Microsoft**: solo con `EDGE_EN_VIVO=1`, que las
    /// pruebas de siempre no dependan de la red ni de un servicio que no es
    /// nuestro (`EdgeVozEnVivoTest`). Sirve para ver, si falla en el PC, si
    /// es que Microsoft cambio algo.
    #[cfg(windows)]
    #[test]
    fn microsoft_devuelve_la_lista_de_voces_y_el_mp3_de_una_frase() {
        if std::env::var("EDGE_EN_VIVO").as_deref() != Ok("1") {
            return;
        }
        let carpeta = std::env::temp_dir().join(format!("pixpin-edge-{}", std::process::id()));
        let voces = voces(&carpeta);
        println!(
            "voces: {}, en español: {}",
            voces.len(),
            voces.iter().filter(|v| v.idioma.starts_with("es")).count()
        );
        assert!(voces.len() > 100);
        let f = archivo(
            &carpeta,
            "Hola, esto es PixPin leyendo en voz alta con Microsoft.",
            "es-ES-ElviraNeural",
            |t| vec![t.to_string()],
        )
        .unwrap();
        let b = std::fs::read(&f).unwrap();
        println!("mp3: {} bytes", b.len());
        if let Ok(salida) = std::env::var("EDGE_SALIDA") {
            let _ = std::fs::copy(&f, salida);
        }
        assert!(b.len() > 5_000);
        // Empieza como un MP3 (marco de sincronia 0xFFF…) o con etiqueta ID3.
        assert!(b[0] == 0xFF || &b[..3] == b"ID3");
        // Y Windows lo abre: suena unos segundos (Media Foundation, lo mismo
        // que lo reproduce en el lector).
        let pcm = crate::pcm::decodificar(&f).unwrap();
        let ms = crate::pcm::duracion_ms(&pcm);
        println!("dura {ms} ms");
        assert!(ms > 2_000, "{ms}");
        // La segunda vez sale de la cache, sin red: el mismo fichero.
        let otra = archivo(&carpeta, "Hola, esto es PixPin leyendo en voz alta con Microsoft.", "es-ES-ElviraNeural", |_| {
            panic!("no tenia que pedirlo otra vez")
        })
        .unwrap();
        assert_eq!(otra, f);
        let _ = std::fs::remove_dir_all(&carpeta);
    }
}
