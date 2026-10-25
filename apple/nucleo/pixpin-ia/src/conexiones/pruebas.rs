//! **Las conexiones de punta a punta, sin red**: un servidor HTTP falso en
//! 127.0.0.1 hace de Google (pagina de permiso, servidor de tokens y API de
//! Drive) y un «navegador» de mentira vuelve al puerto local como lo haria
//! el de verdad. Por el mismo WinHTTP que en la app.

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Map, Value, json};

use super::acciones::{self as acc, Chat};
use super::drive::GOOGLE_DRIVE;
use super::guardado::{self, Cliente, Guardado, Tokens};
use super::url::{consulta, valor};
use super::*;

// ---------------------------------------------------------------------------
// El servidor falso

#[derive(Debug, Clone)]
struct Recibido {
    metodo: String,
    /// Con la consulta.
    ruta: String,
    cabeceras: HashMap<String, String>,
    cuerpo: Vec<u8>,
}

impl Recibido {
    fn camino(&self) -> &str {
        self.ruta
            .split_once('?')
            .map_or(self.ruta.as_str(), |(c, _)| c)
    }
    fn consulta(&self) -> Vec<(String, String)> {
        consulta(self.ruta.split_once('?').map_or("", |(_, q)| q))
    }
    fn formulario(&self) -> Vec<(String, String)> {
        consulta(&String::from_utf8_lossy(&self.cuerpo))
    }
    fn token(&self) -> &str {
        self.cabeceras
            .get("authorization")
            .and_then(|a| a.strip_prefix("Bearer "))
            .unwrap_or("")
    }
}

/// Lo que el «Google» falso sabe y apunta.
#[derive(Default)]
struct Mundo {
    recibidos: Vec<Recibido>,
    /// El reto PKCE que vio la pagina de permiso.
    reto: String,
    /// Los tokens de acceso que la API acepta.
    validos: Vec<String>,
    /// Cuantos tokens de acceso se han dado al refrescar.
    refrescos: usize,
}

struct Falso {
    base: String,
    mundo: Arc<Mutex<Mundo>>,
    parar: Arc<AtomicBool>,
}

impl Drop for Falso {
    fn drop(&mut self) {
        self.parar.store(true, Ordering::SeqCst);
    }
}

impl Falso {
    fn nuevo() -> Falso {
        let oyente = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://127.0.0.1:{}", oyente.local_addr().unwrap().port());
        let mundo = Arc::new(Mutex::new(Mundo {
            validos: vec!["ACC-1".into()],
            ..Default::default()
        }));
        let parar = Arc::new(AtomicBool::new(false));
        oyente.set_nonblocking(true).unwrap();
        let (m, p, b) = (mundo.clone(), parar.clone(), base.clone());
        std::thread::spawn(move || {
            while !p.load(Ordering::SeqCst) {
                match oyente.accept() {
                    Ok((flujo, _)) => {
                        let (m, b) = (m.clone(), b.clone());
                        std::thread::spawn(move || servir(flujo, &m, &b));
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(5)),
                }
            }
        });
        Falso { base, mundo, parar }
    }

    fn extremos(&self) -> Extremos {
        GOOGLE_DRIVE.extremos_en(&self.base)
    }

    fn recibidos(&self) -> Vec<Recibido> {
        self.mundo.lock().unwrap().recibidos.clone()
    }
}

