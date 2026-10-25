//! **Pasar archivos de un aparato a otro, rapido**, en las dos formas de
//! Windows y Android:
//!
//! 1. **«Enviar a…» un aparato del grupo** (`suelto`,
//!    `pixpin_sincro::al_lienzo`): por el canal cifrado del grupo y el puerto
//!    de siempre, sin codigos. Lo que llega va **a lo que el otro tenga
//!    delante**: el chat abierto recibe cualquier cosa, un lienzo solo fotos
//!    (y si no es foto, se niega antes de que viaje un byte) y, sin nada
//!    abierto, la conversacion general («Mensajes guardados»). Es
//!    `al_frente.rs` de Windows; lo que esta delante lo dice Swift con
//!    [`Nucleo::poner_al_frente`].
//! 2. **«Enviar por Wi-Fi» con un codigo de seis cifras** (`envio`, sin
//!    grupo): quien envia abre una puerta (`_pixpinenvio._tcp`, QR
//!    `pixpin-envio:1:<codigo>:<ip>:<puerto>`) y aprueba a cada aparato que
//!    llega ([`PuertaEnvio`]); o al reves, quien recibe espera
//!    (`_pixpinrecibe._tcp`, QR `pixpin-recibe:1:…`, [`PuertaRecibir`]) y el
//!    que envia teclea su codigo. Cinco codigos malos queman la puerta. Se
//!    mandan **archivos, proyectos enteros y lienzos** (un lienzo sale como
//!    lienzo, un `.pixpin` de una hoja que el movil abre en su editor:
//!    [`crate::lienzo_suelto`]). Lo recibido va a su sitio como en Windows
//!    ([`crate::recepcion`]): un proyecto a la lista, un lienzo a su
//!    proyecto y lo demas a «Mensajes guardados»; de lo que ya se tiene (los
//!    tres codigos) se pregunta antes de aceptar **«Actualizar el que
//!    tengo» o «Crear como nuevo»** ([`DecidirOferta`]).
//!
//! Todo bloqueante y sin hilos propios: las puertas se atienden desde un
//! hilo de Swift (como [`crate::Escucha`]) y sus avisos (quien llega, cuanto
//! va) salen por [`PuertaEnvio::siguiente_aviso`], que tambien bloquea sin
//! gastar nada. Nada sondea.

use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use pixpin_proyecto::vista::DiscoPc;
use pixpin_sincro::al_lienzo::{self, Conexion, Donde, ErrorAlLienzo, Llegada, Recibe, Suelto};
use pixpin_sincro::disco::Disco;
use pixpin_sincro::envio::{self, Elemento, Emisor, Final, Receptor, Remitente};
use pixpin_sincro::mensajes::Hola;

use crate::red::{conectar, nonce};
use crate::sincro::{AnuncioBonjour, ParTxt};
use crate::{ErrorNucleo, Nucleo};

// ===================================================================
// 1. Lo que esta delante y el `suelto`
// ===================================================================

/// **Lo que el usuario tiene delante en este Mac** (el `Frente` de
/// Windows): a donde va un archivo suelto que llega de otro aparato. Gana lo
/// ultimo que se puso; Swift lo pone al abrir un chat o un lienzo y lo
/// quita (`Nada`) al cerrar la ventana.
#[derive(Debug, Clone, Default, PartialEq, Eq, uniffi::Enum)]
pub enum FrenteMac {
    /// Nada abierto: lo que llega va a «Mensajes guardados».
    #[default]
    Nada,
    /// El chat `id` (`nombre`) esta abierto: recibe cualquier cosa.
    Chat { id: String, nombre: String },
    /// Un lienzo del chat `chat` esta abierto: solo acepta fotos. La foto
    /// queda en `sincro/al-lienzo/` y Swift la mete en el lienzo
    /// ([`ArchivoLlegado::ruta`]).
    Lienzo { chat: String },
}

/// Donde quedo un archivo suelto (`al_lienzo::Donde`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum DondeLlego {
    /// En el lienzo que estaba delante.
    Lienzo,
    /// En el chat que estaba abierto.
    ChatAbierto,
    /// En la conversacion general («Mensajes guardados»).
    Chat,
}

impl From<Donde> for DondeLlego {
    fn from(d: Donde) -> Self {
        match d {
            Donde::Lienzo => DondeLlego::Lienzo,
            Donde::ChatAbierto => DondeLlego::ChatAbierto,
            Donde::Chat => DondeLlego::Chat,
        }
    }
}

/// Un archivo suelto que llego a este Mac.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ArchivoLlegado {
    pub nombre: String,
    pub donde: DondeLlego,
    /// El id del chat donde quedo (o el del lienzo).
    pub chat: String,
    /// El id del mensaje nuevo, si entro en un chat.
    pub mensaje: Option<String>,
    /// La ruta absoluta de la foto que espera a su lienzo
    /// (`sincro/al-lienzo/<ms>-<nombre>`), si fue a un lienzo.
    pub ruta: Option<String>,
}

/// Como quedo cada archivo mandado con [`Nucleo::enviar_a_aparato`].
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct EnvioSuelto {
    pub nombre: String,
    /// Donde quedo en el otro aparato; `None` si no se mando.
    pub donde: Option<DondeLlego>,
    /// El nombre del chat abierto del otro, si quedo en uno.
    pub chat: Option<String>,
    /// Por que no se mando (el otro tiene un lienzo delante y no es una
    /// foto, es un PixPin de antes, se corto...).
    pub error: Option<String>,
}

/// A donde va algo que llega, segun lo que hay delante. Pura (`decidir` de
/// Windows): un lienzo solo acepta fotos.
fn decidir(frente: &FrenteMac, es_imagen: bool) -> Result<FrenteMac, ()> {
    match frente {
        FrenteMac::Lienzo { .. } if !es_imagen => Err(()),
        otro => Ok(otro.clone()),
    }
}

/// Un nombre de fichero sin rutas, o «archivo».
fn solo_nombre(nombre: &str) -> String {
    Path::new(&nombre.replace('\\', "/"))
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .filter(|n| !n.trim().is_empty() && n != "." && n != "..")
        .unwrap_or_else(|| "archivo".into())
}

/// **Quien atiende un `suelto` en este Mac** (`AlFrente` de Windows): lo
/// pide el `Respondedor` de la [`crate::Escucha`], uno por archivo, y apunta
/// donde quedo cada uno.
pub(crate) struct RecibeAlFrente<'a> {
    nucleo: &'a Nucleo,
    /// Este aparato, para el «solo acepta fotos».
    yo: &'a str,
    llegadas: &'a std::cell::RefCell<Vec<ArchivoLlegado>>,
}

