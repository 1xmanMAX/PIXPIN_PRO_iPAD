//! Las piezas de siempre para hablar con Windows desde un hilo propio: COM,
//! eventos, la prioridad de audio (MMCSS) y el reloj de rendimiento.

use std::sync::OnceLock;

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Com::{
    COINIT_MULTITHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize,
};
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows::Win32::System::Threading::{
    AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW, CreateEventW,
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
/// el antivirus), el anillo de la tarjeta se vacia a tiempo. Si MMCSS no
/// esta (servicio parado), se sigue con prioridad normal: graba igual, solo
/// que con menos margen.
pub(crate) struct Mmcss(Option<HANDLE>);

impl Mmcss {
    pub(crate) fn pro_audio() -> Mmcss {
        Mmcss::con(w!("Pro Audio"))
    }

    pub(crate) fn audio() -> Mmcss {
        Mmcss::con(w!("Audio"))
    }

    fn con(tarea: PCWSTR) -> Mmcss {
        let mut indice = 0u32;
        // SAFETY: `tarea` es una cadena estatica terminada en cero e
        // `indice` un local vivo; el HANDLE devuelto se suelta en `Drop`.
        match unsafe { AvSetMmThreadCharacteristicsW(tarea, &mut indice) } {
            Ok(h) => Mmcss(Some(h)),
            Err(e) => {
                tracing::debug!(?e, "sin prioridad de audio (MMCSS)");
                Mmcss(None)
            }
        }
    }
}

impl Drop for Mmcss {
    fn drop(&mut self) {
        if let Some(h) = self.0.take() {
            // SAFETY: el HANDLE salio de `AvSetMmThreadCharacteristicsW` en
            // este mismo hilo y no se ha soltado antes.
            unsafe {
                let _ = AvRevertMmThreadCharacteristics(h);
            }
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
