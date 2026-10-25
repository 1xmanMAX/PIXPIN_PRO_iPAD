//! **De lo que haya en el fichero a PCM de 16 bits, mono, 16 kHz.**
//!
//! El movil lo hace con `MediaExtractor` + `MediaCodec` y se escribe a mano
//! la mezcla a mono y el remuestreo por interpolacion lineal
//! (`Transcriptor.decodificar` y `Remuestreador`, `Transcriptor.kt:431-542`).
//!
//! Aqui no hace falta escribir nada de eso: al `IMFSourceReader` de Media
//! Foundation se le **pide** la salida en el formato que se quiere —PCM de
//! 16 bits, un canal, 16 000 muestras por segundo— y el mismo mete por
//! dentro el descodificador del contenedor y el remuestreador del sistema.
//! Sale gratis y es el mismo camino por el que `pixpin-audio` reproduce y
//! `pixpin-record` escribe video. De regalo entra cualquier cosa que
//! Windows sepa abrir: el `.m4a` nuestro, el `.mp3`, el `.wav`, y el `.ogg`
//! de WhatsApp en un Windows con el paquete de Opus.
//!
//! Este modulo habla con el sistema operativo: cada bloque `unsafe` lleva
//! su `// SAFETY:`.

use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::sync::Once;

use windows::Win32::Media::MediaFoundation::{
    IMFSample, MF_MT_AUDIO_AVG_BYTES_PER_SECOND, MF_MT_AUDIO_BITS_PER_SAMPLE,
    MF_MT_AUDIO_BLOCK_ALIGNMENT, MF_MT_AUDIO_NUM_CHANNELS, MF_MT_AUDIO_SAMPLES_PER_SECOND,
    MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE, MF_SOURCE_READER_ALL_STREAMS,
    MF_SOURCE_READER_FIRST_AUDIO_STREAM, MF_SOURCE_READERF_ENDOFSTREAM, MF_VERSION,
    MFAudioFormat_PCM, MFCreateMediaType, MFCreateSourceReaderFromURL, MFMediaType_Audio,
    MFSTARTUP_LITE, MFStartup,
};
use windows::core::PCWSTR;

use crate::{BYTES_POR_SEGUNDO, ErrorVoz, HERCIOS, en};

/// Media Foundation se arranca una sola vez por proceso.
///
/// `MFStartup`/`MFShutdown` llevan su propia cuenta, asi que este arranque
/// convive con el del reproductor y el de la grabadora sin pisarse. No se
/// llama a `MFShutdown`: la plataforma vive lo que viva el proceso, igual
/// que en `pixpin-audio` y en `pixpin-record`.
static ARRANQUE: Once = Once::new();

fn arrancar() {
    ARRANQUE.call_once(|| {
        // SAFETY: `MFStartup` no tiene mas precondicion que recibir la
        // version de la cabecera con la que se compilo. Si falla, la
        // primera llamada que dependa de ella devolvera su propio error
        // con contexto, que es mas util que reventar aqui.
        unsafe {
            let _ = MFStartup(MF_VERSION, MFSTARTUP_LITE);
        }
    });
}

/// Cuantas muestras de 16 kHz ocupa un milisegundo. Para pasar de posicion
/// a tiempo sin repetir la division por todas partes.
pub fn ms_de_muestra(muestra: usize) -> i64 {
    (muestra as i64) * 1000 / HERCIOS as i64
}

/// La duracion, en milisegundos, de un PCM ya decodificado.
pub fn duracion_ms(muestras: &[i16]) -> i64 {
    ms_de_muestra(muestras.len())
}