impl<'a> RecibeAlFrente<'a> {
    pub(crate) fn nuevo(
        nucleo: &'a Nucleo,
        yo: &'a str,
        llegadas: &'a std::cell::RefCell<Vec<ArchivoLlegado>>,
    ) -> Self {
        RecibeAlFrente {
            nucleo,
            yo,
            llegadas,
        }
    }

    fn al_chat(&self, chat: &str, nombre: &str, fichero: &Path) -> Result<String, ErrorNucleo> {
        let m = self.nucleo.adjuntar(
            chat.to_string(),
            fichero.display().to_string(),
            Some(nombre.to_string()),
        )?;
        Ok(m.id)
    }
}

impl Recibe for RecibeAlFrente<'_> {
    fn aceptar(&mut self, p: &Suelto) -> Result<(), String> {
        decidir(&self.nucleo.al_frente(), p.es_imagen())
            .map(|_| ())
            .map_err(|_| format!("{} {}", self.yo, al_lienzo::SOLO_FOTOS))
    }

    fn guardar(&mut self, p: &Suelto, fichero: &Path) -> Result<Llegada, String> {
        // Se decide otra vez: entre el «vale» y el ultimo trozo pudo cerrarse
        // el lienzo o cambiar el chat.
        let destino = decidir(&self.nucleo.al_frente(), p.es_imagen())
            .map_err(|_| format!("{} {}", self.yo, al_lienzo::SOLO_FOTOS))?;
        let nombre = solo_nombre(&p.nombre);
        let guardados = || -> Result<(String, String), String> {
            let g = self.nucleo.chat_guardados().map_err(|e| e.to_string())?;
            let m = self
                .al_chat(&g.id, &nombre, fichero)
                .map_err(|e| e.to_string())?;
            Ok((g.id, m))
        };
        let (llegada, archivo) = match destino {
            FrenteMac::Lienzo { chat } => {
                let carpeta = self.nucleo.ruta().join("sincro").join("al-lienzo");
                std::fs::create_dir_all(&carpeta).map_err(|e| e.to_string())?;
                let propia = carpeta.join(format!("{}-{nombre}", crate::ahora_ms()));
                std::fs::copy(fichero, &propia).map_err(|e| e.to_string())?;
                (
                    Llegada::en(Donde::Lienzo),
                    ArchivoLlegado {
                        nombre,
                        donde: DondeLlego::Lienzo,
                        chat,
                        mensaje: None,
                        ruta: Some(propia.display().to_string()),
                    },
                )
            }
            FrenteMac::Chat { id, nombre: titulo } => match self.al_chat(&id, &nombre, fichero) {
                Ok(m) => (
                    Llegada {
                        donde: Donde::ChatAbierto,
                        chat: Some(titulo),
                    },
                    ArchivoLlegado {
                        nombre,
                        donde: DondeLlego::ChatAbierto,
                        chat: id,
                        mensaje: Some(m),
                        ruta: None,
                    },
                ),
                // El chat ya no esta (se borro mientras): a la general.
                Err(ErrorNucleo::NoExiste { .. }) => {
                    let (g, m) = guardados()?;
                    (
                        Llegada::en(Donde::Chat),
                        ArchivoLlegado {
                            nombre,
                            donde: DondeLlego::Chat,
                            chat: g,
                            mensaje: Some(m),
                            ruta: None,
                        },
                    )
                }
                Err(e) => return Err(e.to_string()),
            },
            FrenteMac::Nada => {
                let (g, m) = guardados()?;
                (
                    Llegada::en(Donde::Chat),
                    ArchivoLlegado {
                        nombre,
                        donde: DondeLlego::Chat,
                        chat: g,
                        mensaje: Some(m),
                        ruta: None,
                    },
                )
            }
        };
        self.llegadas.borrow_mut().push(archivo);
        Ok(llegada)
    }
}

#[uniffi::export]
impl Nucleo {
    /// **Lo que esta delante ahora** (lo llama Swift al abrir o cerrar un
    /// chat o un lienzo): a donde va un archivo suelto que llegue.
    pub fn poner_al_frente(&self, frente: FrenteMac) {
        *self.frente.lock().unwrap_or_else(|e| e.into_inner()) = frente;
    }

    /// Lo que esta delante, como lo dejo [`Nucleo::poner_al_frente`].
    pub fn al_frente(&self) -> FrenteMac {
        self.frente
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// **«Enviar a…» un aparato del grupo**: manda `rutas` (ficheros) a lo
    /// que el aparato de `host:puerto` tenga delante, por el canal del
    /// grupo, uno detras de otro en la misma conexion. Devuelve como quedo
    /// cada uno: uno que el otro no acepta (lienzo delante y no es foto) no
    /// corta los demas; si se corta la red, los que faltan salen con error.
    pub fn enviar_a_aparato(
        &self,
        host: String,
        puerto: u16,
        rutas: Vec<String>,
    ) -> Result<Vec<EnvioSuelto>, ErrorNucleo> {
        let i = DiscoPc::nuevo(self.ruta()).identidad()?;
        let codigo = i.codigo.clone().ok_or_else(|| ErrorNucleo::Argumento {
            detalle: "este aparato no esta en un grupo".into(),
        })?;
        let flujo = conectar(&host, puerto)?;
        let hola = Hola {
            yo: i.yo.clone(),
            reloj: crate::ahora_ms(),
            puerto: self.mi_puerto(),
            ..Default::default()
        };
        let mut c = Conexion::abrir(flujo, &codigo, nonce()?, hola).map_err(|e| {
            if e.es_codigo_distinto() {
                ErrorNucleo::Sincro {
                    detalle: format!(
                        "{host} no aceptó la conexión: ¿tiene el mismo código de grupo?"
                    ),
                }
            } else {
                ErrorNucleo::Sincro {
                    detalle: e.to_string(),
                }
            }
        })?;
        let mut salida = Vec::with_capacity(rutas.len());
        let mut cortado: Option<String> = None;
        for ruta in rutas {
            let ruta = PathBuf::from(ruta);
            let nombre = solo_nombre(&ruta.to_string_lossy());
            if let Some(e) = &cortado {
                salida.push(EnvioSuelto {
                    nombre,
                    donde: None,
                    chat: None,
                    error: Some(e.clone()),
                });
                continue;
            }
            match c.mandar(&ruta, &nombre) {
                Ok(l) => salida.push(EnvioSuelto {
                    nombre,
                    donde: Some(l.donde.into()),
                    chat: l.chat,
                    error: None,
                }),
                Err(e) => {
                    // Estos dejan la conexion sana: no salio ningun trozo.
                    let sana = matches!(
                        e,
                        ErrorAlLienzo::SoloFotos
                            | ErrorAlLienzo::MovilSinSoporte
                            | ErrorAlLienzo::Remoto(_)
                    ) || matches!(&e, ErrorAlLienzo::Io(x) if x.kind() == std::io::ErrorKind::NotFound);
                    let texto = e.to_string();
                    if !sana {
                        cortado = Some(texto.clone());
                    }
                    salida.push(EnvioSuelto {
                        nombre,
                        donde: None,
                        chat: None,
                        error: Some(texto),
                    });
                }
            }
        }
        if cortado.is_none() {
            c.adios();
        }
        Ok(salida)
    }
}

// ===================================================================
// 2. El envio con codigo de seis cifras
// ===================================================================

/// Lo que se saca de un codigo de envio tecleado o de su QR.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CodigoDeEnvio {
    /// Las seis cifras.
    pub codigo: String,
    /// Donde esta el otro, si el QR lo dice (si no, se busca en Bonjour por
    /// la etiqueta: [`etiqueta_de_envio`]).
    pub host: Option<String>,
    pub puerto: u16,
    /// Es el QR de quien ESPERA para recibir: a ese se le manda.
    pub espera_recibir: bool,
}

