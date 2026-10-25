//! **llama.cpp en un proceso aparte** (`llama-server`), hablado por HTTP
//! local.
//!
//! ## Por que un proceso y no una libreria enlazada
//!
//! - **Ni un byte ni un minuto de compilacion en PixPin.** Enlazar llama.cpp
//!   (crates `llama-cpp-2` y similares) pide cmake + el compilador de C++ en
//!   cada `cargo build`, y anade decenas de MB al ejecutable que todos los
//!   usuarios bajan aunque nunca usen el asistente. Asi el motor es un zip
//!   oficial que se baja con el modelo.
//! - **Memoria que se devuelve entera.** Matar el proceso tras unos minutos
//!   sin uso ([`Config::reposo`]) devuelve los ~3 GB al sistema de golpe; un
//!   modelo cargado dentro de PixPin dejaria el monton fragmentado.
//! - **Un fallo del motor no tumba PixPin**: si `llama-server` revienta (sin
//!   memoria de video, un modelo raro), el chat dice que fallo y ya.
//! - Va dentro de un **trabajo de Windows** con «matar al cerrar»: si PixPin
//!   muere de golpe, el modelo no se queda huerfano comiendo memoria. En
//!   Mac y Linux va en su **propio grupo de procesos** y se mata al soltar
//!   el motor (`Drop`); si PixPin muere de golpe (un `kill -9`) no hay un
//!   equivalente sin `unsafe` y se queda hasta que se cierre a mano (en
//!   macOS lo puede vigilar la app desde Swift). En iPad e iPhone no se
//!   pueden lanzar procesos: alli llama.cpp va enlazado (Metal) desde Swift
//!   y este modulo no se usa, pero si el protocolo ([`LectorHttp`] y la
//!   peticion de [`crate::motor`]).
//!
//! El servidor escucha solo en `127.0.0.1`, en un puerto libre al azar y con
//! una clave nueva en cada arranque (`--api-key`): ni otra maquina ni una
//! pagina web abierta en el navegador pueden usarlo.
//!
//! Lo de Windows habla con el sistema operativo: cada `unsafe` lleva su
//! `// SAFETY:`. Lo demas (argumentos, elegir dispositivo, el cliente
//! HTTP/1.1 sobre `TcpStream` hacia `127.0.0.1`) es portable.

use std::fs::File;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
#[cfg(windows)]
use std::os::windows::io::AsRawHandle;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

#[cfg(windows)]
use windows::Win32::Foundation::{CloseHandle, HANDLE};
#[cfg(windows)]
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};

use crate::ErrorIa;
use crate::motor::{Motor, Peticion, Respuesta, Seguir, Trozo};
use crate::sse::Acumulador;

/// `CREATE_NO_WINDOW`: que no asome una consola negra.
#[cfg(windows)]
const SIN_VENTANA: u32 = 0x0800_0000;

/// Lo que cada sistema necesita al lanzar el motor: en Windows, sin
/// consola; en Mac y Linux, en su propio grupo de procesos (un Ctrl+C en
/// la terminal de PixPin no le llega a medias y se mata entero al soltarlo).
fn en_segundo_plano(orden: &mut Command) -> &mut Command {
    #[cfg(windows)]
    orden.creation_flags(SIN_VENTANA);
    #[cfg(unix)]
    orden.process_group(0);
    orden
}

/// Los tokens de contexto con los que se arranca el modelo. El agente los
/// usa para no pedir mas respuesta de la que cabe (`agente::tope_de_tokens`).
pub const CONTEXTO: u32 = 8192;

/// El `--cache-reuse` de fabrica (ver [`Config::reutilizar`]). Medido el
/// 14-oct en el banco con el 2B en la MX250, 13 ordenes por el modelo: la
/// mediana del total bajo de 11,4 a 10,0 s y la del primer trozo de 7,1 a
/// 6,7 s (y el banco paso de 12/13 a 13/13). Poco, porque las herramientas
/// de cada turno van delante del sistema en la plantilla de Qwen y cambian;
/// pero no cuesta nada.
pub const REUTILIZAR: Option<u32> = Some(256);

