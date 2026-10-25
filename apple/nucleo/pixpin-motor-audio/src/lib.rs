//! **El motor de audio** de PixPin Max: el microfono y el sonido del equipo,
//! juntos, a la par durante horas y gastando lo minimo.
//!
//! Lo usan las notas de voz, el microfono flotante y la grabadora de clases
//! y reuniones (a traves de `pixpin-audio::Grabadora`), y el motor de video
//! de la reunion, que se suscribe a su PCM. La arquitectura entera esta en
//! `docs/motor-de-audio.md`; aqui, el reparto:
//!
//! - **Aritmetica pura, probada en Linux**: [`formato`] (leer el
//!   `WAVEFORMATEX` y bajar a mono o estereo), [`anillo`] (la cola sin
//!   cerrojos entre captura y mezcla), [`reloj`] (rellenar los huecos de
//!   silencio por el reloj del sistema), [`deriva`] (seguir el reloj de cada
//!   tarjeta), [`remuestreo`] (seno enventanado polifasico), [`dsp`] (paso
//!   alto, puerta, realce, limitador), [`nucleo`] (la mezcla), [`pcm`] (el
//!   reparto a los suscriptores) y [`medidas`].
//! - **Windows** (`cfg(windows)`): la captura WASAPI de cada fuente en su
//!   hilo, los avisos de cambio de aparato, la captura por proceso, el
//!   `.m4a` y [`MotorAudio`], que lo junta todo.

pub mod anillo;
pub mod deriva;
pub mod dsp;
pub mod formato;
pub mod medidas;
pub mod nucleo;
pub mod opciones;
pub mod pcm;
pub mod reloj;
pub mod remuestreo;

#[cfg(windows)]
mod motor;
#[cfg(windows)]
mod sistema;

pub use medidas::{Estadisticas, EstadisticasFuente, Nivel, Niveles};
pub use opciones::{Equipo, MUESTREOS, Microfono, Opciones, Salida};
pub use pcm::{BloquePcm, Receptor, SinBloque, Suscripcion, suscripcion};

#[cfg(windows)]
pub use motor::{MotorAudio, Resumen};
#[cfg(windows)]
pub use sistema::aparatos::{Aparato, microfonos};

/// Lo que puede salir mal.
///
/// Como en `pixpin-audio`, cada variante dice **que hacer**: «no hay
/// microfono» se le puede decir al usuario; un HRESULT suelto, no.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ErrorMotor {
    #[error("no se pidio ninguna fuente (ni microfono ni sonido del equipo)")]
    SinFuentes,
    #[error("no hay ningun microfono disponible (ni conectado, ni con permiso)")]
    SinMicrofono,
    #[error("no se puede oir lo que suena en el equipo (no hay altavoces ni cascos activos)")]
    SinSonidoDelEquipo,
    #[error(
        "este Windows no trae el codificador AAC (suele ser una edicion N sin el \
         paquete de caracteristicas multimedia)"
    )]
    SinCodificadorAac,
    #[error(
        "el aparato entrega un formato que no se sabe leer (etiqueta {etiqueta:#06x}, {bits} bits)"
    )]
    FormatoDesconocido { etiqueta: u16, bits: u16 },
    #[error(
        "la salida tiene que ser de 1 o 2 canales a 44 100 o 48 000 Hz (se pidio {canales} a {muestreo})"
    )]
    SalidaNoValida { canales: u16, muestreo: u32 },
    #[error("no se pudo crear {ruta}")]
    Fichero { ruta: String },
    #[error("un hilo del motor de audio se cayo")]
    HiloCaido,
    #[error("Windows fallo al {paso} (0x{codigo:08X})")]
    Windows { paso: &'static str, codigo: i32 },
}

impl From<formato::ErrorFormato> for ErrorMotor {
    fn from(e: formato::ErrorFormato) -> ErrorMotor {
        match e {
            formato::ErrorFormato::Desconocido { etiqueta, bits } => {
                ErrorMotor::FormatoDesconocido { etiqueta, bits }
            }
            formato::ErrorFormato::Desalineada { bits, .. } => {
                ErrorMotor::FormatoDesconocido { etiqueta: 0, bits }
            }
            formato::ErrorFormato::Corta | formato::ErrorFormato::Vacia => {
                ErrorMotor::FormatoDesconocido {
                    etiqueta: 0,
                    bits: 0,
                }
            }
        }
    }
}

/// Envuelve un fallo de Windows diciendo en que paso ocurrio.
#[cfg(windows)]
pub(crate) fn en(paso: &'static str) -> impl Fn(windows::core::Error) -> ErrorMotor {
    move |e| ErrorMotor::Windows {
        paso,
        codigo: e.code().0,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_codigo_de_windows_se_lee_en_hexadecimal() {
        let e = ErrorMotor::Windows {
            paso: "abrir el microfono",
            codigo: 0x8889_0004_u32 as i32,
        };
        assert_eq!(
            e.to_string(),
            "Windows fallo al abrir el microfono (0x88890004)"
        );
    }
}