/// Lee lo tecleado o pegado: el texto entero de un QR (`pixpin-envio:…` o
/// `pixpin-recibe:…`) o solo las seis cifras. `None` si no es ni una cosa
/// ni otra.
#[uniffi::export]
pub fn leer_codigo_de_envio(texto: String) -> Option<CodigoDeEnvio> {
    envio::leer_tecleado(&texto).map(|q| CodigoDeEnvio {
        codigo: q.codigo,
        host: q.host,
        puerto: q.puerto,
        espera_recibir: q.espera_recibir,
    })
}

/// `482 913`.
#[uniffi::export]
pub fn codigo_de_envio_legible(codigo: String) -> String {
    envio::legible(&codigo)
}

/// La etiqueta (TXT `g`) con la que se anuncia quien tiene ese codigo, para
/// encontrarle en Bonjour sin publicar el codigo. Cuesta PBKDF2: fuera del
/// hilo de la interfaz.
#[uniffi::export]
pub fn etiqueta_de_envio(codigo: String) -> String {
    envio::etiqueta(&codigo)
}

/// `_pixpinenvio._tcp`: quien envia y espera.
#[uniffi::export]
pub fn tipo_de_servicio_envio() -> String {
    envio::SERVICIO.trim_end_matches('.').to_string()
}

/// `_pixpinrecibe._tcp`: quien espera para recibir.
#[uniffi::export]
pub fn tipo_de_servicio_recibir() -> String {
    envio::SERVICIO_RECIBIR.trim_end_matches('.').to_string()
}

/// Un codigo nuevo de seis cifras, uniforme, del azar del sistema.
fn codigo_nuevo() -> Result<String, ErrorNucleo> {
    let mut c = String::with_capacity(envio::CIFRAS);
    for _ in 0..envio::CIFRAS {
        let d = crate::sincro::azar_menor_que(10)?;
        c.push(char::from(b'0' + d as u8));
    }
    Ok(c)
}

/// El anuncio de una puerta de envio: `PixPin <nombre>` del `tipo` con el
/// TXT `g` (la etiqueta del codigo) y `n`, como Windows.
fn anuncio_de_puerta(tipo: &str, codigo: &str, nombre: &str) -> AnuncioBonjour {
    let etiqueta = envio::etiqueta(codigo);
    let corto: String = nombre
        .chars()
        .take(crate::sincro::LETRAS_DEL_NOMBRE)
        .collect();
    AnuncioBonjour {
        instancia: crate::sincro::recortar_etiqueta(format!("PixPin {nombre}").trim()),
        tipo: tipo.trim_end_matches('.').to_string(),
        txt: vec![
            ParTxt {
                clave: "g".into(),
                valor: etiqueta.clone(),
            },
            ParTxt {
                clave: "n".into(),
                valor: corto,
            },
        ],
        etiqueta,
        id: String::new(),
    }
}

/// El QR de una puerta: con la IPv4 si la hay. **Una IPv6 no cabe**: el QR
/// se parte por `:` en Windows y Android y la romperia; sin host, el otro
/// busca en Bonjour por la etiqueta.
fn host_para_qr(host: Option<String>) -> String {
    host.filter(|h| crate::red::es_ipv4(h))
        .map(|h| crate::red::limpiar_host(&h).to_string())
        .unwrap_or_default()
}

/// Como acabo una conexion a una puerta de envio, o un envio a quien
/// espera.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum FinEnvio {
    /// La puerta se cerro: no llego nadie.
    Cerrada,
    /// Se mando todo a `nombre`.
    Enviado { nombre: String },
    /// `nombre` dijo que no.
    Rechazado { nombre: String },
    /// Aqui no se aprobo a `nombre`: ni se le ofrecio.
    SinAprobar { nombre: String },
    /// Alguien llamo con otro codigo (`intentos` seguidos).
    CodigoMalo { intentos: u32 },
    /// Cinco codigos malos: la puerta se cerro sola.
    Quemado,
    /// Se corto a medias.
    Fallo { detalle: String },
}

/// Lo que va contando una [`PuertaEnvio`] mientras esta abierta.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum AvisoEnvio {
    /// `nombre` llamo con el codigo bueno y **espera que se le apruebe**
    /// ([`PuertaEnvio::aprobar`]).
    Llega { llamada: u64, nombre: String },
    /// Lo que va saliendo hacia esa llamada.
    Avance {
        llamada: u64,
        hechos: u64,
        total: u64,
    },
    /// Como acabo esa llamada.
    Fin { llamada: u64, fin: FinEnvio },
    /// La puerta se cerro: no habra mas avisos.
    Cerrada,
}

/// Lo que espera la aprobacion del usuario, como mucho: luego es «no».
const ESPERA_DE_APROBACION: Duration = Duration::from_secs(5 * 60);

