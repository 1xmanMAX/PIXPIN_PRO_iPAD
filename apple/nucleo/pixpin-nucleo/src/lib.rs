//! **El puente entre el nucleo de Rust y la app de Apple** (fase 1 del plan,
//! `docs/investigacion/2026-10-15-mac-ipad-iphone.md`).
//!
//! Una API pequena, estable y de grano grueso sobre los crates de
//! `apple/nucleo/`, expuesta a Swift con UniFFI (macros, sin `.udl`). No
//! inventa nada: el formato, el cuaderno y la sincronizacion son los mismos
//! crates que usa Windows, para que un Mac sea un aparato mas del grupo.
//!
//! # La regla de rendimiento (§4bis del plan)
//!
//! **Cruzar el puente pocas veces y con mucho dentro.** Cada llamada de Swift
//! a Rust copia sus argumentos y su resultado (UniFFI los serializa en un
//! `RustBuffer`), asi que:
//!
//! - Nada de un objeto por figura ni por mensaje. El lienzo cruza como **un
//!   solo bufer plano** de `f32` (ver [`lienzo`]): una llamada por escena o
//!   por fotograma, y Swift construye sus `CGPath` en una pasada.
//! - Un fichero entra al chat con [`Nucleo::adjuntar`] (se copia, no se
//!   lee entero en memoria) y su ruta en el disco sale de
//!   [`Nucleo::ruta_absoluta`]; las fotos de un lienzo, todas juntas, de
//!   [`Nucleo::imagenes_del_lienzo`] (ver [`adjuntos`]).
//! - Los chats se leen **por paginas** ([`Nucleo::mensajes`]): nunca el chat
//!   entero. El indice de lineas de cada cuaderno se guarda en una cache con
//!   tope ([`chats`]) y se vacia con el aviso de memoria del sistema
//!   ([`Nucleo::vaciar_caches`]).
//! - La galeria de capturas, el timeline y la busqueda tambien van por
//!   paginas ([`galeria`], [`timeline`], [`buscar`]); las tareas de todos
//!   los chats saltan sin leer las lineas que no son listas ([`tareas`]).
//! - Lo que va de Rust a Swift mientras pasa (el texto del asistente en
//!   vivo) cruza por un interfaz que implementa Swift ([`OyenteIa`]) en
//!   **tandas**, no palabra a palabra. Las claves de la nube viven en el
//!   Llavero del Mac por otro interfaz de Swift ([`LlaveroApp`], ver
//!   [`llavero`]).
//! - Lo que tarda (sincronizar, leer un lienzo grande) es **bloqueante y sin
//!   hilos propios**: Swift lo llama desde una `Task` con prioridad
//!   `.utility`, y el sistema lo manda a los nucleos de eficiencia. Rust no
//!   arranca hilos ni temporizadores que nadie pidio.
//!
//! # Panicos
//!
//! UniFFI envuelve cada llamada en `catch_unwind`: con `panic = "unwind"`
//! un panico de Rust llega a Swift como error y la app sigue viva. El
//! perfil `release` del espacio de trabajo usa `panic = "abort"` (el de
//! Windows, que es un ejecutable entero en Rust): ahi un panico cerraria la
//! app de Apple sin decir nada. Por eso el xcframework se compila con el
//! perfil `ffi` (`apple/Cargo.toml`), que hereda de `release` y solo cambia
//! eso. Ver `herramientas/xcframework.sh`.

uniffi::setup_scaffolding!();