/// Donde corre el modelo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dispositivo {
    /// Solo el procesador.
    Cpu,
    /// Todo en esta tarjeta (`Vulkan1`...), con el proyector de vision en la
    /// CPU: en la MX250 de 2 GB no cabian los dos (medido).
    Gpu(String),
}

#[derive(Debug, Clone)]
pub struct Config {
    pub servidor: PathBuf,
    pub modelo: PathBuf,
    pub vision: Option<PathBuf>,
    /// Tokens de contexto. 8192 cabe de sobra una conversacion corta con una
    /// imagen y el catalogo de herramientas.
    pub contexto: u32,
    /// Hilos de CPU. `None`: los nucleos fisicos (llama.cpp lo decide).
    pub hilos: Option<u32>,
    pub dispositivo: Dispositivo,
    /// Tras cuanto sin usarlo se cierra el modelo para devolver la memoria.
    pub reposo: Duration,
    /// Cuanto se espera a que cargue.
    pub arranque_max: Duration,
    /// Donde se escribe lo que dice el servidor (para diagnosticar).
    pub registro: Option<PathBuf>,
    /// `--cache-reuse N`: reutilizar trozos de N tokens de la peticion
    /// anterior aunque algo de delante haya cambiado (las herramientas de
    /// cada turno). `None`: solo el prefijo identico (`cache_prompt`).
    pub reutilizar: Option<u32>,
}

impl Config {
    pub fn nueva(servidor: PathBuf, modelo: PathBuf, vision: Option<PathBuf>) -> Config {
        Config {
            servidor,
            modelo,
            vision,
            contexto: CONTEXTO,
            hilos: None,
            dispositivo: Dispositivo::Cpu,
            reposo: Duration::from_secs(5 * 60),
            arranque_max: Duration::from_secs(120),
            registro: Some(std::env::temp_dir().join("pixpin-ia-servidor.log")),
            reutilizar: REUTILIZAR,
        }
    }

    /// Los argumentos de `llama-server`.
    pub fn argumentos(&self, puerto: u16, clave: &str) -> Vec<String> {
        let mut a: Vec<String> = vec![
            "-m".into(),
            self.modelo.display().to_string(),
            "--host".into(),
            "127.0.0.1".into(),
            "--port".into(),
            puerto.to_string(),
            "--api-key".into(),
            clave.into(),
            "-c".into(),
            self.contexto.to_string(),
            // La plantilla de chat del propio GGUF: es la que sabe escribir
            // y leer las llamadas a herramientas de Qwen.
            "--jinja".into(),
            "-np".into(),
            "1".into(),
            "--no-ui".into(),
        ];
        if let Some(v) = &self.vision {
            a.extend(["--mmproj".into(), v.display().to_string()]);
        }
        if let Some(n) = self.reutilizar {
            a.extend(["--cache-reuse".into(), n.to_string()]);
        }
        if let Some(h) = self.hilos {
            a.extend(["-t".into(), h.to_string()]);
        }
        match &self.dispositivo {
            Dispositivo::Cpu => a.extend(["-dev".into(), "none".into(), "-ngl".into(), "0".into()]),
            Dispositivo::Gpu(d) => a.extend([
                "-dev".into(),
                d.clone(),
                "-ngl".into(),
                "99".into(),
                "--no-mmproj-offload".into(),
            ]),
        }
        a
    }
}

