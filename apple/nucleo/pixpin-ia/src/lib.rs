//! **Un modelo de lenguaje pequeno, en este ordenador y sin red.**
//!
//! Pedido del usuario (13-oct-2026): un asistente local que viva en el chat y
//! que, ademas de contestar, use **herramientas** de PixPin: dibujar en el
//! lienzo lo que se le pide o lo que ve en una foto, hacer graficas, pasar
//! por OCR un PDF escaneado... Aceptando alguna alucinacion pequena, pero sin
//! nada pesado. El analisis completo, con las medidas en este PC, esta en
//! `docs/investigacion/2026-10-13-ia-local.md`.
//!
//! ## Lo que se eligio y por que (resumen)
//!
//! - **Modelo: Qwen3.5** (Apache-2.0), en GGUF cuantizado a 4 bits. El 2B
//!   por omision (ve imagenes y llama herramientas bien); el 0.8B como
//!   «ligero» para equipos flojos o moviles. Los dos tienen vision.
//! - **Motor: llama.cpp** (MIT), su `llama-server` **en un proceso aparte**.
//!   El mismo GGUF corre en Android con llama.cpp, que es la condicion para
//!   que el PC y el movil se porten igual. Ni un byte en el ejecutable: el
//!   motor (unos 30 MB) y el modelo se bajan cuando el usuario lo pide.
//!
//! ## Las piezas
//!
//! - [`modelos`]: que se baja, de donde, cuanto pesa y su huella.
//! - [`descarga`]: bajar y verificar (SHA-256) a
//!   `%LOCALAPPDATA%\PixPinMax\modelos`, nunca en el instalador.
//! - [`motor`]: el trait [`Motor`] —lo unico que ve el resto— y los tipos de
//!   la conversacion. Detras puede ir llama.cpp ([`servidor`]) o un motor
//!   falso para las pruebas ([`motor::MotorGuionado`]).
//! - [`servidor`]: lanzar `llama-server`, hablarle por HTTP local con
//!   streaming, y matarlo al rato de no usarlo para devolver la memoria.
//! - [`herramientas`]: el **protocolo comun PC/Android** (nombre,
//!   descripcion, parametros en JSON Schema) y la validacion estricta de lo
//!   que devuelve el modelo.
//! - [`lienzo`]: de una llamada validada a elementos de **Excalidraw**, el
//!   formato que ya comparten el lienzo del PC y el de Android.
//! - [`nube`]: el mismo [`Motor`] con Claude Haiku 5.5 o DeepSeek V4.1 Flash
//!   por su API, con la clave del usuario cifrada con DPAPI (en Apple, en
//!   el Llavero: [`nube::claves::Llavero`]).
//! - [`agente`]: el bucle de un turno: pedir, validar, ejecutar, devolver el
//!   resultado al modelo, y reintentar si el modelo se equivoco.
//! - [`pensar`], [`hoja_trabajo`] y [`contexto`] (13-oct): el pensamiento a
//!   la vista, la peticion mejorada, la hoja de trabajo que el agente
//!   reescribe cada turno y el reinicio del contexto al 70 % con
//!   `buscar_historial` para lo viejo ([`agente::turno_con`]).
//! - [`complejidad`] (15-oct): si un turno piensa. En automatico, solo las
//!   tareas complejas, con unas reglas fijas sobre el pedido (sin modelo) y
//!   el mismo criterio para el local, Claude y DeepSeek.
//! - [`calcar`]: copiar una imagen al lienzo con un algoritmo clasico
//!   (k-medias + marching squares, o Canny + cadenas; RDP y suavizado): el
//!   modelo solo elige el detalle.
//! - [`acciones`]: toda la app como herramientas, generadas de UNA tabla
//!   que produce pedidos del protocolo (`docs/protocolo-pedidos.md`); lo
//!   que borra o sale del PC lleva su pregunta de confirmacion.
//! - [`seleccion`]: las 6 herramientas que vienen a cuento en cada turno.
//!   Y si una foto merece el modelo de mejor vista
//!   ([`seleccion::pide_mejor_vista`], Gemma 4 E2B, opcional).
//! - [`relevo`]: varios modelos locales con un solo `llama-server` cargado a
//!   la vez (cambiar suelta al otro).
//! - [`conocimiento`]: el resumen de PixPin y el indice de `ayuda_app`.
//! - [`rapido`], [`tareas`] y [`fechas`] (14-oct): las ordenes simples sin
//!   modelo (<100 ms), las tareas por su nombre («la tarea comprar pan»,
//!   «mañana a las 10») y las consultas («¿que tengo pendiente?»).
//! - [`documentos`]: `buscar_en_mis_cosas`, un indice BM25 local de los
//!   documentos, notas y chats del usuario, con el origen de cada trozo.
//! - [`edicion`] (14-oct): editar lo que ya hay en el lienzo (elegir por
//!   descripcion, recortar y pegar fotos, lapiz/grafito/resaltador, rectas y
//!   circulos exactos, cortar y extender lineas) con las operaciones del
//!   motor del lienzo, un paso de deshacer por llamada, y el resumen del
//!   lienzo destino para el modelo.
//!
//! - [`hoja`] (14-oct): una foto o una pagina de una hoja al lienzo,
//!   interpretada zona a zona (texto, tablas, formulas, fotos, rayas), solo
//!   con la nube.
//! - [`puente`]: las herramientas en JSON de ida y vuelta, para cualquier
//!   motor (Foundation Models desde Swift, Android...).
//!
//! ## Fuera de Windows (Mac, iPad, iPhone)
//!
//! Esta copia vive en `apple/nucleo/` y compila sin Windows. Lo que en
//! Windows hace el sistema va aparte:
//!
//! - **Bajar** el motor y los modelos por HTTPS: en Windows con WinHTTP
//!   ([`descarga`]); en Apple con URLSession desde Swift, que despues usa
//!   [`descarga::verificar`] para la huella.
//! - **La nube** (Claude, DeepSeek): en Windows por WinHTTP
//!   ([`nube::http`]); fuera, el nucleo solo habla `http://` con este
//!   equipo y el HTTPS lo pone Swift. Ningun TLS propio en el ejecutable.
//! - **`llama-server`**: en Windows dentro de un trabajo que lo mata si
//!   PixPin muere; en Mac en su grupo de procesos y muerto al soltarlo
//!   ([`servidor`]). En iPad e iPhone no hay procesos: llama.cpp con Metal
//!   iria enlazado desde Swift.
//! - **Las claves**: DPAPI en Windows; en Apple el Llavero, que Swift
//!   instala con [`nube::claves::instalar_llavero`].
//!
//! **El asistente por omision en Apple es Foundation Models** (el modelo
//! del sistema, gratis y en el aparato) y llama.cpp con Metal el respaldo
//! (el mismo GGUF que en Windows y Android). Para que los dos hagan lo
//! mismo, las herramientas no dependen del motor: el catalogo, la
//! validacion y el paso al lienzo son funciones puras de JSON a JSON
//! ([`puente::catalogo_para`], [`puente::llamar`]) que Swift llama desde el
//! *tool calling* de Foundation Models igual que [`agente`] las usa con
//! llama.cpp.
//!
//! ## Puntos de enganche para la UI (otros agentes)
//!
//! - El chat implementa [`agente::Ejecutor`] (que hace cada herramienta de
//!   verdad: anadir al lienzo, OCR de Windows...) y escucha los
//!   [`agente::Evento`] para pintar la respuesta en vivo.
//! - El lienzo recibe un `.excalidraw` ya hecho ([`lienzo::documento`]) y lo
//!   lee con `pixpin_motor2d::excalidraw::leer`, como un fichero del movil.