pub mod adjuntos;
pub mod ajustes;
// Ajustes → Tareas, Lienzo y Pinear del iPad (25-oct), aparte de `AjustesMac`.
pub mod ajustes_organizar;
pub mod aligerar;
/// «Enviar a Anki» del menu de un mensaje (Windows 2561be0).
pub mod anki;
pub mod anotador_al_chat;
pub mod aparatos;
pub mod asistente;
pub mod automatizaciones;
pub mod burbujas;
pub mod buscar;
pub mod chats;
/// Grabar una clase o una reunion: lo que se guarda en el chat (Windows edd0ffa).
pub mod clase;
pub mod compartidos;
pub mod compartir;
/// Compartir una grabacion: su pagina web, sus hojas A4, Word, Markdown,
/// texto y Anki (Windows 2921a1d).
pub mod compartir_grabacion;
pub mod conversacion;
pub mod copias;
pub mod diapositivas;
pub mod documentos;
pub mod editor;
pub mod editor_construir;
pub mod editor_estilo;
pub mod editor_exportar;
pub mod editor_figuras;
pub mod editor_herramientas;
pub mod editor_imagenes;
pub mod editor_marcos;
pub mod editor_tapado;
/// Grabar clase en dos modos y la ventana «Estudiar la clase» (Windows
/// 3af2647 y ce6c058).
pub mod estudiar_clase;
pub mod galeria_figuras;
pub mod grafito_plano;
// Editar una tabla dibujada con su cajetin (iPad, 25-oct).
pub mod editor_tabla;
pub mod editor_ventana;
pub mod envio;
// La isla de la ficha del chat (Windows 71bd8b5).
pub mod ficha_del_chat;
pub mod galeria;
pub mod galeria_sincro;
pub mod hojas;
pub mod lecciones;
pub mod lienzo;
pub mod lienzo_suelto;
// Medir el texto del lienzo con las letras de verdad (iPad, 25-oct).
pub mod letras_lienzo;
// Las palabras magicas de Pinear (iPad, 25-oct; `pixpin_pin::magia`).
pub mod magia;
// Fusionar paginas del PDF del proyecto en un lienzo (iPad, 25-oct).
pub mod fusionar_paginas;
// La zona al chat como sublienzo y el salto por su enlace (iPad, 25-oct).
pub mod marcas_lienzo;
/// La mezcla y el tratamiento del sonido del motor de audio de Windows
/// (38204cb): microfono y sonido del Mac por bloques.
pub mod mezcla_audio;
/// El motor de video de la clase sin nada del Mac (Windows bea497b):
/// perfiles, cadena de caidas, ritmo variable y reloj.
pub mod motor_video;
pub mod zona_al_chat;
// El horario de clases que llama (Windows, 17-oct).
// Google Calendar, solo para ver (Windows, 18-oct).
pub mod calendario_google;
pub mod carpetas;
pub mod horario;
// Clip → «Página de un proyecto» (Windows, `ventana_chat/paginas.rs`).
pub mod llamada;
pub mod llavero;
pub mod mini_tiempos;
pub mod miniapps;
pub mod modelos3d;
/// La nota rica del chat: su Markdown con formato y sus fotos en album
/// dentro de la burbuja (Windows 974d5ba).
pub mod nota_rica;
pub mod notas;
pub mod office;
pub mod paginas;
pub mod planos;
// Renombrar y archivar proyectos (iPad, 25-oct).
pub mod proyectos;
pub mod recepcion;
pub mod recordatorios;
pub mod red;
pub mod sincro;
pub mod sincro_auto;
pub mod tareas;
pub mod timeline;
pub mod turnos;
pub mod unidades;
pub mod version;
pub mod voz;

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU16;
use std::sync::{Arc, Mutex};

