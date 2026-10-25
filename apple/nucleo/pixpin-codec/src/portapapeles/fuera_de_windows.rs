//! **El portapapeles fuera de Windows**: las mismas firmas que la version de
//! Win32, para que el resto del nucleo compile igual en todas partes.
//!
//! En Mac, iPad e iPhone el portapapeles lo lleva Swift (`NSPasteboard`,
//! `UIPasteboard`): copiar da `ErrorCodec::NoSoportadoAqui`, que lo dice, y
//! leer no ve nada. Sin estado ni hilos: no cuesta nada tenerlo.

use super::ContenidoPortapapeles;
use crate::imagen::{ErrorCodec, ImagenRgba};

/// Quien hace aqui lo que en Windows hace el portapapeles de Win32.
const SUSTITUTO: &str = "NSPasteboard/UIPasteboard";

fn no_soportado(que: &str) -> ErrorCodec {
    ErrorCodec::NoSoportadoAqui {
        que: que.to_owned(),
        sustituto: SUSTITUTO,
    }
}

/// Ver `copiar_imagen` de Windows. Aqui: `NoSoportadoAqui`.
pub fn copiar_imagen(_imagen: &ImagenRgba) -> Result<(), ErrorCodec> {
    Err(no_soportado("copiar una imagen al portapapeles"))
}

/// Ver `copiar_imagen_y_ficheros` de Windows. Aqui: `NoSoportadoAqui`.
pub fn copiar_imagen_y_ficheros(
    _imagen: &ImagenRgba,
    _rutas: &[std::path::PathBuf],
) -> Result<(), ErrorCodec> {
    Err(no_soportado("copiar imagen y ficheros al portapapeles"))
}

/// Ver `copiar_texto` de Windows. Aqui: `NoSoportadoAqui`.
pub fn copiar_texto(_texto: &str) -> Result<(), ErrorCodec> {
    Err(no_soportado("copiar texto al portapapeles"))
}

/// Ver `copiar_ficheros` de Windows. Aqui: `NoSoportadoAqui`.
pub fn copiar_ficheros(_rutas: &[std::path::PathBuf]) -> Result<(), ErrorCodec> {
    Err(no_soportado("copiar ficheros al portapapeles"))
}

/// Ver `leer` de Windows. Aqui no hay portapapeles que leer: `None`, lo
/// mismo que un portapapeles vacio.
pub fn leer() -> Option<ContenidoPortapapeles> {
    None
}

/// Ver `secuencia` de Windows. Aqui no cambia nunca: 0.
pub fn secuencia() -> u32 {
    0
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn copiar_dice_que_lo_hace_swift_y_leer_no_ve_nada() {
        let img = ImagenRgba {
            ancho: 1,
            alto: 1,
            pixeles: vec![1, 2, 3, 255],
        };
        let e = copiar_imagen(&img).unwrap_err();
        assert!(e.to_string().contains("NSPasteboard"), "{e}");
        assert!(copiar_texto("hola").is_err());
        assert!(copiar_ficheros(&[]).is_err());
        assert!(copiar_imagen_y_ficheros(&img, &[]).is_err());
        // Caso negativo: sin portapapeles no se inventa contenido.
        assert_eq!(leer(), None);
        assert_eq!(secuencia(), 0);
    }
}
