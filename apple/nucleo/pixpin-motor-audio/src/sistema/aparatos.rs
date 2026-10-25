//! **Los aparatos**: la lista de microfonos y el aviso de que el de por
//! defecto cambio.
//!
//! Windows avisa por `IMMNotificationClient` desde un hilo suyo, y la
//! documentacion es clara: en esos avisos no se puede esperar ni volver a
//! llamar al enumerador. Aqui solo se sube un contador atomico; el hilo de
//! captura lo mira en cada vuelta (cada 10 ms como mucho) y es el quien
//! cierra y vuelve a abrir.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::Media::Audio::{
    DEVICE_STATE, DEVICE_STATE_ACTIVE, EDataFlow, ERole, IMMDevice, IMMDeviceEnumerator,
    IMMNotificationClient, IMMNotificationClient_Impl, MMDeviceEnumerator, eCapture, eConsole,
};
use windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PropVariantToStringAlloc};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, STGM_READ};
use windows::core::{PCWSTR, PWSTR, implement};

use super::com::{ComDelHilo, MemoriaCom};
use crate::{ErrorMotor, en};

/// El enumerador de aparatos de este hilo (COM ya iniciado).
pub(crate) fn enumerador() -> Result<IMMDeviceEnumerator, ErrorMotor> {
    // SAFETY: crear el enumerador no tiene precondiciones mas alla de COM
    // iniciado en el hilo, que es lo primero que hace cada hilo del motor.
    unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER) }
        .map_err(en("abrir la lista de aparatos de sonido"))
}

/// Un `PWSTR` de COM pasado a `String`, soltando su memoria.
///
/// # Safety
///
/// `p` tiene que venir del asignador de tareas de COM y no usarse despues.
unsafe fn cadena_com(p: PWSTR) -> String {
    let _suelta = MemoriaCom(p.0);
    if p.is_null() {
        return String::new();
    }
    // SAFETY: `p` es una cadena ancha terminada en cero que vive hasta que
    // `_suelta` la libere, al salir de aqui.
    unsafe { p.to_string() }.unwrap_or_default()
}

/// El identificador de Windows de un aparato.
///
/// # Safety
///
/// COM iniciado en el hilo; `aparato` vivo.
pub(crate) unsafe fn id_de(aparato: &IMMDevice) -> String {
    // SAFETY: `GetId` reserva la cadena con el asignador de COM y
    // `cadena_com` la suelta.
    unsafe {
        match aparato.GetId() {
            Ok(p) => cadena_com(p),
            Err(_) => String::new(),
        }
    }
}

/// El aviso de Windows: solo cuenta cambios del aparato por defecto del
/// flujo que vigila, en el papel de consola (el que usa el motor).
#[implement(IMMNotificationClient)]
struct Avisos {
    flujo: EDataFlow,
    cambios: Arc<AtomicU64>,
}

impl IMMNotificationClient_Impl for Avisos_Impl {
    fn OnDeviceStateChanged(&self, _: &PCWSTR, _: DEVICE_STATE) -> windows::core::Result<()> {
        Ok(())
    }

    fn OnDeviceAdded(&self, _: &PCWSTR) -> windows::core::Result<()> {
        Ok(())
    }

    fn OnDeviceRemoved(&self, _: &PCWSTR) -> windows::core::Result<()> {
        Ok(())
    }

    fn OnDefaultDeviceChanged(
        &self,
        flujo: EDataFlow,
        rol: ERole,
        _: &PCWSTR,
    ) -> windows::core::Result<()> {
        if flujo == self.flujo && rol == eConsole {
            self.cambios.fetch_add(1, Ordering::Release);
        }
        Ok(())
    }

    fn OnPropertyValueChanged(&self, _: &PCWSTR, _: &PROPERTYKEY) -> windows::core::Result<()> {
        Ok(())
    }
}

/// Un aviso registrado, que se da de baja solo.
pub(crate) struct Vigilancia {
    enumerador: IMMDeviceEnumerator,
    cliente: IMMNotificationClient,
    cambios: Arc<AtomicU64>,
}