/// **La puerta de quien envia** («Enviar por Wi-Fi»): el codigo de seis
/// cifras, su QR y su anuncio, y cada aparato que llega con el codigo bueno
/// espera a que se le apruebe. Se atiende desde un hilo de Swift con
/// [`PuertaEnvio::atender`] y se cuenta por [`PuertaEnvio::siguiente_aviso`].
#[derive(uniffi::Object)]
pub struct PuertaEnvio {
    escucha: TcpListener,
    puerto: u16,
    codigo: String,
    yo: Remitente,
    cosas: Vec<(Elemento, PathBuf)>,
    /// La carpeta de lo empaquetado para mandar (proyectos y lienzos), que
    /// se borra al soltar la puerta.
    temporal: Option<PathBuf>,
    cerrada: AtomicBool,
    malos: AtomicU32,
    llamadas: AtomicU64,
    esperando: AtomicUsize,
    aprobar_todos: AtomicBool,
    decisiones: Mutex<HashMap<u64, mpsc::Sender<bool>>>,
    salen: Mutex<mpsc::Sender<AvisoEnvio>>,
    llegan: Mutex<mpsc::Receiver<AvisoEnvio>>,
}

/// Un archivo como se ofrece (`de_archivo` de Windows).
fn elemento_de_archivo(yo: &str, ruta: &Path) -> Result<(Elemento, PathBuf), ErrorNucleo> {
    let meta = std::fs::metadata(ruta).map_err(|_| ErrorNucleo::NoExiste {
        que: ruta.display().to_string(),
    })?;
    if !meta.is_file() {
        return Err(ErrorNucleo::Argumento {
            detalle: format!("no es un fichero: {}", ruta.display()),
        });
    }
    let nombre = solo_nombre(&ruta.to_string_lossy());
    let mime = al_lienzo::mime_de(&nombre);
    Ok((
        Elemento {
            tipo: envio::ARCHIVO.into(),
            mime: (mime != "application/octet-stream").then(|| mime.to_string()),
            identidad: format!("archivo:{yo}:{nombre}"),
            nombre,
            bytes: meta.len() as i64,
            proyecto: None,
            proyecto_nombre: None,
            creado: 0,
            uid: None,
            codigo_de_chat: None,
            aparato: None,
        },
        ruta.to_path_buf(),
    ))
}

impl Nucleo {
    fn remitente(&self) -> Result<Remitente, ErrorNucleo> {
        let i = self.identidad()?;
        Ok(Remitente {
            codigo: i.yo.codigo(),
            nombre: i.yo.nombre,
            id: i.yo.id,
        })
    }

    /// Lo que se manda de `rutas`: cada archivo tal cual y **cada lienzo
    /// como lienzo** (`.excalidraw` → un `.pixpin` de una hoja, como
    /// `EnviarActivity.deLienzo`), empaquetado en una carpeta temporal que
    /// se devuelve para borrarla despues.
    fn cosas_de(&self, rutas: &[String]) -> Result<Preparado, ErrorNucleo> {
        let id = self.identidad()?.yo.id;
        if rutas.is_empty() {
            return Err(ErrorNucleo::Argumento {
                detalle: "no hay nada que mandar".into(),
            });
        }
        let ahora = crate::ahora_ms();
        let carpeta = self.ruta().join("envio").join(format!("l-{ahora}"));
        let mut hubo_lienzo = false;
        let mut cosas = Vec::new();
        for r in rutas {
            let ruta = Path::new(r);
            if crate::lienzo_suelto::es_lienzo(ruta)
                && let Ok(c) =
                    crate::lienzo_suelto::preparar_envio(self.ruta(), ruta, &carpeta, ahora)
            {
                hubo_lienzo = true;
                cosas.push(c);
                continue;
            }
            // Un lienzo que no se entiende sale como archivo, como Windows.
            match elemento_de_archivo(&id, ruta) {
                Ok(c) => cosas.push(c),
                Err(e) => {
                    if hubo_lienzo {
                        let _ = std::fs::remove_dir_all(&carpeta);
                    }
                    return Err(e);
                }
            }
        }
        Ok((cosas, hubo_lienzo.then_some(carpeta)))
    }

    /// **Un proyecto de la lista**, empaquetado (`de_proyecto` de Windows).
    fn cosas_de_proyecto(&self, ficha: &str) -> Result<Preparado, ErrorNucleo> {
        let ahora = crate::ahora_ms();
        let carpeta = self.ruta().join("envio").join(ahora.to_string());
        match de_proyecto(self.ruta(), ficha, &carpeta, ahora) {
            Ok(c) => Ok((vec![c], Some(carpeta))),
            Err(e) => {
                let _ = std::fs::remove_dir_all(&carpeta);
                Err(e)
            }
        }
    }

    /// La puerta de envio con lo ya preparado.
    fn abrir_puerta_con(
        &self,
        (cosas, temporal): Preparado,
    ) -> Result<Arc<PuertaEnvio>, ErrorNucleo> {
        let abierta = (|| {
            let escucha = crate::red::escuchar(0)?;
            let puerto = escucha.local_addr()?.port();
            let (salen, llegan) = mpsc::channel();
            Ok::<_, ErrorNucleo>(PuertaEnvio {
                escucha,
                puerto,
                codigo: codigo_nuevo()?,
                yo: self.remitente()?,
                cosas,
                temporal: temporal.clone(),
                cerrada: AtomicBool::new(false),
                malos: AtomicU32::new(0),
                llamadas: AtomicU64::new(0),
                esperando: AtomicUsize::new(0),
                aprobar_todos: AtomicBool::new(false),
                decisiones: Mutex::new(HashMap::new()),
                salen: Mutex::new(salen),
                llegan: Mutex::new(llegan),
            })
        })();
        match abierta {
            Ok(p) => Ok(Arc::new(p)),
            Err(e) => {
                if let Some(t) = temporal {
                    let _ = std::fs::remove_dir_all(t);
                }
                Err(e)
            }
        }
    }

    /// Manda `cosas` a quien espera en `host:puerto` con su `codigo`.
    fn mandar_cosas(
        &self,
        host: &str,
        puerto: u16,
        codigo: &str,
        cosas: &[(Elemento, PathBuf)],
    ) -> Result<FinEnvio, ErrorNucleo> {
        let yo = self.remitente()?;
        let flujo = conectar(host, puerto)?;
        let emisor = Emisor::nuevo(&yo, cosas);
        let mut quien = String::new();
        let r = emisor.atender(
            flujo,
            codigo,
            nonce()?,
            true,
            |n| {
                quien = n.to_string();
                true
            },
            |_, _| {},
        );
        Ok(fin_de(r, quien))
    }
}

/// Lo preparado para mandar, y la carpeta temporal que haya que borrar.
type Preparado = (Vec<(Elemento, PathBuf)>, Option<PathBuf>);

/// Las seis cifras limpias, o el error de siempre.
fn valido_o_error(codigo: &str) -> Result<String, ErrorNucleo> {
    let codigo = envio::limpiar(codigo);
    if envio::valido(&codigo) {
        Ok(codigo)
    } else {
        Err(ErrorNucleo::Argumento {
            detalle: "el codigo de envio son seis cifras".into(),
        })
    }
}

