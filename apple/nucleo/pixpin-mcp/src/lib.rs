//! **PixPin como servidor MCP** (integraciones, punto 3; 15-oct-2026): que
//! Claude Desktop, Claude Code o cualquier cliente MCP usen PixPin con las
//! mismas herramientas que ya tiene el asistente.
//!
//! - **Transporte**: stdio, JSON-RPC 2.0, un mensaje por linea
//!   ([`protocolo`]). Nada mas sale por stdout; el registro, por stderr.
//! - **Herramientas que escriben** ([`herramientas`]): las filas de la tabla
//!   del asistente (`pixpin_ia::acciones::todas`), **tal cual**: su JSON
//!   Schema lo genera `acciones::herramienta` y sus argumentos los valida
//!   `acciones::validar`, los mismos que usa el chat. Se hacen en la PixPin
//!   que corre con el pedido `herramienta` ([`escribir`]), que las ejecuta
//!   por el mismo camino que el asistente (tareas por su nombre, documentos,
//!   permiso antes de lo que borra) y deja el resultado en un fichero de
//!   respuesta.
//! - **Herramientas que leen**, aqui mismo y sin la app: buscar en mis cosas
//!   (el indice del asistente), las tareas, los chats ([`leer`]), el texto de
//!   un PDF u Office y el OCR de Windows ([`documento`]), y ver la pantalla o
//!   una ventana ([`pantalla`]). Nunca escriben en la carpeta de datos.
//! - **Seguridad**: lo que borra o saca algo del PC (las filas con
//!   `confirmar`) y lo que cambia lo que ya existe ([`herramientas::MODIFICA`])
//!   lo pregunta PixPin al usuario, en su aviso flotante, antes de hacerlo.
//!   El cliente lo ve marcado con `destructiveHint`.

pub mod documento;
pub mod escribir;
pub mod herramientas;
pub mod leer;
pub mod hora;
pub mod pantalla;
pub mod protocolo;

/// Lo que se dice en `serverInfo`.
pub const NOMBRE: &str = "pixpin";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