impl Vigilancia {
    /// Empieza a vigilar el aparato por defecto de `flujo`. Si Windows no
    /// deja, se sigue sin avisos: un aparato que se va se nota igual, por el
    /// error de la captura.
    pub(crate) fn nueva(enumerador: &IMMDeviceEnumerator, flujo: EDataFlow) -> Option<Vigilancia> {
        let cambios = Arc::new(AtomicU64::new(0));
        let cliente: IMMNotificationClient = Avisos {
            flujo,
            cambios: Arc::clone(&cambios),
        }
        .into();
        // SAFETY: enumerador vivo; el cliente queda guardado en la
        // `Vigilancia` mientras este registrado, y se da de baja en `Drop`.
        match unsafe { enumerador.RegisterEndpointNotificationCallback(&cliente) } {
            Ok(()) => Some(Vigilancia {
                enumerador: enumerador.clone(),
                cliente,
                cambios,
            }),
            Err(e) => {
                tracing::warn!(?e, "sin avisos de cambio de aparato");
                None
            }
        }
    }

    /// Cuantos cambios van.
    pub(crate) fn cambios(&self) -> u64 {
        self.cambios.load(Ordering::Acquire)
    }
}

impl Drop for Vigilancia {
    fn drop(&mut self) {
        // SAFETY: el mismo enumerador y el mismo cliente que se registraron;
        // no se esta dentro de un aviso (eso lo prohibe la documentacion).
        unsafe {
            let _ = self
                .enumerador
                .UnregisterEndpointNotificationCallback(&self.cliente);
        }
    }
}

/// Un microfono que se puede elegir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aparato {
    /// Lo que se pasa en `Microfono::Aparato`.
    pub id: String,
    /// Como lo llama Windows («Microfono (Realtek High Definition Audio)»).
    pub nombre: String,
    /// Si es el de por defecto ahora.
    pub defecto: bool,
}

/// **Los microfonos activos**, con su nombre. En un hilo propio, para no
/// depender del apartamento COM del que llama (la interfaz suele estar en uno
/// de un solo hilo).
pub fn microfonos() -> Result<Vec<Aparato>, ErrorMotor> {
    std::thread::Builder::new()
        .name("pixpin-aparatos".into())
        .spawn(|| {
            let _com = ComDelHilo::nuevo();
            listar()
        })
        .map_err(|_| ErrorMotor::HiloCaido)?
        .join()
        .map_err(|_| ErrorMotor::HiloCaido)?
}

fn listar() -> Result<Vec<Aparato>, ErrorMotor> {
    let enumerador = enumerador()?;
    // SAFETY: llamadas COM sobre objetos recien creados en este hilo. El
    // `PROPVARIANT` del nombre se suelta con `PropVariantClear` y las cadenas
    // con el asignador de COM (`cadena_com`).
    unsafe {
        let defecto = enumerador
            .GetDefaultAudioEndpoint(eCapture, eConsole)
            .map(|a| id_de(&a))
            .unwrap_or_default();
        let lista = enumerador
            .EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE)
            .map_err(en("listar los microfonos"))?;
        let n = lista.GetCount().map_err(en("contar los microfonos"))?;
        let mut aparatos = Vec::with_capacity(n as usize);
        for i in 0..n {
            let Ok(aparato) = lista.Item(i) else {
                continue;
            };
            let id = id_de(&aparato);
            let nombre = aparato
                .OpenPropertyStore(STGM_READ)
                .and_then(|p| p.GetValue(&PKEY_Device_FriendlyName))
                .map(|mut v| {
                    let s = PropVariantToStringAlloc(&v)
                        .map(|p| cadena_com(p))
                        .unwrap_or_default();
                    let _ = PropVariantClear(&mut v);
                    s
                })
                .unwrap_or_default();
            aparatos.push(Aparato {
                defecto: id == defecto,
                nombre: if nombre.is_empty() {
                    id.clone()
                } else {
                    nombre
                },
                id,
            });
        }
        Ok(aparatos)
    }
}