/// Un proyecto de la lista, empaquetado en `carpeta` (`deProyecto` del
/// movil). Su sena entre envios es la de `Recepcion.identidadDe`: el origen
/// si vino de otro sitio, su id si nacio aqui; asi, si vuelve, el otro lado
/// lo reconoce.
fn de_proyecto(
    raiz: &Path,
    id: &str,
    carpeta: &Path,
    ahora: i64,
) -> Result<(Elemento, PathBuf), ErrorNucleo> {
    if pixpin_proyecto::almacen::Indice::leer(raiz)
        .buscar(id)
        .is_none()
    {
        return Err(ErrorNucleo::NoExiste {
            que: format!("el proyecto {id}"),
        });
    }
    let bytes = pixpin_proyecto::almacen::empaquetar(raiz, id, ahora)?;
    let p = pixpin_proyecto::Paquete::desde_bytes(&bytes).map_err(|e| ErrorNucleo::Disco {
        detalle: e.to_string(),
    })?;
    let pr = &p.proyecto;
    let origen = pr
        .resto
        .get("origen")
        .and_then(|v| v.as_str())
        .unwrap_or(&pr.id);
    let destino = carpeta.join(format!("{}.pixpin", envio::nombre_sano(&pr.nombre)));
    std::fs::create_dir_all(carpeta)?;
    std::fs::write(&destino, &bytes)?;
    Ok((
        Elemento {
            tipo: envio::PROYECTO.into(),
            nombre: pr.nombre.clone(),
            bytes: bytes.len() as i64,
            mime: Some("application/zip".into()),
            identidad: format!("proyecto:{origen}"),
            proyecto: None,
            proyecto_nombre: None,
            creado: pr.creado,
            uid: pr.uid.clone(),
            codigo_de_chat: None,
            aparato: pr.aparato.clone(),
        },
        destino,
    ))
}

#[uniffi::export]
impl Nucleo {
    /// **Abre la puerta para enviar** `rutas` con un codigo nuevo de seis
    /// cifras, en un puerto que da el sistema (IPv6 con IPv4 dentro). Un
    /// `.excalidraw` sale como lienzo; lo demas, como archivo.
    pub fn abrir_envio(&self, rutas: Vec<String>) -> Result<Arc<PuertaEnvio>, ErrorNucleo> {
        let preparado = self.cosas_de(&rutas)?;
        self.abrir_puerta_con(preparado)
    }

    /// **Abre la puerta para enviar un proyecto entero** de la lista (su
    /// ficha), empaquetado como `.pixpin`: al otro lado entra en su lista de
    /// proyectos, o pone al dia el mismo si ya lo tenia.
    pub fn abrir_envio_de_proyecto(&self, ficha: String) -> Result<Arc<PuertaEnvio>, ErrorNucleo> {
        let preparado = self.cosas_de_proyecto(&ficha)?;
        self.abrir_puerta_con(preparado)
    }

    /// **Manda `rutas` a quien espera para recibir** en `host:puerto` con su
    /// `codigo` (el que ensena su pantalla). Bloquea hasta terminar.
    pub fn mandar_a_quien_espera(
        &self,
        host: String,
        puerto: u16,
        codigo: String,
        rutas: Vec<String>,
    ) -> Result<FinEnvio, ErrorNucleo> {
        let codigo = valido_o_error(&codigo)?;
        let (cosas, temporal) = self.cosas_de(&rutas)?;
        let r = self.mandar_cosas(&host, puerto, &codigo, &cosas);
        if let Some(t) = temporal {
            let _ = std::fs::remove_dir_all(t);
        }
        r
    }

    /// **Manda un proyecto entero a quien espera para recibir**.
    pub fn mandar_proyecto_a_quien_espera(
        &self,
        host: String,
        puerto: u16,
        codigo: String,
        ficha: String,
    ) -> Result<FinEnvio, ErrorNucleo> {
        let codigo = valido_o_error(&codigo)?;
        let (cosas, temporal) = self.cosas_de_proyecto(&ficha)?;
        let r = self.mandar_cosas(&host, puerto, &codigo, &cosas);
        if let Some(t) = temporal {
            let _ = std::fs::remove_dir_all(t);
        }
        r
    }

    /// **Recibe de quien envia y espera** en `host:puerto` con su `codigo`:
    /// trae lo que ofrece y lo pone en su sitio (proyectos a la lista,
    /// lienzos a su proyecto, lo demas a «Mensajes guardados»). Teclear el
    /// codigo es aceptar; lo que ya se tiene se pone al dia.
    pub fn recibir_envio(
        &self,
        host: String,
        puerto: u16,
        codigo: String,
    ) -> Result<Vec<RecibidoEnvio>, ErrorNucleo> {
        Ok(self
            .recibir_de(&host, puerto, &codigo, None)?
            .unwrap_or_default())
    }

    /// Lo mismo, **preguntando antes de aceptar** (`Oferta` de
    /// `RecibirActivity` del movil): `decidir` ve lo que se ofrece y que de
    /// eso ya se tiene, y contesta que crear como nuevo (o `None`: «No,
    /// gracias»). `None` si se dijo que no.
    pub fn recibir_envio_preguntando(
        &self,
        host: String,
        puerto: u16,
        codigo: String,
        decidir: Box<dyn DecidirOferta>,
    ) -> Result<Option<Vec<RecibidoEnvio>>, ErrorNucleo> {
        self.recibir_de(&host, puerto, &codigo, Some(decidir.as_ref()))
    }

    /// **Espera para recibir** («Recibir por Wi-Fi»): un codigo nuevo de
    /// seis cifras y un puerto; quien envia teclea el codigo (o lo ve en el
    /// QR) y llama. Se atiende desde un hilo de Swift con
    /// [`PuertaRecibir::atender`]; con [`PuertaRecibir::preguntar_con`] se
    /// pregunta antes de aceptar.
    pub fn esperar_envios(self: Arc<Self>) -> Result<Arc<PuertaRecibir>, ErrorNucleo> {
        let escucha = crate::red::escuchar(0)?;
        let puerto = escucha.local_addr()?.port();
        Ok(Arc::new(PuertaRecibir {
            escucha,
            puerto,
            codigo: codigo_nuevo()?,
            nucleo: self,
            cerrada: AtomicBool::new(false),
            malos: AtomicU32::new(0),
            esperando: AtomicUsize::new(0),
            decidir: Mutex::new(None),
        }))
    }
}

