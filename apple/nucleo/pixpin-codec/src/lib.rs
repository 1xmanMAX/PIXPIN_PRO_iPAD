//! pixpin-codec — codificacion y decodificacion de imagen.
//!
//! Este crate habla con librerias C. El `unsafe` esta permitido, pero cada
//! bloque lleva su comentario `// SAFETY:`.
//!
//! **Fuera de Windows** (la copia de `apple/nucleo/` para Mac, iPad e
//! iPhone) compila y se prueba lo puro: leer y escribir PNG, JPEG, WebP y BMP
//! con `image`, el GIF propio, coser capturas, los filtros, redimensionar,
//! las medidas y la orientacion EXIF, y entender un CF_HTML o un RTF. Lo que
//! en Windows hace el sistema queda tras `#[cfg(windows)]`, tal cual:
//!
//! - WIC (`wic`, y leer a la medida en `vista`): HEIC, AVIF, TIFF, GIF, ICO,
//!   JPEG XL y RAW. En Apple los lee **ImageIO** desde Swift
//!   (`CGImageSource`, y `CGImageSourceCreateThumbnailAtIndex` para leer
//!   reducido), por hardware donde lo hay. Aqui `imagen::cargar` y
//!   `vista::cargar_para_ver` devuelven `ErrorCodec::NoSoportadoAqui`, que
//!   lo dice, en vez de descomprimir en software.
//! - El portapapeles (CF_DIB, CF_HDROP, CF_HTML; GDI y Shell). En Apple lo
//!   lleva Swift con `NSPasteboard`/`UIPasteboard`; aqui copiar da
//!   `NoSoportadoAqui` y leer no ve nada.
#![deny(clippy::undocumented_unsafe_blocks)]

pub mod cosido;
pub mod filtros;
pub mod gif;
pub mod imagen;
pub mod portapapeles;
pub mod vista;
pub mod wic;

pub use cosido::{
    Cosedor, Orden, Plan, Resultado, SIN_ENCAJE, encontrar_desplazamiento, es_lisa, firmas,
    franjas_fijas,
};
pub use gif::{ErrorGif, OpcionesGif, codificar as codificar_gif};
pub use imagen::{
    ErrorCodec, FormatoImagen, ImagenRgba, cargar, codificar_png, guardar, redimensionar,
};
#[cfg(windows)]
pub use portapapeles::construir_hdrop;
pub use portapapeles::{
    ContenidoPortapapeles, copiar_ficheros, copiar_imagen, copiar_imagen_y_ficheros, copiar_texto,
    leer,
};