/// **Elige donde correr** a partir de `llama-server --list-devices`.
///
/// Una tarjeta dedicada en la que quepa el modelo con holgura, si la hay; si
/// no, la CPU. Las graficas integradas se descartan: en este PC la Intel UHD
/// iba **mas lenta que la CPU** (10 frente a 15-25 tokens/s, medido).
pub fn elegir_dispositivo(listado: &str, bytes_modelo: u64) -> Dispositivo {
    let mut mejor: Option<(String, u64)> = None;
    for linea in listado.lines() {
        // «  Vulkan1: NVIDIA GeForce MX250 (2182 MiB, 1887 MiB free)»
        let Some((nombre, resto)) = linea.trim().split_once(": ") else {
            continue;
        };
        if nombre.contains(' ') || !nombre.chars().any(|c| c.is_ascii_digit()) {
            continue;
        }
        let integrada = [
            "Intel(R) UHD",
            "Intel(R) HD",
            "Iris",
            "Radeon(TM) Graphics",
            "Radeon Graphics",
            "Adreno",
            "Microsoft Basic",
        ]
        .iter()
        .any(|p| resto.contains(p));
        if integrada {
            continue;
        }
        let libre = resto
            .rsplit_once(", ")
            .and_then(|(_, l)| l.split_whitespace().next())
            .and_then(|n| n.parse::<u64>().ok())
            .unwrap_or(0)
            * 1024
            * 1024;
        // El modelo, su cache y 300 MB de margen para lo que pinta Windows.
        if libre >= bytes_modelo + bytes_modelo / 4 + 300 * 1024 * 1024
            && mejor.as_ref().is_none_or(|(_, m)| libre > *m)
        {
            mejor = Some((nombre.to_string(), libre));
        }
    }
    mejor.map_or(Dispositivo::Cpu, |(n, _)| Dispositivo::Gpu(n))
}

/// Pregunta al servidor que tarjetas ve (tarda ~1 s).
pub fn listar_dispositivos(servidor: &Path) -> String {
    en_segundo_plano(&mut Command::new(servidor))
        .arg("--list-devices")
        .stdin(Stdio::null())
        .output()
        .map(|o| {
            format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            )
        })
        .unwrap_or_default()
}

/// Un trabajo de Windows que mata a su proceso al cerrarse.
#[cfg(windows)]
struct Trabajo(HANDLE);

/// Fuera de Windows no hay trabajos: el proceso va en su grupo y lo mata el
/// `Drop` de [`Proceso`].
#[cfg(not(windows))]
struct Trabajo;

#[cfg(not(windows))]
impl Trabajo {
    fn para(_hijo: &Child) -> Option<Trabajo> {
        None
    }
}

// SAFETY: un HANDLE de trabajo se puede usar y cerrar desde cualquier hilo.
#[cfg(windows)]
unsafe impl Send for Trabajo {}

#[cfg(windows)]
impl Trabajo {
    fn para(hijo: &Child) -> Option<Trabajo> {
        // SAFETY: se crea un trabajo anonimo; la estructura de limites vive en
        // la pila durante la llamada y su tamano es el que se pasa; el asa del
        // hijo es valida mientras exista `hijo`, que se presta aqui.
        unsafe {
            let job = CreateJobObjectW(None, None).ok()?;
            let trabajo = Trabajo(job);
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
            .ok()?;
            AssignProcessToJobObject(job, HANDLE(hijo.as_raw_handle())).ok()?;
            Some(trabajo)
        }
    }
}

