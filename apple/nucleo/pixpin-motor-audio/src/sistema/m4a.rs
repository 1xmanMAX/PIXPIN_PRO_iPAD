//! **El `.m4a`**: AAC-LC dentro de MPEG-4, con el `IMFSinkWriter` y el
//! codificador AAC del sistema, como siempre (`pixpin-audio`, y el MP4 de
//! `pixpin-record`).
//!
//! El mismo fichero que antes, para que nada de lo que lo lee cambie: el
//! movil lo reproduce, `pixpin-voz` lo descodifica con `IMFSourceReader` a
//! 16 kHz mono para pasarlo a texto, y el reproductor del chat lo abre con
//! `IMFMediaEngine`. Mono a 96 kbps para las notas de voz (lo mas bajo que
//! admite el codificador de Windows), estereo a 128.
//!
//! Esto es solo la [`SalidaAac`] de Windows: el hilo, la cola larga y tapar
//! con silencio lo que se pierda son de [`crate::plataforma::escribir_aac`],
//! igual en todas las plataformas.

use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows::Win32::Media::MediaFoundation::{
    IMFAttributes, IMFSinkWriter, MF_E_INVALIDMEDIATYPE, MF_E_TOPO_CODEC_NOT_FOUND,
    MF_MT_AAC_PAYLOAD_TYPE, MF_MT_AUDIO_AVG_BYTES_PER_SECOND, MF_MT_AUDIO_BITS_PER_SAMPLE,
    MF_MT_AUDIO_BLOCK_ALIGNMENT, MF_MT_AUDIO_NUM_CHANNELS, MF_MT_AUDIO_SAMPLES_PER_SECOND,
    MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE, MF_TRANSCODE_CONTAINERTYPE, MF_VERSION, MFAudioFormat_AAC,
    MFAudioFormat_PCM, MFCreateAttributes, MFCreateMediaType, MFCreateMemoryBuffer, MFCreateSample,
    MFCreateSinkWriterFromURL, MFMediaType_Audio, MFSTARTUP_LITE, MFStartup,
    MFTranscodeContainerType_MPEG4,
};
use windows::core::PCWSTR;

use super::com::ComDelHilo;
use crate::plataforma::SalidaAac;
use crate::reloj::duracion_de;
use crate::{ErrorMotor, en};

/// Bytes por segundo del AAC. El codificador de Windows solo admite 12 000,
/// 16 000, 20 000 o 24 000 (96 a 192 kbps).
fn bytes_por_segundo(canales: u16) -> u32 {
    if canales == 1 { 12_000 } else { 16_000 }
}

/// Un `.m4a` abierto, en el hilo que lo abrio.
///
/// El orden de los campos es el orden en que se sueltan: el COM, el ultimo.
pub(crate) struct Escritor {
    escritor: IMFSinkWriter,
    flujo: u32,
    muestreo: u32,
    canales: u16,
    escritos: u64,
    _com: ComDelHilo,
}

impl Escritor {
    /// Abre `ruta` en el hilo que llama (COM y Media Foundation incluidos).
    pub(crate) fn abrir(ruta: &Path, muestreo: u32, canales: u16) -> Result<Escritor, ErrorMotor> {
        let com = ComDelHilo::nuevo();
        arrancar_media_foundation();
        let ancha: Vec<u16> = ruta
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let alineado = 2 * u32::from(canales);
        // SAFETY: llamadas COM sobre objetos recien creados en este hilo; el
        // unico puntero crudo es la ruta terminada en cero, viva hasta el
        // final de la funcion.
        unsafe {
            let mut atributos: Option<IMFAttributes> = None;
            MFCreateAttributes(&mut atributos, 1).map_err(en("crear los atributos del .m4a"))?;
            let atributos = atributos.ok_or(ErrorMotor::Windows {
                paso: "crear los atributos del .m4a",
                codigo: 0x8000_4005_u32 as i32,
            })?;
            atributos
                .SetGUID(&MF_TRANSCODE_CONTAINERTYPE, &MFTranscodeContainerType_MPEG4)
                .map_err(en("elegir el contenedor MPEG-4"))?;
            let escritor = MFCreateSinkWriterFromURL(PCWSTR(ancha.as_ptr()), None, &atributos)
                .map_err(|_| ErrorMotor::Fichero {
                    ruta: ruta.display().to_string(),
                })?;

            let salida = MFCreateMediaType().map_err(en("crear el tipo de salida"))?;
            salida
                .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)
                .map_err(en("marcar la salida como sonido"))?;
            salida
                .SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_AAC)
                .map_err(en("marcar la salida como AAC"))?;
            salida
                .SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)
                .map_err(en("poner los bits de la salida"))?;
            salida
                .SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, muestreo)
                .map_err(en("poner el muestreo de la salida"))?;
            salida
                .SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, u32::from(canales))
                .map_err(en("poner los canales de la salida"))?;
            salida
                .SetUINT32(
                    &MF_MT_AUDIO_AVG_BYTES_PER_SECOND,
                    bytes_por_segundo(canales),
                )
                .map_err(en("poner el bitrate"))?;
            // Cero es «AAC crudo», lo que va dentro de un MPEG-4 (uno seria
            // ADTS, y el contenedor lo rechaza).
            salida
                .SetUINT32(&MF_MT_AAC_PAYLOAD_TYPE, 0)
                .map_err(en("poner el tipo de carga AAC"))?;
            let flujo = escritor
                .AddStream(&salida)
                .map_err(sin_codificador("anadir el flujo de sonido"))?;

            let entrada = MFCreateMediaType().map_err(en("crear el tipo de entrada"))?;
            entrada
                .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)
                .map_err(en("marcar la entrada como sonido"))?;
            entrada
                .SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM)
                .map_err(en("marcar la entrada como PCM"))?;
            entrada
                .SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)
                .map_err(en("poner los bits de la entrada"))?;
            entrada
                .SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, muestreo)
                .map_err(en("poner el muestreo de la entrada"))?;
            entrada
                .SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, u32::from(canales))
                .map_err(en("poner los canales de la entrada"))?;
            entrada
                .SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, alineado)
                .map_err(en("poner el alineamiento de la entrada"))?;
            entrada
                .SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, muestreo * alineado)
                .map_err(en("poner los bytes por segundo de la entrada"))?;
            escritor
                .SetInputMediaType(flujo, &entrada, None)
                .map_err(sin_codificador("encajar el PCM con el codificador AAC"))?;
            escritor
                .BeginWriting()
                .map_err(en("empezar a escribir el .m4a"))?;
            Ok(Escritor {
                escritor,
                flujo,
                muestreo,
                canales,
                escritos: 0,
                _com: com,
            })
        }
    }
}