pub use ajustes::AjustesMac;
pub use anki::EnvioAnki;
pub use asistente::{CorteIa, HerramientaUsada, MensajeIa, OyenteIa, RespuestaIa};
pub use buscar::{Busqueda, ClaseDeResultado, PaginaBusqueda, ResultadoBusqueda};
pub use chats::{FichaChat, MensajeChat, PaginaMensajes};
pub use clase::{CapturaClase, CapturaDeLaClase, ClaseGuardada};
pub use compartidos::{
    AvisoInvitacion, ChatCompartidoMac, CodigoDeInvitacion, Invitacion, PersonaCompartida,
    ResumenPersona,
};
pub use documentos::{DondeAnota, LecturaDocumento, MarcaDocumento};
pub use editor::{EditorLienzo, FasePuntero, PeticionLienzo};
pub use editor_estilo::{
    CambioDeEstilo, ColorEstilo, EstiloLienzo, GrosorEstilo, LineaEstilo, RellenoEstilo,
};
pub use envio::{
    ArchivoLlegado, AvisoEnvio, CodigoDeEnvio, DondeLlego, EnvioSuelto, FinEnvio, FinRecibir,
    FrenteMac, PuertaEnvio, PuertaRecibir, RecibidoEnvio,
};
pub use estudiar_clase::{
    CapturaParaEstudiar, ClaseParaEstudiar, EnUnaClase, FilaDeClase, LineaDeClase, ModoDeGrabar,
    SenalDeClase,
};
pub use galeria::{CapturaBorrada, CapturaCaducada, CapturaGaleria, PaginaGaleria};
pub use lecciones::{
    EtiquetaContada, FichaLeccion, FiltroLecciones, LeccionEditada, NotaRepaso, PaginaLecciones,
    PropuestaLeccion,
};
pub use lienzo::{CajaLienzo, ImagenDelLienzo, Lienzo, LienzoPlano};
pub use llamada::LlamadaSecreta;
pub use llavero::LlaveroApp;
pub use marcas_lienzo::{CamaraDelLienzo, MarcaDelLienzo, MarcasDelLienzo};
pub use mezcla_audio::{BloqueMezclado, FuenteDeMezcla, MezclaDeAudio, PapelDeAudio};
pub use miniapps::{AccionContador, CuentaMini, GastoMini, GiroRuleta, LibroMini, ResumenMini};
pub use modelos3d::{
    Caja3d, Camara3d, Categoria3d, ClaseDeModelo, Eleccion3d, Filtro3d, Indices3d, InfoElemento3d,
    Modelo3dApple, Nivel3d, Punto3d, ResumenModelo3d,
};
pub use motor_video::{CaminoDelVideo, CodificadorVideo, PerfilVideo, RitmoDelVideo};
pub use planos::{
    AnotadoPlano, BuferesGrupo, CajaPlano, CapaPlano, CotaPlano, EncajePlano, Enganche, GrupoPlano,
    NoDibujado, Plano, PuntoPlano, ResumenPlano, TipoEnganche,
};
pub use recordatorios::{RecordatorioMac, TareaRecordada};
pub use sincro::{AnuncioBonjour, Atendido, Escucha, ParTxt, ResumenSincro};
pub use tareas::{
    ListaTareas, MarcasTarea, Tarea, marcas_de_tarea, tarea_con_nota, tarea_entendida,
};
pub use timeline::{DiaTimeline, MomentoTimeline, PaginaDias, PaginaMomentos};
pub use turnos::{CambioDeTurno, TurnoVoz};
pub use voz::{GrabacionVoz, SegmentoVoz};

/// Lo que puede salir mal, tal como lo ve Swift (un `enum` que `throws`).
///
/// Pocas variantes y con el detalle en texto: Swift decide que ensenar por
/// la variante, y el texto es para el registro.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum ErrorNucleo {
    /// Leer o escribir en el disco.
    #[error("disco: {detalle}")]
    Disco { detalle: String },
    /// Lo pedido no esta (un chat, un lienzo).
    #[error("no existe: {que}")]
    NoExiste { que: String },
    /// Un `.excalidraw` que no se entiende.
    #[error("lienzo: {detalle}")]
    Lienzo { detalle: String },
    /// La red o el protocolo de sincronizar.
    #[error("sincronizar: {detalle}")]
    Sincro { detalle: String },
    /// Un argumento que no vale (un codigo de grupo mal escrito...).
    #[error("argumento: {detalle}")]
    Argumento { detalle: String },
    /// `pixpinmax.toml` no se pudo leer o escribir (o no se entiende).
    #[error("ajustes: {detalle}")]
    Ajustes { detalle: String },
    /// El asistente: no hay motor, se corto, o contesto algo que no vale.
    /// El texto ya va en palabras para el usuario.
    #[error("{detalle}")]
    Asistente { detalle: String },
}

impl From<std::io::Error> for ErrorNucleo {
    fn from(e: std::io::Error) -> Self {
        ErrorNucleo::Disco {
            detalle: e.to_string(),
        }
    }
}

impl From<pixpin_sincro::protocolo::ErrorSincro> for ErrorNucleo {
    fn from(e: pixpin_sincro::protocolo::ErrorSincro) -> Self {
        ErrorNucleo::Sincro {
            detalle: e.to_string(),
        }
    }
}

/// Quien es este aparato.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct InfoAparato {
    /// El id al azar, fijo para siempre. No se ensena.
    pub id: String,
    pub nombre: String,
    /// Los cuatro signos fijos (`K7Q2`) que lleva cada mensaje nacido aqui.
    pub codigo: String,
    /// La letra en el grupo, si esta en uno.
    pub letra: Option<String>,
    /// El codigo del grupo, si esta en uno.
    pub grupo: Option<String>,
}