#[cfg(windows)]
impl Drop for Trabajo {
    fn drop(&mut self) {
        // SAFETY: el asa la creo `CreateJobObjectW`, es de este `Trabajo` y no
        // se cerro antes.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

struct Proceso {
    hijo: Child,
    puerto: u16,
    clave: String,
    _trabajo: Option<Trabajo>,
    pub cargado_en: Duration,
}

impl Drop for Proceso {
    fn drop(&mut self) {
        let _ = self.hijo.kill();
        let _ = self.hijo.wait();
    }
}

struct Estado {
    proceso: Option<Proceso>,
    ultimo_uso: Instant,
    ocupado: bool,
    vigia: bool,
}

/// **El motor llama.cpp.** Carga el modelo en la primera peticion y lo
/// suelta tras [`Config::reposo`] sin uso.
pub struct MotorLlama {
    config: Config,
    estado: Arc<Mutex<Estado>>,
    /// Una peticion a la vez (el servidor va con una sola ranura).
    turno: Mutex<()>,
}

impl MotorLlama {
    pub fn nuevo(config: Config) -> MotorLlama {
        MotorLlama {
            config,
            estado: Arc::new(Mutex::new(Estado {
                proceso: None,
                ultimo_uso: Instant::now(),
                ocupado: false,
                vigia: false,
            })),
            turno: Mutex::new(()),
        }
    }

    /// ¿Esta el modelo en memoria ahora mismo?
    pub fn cargado(&self) -> bool {
        self.estado.lock().is_ok_and(|e| e.proceso.is_some())
    }

    /// Cuanto tardo en cargar la ultima vez.
    pub fn tiempo_de_carga(&self) -> Option<Duration> {
        self.estado
            .lock()
            .ok()?
            .proceso
            .as_ref()
            .map(|p| p.cargado_en)
    }

    /// El identificador del proceso (para medir su memoria en las pruebas).
    pub fn pid(&self) -> Option<u32> {
        self.estado
            .lock()
            .ok()?
            .proceso
            .as_ref()
            .map(|p| p.hijo.id())
    }

    /// Carga el modelo ya, sin esperar a la primera pregunta (el chat lo
    /// llama al abrir el asistente para que la primera respuesta no tarde).
    pub fn precargar(&self) -> Result<(u16, String), ErrorIa> {
        let mut e = self
            .estado
            .lock()
            .map_err(|_| ErrorIa::Arranque("candado envenenado".into()))?;
        let vivo = match e.proceso.as_mut() {
            Some(p) => matches!(p.hijo.try_wait(), Ok(None)),
            None => false,
        };
        if !vivo {
            e.proceso = None;
            e.proceso = Some(arrancar(&self.config)?);
        }
        e.ultimo_uso = Instant::now();
        if !e.vigia {
            e.vigia = true;
            vigilar(Arc::downgrade(&self.estado), self.config.reposo);
        }
        let p = e.proceso.as_ref().expect("recien puesto");
        Ok((p.puerto, p.clave.clone()))
    }

    fn marcar(&self, ocupado: bool) {
        if let Ok(mut e) = self.estado.lock() {
            e.ocupado = ocupado;
            e.ultimo_uso = Instant::now();
        }
    }
}

impl Drop for MotorLlama {
    fn drop(&mut self) {
        if let Ok(mut e) = self.estado.lock() {
            e.proceso = None;
        }
    }
}

impl Motor for MotorLlama {
    fn es_local(&self) -> bool {
        true
    }

    fn precalentar(&self) {
        if let Err(e) = self.precargar() {
            tracing::warn!(%e, "asistente: no se pudo precargar el modelo");
        }
    }

    fn completar(
        &self,
        peticion: &Peticion,
        al_trozo: &mut dyn FnMut(&Trozo) -> Seguir,
    ) -> Result<Respuesta, ErrorIa> {
        let _turno = self
            .turno
            .lock()
            .map_err(|_| ErrorIa::Conexion("candado envenenado".into()))?;
        let (puerto, clave) = self.precargar()?;
        self.marcar(true);
        let r = pedir(puerto, &clave, peticion, al_trozo);
        self.marcar(false);
        r
    }

    fn soltar(&self) {
        if let Ok(mut e) = self.estado.lock() {
            e.proceso = None;
        }
    }
}

/// El hilo que cierra el modelo tras el reposo. Muere solo cuando el motor
/// desaparece o el modelo se cierra.
fn vigilar(estado: Weak<Mutex<Estado>>, reposo: Duration) {
    let paso = (reposo / 4).clamp(Duration::from_millis(200), Duration::from_secs(15));
    let _ = std::thread::Builder::new()
        .name("pixpin-ia-reposo".into())
        .spawn(move || {
            loop {
                std::thread::sleep(paso);
                let Some(estado) = estado.upgrade() else {
                    return;
                };
                let Ok(mut e) = estado.lock() else { return };
                if e.proceso.is_none() {
                    e.vigia = false;
                    return;
                }
                if !e.ocupado && e.ultimo_uso.elapsed() >= reposo {
                    tracing::info!(
                        "asistente: sin uso, se cierra el modelo para soltar la memoria"
                    );
                    e.proceso = None;
                    e.vigia = false;
                    return;
                }
            }
        });
}

fn puerto_libre() -> Result<u16, ErrorIa> {
    let l = TcpListener::bind("127.0.0.1:0")?;
    Ok(l.local_addr()?.port())
}

/// Una clave de un solo uso, de la semilla al azar que trae la biblioteca
/// estandar (`RandomState`).
fn clave_nueva() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut s = String::new();
    for i in 0..2u64 {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u64(i);
        h.write_u128(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
        );
        h.write_u32(std::process::id());
        s.push_str(&format!("{:016x}", h.finish()));
    }
    s
}

fn arrancar(c: &Config) -> Result<Proceso, ErrorIa> {
    for (que, ruta) in [
        ("el motor", Some(&c.servidor)),
        ("el modelo", Some(&c.modelo)),
        ("la vision", c.vision.as_ref()),
    ] {
        if let Some(r) = ruta
            && !r.is_file()
        {
            return Err(ErrorIa::Falta(format!("{que} ({})", r.display())));
        }
    }
    let puerto = puerto_libre()?;
    let clave = clave_nueva();
    let mut orden = Command::new(&c.servidor);
    en_segundo_plano(&mut orden)
        .args(c.argumentos(puerto, &clave))
        .current_dir(c.servidor.parent().unwrap_or(Path::new(".")))
        .stdin(Stdio::null())
        .stdout(Stdio::null());
    match c.registro.as_ref().and_then(|r| File::create(r).ok()) {
        Some(f) => orden.stderr(Stdio::from(f)),
        None => orden.stderr(Stdio::null()),
    };
    let t0 = Instant::now();
    let hijo = orden
        .spawn()
        .map_err(|e| ErrorIa::Arranque(e.to_string()))?;
    let trabajo = Trabajo::para(&hijo);
    let mut p = Proceso {
        hijo,
        puerto,
        clave,
        _trabajo: trabajo,
        cargado_en: Duration::ZERO,
    };
    loop {
        if let Ok(Some(salida)) = p.hijo.try_wait() {
            return Err(ErrorIa::Arranque(format!(
                "el motor se cerro ({salida}){}",
                cola_del_registro(c)
            )));
        }
        if salud(puerto) {
            p.cargado_en = t0.elapsed();
            tracing::info!("asistente: modelo cargado en {:?}", p.cargado_en);
            return Ok(p);
        }
        if t0.elapsed() > c.arranque_max {
            return Err(ErrorIa::Arranque(format!(
                "no cargo en {:?}",
                c.arranque_max
            )));
        }
        std::thread::sleep(Duration::from_millis(150));
    }
}

/// Las ultimas lineas del registro, para el mensaje de error.
fn cola_del_registro(c: &Config) -> String {
    let Some(texto) = c
        .registro
        .as_ref()
        .and_then(|r| std::fs::read_to_string(r).ok())
    else {
        return String::new();
    };
    let lineas: Vec<&str> = texto.lines().filter(|l| !l.trim().is_empty()).collect();
    let cola = lineas[lineas.len().saturating_sub(3)..].join(" | ");
    format!(": {cola}")
}

/// `GET /health` contesta 200 cuando el modelo ya esta cargado (503 mientras
/// carga). Publica: en Mac tambien sirve para ver si el usuario ya tiene un
/// `llama-server` suyo escuchando en `127.0.0.1` (`pixpin-nucleo`).
pub fn salud(puerto: u16) -> bool {
    let Ok(mut s) =
        TcpStream::connect_timeout(&([127, 0, 0, 1], puerto).into(), Duration::from_millis(300))
    else {
        return false;
    };
    let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
    if s.write_all(b"GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .is_err()
    {
        return false;
    }
    let mut buf = [0u8; 64];
    let n = s.read(&mut buf).unwrap_or(0);
    estado_http(&buf[..n]) == Some(200)
}

fn estado_http(cabecera: &[u8]) -> Option<u16> {
    let t = std::str::from_utf8(cabecera.get(..cabecera.len().min(32))?).ok()?;
    t.strip_prefix("HTTP/1.")?
        .get(2..)?
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}

/// La peticion de chat, en vivo. Publica para hablar con un `llama-server`
/// que no lanzo PixPin (el del usuario en Mac, sin `--api-key`: entonces la
/// clave da igual).
pub fn pedir(
    puerto: u16,
    clave: &str,
    peticion: &Peticion,
    al_trozo: &mut dyn FnMut(&Trozo) -> Seguir,
) -> Result<Respuesta, ErrorIa> {
    let cuerpo = peticion.a_json(true).to_string();
    let mut s =
        TcpStream::connect(("127.0.0.1", puerto)).map_err(|e| ErrorIa::Conexion(e.to_string()))?;
    // Leer una imagen grande en CPU puede tardar; mas de 5 minutos sin un
    // byte es que algo se colgo.
    s.set_read_timeout(Some(Duration::from_secs(300)))?;
    let cabecera = format!(
        "POST /v1/chat/completions HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {clave}\r\n\
         Content-Type: application/json\r\nAccept: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        cuerpo.len()
    );
    s.write_all(cabecera.as_bytes())
        .map_err(|e| ErrorIa::Conexion(e.to_string()))?;
    s.write_all(cuerpo.as_bytes())
        .map_err(|e| ErrorIa::Conexion(e.to_string()))?;

    let mut lector = LectorHttp::nuevo();
    let mut acumulador = Acumulador::nuevo();
    let mut buf = vec![0u8; 16 * 1024];
    loop {
        let n = s
            .read(&mut buf)
            .map_err(|e| ErrorIa::Conexion(e.to_string()))?;
        if n == 0 {
            break;
        }
        let cuerpo = lector.meter(&buf[..n])?;
        if cuerpo.is_empty() {
            continue;
        }
        if let Some(estado) = lector.estado
            && estado != 200
        {
            // Juntar el cuerpo del error para el mensaje.
            let mut resto = cuerpo;
            while let Ok(n) = s.read(&mut buf) {
                if n == 0 {
                    break;
                }
                resto.extend(lector.meter(&buf[..n])?);
            }
            return Err(ErrorIa::Protocolo(format!(
                "HTTP {estado}: {}",
                String::from_utf8_lossy(&resto)
                    .chars()
                    .take(300)
                    .collect::<String>()
            )));
        }
        for t in acumulador.meter(&cuerpo)? {
            if al_trozo(&t) == Seguir::Parar {
                // Cerrar el enchufe hace que llama-server deje de generar.
                drop(s);
                return Ok(acumulador.respuesta(true));
            }
        }
        if acumulador.acabado() || lector.terminado {
            break;
        }
    }
    Ok(acumulador.respuesta(false))
}

/// La respuesta HTTP/1.1 por partes: la cabecera y luego el cuerpo, que
/// `llama-server` manda troceado (`Transfer-Encoding: chunked`).
pub struct LectorHttp {
    pendiente: Vec<u8>,
    pub estado: Option<u16>,
    /// La cabecera de la respuesta tal cual (sin la linea en blanco final).
    pub cabeceras: String,
    troceado: bool,
    en_cuerpo: bool,
    /// Bytes que faltan del trozo actual (troceado).
    falta: usize,
    /// Falta el CRLF que cierra el trozo.
    cola: bool,
    pub terminado: bool,
}

impl LectorHttp {
    pub fn nuevo() -> LectorHttp {
        LectorHttp {
            pendiente: Vec::new(),
            estado: None,
            cabeceras: String::new(),
            troceado: false,
            en_cuerpo: false,
            falta: 0,
            cola: false,
            terminado: false,
        }
    }

    /// Mete bytes y devuelve los del cuerpo que ya se pueden leer.
    pub fn meter(&mut self, bytes: &[u8]) -> Result<Vec<u8>, ErrorIa> {
        self.pendiente.extend_from_slice(bytes);
        if !self.en_cuerpo {
            let Some(fin) = self.pendiente.windows(4).position(|v| v == b"\r\n\r\n") else {
                if self.pendiente.len() > 64 * 1024 {
                    return Err(ErrorIa::Protocolo("cabecera HTTP interminable".into()));
                }
                return Ok(Vec::new());
            };
            let cabecera = String::from_utf8_lossy(&self.pendiente[..fin]).to_string();
            self.pendiente.drain(..fin + 4);
            self.estado = estado_http(cabecera.as_bytes());
            if self.estado.is_none() {
                return Err(ErrorIa::Protocolo("respuesta que no es HTTP".into()));
            }
            self.troceado = cabecera.lines().any(|l| {
                l.to_ascii_lowercase().starts_with("transfer-encoding:")
                    && l.to_ascii_lowercase().contains("chunked")
            });
            self.en_cuerpo = true;
            self.cabeceras = cabecera;
        }
        if !self.troceado {
            return Ok(std::mem::take(&mut self.pendiente));
        }
        let mut salida = Vec::new();
        loop {
            if self.terminado {
                self.pendiente.clear();
                break;
            }
            if self.cola {
                // Tras los datos de cada trozo va un «\r\n».
                if self.pendiente.len() < 2 {
                    break;
                }
                self.pendiente.drain(..2);
                self.cola = false;
            }
            if self.falta > 0 {
                let n = self.falta.min(self.pendiente.len());
                salida.extend(self.pendiente.drain(..n));
                self.falta -= n;
                if self.falta > 0 {
                    break;
                }
                self.cola = true;
                continue;
            }
            let Some(fin) = self.pendiente.windows(2).position(|v| v == b"\r\n") else {
                break;
            };
            let linea = String::from_utf8_lossy(&self.pendiente[..fin]).to_string();
            let tam = usize::from_str_radix(linea.split(';').next().unwrap_or("").trim(), 16)
                .map_err(|_| ErrorIa::Protocolo(format!("trozo HTTP ilegible: {linea:.40}")))?;
            self.pendiente.drain(..fin + 2);
            if tam == 0 {
                self.terminado = true;
                continue;
            }
            self.falta = tam;
        }
        Ok(salida)
    }
}

impl LectorHttp {
    /// El valor de una cabecera de la respuesta (sin distinguir mayusculas).
    pub fn cabecera(&self, nombre: &str) -> Option<&str> {
        self.cabeceras.lines().skip(1).find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.trim().eq_ignore_ascii_case(nombre).then(|| v.trim())
        })
    }
}

