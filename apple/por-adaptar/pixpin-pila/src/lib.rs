//! pixpin-pila — la pila de capturas y su icono en la esquina.
//!
//! El usuario lo pidio asi: «a la hora de tomar capturas [...] quiero que me
//! aparezca un pequeno icono en la esquina de la pantalla, como un recuadro
//! de una imagen. Si yo no pego esa captura [...] y enseguida tomo otra
//! captura, que ese icono se haga como un apilado de capturas [...] y al
//! final con un solo Ctrl+V yo pueda pegarlas todas».
//!
//! El crate esta partido en dos mitades que no se mezclan:
//!
//! - [`pila`] es logica pura. Cuando una captura entra en la tanda, cuando la
//!   tanda se cierra, que hay marcado y que se acaba copiando. Sin Windows,
//!   sin reloj propio y sin ficheros: se prueba entera en milisegundos.
//! - [`ventana`] es la ventanita flotante: Win32 y Direct2D, igual que
//!   `pixpin-pin`, del que copia la receta de no robar el foco nunca.
//!
//! Este crate habla con Win32; `unsafe` permitido con `// SAFETY:` por
//! bloque. Como `pixpin-pin`, es capa L2 y no puede ver `pixpin-store`: los
//! ajustes llegan ya masticados desde el ejecutable.
#![deny(clippy::undocumented_unsafe_blocks)]

pub mod pila;
pub mod ventana;

pub use pila::{Captura, Copia, Efecto, Esquina, Pila, TOPE_CAPTURAS, rect_del_icono};
pub use ventana::{
    AccionPila, ErrorPila, IconoPila, LADO_ICONO_LOGICO, MARGEN_ICONO_LOGICO, TextosPila,
    pintar_pila,
};
