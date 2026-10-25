//! **El sonido de un solo proceso** (Windows 10 2004 en adelante): grabar la
//! reunion de Meet o de Zoom sin que se cuelen las notificaciones del correo
//! ni la musica de otra ventana.
//!
//! No hay aparato que abrir: se activa el aparato virtual
//! `VAD\Process_Loopback` con `ActivateAudioInterfaceAsync`, pasandole el
//! proceso (y sus hijos: el sonido de Chrome sale de un proceso hijo del de
//! la ventana) en un `PROPVARIANT` de tipo blob. La activacion es
//! asincrona: Windows llama a un objeto nuestro cuando termina, y aqui se
//! espera a ese aviso con un evento.
//!
//! **La comprobacion de que se puede** es esta misma activacion: en un
//! Windows anterior a 2004 el aparato virtual no existe y falla en el acto;
//! entonces quien llama graba todo el sonido del equipo y lo dice en las
//! estadisticas. Mirar el numero de version no serviria de mas y fallaria
//! con las ediciones que lo mienten por compatibilidad.
//!
//! Este cliente no tiene formato de mesa (`GetMixFormat` no esta): se le
//! pide uno y Windows convierte, que aqui no hay otra.

use std::sync::Arc;

use windows::Win32::Foundation::WAIT_OBJECT_0;
use windows::Win32::Media::Audio::{
    AUDIOCLIENT_ACTIVATION_PARAMS, AUDIOCLIENT_ACTIVATION_PARAMS_0,
    AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK, AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS,
    ActivateAudioInterfaceAsync, IActivateAudioInterfaceAsyncOperation,
    IActivateAudioInterfaceCompletionHandler, IActivateAudioInterfaceCompletionHandler_Impl,
    IAudioClient, PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
    VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK, WAVEFORMATEX,
};
use windows::Win32::System::Com::BLOB;
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Threading::{SetEvent, WaitForSingleObject};
use windows::Win32::System::Variant::VT_BLOB;
use windows::core::{HRESULT, IUnknown, Interface, Ref, implement};

use super::com::Evento;
use crate::formato::{ETIQUETA_FLOTANTE, ETIQUETA_PCM};
use crate::{ErrorMotor, en};

/// Lo que se espera a que Windows conteste la activacion.
const ESPERA_ACTIVACION_MS: u32 = 5_000;

/// El aviso de que la activacion termino: solo senala el evento (corre en un
/// hilo de Windows; ahi no se puede hacer nada mas).
#[implement(IActivateAudioInterfaceCompletionHandler)]
struct Activado {
    listo: Arc<Evento>,
}

impl IActivateAudioInterfaceCompletionHandler_Impl for Activado_Impl {
    fn ActivateCompleted(
        &self,
        _: Ref<IActivateAudioInterfaceAsyncOperation>,
    ) -> windows::core::Result<()> {
        // SAFETY: el evento vive mientras viva este objeto (lo sujeta el
        // `Arc`), y senalarlo desde otro hilo es su uso normal.
        unsafe { SetEvent(self.listo.0) }
    }
}

/// Un `IAudioClient` que captura solo el proceso `pid` y sus hijos.
///
/// # Safety
///
/// COM iniciado en el hilo, en el apartamento multihilo (el aviso llega por
/// otro hilo y en uno de un solo hilo sin bomba de mensajes no llegaria).
pub(crate) unsafe fn cliente(pid: u32) -> Result<IAudioClient, ErrorMotor> {
    let parametros = AUDIOCLIENT_ACTIVATION_PARAMS {
        ActivationType: AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
        Anonymous: AUDIOCLIENT_ACTIVATION_PARAMS_0 {
            ProcessLoopbackParams: AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS {
                TargetProcessId: pid,
                ProcessLoopbackMode: PROCESS_LOOPBACK_MODE_INCLUDE_TARGET_PROCESS_TREE,
            },
        },
    };
    let mut variante = PROPVARIANT::default();
    // SAFETY: se escribe la variante de blob de un `PROPVARIANT` recien
    // puesto a cero. El blob apunta a `parametros`, que vive en esta pila
    // hasta el final de la funcion, despues de que Windows lo haya leido
    // (la activacion termina antes de volver). No se llama a
    // `PropVariantClear`: el blob no es memoria de COM y no hay que soltarlo.
    unsafe {
        let dentro = &mut *variante.Anonymous.Anonymous;
        dentro.vt = VT_BLOB;
        dentro.Anonymous.blob = BLOB {
            cbSize: size_of::<AUDIOCLIENT_ACTIVATION_PARAMS>() as u32,
            pBlobData: &parametros as *const _ as *mut u8,
        };
    }
    let listo = Arc::new(Evento::nuevo()?);
    let aviso: IActivateAudioInterfaceCompletionHandler = Activado {
        listo: Arc::clone(&listo),
    }
    .into();
    // SAFETY: todos los punteros son a locales vivos durante la llamada y la
    // espera; el aviso queda sujeto por `aviso` hasta el final.
    unsafe {
        let operacion = ActivateAudioInterfaceAsync(
            VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
            &IAudioClient::IID,
            Some(&variante),
            &aviso,
        )
        .map_err(en("pedir el sonido de un proceso"))?;
        if WaitForSingleObject(listo.0, ESPERA_ACTIVACION_MS) != WAIT_OBJECT_0 {
            return Err(ErrorMotor::Windows {
                paso: "esperar el sonido de un proceso",
                codigo: 0x8000_0005_u32 as i32,
            });
        }
        let mut resultado = HRESULT(0);
        let mut interfaz: Option<IUnknown> = None;
        operacion
            .GetActivateResult(&mut resultado, &mut interfaz)
            .map_err(en("recoger el sonido de un proceso"))?;
        resultado
            .ok()
            .map_err(en("activar el sonido de un proceso"))?;
        interfaz
            .ok_or(ErrorMotor::Windows {
                paso: "activar el sonido de un proceso",
                codigo: 0x8000_4003_u32 as i32,
            })?
            .cast::<IAudioClient>()
            .map_err(en("activar el sonido de un proceso"))
    }
}

/// Los formatos que se le piden, por orden: coma flotante (lo que la mesa
/// ya usa por dentro, sin perder nada) y, si no lo acepta, enteros de 16.
pub(crate) fn formatos(tasa: u32) -> [WAVEFORMATEX; 2] {
    let uno = |etiqueta: u16, bits: u16| {
        let alineado = 2 * bits / 8;
        WAVEFORMATEX {
            wFormatTag: etiqueta,
            nChannels: 2,
            nSamplesPerSec: tasa,
            nAvgBytesPerSec: tasa * u32::from(alineado),
            nBlockAlign: alineado,
            wBitsPerSample: bits,
            cbSize: 0,
        }
    };
    [uno(ETIQUETA_FLOTANTE, 32), uno(ETIQUETA_PCM, 16)]
}
