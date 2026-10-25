//! pixpin-cad — ver planos DWG y DXF en una ventana como un pin, fluido
//! aunque el plano sea grande y el equipo modesto.
//!
//! - [`leer`]: abrir el fichero con `opencadcodec` (MPL-2.0).
//! - [`convertir`]: del plano a rayas y triangulos ([`modelo::Modelo`]).
//! - [`modelo`]: lo que va a la tarjeta grafica, ordenado para dibujar solo
//!   lo que se ve, y su cache en disco.
//!
//! **Fuera de Windows** (Mac, iPad e iPhone) compila todo menos lo que
//! pinta: `gpu` y `gpu3d` (D3D11 + HLSL) y las ventanas (`ventana`,
//! `ventana3d`, Win32) van solo con `cfg(windows)`. Lo demas es Rust puro:
//! leer, convertir, medir, anotar, la pagina web y los bufferes que van a la
//! tarjeta ([`modelo::Modelo`] y [`modelo3d::Modelo3d`], planos y
//! `#[repr(C)]`), que en Apple dibuja Metal desde Swift sin copiarlos. La
//! camara 3D esta en [`orbita`] y el enganche de las cotas en [`enganche`].
// `!(x > 0.0)` va a proposito en todo el crate: tambien descarta NaN. Lo
// avisa un Clippy nuevo; el codigo es el de Windows, tal cual.
#![allow(clippy::neg_cmp_op_on_partial_ord)]
pub mod convertir;
pub mod croquis;
pub mod enganche;
pub mod convertir3d;
#[cfg(windows)]
pub mod gpu;
#[cfg(windows)]
pub mod gpu3d;
pub mod leer;
pub mod letras;
pub mod marcadores;
pub mod modelo;
pub mod modelo3d;
pub mod orbita;
pub mod proxy;
pub mod regla;
pub mod anotado;
#[cfg(windows)]
pub mod ventana3d;
pub mod relleno;
pub mod shx;
pub mod teselar;
pub mod texto;
#[cfg(windows)]
pub mod ventana;
pub mod web;
/// Pintar un plano a PNG en la CPU: para las pruebas y, con el rasgo
/// `pintar-png`, para las de la app (el banco del asistente).
#[cfg(any(test, feature = "pintar-png"))]
pub mod pruebas_png;