impl Nucleo {
    fn recibir_de(
        &self,
        host: &str,
        puerto: u16,
        codigo: &str,
        decidir: Option<&dyn DecidirOferta>,
    ) -> Result<Option<Vec<RecibidoEnvio>>, ErrorNucleo> {
        let codigo = valido_o_error(codigo)?;
        let flujo = conectar(host, puerto)?;
        let r =
            recibir_por(self, flujo, &codigo, true, decidir).map_err(|e| ErrorNucleo::Sincro {
                detalle: e.to_string(),
            })?;
        self.vaciar_caches();
        Ok(r.map(|(_, cosas)| cosas))
    }
}

fn fin_de(r: Result<Final, envio::ErrorEnvio>, quien: String) -> FinEnvio {
    match r {
        Ok(Final::Enviado) => FinEnvio::Enviado { nombre: quien },
        Ok(Final::Rechazado) => FinEnvio::Rechazado { nombre: quien },
        Ok(Final::SinAprobar) => FinEnvio::SinAprobar { nombre: quien },
        Err(e) if e.es_codigo_distinto() => FinEnvio::CodigoMalo { intentos: 1 },
        Err(e) => FinEnvio::Fallo {
            detalle: e.to_string(),
        },
    }
}

/// Espera la siguiente llamada **de verdad**, contando a quien espera (para
/// despertarlo al cerrar). Una conexion que se cierra sin decir nada (la que
/// abre Swift para saber la direccion de un servicio de Bonjour, un
/// escaner de puertos) no cuenta: se sigue esperando.
pub(crate) fn aceptar(
    escucha: &TcpListener,
    esperando: &AtomicUsize,
    cerrada: &AtomicBool,
) -> Result<Option<TcpStream>, ErrorNucleo> {
    loop {
        if cerrada.load(Ordering::SeqCst) {
            return Ok(None);
        }
        esperando.fetch_add(1, Ordering::SeqCst);
        let r = escucha.accept();
        esperando.fetch_sub(1, Ordering::SeqCst);
        let (flujo, _) = r?;
        if cerrada.load(Ordering::SeqCst) {
            return Ok(None);
        }
        // Quien llama habla primero (el saludo del canal): sin nada en 5 s,
        // no es nadie.
        flujo.set_read_timeout(Some(Duration::from_secs(5)))?;
        if !matches!(flujo.peek(&mut [0u8; 1]), Ok(1)) {
            continue;
        }
        flujo.set_read_timeout(Some(crate::red::ESPERA_AL_LEER))?;
        flujo.set_nodelay(true)?;
        return Ok(Some(flujo));
    }
}

/// Despierta a todos los que esperan en `accept` de `puerto`.
fn despertar_a_todos(puerto: u16, esperando: &AtomicUsize) {
    for _ in 0..50 {
        let n = esperando.load(Ordering::SeqCst);
        if n == 0 {
            break;
        }
        for _ in 0..n {
            crate::red::despertar(puerto);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[uniffi::export]
impl PuertaEnvio {
    /// Las seis cifras.
    pub fn codigo(&self) -> String {
        self.codigo.clone()
    }

    pub fn puerto(&self) -> u16 {
        self.puerto
    }

    /// El texto del QR (`pixpin-envio:1:<codigo>:<ip>:<puerto>`), con la
    /// IPv4 de este Mac que da Swift; una IPv6 no cabe y va vacio.
    pub fn texto_del_qr(&self, host: Option<String>) -> String {
        envio::texto_del_qr(&self.codigo, &host_para_qr(host), self.puerto)
    }

    /// Lo que hay que anunciar en Bonjour (`_pixpinenvio._tcp`). Cuesta
    /// PBKDF2: fuera del hilo de la interfaz.
    pub fn anuncio(&self, nombre: String) -> AnuncioBonjour {
        anuncio_de_puerta(envio::SERVICIO, &self.codigo, &nombre)
    }

    /// **Atiende una llamada**: espera a que llegue alguien, le pide al
    /// usuario que lo apruebe (aviso `Llega`) y, si dice que si, le manda
    /// todo. Varios hilos pueden atender a la vez.
    pub fn atender(&self) -> Result<FinEnvio, ErrorNucleo> {
        let Some(flujo) = aceptar(&self.escucha, &self.esperando, &self.cerrada)? else {
            return Ok(FinEnvio::Cerrada);
        };
        let llamada = self.llamadas.fetch_add(1, Ordering::SeqCst) + 1;
        let emisor = Emisor::nuevo(&self.yo, &self.cosas);
        let mut quien = String::new();
        let mut ultimo = 0u64;
        let r = emisor.atender(
            flujo,
            &self.codigo,
            nonce()?,
            false,
            |nombre| {
                quien = nombre.to_string();
                self.esperar_aprobacion(llamada, nombre)
            },
            |hechos, total| {
                // Un aviso por cada 1 % (y el ultimo), no por trozo.
                let paso = (total / 100).max(1);
                if hechos == total || hechos >= ultimo + paso {
                    ultimo = hechos;
                    self.avisar(AvisoEnvio::Avance {
                        llamada,
                        hechos,
                        total,
                    });
                }
            },
        );
        let fin = match fin_de(r, quien) {
            FinEnvio::CodigoMalo { .. } => {
                let malos = self.malos.fetch_add(1, Ordering::SeqCst) + 1;
                if malos >= envio::INTENTOS {
                    self.cerrar();
                    FinEnvio::Quemado
                } else {
                    FinEnvio::CodigoMalo { intentos: malos }
                }
            }
            otro => {
                self.malos.store(0, Ordering::SeqCst);
                otro
            }
        };
        self.avisar(AvisoEnvio::Fin {
            llamada,
            fin: fin.clone(),
        });
        Ok(fin)
    }

    /// La respuesta del usuario a un aviso `Llega`.
    pub fn aprobar(&self, llamada: u64, si: bool) {
        if let Some(t) = self
            .decisiones
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&llamada)
        {
            let _ = t.send(si);
        }
    }

    /// «Aprobar a todos»: los que esperan y los que lleguen.
    pub fn aprobar_todos(&self) {
        self.aprobar_todos.store(true, Ordering::SeqCst);
        for (_, t) in self
            .decisiones
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain()
        {
            let _ = t.send(true);
        }
    }

    /// **El siguiente aviso** (bloquea hasta que haya uno, sin gastar nada).
    /// Tras `Cerrada` no hay mas.
    pub fn siguiente_aviso(&self) -> AvisoEnvio {
        self.llegan
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .recv()
            .unwrap_or(AvisoEnvio::Cerrada)
    }

    /// Cierra la puerta: el codigo deja de valer, lo que esperaba
    /// aprobacion es «no» y los hilos que esperan vuelven.
    pub fn cerrar(&self) {
        if self.cerrada.swap(true, Ordering::SeqCst) {
            return;
        }
        for (_, t) in self
            .decisiones
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain()
        {
            let _ = t.send(false);
        }
        despertar_a_todos(self.puerto, &self.esperando);
        self.avisar(AvisoEnvio::Cerrada);
    }
}

impl PuertaEnvio {
    fn avisar(&self, a: AvisoEnvio) {
        let _ = self.salen.lock().unwrap_or_else(|e| e.into_inner()).send(a);
    }

    fn esperar_aprobacion(&self, llamada: u64, nombre: &str) -> bool {
        if self.aprobar_todos.load(Ordering::SeqCst) {
            return true;
        }
        if self.cerrada.load(Ordering::SeqCst) {
            return false;
        }
        let (t, r) = mpsc::channel();
        self.decisiones
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(llamada, t);
        self.avisar(AvisoEnvio::Llega {
            llamada,
            nombre: nombre.to_string(),
        });
        let si = r.recv_timeout(ESPERA_DE_APROBACION).unwrap_or(false);
        self.decisiones
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&llamada);
        si
    }
}

impl Drop for PuertaEnvio {
    fn drop(&mut self) {
        self.cerrada.store(true, Ordering::SeqCst);
        if let Some(t) = &self.temporal {
            let _ = std::fs::remove_dir_all(t);
        }
    }
}

// ------------------------------------------------------------ recibir

/// Algo recibido con un codigo, ya en su sitio.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RecibidoEnvio {
    pub nombre: String,
    /// `proyecto` (nuevo en la lista, o un lienzo en su proyecto),
    /// `proyecto_al_dia` (encima del que ya habia, con copia antes),
    /// `guardados` («Mensajes guardados»), `guardados_al_dia` (un archivo
    /// que vuelve, puesto al dia en su burbuja) o `carpeta` (no se entendio:
    /// queda en `recibidos/`).
    pub donde: String,
    /// El id del chat donde quedo (vacio si en la carpeta).
    pub chat: String,
}