impl Default for LectorHttp {
    fn default() -> Self {
        LectorHttp::nuevo()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const LISTADO: &str = "load_backend: loaded Vulkan backend from C:\\x\\ggml-vulkan.dll\n\
        Available devices:\n  Vulkan0: Intel(R) UHD Graphics (8237 MiB, 7448 MiB free)\n  Vulkan1: NVIDIA GeForce MX250 (2182 MiB, 1887 MiB free)\n";

    #[test]
    fn elige_la_tarjeta_dedicada_si_cabe() {
        // El 0.8B (533 MB) cabe en la MX250.
        assert_eq!(
            elegir_dispositivo(LISTADO, 532_517_120),
            Dispositivo::Gpu("Vulkan1".into())
        );
        // El 2B (1,28 GB) tambien, por poco.
        assert_eq!(
            elegir_dispositivo(LISTADO, 1_280_835_840),
            Dispositivo::Gpu("Vulkan1".into())
        );
        // Uno de 3 GB no: a la CPU, nunca a la integrada.
        assert_eq!(elegir_dispositivo(LISTADO, 3_106_738_272), Dispositivo::Cpu);
    }

    #[test]
    fn sin_tarjetas_va_a_la_cpu() {
        assert_eq!(
            elegir_dispositivo("Available devices:\n  (none)\n", 1),
            Dispositivo::Cpu
        );
        assert_eq!(elegir_dispositivo("", 1), Dispositivo::Cpu);
        assert_eq!(
            elegir_dispositivo("basura: sin numeros", 1),
            Dispositivo::Cpu
        );
    }

    #[test]
    fn los_argumentos_llevan_lo_necesario() {
        let mut c = Config::nueva("s.exe".into(), "m.gguf".into(), Some("v.gguf".into()));
        let a = c.argumentos(1234, "k").join(" ");
        assert!(a.contains("--host 127.0.0.1 --port 1234 --api-key k"));
        assert!(a.contains("--jinja"));
        assert!(a.contains("--mmproj v.gguf"));
        assert!(a.contains("-dev none -ngl 0"));
        c.dispositivo = Dispositivo::Gpu("Vulkan1".into());
        let a = c.argumentos(1, "k").join(" ");
        assert!(a.contains("-dev Vulkan1 -ngl 99 --no-mmproj-offload"));
        assert!(a.contains("--cache-reuse 256"), "{a}");
        c.reutilizar = None;
        assert!(!c.argumentos(1, "k").join(" ").contains("cache-reuse"));
    }

    #[test]
    fn el_cuerpo_troceado_se_junta_aunque_llegue_a_pedazos() {
        let resp = b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhola \r\n6;x=1\r\nmundo!\r\n0\r\n\r\n";
        for paso in [1, 2, 3, 7, 1000] {
            let mut l = LectorHttp::nuevo();
            let mut cuerpo = Vec::new();
            for c in resp.chunks(paso) {
                cuerpo.extend(l.meter(c).unwrap());
            }
            assert_eq!(
                String::from_utf8(cuerpo).unwrap(),
                "hola mundo!",
                "paso {paso}"
            );
            assert_eq!(l.estado, Some(200));
            assert!(l.terminado);
        }
    }

    #[test]
    fn el_cuerpo_sin_trocear_pasa_tal_cual() {
        let mut l = LectorHttp::nuevo();
        let c = l
            .meter(b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 3\r\n\r\nno!")
            .unwrap();
        assert_eq!(c, b"no!");
        assert_eq!(l.estado, Some(401));
        assert_eq!(l.cabecera("content-length"), Some("3"));
        assert_eq!(l.cabecera("retry-after"), None);
    }

    #[test]
    fn lo_que_no_es_http_es_error() {
        assert!(
            LectorHttp::nuevo()
                .meter(b"SSH-2.0-OpenSSH\r\n\r\n")
                .is_err()
        );
        let mut l = LectorHttp::nuevo();
        assert!(
            l.meter(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nzz\r\n")
                .is_err()
        );
    }

    #[test]
    fn falta_el_motor_se_dice_claro() {
        let m = MotorLlama::nuevo(Config::nueva(
            "C:\\no\\existe.exe".into(),
            "m.gguf".into(),
            None,
        ));
        let e = m
            .completar(&Peticion::nueva(vec![]), &mut |_| Seguir::Si)
            .unwrap_err();
        assert!(matches!(e, ErrorIa::Falta(_)), "{e}");
        assert!(!m.cargado());
    }

    #[test]
    fn las_claves_no_se_repiten() {
        let a = clave_nueva();
        assert_eq!(a.len(), 32);
        assert_ne!(a, clave_nueva());
    }
}