/// **Una carpeta de datos de PixPin abierta**: el objeto con el que trabaja
/// Swift. Es barato de crear y se puede usar desde varios hilos.
#[derive(uniffi::Object)]
pub struct Nucleo {
    raiz: PathBuf,
    /// El nombre del aparato si hay que crear la identidad (lo pone Swift:
    /// `Host.current().localizedName` o `UIDevice.current.name`).
    nombre: String,
    pub(crate) cache: Mutex<chats::Cache>,
    /// Las lecciones ya leidas (ver [`lecciones`]).
    pub(crate) lecciones: Mutex<lecciones::Cache>,
    /// La lista de `capturas/` de la ultima lectura (ver [`galeria`]).
    pub(crate) cache_galeria: Mutex<galeria::CacheGaleria>,
    /// Los momentos del timeline de la ultima lectura (ver [`timeline`]).
    pub(crate) cache_timeline: Mutex<timeline::CacheTimeline>,
    /// Lo que el usuario tiene delante, para los archivos sueltos que
    /// llegan (ver [`envio`]).
    pub(crate) frente: Mutex<envio::FrenteMac>,
    /// El puerto de la ultima [`Escucha`] abierta (0 si ninguna): se le dice
    /// al otro en el saludo, como Windows.
    pub(crate) puerto_escucha: AtomicU16,
}

#[uniffi::export]
impl Nucleo {
    /// Abre (o crea) la carpeta de datos `raiz`. Si no tiene identidad, nace
    /// una con `nombre_del_aparato`: el formato es el de Android y Windows
    /// (`sincro/identidad.json`), asi que una carpeta copiada de Windows se
    /// abre tal cual y conserva su identidad.
    #[uniffi::constructor]
    pub fn abrir(raiz: String, nombre_del_aparato: String) -> Result<Arc<Nucleo>, ErrorNucleo> {
        let raiz = PathBuf::from(raiz);
        std::fs::create_dir_all(&raiz)?;
        pixpin_proyecto::identidad::Identidad::leer_o_crear(&raiz, &nombre_del_aparato)?;
        Ok(Arc::new(Nucleo {
            raiz,
            nombre: nombre_del_aparato,
            cache: Mutex::new(chats::Cache::default()),
            lecciones: Mutex::new(lecciones::Cache::default()),
            cache_galeria: Mutex::new(galeria::CacheGaleria::default()),
            cache_timeline: Mutex::new(timeline::CacheTimeline::default()),
            frente: Mutex::new(envio::FrenteMac::Nada),
            puerto_escucha: AtomicU16::new(0),
        }))
    }

    /// La carpeta de datos.
    pub fn raiz(&self) -> String {
        self.raiz.display().to_string()
    }

    /// Quien es este aparato y en que grupo esta.
    pub fn aparato(&self) -> Result<InfoAparato, ErrorNucleo> {
        let i = self.identidad()?;
        Ok(InfoAparato {
            codigo: i.yo.codigo(),
            id: i.yo.id,
            nombre: i.yo.nombre,
            letra: i.yo.letra,
            grupo: i.codigo,
        })
    }

    /// Suelta las caches (indices de los cuadernos). Para el aviso de
    /// memoria del sistema (`didReceiveMemoryWarning`,
    /// `DispatchSource.makeMemoryPressureSource`): se rehacen solas al
    /// volver a leer.
    pub fn vaciar_caches(&self) {
        self.cache_tomada().vaciar();
        self.soltar_lecciones();
        self.galeria_tomada().vaciar();
        self.timeline_tomada().vaciar();
    }
}

impl Nucleo {
    /// Cuantos chats tienen su indice en la cache (para comprobar el tope).
    pub fn chats_en_cache(&self) -> usize {
        self.cache_tomada().cuantos()
    }

    pub(crate) fn ruta(&self) -> &Path {
        &self.raiz
    }

    pub(crate) fn identidad(&self) -> Result<pixpin_proyecto::identidad::Identidad, ErrorNucleo> {
        Ok(pixpin_proyecto::identidad::Identidad::leer_o_crear(
            &self.raiz,
            &self.nombre,
        )?)
    }

    /// La cache, aunque un hilo muriera con ella tomada: es solo una cache.
    pub(crate) fn cache_tomada(&self) -> std::sync::MutexGuard<'_, chats::Cache> {
        self.cache.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn galeria_tomada(&self) -> std::sync::MutexGuard<'_, galeria::CacheGaleria> {
        self.cache_galeria.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(crate) fn timeline_tomada(&self) -> std::sync::MutexGuard<'_, timeline::CacheTimeline> {
        self.cache_timeline
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }
}

/// La hora de ahora en milisegundos desde 1970, como la de todo el formato.
pub(crate) fn ahora_ms() -> i64 {
    pixpin_sistema::entorno::ahora_utc_ms()
}