impl SalidaAac for Escritor {
    /// Un bloque intercalado de 16 bits al fichero.
    fn escribir(&mut self, muestras: &[i16]) -> Result<(), ErrorMotor> {
        if muestras.is_empty() {
            return Ok(());
        }
        let bytes = std::mem::size_of_val(muestras);
        let marcos = (muestras.len() / self.canales as usize) as u64;
        // SAFETY: `MFCreateMemoryBuffer` solo reserva; `Lock` da un puntero a
        // `bytes` bytes del buffer, valido hasta el `Unlock`, que se hace
        // justo despues; el origen son `bytes` bytes de un slice vivo y se
        // copian como bytes porque el destino no promete alineacion de i16.
        unsafe {
            let buffer =
                MFCreateMemoryBuffer(bytes as u32).map_err(en("reservar un trozo de sonido"))?;
            let mut destino: *mut u8 = std::ptr::null_mut();
            buffer
                .Lock(&mut destino, None, None)
                .map_err(en("bloquear el trozo de sonido"))?;
            std::ptr::copy_nonoverlapping(muestras.as_ptr() as *const u8, destino, bytes);
            buffer
                .Unlock()
                .map_err(en("desbloquear el trozo de sonido"))?;
            buffer
                .SetCurrentLength(bytes as u32)
                .map_err(en("medir el trozo de sonido"))?;
            let muestra = MFCreateSample().map_err(en("crear la muestra de sonido"))?;
            muestra
                .AddBuffer(&buffer)
                .map_err(en("montar la muestra"))?;
            muestra
                .SetSampleTime(duracion_de(self.escritos, self.muestreo) as i64)
                .map_err(en("fechar la muestra"))?;
            muestra
                .SetSampleDuration(duracion_de(marcos, self.muestreo) as i64)
                .map_err(en("medir la muestra"))?;
            self.escritor
                .WriteSample(self.flujo, &muestra)
                .map_err(en("escribir un trozo de sonido"))?;
        }
        self.escritos += marcos;
        Ok(())
    }

    fn cerrar(&mut self) -> Result<(), ErrorMotor> {
        // SAFETY: escritor vivo y en escritura; `Finalize` cierra el indice
        // del MPEG-4, sin el cual el fichero no se puede abrir.
        unsafe { self.escritor.Finalize() }.map_err(en("cerrar el .m4a (Finalize)"))
    }
}

/// El error de Media Foundation cuando no hay MFT de AAC es el mismo que
/// cuando el tipo no le gusta: los dos se traducen al que el usuario puede
/// arreglar (las ediciones N de Windows vienen sin el paquete multimedia).
fn sin_codificador(paso: &'static str) -> impl Fn(windows::core::Error) -> ErrorMotor {
    move |e| {
        if e.code() == MF_E_TOPO_CODEC_NOT_FOUND || e.code() == MF_E_INVALIDMEDIATYPE {
            ErrorMotor::SinCodificadorAac
        } else {
            en(paso)(e)
        }
    }
}

/// Media Foundation se arranca una vez por proceso y no se apaga.
fn arrancar_media_foundation() {
    static ARRANQUE: std::sync::Once = std::sync::Once::new();
    ARRANQUE.call_once(|| {
        // SAFETY: `MFStartup` solo pide la version con la que se compilo.
        // Un fallo aqui reaparece como error al crear el escritor.
        unsafe {
            let _ = MFStartup(MF_VERSION, MFSTARTUP_LITE);
        }
    });
}