/// Decodifica un audio entero a PCM de 16 bits, mono, a [`HERCIOS`].
///
/// Devuelve las muestras en memoria y no un fichero temporal —el movil
/// escribe uno (`Transcriptor.kt:431`)— porque aqui la nota de voz cabe de
/// sobra: una hora de audio a 16 kHz son 115 MB, y una nota de voz de un
/// chat dura minutos, no horas.
pub fn decodificar(ruta: &Path) -> Result<Vec<i16>, ErrorVoz> {
    arrancar();

    let ancha: Vec<u16> = ruta
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // SAFETY: `ancha` es una cadena UTF-16 terminada en cero que vive hasta
    // despues de la llamada; el lector no se queda con el puntero. Se pasa
    // `None` como atributos, que la documentacion admite.
    let lector =
        unsafe { MFCreateSourceReaderFromURL(PCWSTR(ancha.as_ptr()), None) }.map_err(|_| {
            ErrorVoz::NoEsAudio {
                ruta: ruta.display().to_string(),
            }
        })?;

    let tipo = {
        // SAFETY: `MFCreateMediaType` no tiene precondiciones.
        let tipo = unsafe { MFCreateMediaType() }.map_err(en("crear el tipo de salida"))?;
        // SAFETY: el tipo acaba de crearse y los GUID son constantes del
        // propio sistema; poner atributos en un tipo vacio no tiene mas
        // condiciones.
        unsafe {
            tipo.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)
                .map_err(en("declarar que la salida es audio"))?;
            tipo.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM)
                .map_err(en("pedir la salida en PCM"))?;
            tipo.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)
                .map_err(en("pedir muestras de 16 bits"))?;
            tipo.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, HERCIOS)
                .map_err(en("pedir 16 kHz"))?;
            tipo.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 1)
                .map_err(en("pedir un solo canal"))?;
            // Coherentes con lo de arriba: 2 bytes por muestra, un canal.
            tipo.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 2)
                .map_err(en("declarar el alineamiento"))?;
            tipo.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, BYTES_POR_SEGUNDO as u32)
                .map_err(en("declarar el caudal"))?;
        }
        tipo
    };

    let todas = MF_SOURCE_READER_ALL_STREAMS.0 as u32;
    let audio = MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32;

    // SAFETY: el lector esta vivo y los indices son las constantes que la
    // propia API define para «todas» y «la primera de audio». Apagar todas
    // y encender solo la de audio es la receta documentada: si el fichero
    // trae video (un `.mp4` con voz), no se descodifica ni un fotograma.
    unsafe {
        lector
            .SetStreamSelection(todas, false)
            .map_err(en("apagar las pistas"))?;
        lector
            .SetStreamSelection(audio, true)
            .map_err(|_| ErrorVoz::NoEsAudio {
                ruta: ruta.display().to_string(),
            })?;
        lector
            .SetCurrentMediaType(audio, None, &tipo)
            .map_err(en("pedir el formato de salida"))?;
    }

    let mut muestras: Vec<i16> = Vec::new();
    loop {
        let mut banderas: u32 = 0;
        let mut trozo: Option<IMFSample> = None;
        // SAFETY: los tres punteros apuntan a variables locales vivas
        // durante toda la llamada; `ReadFlags` a cero es la lectura
        // sincrona, que es lo que se quiere en un hilo de fondo.
        unsafe {
            lector
                .ReadSample(audio, 0, None, Some(&mut banderas), None, Some(&mut trozo))
                .map_err(en("leer del audio"))?;
        }

        if banderas & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
            break;
        }
        let Some(trozo) = trozo else {
            // Sin muestra y sin fin de fichero: el lector esta cambiando de
            // formato o saltando un hueco. Se sigue leyendo.
            continue;
        };

        // SAFETY: la muestra acaba de llegar viva; `ConvertToContiguousBuffer`
        // devuelve un buffer propio. `Lock` entrega un puntero valido y una
        // longitud en bytes que solo se leen entre `Lock` y `Unlock`, y el
        // copiado se hace dentro de ese par. `Unlock` se llama en todos los
        // caminos porque entre medias no hay ningun `?`.
        unsafe {
            let buffer = trozo
                .ConvertToContiguousBuffer()
                .map_err(en("juntar el trozo de audio"))?;
            let mut datos: *mut u8 = std::ptr::null_mut();
            let mut largo: u32 = 0;
            buffer
                .Lock(&mut datos, None, Some(&mut largo))
                .map_err(en("abrir el trozo de audio"))?;
            if !datos.is_null() {
                // El formato pedido es PCM de 16 bits: los bytes van de dos
                // en dos y en el orden del aparato. Una longitud impar —que
                // no deberia ocurrir— se recorta en vez de leer medio entero.
                let cuantas = largo as usize / 2;
                muestras.reserve(cuantas);
                for i in 0..cuantas {
                    let bajo = *datos.add(i * 2);
                    let alto = *datos.add(i * 2 + 1);
                    muestras.push(i16::from_le_bytes([bajo, alto]));
                }
            }
            let _ = buffer.Unlock();
        }
    }

    if muestras.is_empty() {
        return Err(ErrorVoz::AudioVacio);
    }
    Ok(muestras)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_tiempo_sale_de_contar_muestras_a_dieciseis_mil_por_segundo() {
        assert_eq!(ms_de_muestra(0), 0);
        assert_eq!(ms_de_muestra(HERCIOS as usize), 1_000);
        assert_eq!(duracion_ms(&vec![0i16; HERCIOS as usize / 2]), 500);
    }

    #[test]
    fn un_fichero_que_no_es_audio_no_se_decodifica() {
        let ruta = std::env::temp_dir().join("pixpin-voz-no-es-audio.m4a");
        std::fs::write(&ruta, b"esto no es un audio").unwrap();
        let salida = decodificar(&ruta);
        let _ = std::fs::remove_file(&ruta);
        assert!(
            matches!(salida, Err(ErrorVoz::NoEsAudio { .. })),
            "se esperaba «no es audio», y salio {salida:?}"
        );
    }

    #[test]
    fn un_fichero_que_no_existe_tampoco() {
        let salida = decodificar(Path::new("C:/no/existe/esto.m4a"));
        assert!(matches!(salida, Err(ErrorVoz::NoEsAudio { .. })));
    }
}
