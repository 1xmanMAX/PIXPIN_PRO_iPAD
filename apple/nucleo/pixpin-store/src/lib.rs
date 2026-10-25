//! pixpin-store — ver docs/superpowers/specs/2026-08-09-pixpin-pc-master-design.md
//!
//! Este crate no puede contener `unsafe`. Si alguna vez lo necesitara, seria
//! señal de que la frontera de capas se ha roto y hay que arreglar el diseño,
//! no relajar la regla.
#![forbid(unsafe_code)]

pub mod ajustes;
pub mod almacen;
pub mod bandeja;
pub mod comandos;
pub mod estado;
pub mod herramientas;
pub mod idioma;
pub mod regiones;
pub mod rutas;

pub use ajustes::{
    Ajustes, Asistente, Atajos, Capturas, ErrorAjustes, EsquinaPila, FormatoColor, ModoDeIdiomas,
    PreferenciaIdioma, Rendimiento, Sincro, Suavizado, Tinta, Voz, cargar, guardar,
};
pub use comandos::{CATALOGO, Comando, Descriptor, Enlaces};
pub use idioma::{Catalogo, Idioma, resolver_idioma};
pub use rutas::{NOMBRE_AJUSTES, Ubicacion, resolver};

pub use almacen::{Almacen, ColorGrupo, Entrada, ErrorAlmacen, Grupo, PinGuardado, TipoEntrada};
