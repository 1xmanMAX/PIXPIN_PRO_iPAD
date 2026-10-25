//! Las piezas de siempre para hablar con Windows desde un hilo propio: COM,
//! eventos, la prioridad de audio (MMCSS) y el reloj de rendimiento.

use std::sync::OnceLock;

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Com::{
    COINIT_MULTITHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize,
};
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows::Win32::System::Threading::{
    AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW, CreateEventW, GetCurrentThread,
    SetThreadPriority, THREAD_PRIORITY, THREAD_PRIORITY_HIGHEST, THREAD_PRIORITY_NORMAL,
    THREAD_PRIORITY_TIME_CRITICAL,
};
use windows::core::{PCWSTR, w};

use crate::{ErrorMotor, en};

/// COM inicializado en este hilo (apartamento **multihilo**, que es el que
/// pide WASAPI para no serializar las llamadas por una bomba de mensajes) y
/// soltado al salir **solo si la cuenta subio**.
///
/// `CoInitializeEx` devuelve `S_OK` o `S_FALSE` (ya estaba en el MTA) y en
/// los dos casos sube la cuenta, que hay que bajar; con `RPC_E_CHANGED_MODE`
/// (el hilo ya estaba en otro apartamento) no la sube, y llamar a
/// `CoUninitialize` cerraria el COM de otro.
pub(crate) struct ComDelHilo {
    nuestro: bool,
}

impl ComDelHilo {
    pub(crate) fn nuevo() -> ComDelHilo {
        // SAFETY: entrar en COM no tiene precondiciones; se guarda si la
        // cuenta subio para deshacerlo con exactitud en `Drop`.
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        ComDelHilo {
            nuestro: hr.is_ok(),
        }
    }
}

impl Drop for ComDelHilo {
    fn drop(&mut self) {
        if self.nuestro {
            // SAFETY: empareja el `CoInitializeEx` de arriba. El guardia se
            // declara el primero en cada hilo, asi que se destruye el
            // ultimo, cuando ya no queda vivo ningun objeto COM de aqui.
            unsafe { CoUninitialize() };
        }
    }
}

/// Un evento de reinicio automatico que se cierra solo.
pub(crate) struct Evento(pub(crate) HANDLE);

// SAFETY: un HANDLE de evento es un numero del nucleo valido en cualquier
// hilo del proceso; esperar o senalar desde otro hilo es su uso normal.
unsafe impl Send for Evento {}
// SAFETY: igual que arriba; `SetEvent` y las esperas son seguras a la vez.
unsafe impl Sync for Evento {}

impl Evento {
    pub(crate) fn nuevo() -> Result<Evento, ErrorMotor> {
        // SAFETY: evento sin nombre ni descriptor de seguridad, de reinicio
        // automatico y sin senalar. El HANDLE es nuestro y se cierra en
        // `Drop`.
        let h = unsafe { CreateEventW(None, false, false, PCWSTR::null()) }
            .map_err(en("crear un evento"))?;
        Ok(Evento(h))
    }
}

impl Drop for Evento {
    fn drop(&mut self) {
        // SAFETY: el HANDLE lo creamos nosotros y no se cerro antes; quien lo
        // usaba (el `IAudioClient`) ya esta parado y suelto para cuando esto
        // corre: en las estructuras que los guardan, el cliente va declarado
        // antes que el evento, y Rust suelta los campos en ese orden.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// **Prioridad de audio** para este hilo (el servicio MMCSS de Windows), y
/// de vuelta a la normal al salir.
///
/// Con «Pro Audio» el planificador le da al hilo de captura prioridad casi
/// de tiempo real: aunque otra aplicacion se coma la CPU (un i3 compilando,
/// el antivirus), el anillo de la tarjeta se vacia a tiempo.
///
/// Si MMCSS no esta (el servicio «Programador de clases multimedia»
/// parado, o una version recortada de Windows), se sube la prioridad del
/// hilo a mano, como hace cpal (`boost_current_thread_priority` en
/// `src/host/wasapi/stream.rs`): `THREAD_PRIORITY_TIME_CRITICAL` para la
/// captura, `THREAD_PRIORITY_HIGHEST` para la mezcla. Y si tampoco, se sigue
/// con la normal: graba igual, con menos margen.
pub(crate) enum Mmcss {
    Tarea(HANDLE),
    Hilo,
    Nada,
}

impl Mmcss {
    pub(crate) fn pro_audio() -> Mmcss {
        Mmcss::con(w!("Pro Audio"), THREAD_PRIORITY_TIME_CRITICAL)
    }

    pub(crate) fn audio() -> Mmcss {
        Mmcss::con(w!("Audio"), THREAD_PRIORITY_HIGHEST)
    }

    fn con(tarea: PCWSTR, respaldo: THREAD_PRIORITY) -> Mmcss {
        let mut indice = 0u32;
        // SAFETY: `tarea` es una cadena estatica terminada en cero e
        // `indice` un local vivo; el HANDLE devuelto se suelta en `Drop`.
        match unsafe { AvSetMmThreadCharacteristicsW(tarea, &mut indice) } {
            Ok(h) => Mmcss::Tarea(h),
            Err(e) => {
                // SAFETY: el pseudo-HANDLE del hilo actual no se cierra; subir
                // la prioridad propia no necesita permisos especiales.
                let hilo = unsafe { SetThreadPriority(GetCurrentThread(), respaldo) };
                tracing::debug!(?e, a_mano = hilo.is_ok(), "sin prioridad de audio (MMCSS)");
                if hilo.is_ok() {
                    Mmcss::Hilo
                } else {
                    Mmcss::Nada
                }
            }
        }
    }
}

impl Drop for Mmcss {
    fn drop(&mut self) {
        match std::mem::replace(self, Mmcss::Nada) {
            // SAFETY: el HANDLE salio de `AvSetMmThreadCharacteristicsW` en
            // este mismo hilo y no se ha soltado antes.
            Mmcss::Tarea(h) => unsafe {
                let _ = AvRevertMmThreadCharacteristics(h);
            },
            // SAFETY: este guardia vive y muere en el hilo que subio.
            Mmcss::Hilo => unsafe {
                let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_NORMAL);
            },
            Mmcss::Nada => {}
        }
    }
}

/// Memoria reservada por COM (`GetMixFormat`, `GetId`) que hay que soltar con
/// `CoTaskMemFree` y no con el asignador de Rust.
pub(crate) struct MemoriaCom<T>(pub(crate) *mut T);

impl<T> Drop for MemoriaCom<T> {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: el puntero salio del asignador de tareas de COM, este
            // guardia es su unico dueno y no se ha liberado antes.
            unsafe { CoTaskMemFree(Some(self.0 as *const _)) };
        }
    }
}

/// El reloj de rendimiento en unidades de 100 ns: el mismo con el que WASAPI
/// fecha cada paquete (`pu64QPCPosition`).
pub(crate) fn qpc_100ns() -> u64 {
    static FRECUENCIA: OnceLock<u64> = OnceLock::new();
    let f = *FRECUENCIA.get_or_init(|| {
        let mut f = 0i64;
        // SAFETY: escribe en un local vivo. Desde Windows XP no falla.
        let _ = unsafe { QueryPerformanceFrequency(&mut f) };
        f.max(1) as u64
    });
    let mut c = 0i64;
    // SAFETY: escribe en un local vivo.
    let _ = unsafe { QueryPerformanceCounter(&mut c) };
    (u128::from(c.max(0) as u64) * 10_000_000 / u128::from(f)) as u64
}