pub mod acciones;
pub mod acciones_documentos;
pub mod agente;
pub mod anotacion;
pub mod base64;
pub mod calcar;
pub mod calculo;
pub mod conocimiento;
pub mod complejidad;
pub mod conexiones;
pub mod contexto;
pub mod cronograma;
pub mod diagrama;
pub mod descarga;
// Las fechas dichas viven en `pixpin-frase` (18-oct-2026): las comparte el
// plugin de Flow Launcher, que no puede cargar este crate.
pub use pixpin_frase::fechas;
pub mod documentos;
pub mod edicion;
pub mod ecuacion;
pub mod figuras;
pub mod fisica;
pub mod funciones;
pub mod herramientas;
pub mod hoja;
pub mod hoja_trabajo;
pub mod lienzo;
pub mod modelos;
pub mod motor;
pub mod nube;
pub mod ojos;
pub mod pizarra;
pub mod relevo;
pub mod pensar;
pub mod problema;
pub mod puente;
pub mod rapido;
pub mod rotulos;
pub mod seleccion;
pub mod servidor;
pub mod tabla_escaneada;
pub mod tabla_aspecto;
pub mod revision_tabla;
pub mod sse;
pub mod tareas;
pub mod svg;

pub use agente::{Ejecutor, Evento, turno};
pub use herramientas::{Herramienta, Llamada, catalogo};
pub use motor::{Mensaje, Motor, Parte, Peticion, Respuesta, Rol};

/// Lo que puede salir mal. Los textos van en espanol porque acaban en la
/// burbuja del chat.
#[derive(Debug, thiserror::Error)]
pub enum ErrorIa {
    #[error("no se pudo bajar {que}: {detalle}")]
    Descarga { que: String, detalle: String },
    #[error("el fichero {que} esta danado (su huella no coincide); se borra y hay que bajarlo otra vez")]
    HuellaDistinta { que: String },
    #[error("falta {0}: hay que bajar el asistente primero")]
    Falta(String),
    #[error("no se pudo arrancar el modelo: {0}")]
    Arranque(String),
    #[error("el modelo dejo de responder: {0}")]
    Conexion(String),
    #[error("el modelo contesto algo que no se entiende: {0}")]
    Protocolo(String),
    /// Un fallo de la API de la nube (clave, saldo, red, servidor), ya
    /// dicho para el usuario: que paso y que hacer.
    #[error("{0}")]
    Nube(String),
    #[error("cancelado")]
    Cancelado,
    #[error("{0}")]
    Io(#[from] std::io::Error),
}