/// Una cosa de lo que se ofrece, para decidir antes de aceptar.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CosaOfrecida {
    /// Su sena entre envios: con ella se contesta «crear como nuevo».
    pub identidad: String,
    /// `archivo`, `proyecto` o `lienzo`.
    pub tipo: String,
    pub nombre: String,
    pub bytes: i64,
    /// De que proyecto es (un lienzo).
    pub proyecto_nombre: Option<String>,
    /// **Lo que aqui ya es esa cosa** (los tres codigos), por su nombre:
    /// entonces se elige «Actualizar el que tengo» o «Crear como nuevo».
    pub ya_tienes: Option<String>,
}

/// Lo que ofrece quien envia (`Oferta` de `RecibirActivity`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct OfertaRecibida {
    /// El nombre de quien envia.
    pub de: String,
    /// Sus cuatro signos (`K7Q2`).
    pub de_codigo: String,
    pub cosas: Vec<CosaOfrecida>,
}

/// **Decidir lo que se ofrece** antes de aceptar: devuelve las identidades
/// que se crean como nuevas (lo demas que ya se tenga se pone al dia), o
/// `None` para decir «No, gracias». Se llama desde el hilo que recibe, que
/// se queda esperando lo que tarde el usuario.
#[uniffi::export(callback_interface)]
pub trait DecidirOferta: Send + Sync {
    fn decidir(&self, oferta: OfertaRecibida) -> Option<Vec<String>>;
}

/// La oferta para Swift, con lo que de ella ya se tiene.
pub fn oferta_para_swift(raiz: &Path, o: &envio::Oferta) -> OfertaRecibida {
    let ya = crate::recepcion::que_sustituye(raiz, &o.elementos);
    OfertaRecibida {
        de: o.de.clone(),
        de_codigo: o.de_codigo.clone(),
        cosas: o
            .elementos
            .iter()
            .map(|e| CosaOfrecida {
                identidad: e.identidad.clone(),
                tipo: e.tipo.clone(),
                nombre: e.nombre.clone(),
                bytes: e.bytes,
                proyecto_nombre: e.proyecto_nombre.clone(),
                ya_tienes: ya.get(&e.identidad).cloned(),
            })
            .collect(),
    }
}

/// Como acabo una llamada a una [`PuertaRecibir`].
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum FinRecibir {
    Cerrada,
    /// Llego todo de `de`, ya en su sitio.
    Recibido {
        de: String,
        cosas: Vec<RecibidoEnvio>,
    },
    /// Se dijo «No, gracias» a lo que ofrecia `de`.
    Rechazado {
        de: String,
    },
    CodigoMalo {
        intentos: u32,
    },
    Quemado,
    Fallo {
        detalle: String,
    },
}

/// **La puerta de quien espera para recibir**: su codigo, su QR y su
/// anuncio (`_pixpinrecibe._tcp`).
#[derive(uniffi::Object)]
pub struct PuertaRecibir {
    escucha: TcpListener,
    puerto: u16,
    codigo: String,
    nucleo: Arc<Nucleo>,
    cerrada: AtomicBool,
    malos: AtomicU32,
    esperando: AtomicUsize,
    /// Quien decide cada oferta; sin el, se acepta todo y lo que ya se
    /// tiene se pone al dia.
    decidir: Mutex<Option<Box<dyn DecidirOferta>>>,
}

#[uniffi::export]
impl PuertaRecibir {
    /// **Pregunta antes de aceptar** cada oferta que llegue a esta puerta.
    pub fn preguntar_con(&self, decidir: Box<dyn DecidirOferta>) {
        *self.decidir.lock().unwrap_or_else(|e| e.into_inner()) = Some(decidir);
    }

    pub fn codigo(&self) -> String {
        self.codigo.clone()
    }

    pub fn puerto(&self) -> u16 {
        self.puerto
    }

    /// `pixpin-recibe:1:<codigo>:<ip>:<puerto>` (IPv4 o vacio).
    pub fn texto_del_qr(&self, host: Option<String>) -> String {
        envio::texto_del_qr_de_recepcion(&self.codigo, &host_para_qr(host), self.puerto)
    }

    /// Lo que hay que anunciar en Bonjour (`_pixpinrecibe._tcp`).
    pub fn anuncio(&self, nombre: String) -> AnuncioBonjour {
        anuncio_de_puerta(envio::SERVICIO_RECIBIR, &self.codigo, &nombre)
    }