fn leer_pedido(flujo: &mut TcpStream) -> Option<Recibido> {
    flujo.set_nonblocking(false).ok()?;
    flujo.set_read_timeout(Some(Duration::from_secs(10))).ok()?;
    let mut buf = Vec::new();
    let mut trozo = [0u8; 65536];
    let fin = loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        let n = flujo.read(&mut trozo).ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&trozo[..n]);
    };
    let cabeza = String::from_utf8_lossy(&buf[..fin]).into_owned();
    let mut lineas = cabeza.lines();
    let primera: Vec<&str> = lineas.next()?.split_whitespace().collect();
    let mut cabeceras = HashMap::new();
    for l in lineas {
        if let Some((k, v)) = l.split_once(':') {
            cabeceras.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }
    let largo: usize = cabeceras
        .get("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut cuerpo = buf[fin..].to_vec();
    while cuerpo.len() < largo {
        let n = flujo.read(&mut trozo).ok()?;
        if n == 0 {
            break;
        }
        cuerpo.extend_from_slice(&trozo[..n]);
    }
    Some(Recibido {
        metodo: primera[0].into(),
        ruta: primera.get(1)?.to_string(),
        cabeceras,
        cuerpo,
    })
}

const DOC: &str = "1DocDeGoogleAAAAAAAAAAAAAAAA";
const PDF: &str = "1PdfDeDriveBBBBBBBBBBBBBBBBB";
const CARPETA_OBRA: &str = "1CarpetaObraCCCCCCCCCCCCCCCC";

fn ficha(id: &str) -> Value {
    match id {
        DOC => {
            json!({"id": DOC, "name": "Informe de obra", "mimeType": "application/vnd.google-apps.document", "modifiedTime": "2026-10-12T15:04:05.000Z", "webViewLink": "https://docs.google.com/d/1"})
        }
        PDF => {
            json!({"id": PDF, "name": "Plano.pdf", "mimeType": "application/pdf", "size": "2048", "modifiedTime": "2026-10-10T08:00:00.000Z"})
        }
        _ => {
            json!({"id": CARPETA_OBRA, "name": "Obra", "mimeType": "application/vnd.google-apps.folder"})
        }
    }
}

fn servir(mut flujo: TcpStream, mundo: &Mutex<Mundo>, base: &str) {
    let Some(r) = leer_pedido(&mut flujo) else {
        return;
    };
    let (codigo, extra, cuerpo) = responder(&r, mundo, base);
    mundo.lock().unwrap().recibidos.push(r);
    let mut cabeza = format!(
        "HTTP/1.1 {codigo} X\r\nContent-Length: {}\r\nConnection: close\r\n",
        cuerpo.len()
    );
    for (k, v) in extra {
        cabeza.push_str(&format!("{k}: {v}\r\n"));
    }
    cabeza.push_str("\r\n");
    let _ = flujo.write_all(cabeza.as_bytes());
    let _ = flujo.write_all(&cuerpo);
    let _ = flujo.flush();
}

fn json_resp(codigo: u16, v: Value) -> (u16, Vec<(String, String)>, Vec<u8>) {
    (
        codigo,
        vec![("Content-Type".into(), "application/json".into())],
        v.to_string().into_bytes(),
    )
}

fn responder(
    r: &Recibido,
    mundo: &Mutex<Mundo>,
    base: &str,
) -> (u16, Vec<(String, String)>, Vec<u8>) {
    let camino = r.camino().to_string();
    if camino == "/token" {
        let f = r.formulario();
        let mut m = mundo.lock().unwrap();
        return match valor(&f, "grant_type") {
            Some("authorization_code") => {
                let verificador = valor(&f, "code_verifier").unwrap_or_default();
                if valor(&f, "code") != Some("CODIGO-1") || oauth::reto_de(verificador) != m.reto {
                    json_resp(
                        400,
                        json!({"error": "invalid_grant", "error_description": "codigo o verificador malos"}),
                    )
                } else {
                    json_resp(
                        200,
                        json!({"access_token": "ACC-1", "expires_in": 3599, "refresh_token": "REF-1", "token_type": "Bearer"}),
                    )
                }
            }
            Some("refresh_token") if valor(&f, "refresh_token") == Some("REF-1") => {
                m.refrescos += 1;
                let nuevo = format!("ACC-R{}", m.refrescos);
                m.validos.push(nuevo.clone());
                json_resp(200, json!({"access_token": nuevo, "expires_in": 3599}))
            }
            _ => json_resp(
                400,
                json!({"error": "invalid_grant", "error_description": "Token has been expired or revoked."}),
            ),
        };
    }
    if camino == "/revoke" {
        return (200, vec![], vec![]);
    }
    // La API: solo con un token valido.
    if !mundo.lock().unwrap().validos.iter().any(|t| t == r.token()) {
        return json_resp(
            401,
            json!({"error": {"code": 401, "message": "Invalid Credentials"}}),
        );
    }
    let q = r.consulta();
    match (r.metodo.as_str(), camino.as_str()) {
        ("GET", "/drive/v3/about") => {
            json_resp(200, json!({"user": {"emailAddress": "ana@example.com"}}))
        }
        // Google Calendar (18-oct): dos calendarios a la vista y uno oculto.
        ("GET", "/calendar/v3/calendars/primary") => {
            json_resp(200, json!({"id": "ana@example.com"}))
        }
        ("GET", "/calendar/v3/users/me/calendarList") => json_resp(
            200,
            json!({"items": [
                {"id": "ana@example.com", "summary": "ana@example.com", "summaryOverride": "Personal", "backgroundColor": "#0b8043"},
                {"id": "es.pe#holiday@group.v.calendar.google.com", "summary": "Feriados", "backgroundColor": "#d50000"},
                {"id": "oculto@x", "summary": "Oculto", "hidden": true}
            ]}),
        ),
        ("GET", c) if c.starts_with("/calendar/v3/calendars/") && c.ends_with("/events") => {
            let items = if c.contains("holiday") {
                json!([{"id": "f1", "summary": "Día de la Canción Criolla", "start": {"date": "2026-10-31"}, "end": {"date": "2026-11-01"}}])
            } else if c.contains("oculto") {
                json!([{"id": "no", "summary": "no deberia salir", "start": {"date": "2026-10-20"}}])
            } else {
                json!([
                    {"id": "e2", "summary": "Reunión con el asesor", "location": "Aula 12", "start": {"dateTime": "2026-10-20T16:00:00-05:00"}, "end": {"dateTime": "2026-10-20T17:00:00-05:00"}},
                    {"id": "e1", "summary": "Entrega del informe", "start": {"dateTime": "2026-10-19T09:00:00-05:00"}, "end": {"dateTime": "2026-10-19T10:00:00-05:00"}},
                    {"id": "x", "status": "cancelled", "start": {"dateTime": "2026-10-19T11:00:00-05:00"}}
                ])
            };
            json_resp(200, json!({"items": items}))
        }
        ("GET", "/drive/v3/files") => {
            let consulta = valor(&q, "q").unwrap_or_default();
            let files: Vec<Value> = if consulta.contains("folder") {
                vec![ficha(CARPETA_OBRA)]
            } else if consulta.contains("'Informe") || consulta.contains("'informe") {
                vec![ficha(DOC)]
            } else if consulta.contains("'Plano") {
                vec![ficha(PDF)]
            } else if consulta.contains("'nada") {
                vec![]
            } else {
                vec![ficha(DOC), ficha(PDF)]
            };
            json_resp(200, json!({"files": files}))
        }
        ("GET", c) if c.starts_with("/drive/v3/files/") => {
            let resto = &c["/drive/v3/files/".len()..];
            let (id, export) = match resto.strip_suffix("/export") {
                Some(id) => (id, true),
                None => (resto, false),
            };
            if ![DOC, PDF, CARPETA_OBRA].contains(&id) {
                return json_resp(
                    404,
                    json!({"error": {"code": 404, "message": "File not found"}}),
                );
            }
            if export {
                return match valor(&q, "mimeType") {
                    Some("text/plain") => (
                        200,
                        vec![],
                        "\u{feff}Informe de obra\r\nEl caudal es 0,85 m3/s."
                            .as_bytes()
                            .to_vec(),
                    ),
                    Some(m) if m.contains("wordprocessingml") => {
                        (200, vec![], b"PK-DOCX-FALSO".to_vec())
                    }
                    _ => json_resp(400, json!({"error": {"message": "bad export"}})),
                };
            }
            if valor(&q, "alt") == Some("media") {
                return (200, vec![], b"%PDF-1.4 falso".to_vec());
            }
            json_resp(200, ficha(id))
        }
        ("POST", "/upload/drive/v3/files") if valor(&q, "uploadType") == Some("multipart") => {
            json_resp(
                200,
                json!({"id": "1Subido", "name": "subido", "mimeType": "application/pdf", "webViewLink": "https://drive.google.com/file/d/1Subido"}),
            )
        }
        ("POST", "/upload/drive/v3/files") if valor(&q, "uploadType") == Some("resumable") => (
            200,
            vec![("Location".into(), format!("{base}/subida/sesion-1"))],
            vec![],
        ),
        ("PUT", "/subida/sesion-1") => json_resp(
            200,
            json!({"id": "1Grande", "name": "grande.bin", "mimeType": "application/octet-stream"}),
        ),
        _ => json_resp(404, json!({"error": {"message": "no existe"}})),
    }
}

// ---------------------------------------------------------------------------
// Ayudas

fn carpeta(nombre: &str) -> PathBuf {
    let c = std::env::temp_dir().join(format!(
        "pixpin-conexiones-pr-{nombre}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&c);
    std::fs::create_dir_all(&c).unwrap();
    c
}

const ID_CLIENTE: &str = "123456789-prueba.apps.googleusercontent.com";

fn conectado(c: &Path, acceso: &str, caduca_ms: i64, refresco: &str) {
    guardado::guardar_en(
        c,
        &GOOGLE_DRIVE,
        &Guardado {
            cliente: Some(Cliente {
                id: ID_CLIENTE.into(),
                secreto: "GOCSPX-prueba".into(),
            }),
            tokens: Some(Tokens {
                acceso: acceso.into(),
                refresco: refresco.into(),
                caduca_ms,
            }),
            cuenta: None,
        },
    )
    .unwrap();
}

/// El «navegador»: mira la URL de permiso, apunta el reto y vuelve al
/// puerto local (antes, un favicon, que se ignora).
fn navegador(
    f: &Falso,
    estado_malo: bool,
    respuesta: Arc<Mutex<String>>,
) -> impl Fn(&str) -> Result<(), String> {
    let mundo = f.mundo.clone();
    move |url: &str| {
        let q = consulta(url.split_once('?').unwrap().1);
        assert_eq!(valor(&q, "client_id"), Some(ID_CLIENTE));
        assert_eq!(valor(&q, "code_challenge_method"), Some("S256"));
        assert!(url.contains("/o/oauth2/v2/auth?"), "{url}");
        mundo.lock().unwrap().reto = valor(&q, "code_challenge").unwrap().to_string();
        let vuelta = valor(&q, "redirect_uri").unwrap().to_string();
        assert!(vuelta.starts_with("http://127.0.0.1:"), "{vuelta}");
        let estado = if estado_malo {
            "otro".to_string()
        } else {
            valor(&q, "state").unwrap().to_string()
        };
        let respuesta = respuesta.clone();
        std::thread::spawn(move || {
            let anfitrion = vuelta
                .trim_start_matches("http://")
                .trim_end_matches('/')
                .to_string();
            for ruta in [
                "/favicon.ico".to_string(),
                format!("/?state={estado}&code=CODIGO-1&scope=x"),
            ] {
                let mut s = TcpStream::connect(&anfitrion).unwrap();
                write!(s, "GET {ruta} HTTP/1.1\r\nHost: {anfitrion}\r\n\r\n").unwrap();
                let mut texto = String::new();
                let _ = s.read_to_string(&mut texto);
                *respuesta.lock().unwrap() = texto;
            }
        });
        Ok(())
    }
}

#[derive(Default)]
struct ChatFalso {
    archivos: Vec<(String, PathBuf)>,
    si: bool,
    dir: PathBuf,
    al_chat: RefCell<Vec<(PathBuf, String)>>,
    preguntas: RefCell<Vec<String>>,
    leidos: RefCell<Vec<PathBuf>>,
}

impl Chat for ChatFalso {
    fn archivo(&self, descripcion: Option<&str>) -> Result<(String, PathBuf), String> {
        match descripcion {
            None => self.archivos.last().cloned().ok_or("no hay".into()),
            Some(d) => self
                .archivos
                .iter()
                .find(|(n, _)| n.contains(d))
                .cloned()
                .ok_or("no esta".into()),
        }
    }
    fn texto_de(&self, ruta: &Path) -> Result<String, String> {
        assert!(ruta.is_file(), "se lee de un fichero de verdad");
        self.leidos.borrow_mut().push(ruta.to_path_buf());
        Ok(format!(
            "texto sacado de {} ({} bytes)",
            pixpin_nombre(ruta),
            std::fs::metadata(ruta).unwrap().len()
        ))
    }
    fn al_chat(&self, ruta: &Path, frase: &str) -> Result<(), String> {
        self.al_chat
            .borrow_mut()
            .push((ruta.to_path_buf(), frase.to_string()));
        Ok(())
    }
    fn confirmar(&self, pregunta: &str) -> bool {
        self.preguntas.borrow_mut().push(pregunta.to_string());
        self.si
    }
    fn carpeta(&self) -> PathBuf {
        self.dir.clone()
    }
}

fn pixpin_nombre(r: &Path) -> String {
    r.file_name().unwrap().to_string_lossy().into_owned()
}

fn hacer(
    f: &Falso,
    c: &Path,
    chat: &ChatFalso,
    herramienta: &str,
    args: Value,
) -> Result<String, String> {
    let a = crate::acciones::por_nombre(herramienta).unwrap();
    let args: Map<String, Value> = args.as_object().cloned().unwrap();
    let hecha = crate::acciones::validar(a, &args).map_err(|e| e.to_string())?;
    let ex = f.extremos();
    acc::hacer(&hecha, &|s| Conexion::abrir_en(s, ex.clone(), c), chat)
}

// ---------------------------------------------------------------------------
// Las pruebas

#[test]
fn conectar_de_punta_a_punta_guarda_los_tokens_cifrados_y_la_cuenta() {
    let f = Falso::nuevo();
    let c = carpeta("conectar");
    // Sin ID de cliente no se abre el navegador.
    let nunca = |_: &str| -> Result<(), String> { panic!("no deberia abrir el navegador") };
    let e = conectar_en(
        &GOOGLE_DRIVE,
        &f.extremos(),
        &c,
        &nunca,
        &AtomicBool::new(false),
        Duration::from_secs(5),
    )
    .unwrap_err();
    assert!(e.contains("ID de cliente"), "{e}");

    guardar_cliente_en(&GOOGLE_DRIVE, &c, &format!("{ID_CLIENTE} GOCSPX-prueba")).unwrap();
    let pagina = Arc::new(Mutex::new(String::new()));
    let t = std::time::Instant::now();
    let cuenta = conectar_en(
        &GOOGLE_DRIVE,
        &f.extremos(),
        &c,
        &navegador(&f, false, pagina.clone()),
        &AtomicBool::new(false),
        Duration::from_secs(10),
    )
    .unwrap();
    let ms = t.elapsed().as_millis();
    assert_eq!(cuenta, "ana@example.com");
    assert!(ms < 3000, "conectar tardo {ms} ms");
    let g = guardado::leer_de(&c, &GOOGLE_DRIVE);
    let tok = g.tokens.clone().unwrap();
    assert_eq!(
        (tok.acceso.as_str(), tok.refresco.as_str()),
        ("ACC-1", "REF-1")
    );
    assert!(tok.caduca_ms > oauth::ahora_ms() + 3_000_000);
    assert_eq!(g.cuenta.as_deref(), Some("ana@example.com"));
    // El canje llevo el verificador PKCE, el secreto y la misma vuelta.
    let canje = f
        .recibidos()
        .into_iter()
        .find(|r| r.camino() == "/token")
        .unwrap();
    let form = canje.formulario();
    assert_eq!(valor(&form, "client_secret"), Some("GOCSPX-prueba"));
    assert!(
        valor(&form, "redirect_uri")
            .unwrap()
            .starts_with("http://127.0.0.1:")
    );
    assert_eq!(valor(&form, "code_verifier").unwrap().len(), 43);
    // El navegador vio la pagina de «listo».
    std::thread::sleep(Duration::from_millis(100));
    let html = pagina.lock().unwrap().clone();
    assert!(
        html.starts_with("HTTP/1.1 200") && html.contains("Google Drive conectado"),
        "{html}"
    );
    // Ni el token ni el secreto en claro en el disco.
    let bytes = std::fs::read(guardado::fichero(&c, &GOOGLE_DRIVE)).unwrap();
    for b in [&b"ACC-1"[..], b"REF-1", b"GOCSPX"] {
        assert!(!bytes.windows(b.len()).any(|w| w == b));
    }

    // Desconectar: fuera los tokens (el ID de cliente se queda) y se
    // retira el permiso en Google con el de refresco.
    desconectar_en(&GOOGLE_DRIVE, &f.extremos(), &c).unwrap();
    let g = guardado::leer_de(&c, &GOOGLE_DRIVE);
    assert!(!g.conectado() && g.cuenta.is_none());
    assert_eq!(g.cliente.unwrap().id, ID_CLIENTE);
    let revocado = f
        .recibidos()
        .into_iter()
        .find(|r| r.camino() == "/revoke")
        .unwrap();
    assert_eq!(valor(&revocado.formulario(), "token"), Some("REF-1"));
    let e = Conexion::abrir_en(&GOOGLE_DRIVE, f.extremos(), &c)
        .err()
        .unwrap();
    assert!(e.contains("Ajustes → Asistente → Conexiones"), "{e}");
    let _ = std::fs::remove_dir_all(&c);
}

#[test]
fn una_vuelta_con_otro_state_no_conecta() {
    let f = Falso::nuevo();
    let c = carpeta("state");
    guardar_cliente_en(&GOOGLE_DRIVE, &c, ID_CLIENTE).unwrap();
    let pagina = Arc::new(Mutex::new(String::new()));
    let e = conectar_en(
        &GOOGLE_DRIVE,
        &f.extremos(),
        &c,
        &navegador(&f, true, pagina),
        &AtomicBool::new(false),
        Duration::from_secs(10),
    )
    .unwrap_err();
    assert!(e.contains("state"), "{e}");
    assert!(!guardado::leer_de(&c, &GOOGLE_DRIVE).conectado());
    assert!(
        f.recibidos().iter().all(|r| r.camino() != "/token"),
        "no se canjea nada"
    );
    // Y parar corta la espera.
    let parar = AtomicBool::new(true);
    let nada = |_: &str| Ok(());
    let e = conectar_en(
        &GOOGLE_DRIVE,
        &f.extremos(),
        &c,
        &nada,
        &parar,
        Duration::from_secs(10),
    )
    .unwrap_err();
    assert_eq!(e, "cancelado");
    let _ = std::fs::remove_dir_all(&c);
}

#[test]
fn buscar_renueva_el_token_caducado_y_da_ids() {
    let f = Falso::nuevo();
    let c = carpeta("buscar");
    conectado(&c, "ACC-VIEJO", 0, "REF-1");
    let chat = ChatFalso::default();
    let r = hacer(&f, &c, &chat, acc::BUSCAR, json!({"texto": "informe"})).unwrap();
    assert!(
        r.contains("«Informe de obra» (Documento de Google, modificado 2026-10-12)"),
        "{r}"
    );
    assert!(r.contains(DOC));
    let pedidos = f.recibidos();
    let lista = pedidos
        .iter()
        .find(|r| r.camino() == "/drive/v3/files")
        .unwrap();
    assert_eq!(lista.token(), "ACC-R1", "con el token renovado");
    let q = valor(&lista.consulta(), "q").unwrap().to_string();
    assert!(
        q.contains("name contains 'informe'") && q.contains("trashed = false"),
        "{q}"
    );
    assert!(
        valor(&lista.consulta(), "orderBy").is_none(),
        "Drive no ordena una busqueda de texto"
    );
    // El token nuevo quedo guardado.
    assert_eq!(
        guardado::leer_de(&c, &GOOGLE_DRIVE).tokens.unwrap().acceso,
        "ACC-R1"
    );
    // Sin texto: lo reciente, ordenado.
    let r = hacer(&f, &c, &chat, acc::BUSCAR, json!({})).unwrap();
    assert!(r.starts_with("2 en Google Drive"), "{r}");
    let ultima = f
        .recibidos()
        .into_iter()
        .rfind(|r| r.camino() == "/drive/v3/files")
        .unwrap();
    assert_eq!(
        valor(&ultima.consulta(), "orderBy"),
        Some("modifiedTime desc")
    );
    assert_eq!(
        f.mundo.lock().unwrap().refrescos,
        1,
        "el renovado se reutiliza"
    );
    let r = hacer(&f, &c, &chat, acc::BUSCAR, json!({"texto": "nada de nada"})).unwrap();
    assert_eq!(r, "No hay nada asi en Google Drive.");
    let _ = std::fs::remove_dir_all(&c);
}

#[test]
fn un_401_renueva_y_repite_una_vez_y_un_permiso_retirado_desconecta() {
    let f = Falso::nuevo();
    let c = carpeta("401");
    // Un token que aun no caduca pero Google ya no acepta.
    conectado(&c, "ACC-RECHAZADO", oauth::ahora_ms() + 3_600_000, "REF-1");
    let chat = ChatFalso::default();
    hacer(&f, &c, &chat, acc::BUSCAR, json!({"texto": "informe"})).unwrap();
    let api: Vec<String> = f
        .recibidos()
        .iter()
        .filter(|r| r.camino() == "/drive/v3/files")
        .map(|r| r.token().to_string())
        .collect();
    assert_eq!(api, vec!["ACC-RECHAZADO", "ACC-R1"]);

    // El de refresco revocado: se borran los tokens y se dice que hay que
    // volver a conectar.
    let c2 = carpeta("revocado");
    conectado(&c2, "ACC-VIEJO", 0, "REF-REVOCADO");
    let e = hacer(&f, &c2, &chat, acc::BUSCAR, json!({"texto": "informe"})).unwrap_err();
    assert!(
        e.contains("volver a conectar") && e.contains("Conexiones"),
        "{e}"
    );
    let g = guardado::leer_de(&c2, &GOOGLE_DRIVE);
    assert!(!g.conectado());
    assert!(g.cliente.is_some(), "el ID de cliente se queda");
    let _ = std::fs::remove_dir_all(&c);
    let _ = std::fs::remove_dir_all(&c2);
}

#[test]
fn leer_un_doc_exporta_texto_y_un_pdf_pasa_por_el_lector_de_la_app() {
    let f = Falso::nuevo();
    let c = carpeta("leer");
    conectado(&c, "ACC-1", oauth::ahora_ms() + 3_600_000, "REF-1");
    let chat = ChatFalso {
        dir: c.join("tmp"),
        ..Default::default()
    };
    // Por el nombre (sin id).
    let r = hacer(
        &f,
        &c,
        &chat,
        acc::LEER,
        json!({"archivo": "Informe de obra"}),
    )
    .unwrap();
    assert!(r.starts_with("«Informe de obra» de Google Drive ("), "{r}");
    assert!(
        r.contains("El caudal es 0,85 m3/s.") && !r.contains('\u{feff}'),
        "{r}"
    );
    let export = f
        .recibidos()
        .into_iter()
        .find(|r| r.camino().ends_with("/export"))
        .unwrap();
    assert_eq!(valor(&export.consulta(), "mimeType"), Some("text/plain"));
    assert!(chat.leidos.borrow().is_empty());
    // Por el id: el PDF se baja y lo lee el lector de la app; el temporal
    // no se queda.
    let r = hacer(&f, &c, &chat, acc::LEER, json!({"archivo": PDF})).unwrap();
    assert!(r.contains("texto sacado de Plano.pdf (14 bytes)"), "{r}");
    assert_eq!(chat.leidos.borrow().len(), 1);
    assert!(!chat.leidos.borrow()[0].exists());
    // Una carpeta no se lee; un id que no existe tampoco.
    let e = hacer(&f, &c, &chat, acc::LEER, json!({"archivo": CARPETA_OBRA})).unwrap_err();
    assert!(e.contains("es una carpeta"), "{e}");
    let e = hacer(&f, &c, &chat, acc::LEER, json!({"archivo": "nada que ver"})).unwrap_err();
    assert!(e.contains("no encontre"), "{e}");
    let _ = std::fs::remove_dir_all(&c);
}

#[test]
fn bajar_un_doc_lo_deja_en_el_chat_como_word() {
    let f = Falso::nuevo();
    let c = carpeta("bajar");
    conectado(&c, "ACC-1", oauth::ahora_ms() + 3_600_000, "REF-1");
    let chat = ChatFalso {
        dir: c.join("bajados"),
        ..Default::default()
    };
    let r = hacer(&f, &c, &chat, acc::BAJAR, json!({"archivo": DOC})).unwrap();
    assert!(
        r.contains("«Informe de obra.docx»") && r.contains("ya esta en el chat"),
        "{r}"
    );
    let al_chat = chat.al_chat.borrow();
    assert_eq!(al_chat.len(), 1);
    assert_eq!(pixpin_nombre(&al_chat[0].0), "Informe de obra.docx");
    assert_eq!(std::fs::read(&al_chat[0].0).unwrap(), b"PK-DOCX-FALSO");
    assert!(al_chat[0].1.contains("De Google Drive"));
    let _ = std::fs::remove_dir_all(&c);
}

#[test]
fn subir_pregunta_antes_y_sin_el_si_no_sale_nada() {
    let f = Falso::nuevo();
    let c = carpeta("subir");
    conectado(&c, "ACC-1", oauth::ahora_ms() + 3_600_000, "REF-1");
    let local = c.join("Memoria.pdf");
    std::fs::write(&local, b"%PDF-1.4 memoria de calculo").unwrap();
    let mut chat = ChatFalso {
        archivos: vec![("Memoria.pdf".into(), local.clone())],
        dir: c.join("tmp"),
        ..Default::default()
    };
    // Validada, la llamada ya dice que hay que preguntar.
    let hecha = crate::acciones::validar(
        crate::acciones::por_nombre(acc::SUBIR).unwrap(),
        &Map::new(),
    )
    .unwrap();
    assert!(hecha.confirmar.is_some());

    let r = hacer(&f, &c, &chat, acc::SUBIR, json!({"carpeta": "Obra"})).unwrap();
    assert!(r.contains("dijo que no"), "{r}");
    assert_eq!(
        chat.preguntas.borrow().as_slice(),
        ["¿Subo «Memoria.pdf» (1 KB) a tu Google Drive en la carpeta «Obra»?"]
    );
    assert!(
        f.recibidos()
            .iter()
            .all(|r| !r.camino().starts_with("/upload")),
        "sin el sí no se sube"
    );

    chat.si = true;
    let r = hacer(
        &f,
        &c,
        &chat,
        acc::SUBIR,
        json!({"carpeta": "Obra", "nombre": "Memoria v2.pdf"}),
    )
    .unwrap();
    assert!(
        r.contains("subido a Google Drive en la carpeta «Obra»")
            && r.contains("https://drive.google.com/file/d/1Subido"),
        "{r}"
    );
    let subida = f
        .recibidos()
        .into_iter()
        .find(|r| r.camino() == "/upload/drive/v3/files")
        .unwrap();
    assert_eq!(valor(&subida.consulta(), "uploadType"), Some("multipart"));
    let ct = subida.cabeceras.get("content-type").unwrap().clone();
    assert!(ct.starts_with("multipart/related; boundary="), "{ct}");
    let cuerpo = String::from_utf8_lossy(&subida.cuerpo).into_owned();
    assert!(
        cuerpo.contains(r#""name":"Memoria v2.pdf""#)
            && cuerpo.contains(&format!(r#""parents":["{CARPETA_OBRA}"]"#)),
        "{cuerpo}"
    );
    assert!(
        cuerpo.contains("Content-Type: application/pdf\r\n\r\n%PDF-1.4 memoria de calculo\r\n--"),
        "{cuerpo}"
    );
    let _ = std::fs::remove_dir_all(&c);
}

#[test]
fn un_archivo_grande_va_por_la_subida_reanudable() {
    let f = Falso::nuevo();
    let c = carpeta("grande");
    conectado(&c, "ACC-1", oauth::ahora_ms() + 3_600_000, "REF-1");
    let local = c.join("grande.bin");
    std::fs::write(&local, vec![7u8; 6 * 1024 * 1024]).unwrap();
    let chat = ChatFalso {
        archivos: vec![("grande.bin".into(), local)],
        si: true,
        dir: c.join("tmp"),
        ..Default::default()
    };
    let r = hacer(&f, &c, &chat, acc::SUBIR, json!({})).unwrap();
    assert!(
        r.contains("«grande.bin» subido a Google Drive (id: 1Grande)"),
        "{r}"
    );
    let rec = f.recibidos();
    let inicio = rec
        .iter()
        .find(|r| r.camino() == "/upload/drive/v3/files")
        .unwrap();
    assert_eq!(valor(&inicio.consulta(), "uploadType"), Some("resumable"));
    assert_eq!(
        inicio
            .cabeceras
            .get("x-upload-content-length")
            .map(String::as_str),
        Some("6291456")
    );
    let put = rec.iter().find(|r| r.metodo == "PUT").unwrap();
    assert_eq!(put.cuerpo.len(), 6 * 1024 * 1024);
    assert_eq!(put.token(), "ACC-1");
    let _ = std::fs::remove_dir_all(&c);
}

#[test]
fn sin_conectar_lo_dice_y_las_filas_son_solo_de_la_nube() {
    let f = Falso::nuevo();
    let c = carpeta("sin");
    let chat = ChatFalso::default();
    let e = hacer(&f, &c, &chat, acc::BUSCAR, json!({"texto": "x"})).unwrap_err();
    assert!(
        e.contains("no esta conectado") && e.contains("Conexiones"),
        "{e}"
    );
    assert!(f.recibidos().is_empty(), "sin conexion no se llama a nadie");
    for n in acc::NOMBRES {
        assert!(crate::acciones::es_solo_nube(n), "{n}");
        assert!(acc::es_de_conexiones(n));
        let a = crate::acciones::por_nombre(n).unwrap();
        assert!(
            a.claves.is_empty(),
            "el modelo local no las elige por palabras"
        );
        assert_eq!(a.accion, acc::CONEXION);
    }
    let cat: Vec<String> = crate::herramientas::catalogo()
        .into_iter()
        .map(|h| h.nombre)
        .collect();
    assert!(
        acc::NOMBRES.iter().all(|n| cat.contains(&n.to_string())),
        "estan en el catalogo de la nube"
    );
    // Un servicio que no existe no pasa la validacion.
    assert!(hacer(&f, &c, &chat, acc::BUSCAR, json!({"servicio": "dropbox"})).is_err());
    assert_eq!(por_id("Google Drive").unwrap().id, "google_drive");
    assert_eq!(por_id("drive").unwrap().id, "google_drive");
    let _ = std::fs::remove_dir_all(&c);
}

#[test]
fn google_calendar_lee_los_eventos_de_los_calendarios_a_la_vista() {
    use super::calendario::{self, GOOGLE_CALENDAR};
    let f = Falso::nuevo();
    let c = carpeta("calendario");
    // Conectado con un token caducado: se renueva solo.
    guardado::guardar_en(
        &c,
        &GOOGLE_CALENDAR,
        &Guardado {
            cliente: Some(Cliente {
                id: ID_CLIENTE.into(),
                secreto: "GOCSPX-prueba".into(),
            }),
            tokens: Some(Tokens {
                acceso: "ACC-viejo".into(),
                refresco: "REF-1".into(),
                caduca_ms: 0,
            }),
            cuenta: None,
        },
    )
    .unwrap();
    let cx =
        Conexion::abrir_en(&GOOGLE_CALENDAR, GOOGLE_CALENDAR.extremos_en(&f.base), &c).unwrap();
    assert_eq!(calendario::cuenta(&cx).unwrap(), "ana@example.com");
    let desde = calendario::ms_de_rfc3339("2026-10-18T00:00:00Z").unwrap();
    let ev = calendario::eventos(&cx, desde, desde + 21 * 86_400_000).unwrap();
    let titulos: Vec<&str> = ev.iter().map(|e| e.titulo.as_str()).collect();
    assert_eq!(
        titulos,
        [
            "Entrega del informe",
            "Reunión con el asesor",
            "Día de la Canción Criolla"
        ],
        "ordenados, sin el oculto ni el cancelado"
    );
    assert_eq!(ev[1].calendario, "Personal");
    assert_eq!(ev[1].lugar, "Aula 12");
    assert!(ev[2].todo_el_dia && ev[2].calendario == "Feriados");
    let pedido = f
        .recibidos()
        .into_iter()
        .find(|r| r.camino().contains("/events") && !r.camino().contains("holiday"))
        .unwrap();
    let q = pedido.consulta();
    assert_eq!(
        valor(&q, "singleEvents"),
        Some("true"),
        "las repeticiones, ya abiertas"
    );
    assert_eq!(valor(&q, "timeMin"), Some("2026-10-18T00:00:00Z"));
    // Y la cache de la app los guarda y los lee igual.
    assert_eq!(calendario::de_json(&calendario::a_json(&ev)), ev);
    let _ = std::fs::remove_dir_all(&c);
}

#[test]
fn el_id_de_cliente_de_google_vale_para_drive_y_calendar() {
    let c = carpeta("cliente-compartido");
    super::guardar_cliente_en(&GOOGLE_DRIVE, &c, &format!("{ID_CLIENTE} GOCSPX-x")).unwrap();
    let g = guardado::leer_de(&c, &super::calendario::GOOGLE_CALENDAR);
    assert_eq!(g.cliente.map(|x| x.id), Some(ID_CLIENTE.to_string()));
    let _ = std::fs::remove_dir_all(&c);
}
