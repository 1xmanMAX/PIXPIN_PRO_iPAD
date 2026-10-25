//! Sincronizar con PixPin Android por la red local.
//!
//! Es un puerto del protocolo del movil, no un diseño nuevo: cada numero,
//! cada rotulo y cada orden estan copiados de `sincro/` de PixPin Android
//! (repositorio `1xmanMAX/PIXPIN_PRO_ANDROID`, commit `05722d9`) para que los
//! dos aparatos se entiendan **byte a byte**. Si algo de aqui parece
//! mejorable, casi siempre la respuesta es que hay que cambiarlo tambien
//! alli, o no habra sincronizacion.
//!
//! Lo que ya esta:
//!
//! - `codigo`: del codigo que se teclea a la clave del grupo (PBKDF2) y a la
//!   etiqueta publica que se anuncia en la red.
//! - `canal`: el saludo, las dos claves de sesion y los tramos cifrados.
//! - `mensajes`: las peticiones y respuestas del dialogo, en JSON.
//! - `canonico`: el JSON canonico y el resumen SHA-256 con que los dos
//!   aparatos saben si algo cambio (sobre un arbol propio que conserva los
//!   numeros tal cual, que `serde_json::Value` no lo hace).
//! - `diferencia`: el plan (traer, mandar, fusionar) a partir de dos
//!   inventarios y lo acordado.
//! - `fusion`: la fusion a tres bandas de lienzos, tablas, croquis y notas,
//!   y los parches con que viajan solo los cambios.
//! - `mezcla`: juntar dos versiones de un proyecto, hoja por hoja.
//! - `grupo`: el codigo del grupo, sus miembros, el codigo fijo de un
//!   aparato y la sena `47a`.
//!
//! - `base`: lo acordado con otro aparato, con el sello exacto del movil.
//! - `kotlin`: los `Mensaje` y `Proyecto` de Android como los escribe
//!   kotlinx, y el resumen de un mensaje.
//! - `disco`: lo que la sincronizacion lee y escribe de un aparato (un
//!   trait: cada aparato pone como guarda sus chats) y lo que es igual en
//!   todos; `copias`, la copia de antes de tocar nada.
//! - `al_lienzo`: una foto suelta al lienzo abierto del movil (`suelto`).
//! - `galeria`: la galeria de capturas que viaja, con sus fechas de irse.
//! - `compartidos`, `valla` e `invitacion`: los chats compartidos con otras
//!   personas (Android v0.122.0): llaves P-256, listas firmadas, la valla de
//!   archivos, la invitacion `PXI1` y la vuelta entre personas `PXC1`.
//! - `protocolo`: el `Respondedor` y la `Sesion`, peticion a peticion;
//!   `vuelta`, una vuelta entera como la lleva la pantalla del movil.
//!
//! El descubrimiento por mDNS vive en `pixpin-shell`, y el `Disco` del PC
//! (la vista Android sobre su almacen) en `pixpin-proyecto::vista`.

#![forbid(unsafe_code)]

pub mod al_lienzo;
pub mod anotado;
pub mod base;
pub mod canal;
pub mod canonico;
pub mod codigo;
pub mod compartidos;
pub mod copias;
pub mod diferencia;
pub mod disco;
#[cfg(any(test, feature = "simulador"))]
pub mod disco_android;
pub mod envio;
pub mod fusion;
pub mod galeria;
pub mod grupo;
pub mod invitacion;
pub mod kotlin;
pub mod mensajes;
pub mod mezcla;
pub mod protocolo;
pub mod valla;
pub mod vuelta;

/// La version del protocolo que se habla. Viaja en el saludo `hola`.
///
/// **La comprobacion es estricta y sin margen**: Android corta si el numero
/// no es exactamente el suyo. No hay negociacion ni rango, asi que una v3 y
/// una v4 no se hablan, y este numero hay que subirlo a la vez que alli.
///
/// Historia: la 2 mandaba el resumen detras de cada archivo; la 3 trajo los
/// codigos unicos, la fusion sin preguntar y los parches; la 4 (16-sep-2026)
/// hace viajar el borrado de un proyecto con las lapidas de chat.
pub const VERSION: u32 = 4;

/// El tipo de servicio que se anuncia y se busca por mDNS al sincronizar.
pub const SERVICIO: &str = "_pixpin._tcp.";
/// Mandar algo puntual a otro aparato: el que envia escucha y anuncia esto.
pub const SERVICIO_ENVIO: &str = "_pixpinenvio._tcp.";
/// Y al reves: el que RECIBE escucha y ensena su codigo, para que el otro se
/// lo mande. Es el caso de «pasarle esto al ordenador»: el PC ensena su QR y
/// el movil lo escanea.
pub const SERVICIO_RECIBIR: &str = "_pixpinrecibe._tcp.";
/// El puerto que se intenta primero. Si esta ocupado, Android cae a uno
/// efimero y lo dice en el saludo, asi que no se puede dar por fijo.
pub const PUERTO: u16 = 47474;
/// Los cuatro bytes de la sonda que manda quien solo quiere saber si hay
/// alguien escuchando, antes de hablar el protocolo.
pub const SONDA: &[u8; 4] = b"PING";
/// Lo que contesta quien esta vivo.
pub const SONDA_RESPUESTA: &[u8; 4] = b"PONG";
/// Cuantos mensajes se piden de una vez.
pub const MENSAJES_POR_TANDA: usize = 200;
/// Cuantos aparatos caben en un grupo: una letra cada uno, de la A a la Z.
pub const APARATOS_POR_GRUPO: usize = 26;

#[cfg(test)]
mod de_verdad;

#[cfg(test)]
mod compartidos_de_verdad;