    /// Espera a quien envia y recibe todo lo que ofrece.
    pub fn atender(&self) -> Result<FinRecibir, ErrorNucleo> {
        let Some(flujo) = aceptar(&self.escucha, &self.esperando, &self.cerrada)? else {
            return Ok(FinRecibir::Cerrada);
        };
        let decidir = self.decidir.lock().unwrap_or_else(|e| e.into_inner());
        let hecho = recibir_por(&self.nucleo, flujo, &self.codigo, false, decidir.as_deref());
        drop(decidir);
        let fin = match hecho {
            Ok(Some((de, cosas))) => {
                self.malos.store(0, Ordering::SeqCst);
                self.nucleo.vaciar_caches();
                FinRecibir::Recibido { de, cosas }
            }
            Ok(None) => {
                self.malos.store(0, Ordering::SeqCst);
                FinRecibir::Rechazado { de: String::new() }
            }
            Err(e) if e.es_codigo_distinto() => {
                let malos = self.malos.fetch_add(1, Ordering::SeqCst) + 1;
                if malos >= envio::INTENTOS {
                    self.cerrar();
                    FinRecibir::Quemado
                } else {
                    FinRecibir::CodigoMalo { intentos: malos }
                }
            }
            Err(e) => FinRecibir::Fallo {
                detalle: e.to_string(),
            },
        };
        Ok(fin)
    }

    pub fn cerrar(&self) {
        if !self.cerrada.swap(true, Ordering::SeqCst) {
            despertar_a_todos(self.puerto, &self.esperando);
        }
    }
}

/// Recibe por una conexion ya abierta (llamando o esperando) y lo pone en
/// su sitio. Con `decidir`, se pregunta antes de aceptar; `Ok(None)` si se
/// dijo que no. El temporal se borra pase lo que pase.
fn recibir_por(
    nucleo: &Nucleo,
    flujo: TcpStream,
    codigo: &str,
    inicia: bool,
    decidir: Option<&dyn DecidirOferta>,
) -> Result<Option<(String, Vec<RecibidoEnvio>)>, envio::ErrorEnvio> {
    let i = nucleo
        .identidad()
        .map_err(|e| envio::ErrorEnvio::Escritura(std::io::Error::other(e.to_string())))?;
    let azar =
        nonce().map_err(|e| envio::ErrorEnvio::Escritura(std::io::Error::other(e.to_string())))?;
    let mut r = Receptor::conectar(flujo, codigo, &i.yo.nombre, &i.yo.id, azar, inicia)?;
    // **Que de esto ya se tiene**, antes de preguntar: es lo que hace que
    // salgan «Actualizar el que tengo» y «Crear como nuevo».
    let como_nuevo: crate::recepcion::ComoNuevo = match decidir {
        None => Default::default(),
        Some(d) => match d.decidir(oferta_para_swift(nucleo.ruta(), &r.oferta)) {
            Some(nuevos) => nuevos.into_iter().map(|n| (n, true)).collect(),
            None => {
                r.rechazar();
                return Ok(None);
            }
        },
    };
    let temporal = nucleo.ruta().join("sincro").join(format!(
        "recibiendo-{}-{}",
        crate::ahora_ms(),
        std::process::id()
    ));
    let de = r.oferta.de.clone();
    let llegado = r.aceptar(&temporal, |_, _| {});
    let hecho = llegado.map(|cosas| {
        let guardados = crate::recepcion::guardar_lo_recibido(
            nucleo.ruta(),
            &i.yo.id,
            &cosas,
            &como_nuevo,
            crate::ahora_ms(),
        );
        let salida = guardados
            .into_iter()
            .map(|g| RecibidoEnvio {
                nombre: g.nombre,
                donde: g.donde.clave().into(),
                chat: g.chat,
            })
            .collect();
        Some((de, salida))
    });
    let _ = std::fs::remove_dir_all(&temporal);
    hecho
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_lienzo_delante_solo_acepta_fotos() {
        let lienzo = FrenteMac::Lienzo { chat: "c".into() };
        assert!(decidir(&lienzo, true).is_ok());
        assert!(decidir(&lienzo, false).is_err());
        let chat = FrenteMac::Chat {
            id: "c".into(),
            nombre: "Tesis".into(),
        };
        assert_eq!(decidir(&chat, false), Ok(chat.clone()));
        assert_eq!(decidir(&FrenteMac::Nada, false), Ok(FrenteMac::Nada));
    }

    #[test]
    fn los_nombres_que_llegan_no_llevan_rutas() {
        assert_eq!(solo_nombre("../../etc/passwd"), "passwd");
        assert_eq!(solo_nombre("C:\\fotos\\a.png"), "a.png");
        assert_eq!(solo_nombre(".."), "archivo");
        assert_eq!(solo_nombre(""), "archivo");
    }

    #[test]
    fn el_codigo_son_seis_cifras_y_el_qr_solo_lleva_ipv4() {
        for _ in 0..20 {
            assert!(envio::valido(&codigo_nuevo().unwrap()));
        }
        assert_eq!(host_para_qr(Some("192.168.1.4".into())), "192.168.1.4");
        assert_eq!(host_para_qr(Some("fe80::1%en0".into())), "");
        assert_eq!(host_para_qr(None), "");
        let q = leer_codigo_de_envio("pixpin-recibe:1:482913:192.168.1.4:47474".into()).unwrap();
        assert!(q.espera_recibir);
        assert_eq!(q.host.as_deref(), Some("192.168.1.4"));
        assert_eq!(
            leer_codigo_de_envio("482 913".into()).unwrap().codigo,
            "482913"
        );
        assert!(leer_codigo_de_envio("pixpin-chat:1:K7Q2M9XP::1".into()).is_none());
    }

    #[test]
    fn un_proyecto_sin_extension_la_recibe() {
        let mut e = Elemento {
            tipo: envio::PROYECTO.into(),
            nombre: "123".into(),
            bytes: 0,
            mime: None,
            identidad: "proyecto:p1".into(),
            proyecto: None,
            proyecto_nombre: None,
            creado: 0,
            uid: None,
            codigo_de_chat: None,
            aparato: None,
        };
        use crate::recepcion::{ficha_que_llega, nombre_con_extension};
        assert_eq!(nombre_con_extension(&e), "123.pixpin");
        assert_eq!(ficha_que_llega(&e).id, "p1");
        e.tipo = envio::LIENZO.into();
        assert_eq!(nombre_con_extension(&e), "123.pixpin");
        e.tipo = envio::ARCHIVO.into();
        assert_eq!(nombre_con_extension(&e), "123");
    }
}
