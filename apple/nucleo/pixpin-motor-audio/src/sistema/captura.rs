//! **La captura en Windows**: WASAPI en modo compartido, avisado por evento.
//!
//! Esto es solo el [`Aparato`] y la [`Plataforma`] de Windows: el bucle
//! (vaciar, silencio al paso del reloj, volver a abrir con reintentos) es
//! [`crate::plataforma::capturar`], y colocar cada paquete es
//! [`crate::toma::Toma`]. Cada aparato se abre con el formato que ya tiene la
//! mesa de Windows (`GetMixFormat`, sin pedirle conversiones) y:
//!
//! 1. [`Aparato::esperar`] espera el evento de la tarjeta, **como mucho
//!    10 ms**: el sonido del equipo no avisa mientras no suena nada (OBS
//!    hace lo mismo con el loopback: «Windows 7 does not seem to wake up»).
//! 2. [`Aparato::vaciar`] vacia **todos** los paquetes (`GetNextPacketSize`
//!    hasta cero), cada uno con su `GetBuffer`/`ReleaseBuffer` emparejados y
//!    la copia hecha entre los dos. Un paquete `AUDCLNT_BUFFERFLAGS_SILENT`
//!    es silencio aunque el buffer traiga basura; la fecha es la QPC de
//!    `GetBuffer` (`pu64QPCPosition`) salvo con
//!    `AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR`; `DATA_DISCONTINUITY` se le pasa
//!    a la toma, que cree ese hueco.
//!
//! ## Lo de cada caso
//!
//! - **El sonido del equipo** (loopback) se mantiene **despierto** con un
//!   flujo de silencio en los mismos altavoces ([`Latido`]): Windows no
//!   entrega nada del loopback mientras nadie reproduce (lo dice la
//!   documentacion de «Loopback Recording», y OBS lo combate con su
//!   `ClearBuffer`). Con el latido llegan paquetes (de silencio) sin parar y
//!   las fechas mandan; si no se puede abrir, el relleno por el reloj lo
//!   cubre igual.
//! - **Un aparato cogido en exclusiva** por otra aplicacion
//!   (`AUDCLNT_E_DEVICE_IN_USE`) se dice con su paso («otra aplicacion lo
//!   usa en exclusiva»); a mitad de grabacion, se sigue reintentando.
//! - **El tamano de paquete** que se reserva se acota
//!   ([`crate::toma::marcos_de_paquete`]): la captura por proceso dice tener
//!   un anillo de miles de millones de marcos.
//! - **El eco** ([`crate::Opciones::cancelar_eco`]): el microfono se abre
//!   como de comunicaciones y, si Windows lo tiene, con la referencia de los
//!   altavoces (lo de `examples/aec.rs` de wasapi-rs), y sin bajar el
//!   volumen de lo demas.

use windows::Win32::Foundation::E_ACCESSDENIED;
use windows::Win32::Media::Audio::{
    AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY, AUDCLNT_BUFFERFLAGS_SILENT,
    AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR, AUDCLNT_E_DEVICE_IN_USE, AUDCLNT_E_DEVICE_INVALIDATED,
    AUDCLNT_E_SERVICE_NOT_RUNNING, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM,
    AUDCLNT_STREAMFLAGS_EVENTCALLBACK, AUDCLNT_STREAMFLAGS_LOOPBACK, AUDCLNT_STREAMOPTIONS_NONE,
    AudioCategory_Communications, AudioClientProperties, EDataFlow,
    IAcousticEchoCancellationControl, IAudioCaptureClient, IAudioClient, IAudioClient2,
    IAudioRenderClient, IAudioSessionControl, IAudioSessionControl2, IMMDevice,
    IMMDeviceEnumerator, WAVEFORMATEX, eCapture, eConsole, eRender,
};
use windows::Win32::System::Com::CLSCTX_ALL;
use windows::Win32::System::Threading::WaitForSingleObject;
use windows::core::{HSTRING, Interface, PCWSTR};

use super::aparatos::{self, Vigilancia, id_de};
use super::com::{ComDelHilo, Evento, MemoriaCom, qpc_100ns};
use super::proceso;
use crate::formato::{Conversor, Formato};
use crate::opciones::{Equipo, Microfono};
use crate::plataforma::{Aparato, Origen, Pedido, Perdido, Plataforma};
use crate::toma::{Toma, marcos_de_paquete};
use crate::{ErrorMotor, en};

/// El anillo de la tarjeta: 1 s en unidades de 100 ns. Es holgura, no
/// retraso (se vacia en cuanto avisa): un tiron del sistema de hasta un
/// segundo no se come nada. OBS pide 5 s (`BUFFER_TIME_100NS`).
const ANILLO_TARJETA_100NS: i64 = 10_000_000;
/// El del flujo de silencio que mantiene despierto el loopback: 100 ms, y
/// se rellena en cada vuelta (cada 10 ms como mucho).
const ANILLO_LATIDO_100NS: i64 = 1_000_000;

/// Si `origen` sigue al aparato por defecto (y por tanto hay que vigilar sus
/// cambios).
fn sigue_al_de_defecto(origen: &Origen) -> Option<EDataFlow> {
    match origen {
        Origen::Microfono(Microfono::Defecto) => Some(eCapture),
        Origen::Equipo(Equipo::Todo) => Some(eRender),
        _ => None,
    }
}

/// **La plataforma de una fuente en Windows**: su enumerador, sus avisos y
/// el COM de su hilo.
///
/// El orden de los campos es el orden en que se sueltan: el COM, el ultimo.
pub(crate) struct PlataformaWindows {
    vigilancia: Option<Vigilancia>,
    flujo: Option<EDataFlow>,
    enumerador: IMMDeviceEnumerator,
    origen: Origen,
    canales: usize,
    eco: bool,
    /// Con un proceso pedido y conseguido: al volver a abrir se insiste en el
    /// proceso (con silencio mientras tanto). Pasar a grabar todo el equipo
    /// a mitad de una reunion seria colar justo lo que se queria dejar fuera.
    solo_proceso: bool,
    _com: ComDelHilo,
}

/// Abre la fuente de `pedido` en el hilo que llama (el de captura).
pub(crate) fn abrir_fuente(pedido: &Pedido) -> Result<(PlataformaWindows, Abierta), ErrorMotor> {
    // El primero: se suelta el ultimo, cuando ya no queda nada de COM.
    let com = ComDelHilo::nuevo();
    let enumerador = aparatos::enumerador()?;
    let canales = pedido.canales.clamp(1, 2);
    // SAFETY: COM iniciado arriba en este hilo, enumerador vivo.
    let abierta = unsafe {
        abrir(
            &enumerador,
            &pedido.origen,
            canales,
            pedido.cancelar_eco,
            false,
        )
    }?;
    let solo_proceso = abierta.por_proceso;
    let flujo = sigue_al_de_defecto(&pedido.origen).or_else(|| {
        matches!(pedido.origen, Origen::Equipo(Equipo::Proceso(_)) if !solo_proceso)
            .then_some(eRender)
    });
    let vigilancia = flujo.and_then(|f| Vigilancia::nueva(&enumerador, f));
    Ok((
        PlataformaWindows {
            vigilancia,
            flujo,
            enumerador,
            origen: pedido.origen.clone(),
            canales,
            eco: pedido.cancelar_eco,
            solo_proceso,
            _com: com,
        },
        abierta,
    ))
}

impl Plataforma for PlataformaWindows {
    type Aparato = Abierta;

    fn ahora(&self) -> u64 {
        qpc_100ns()
    }

    fn dormir(&mut self, ms: u32) {
        std::thread::sleep(std::time::Duration::from_millis(u64::from(ms)));
    }

    fn reabrir(&mut self, tasa_anillo: u32) -> Result<Abierta, ErrorMotor> {
        let _ = tasa_anillo;
        // SAFETY: COM iniciado en este hilo (lo lleva `self`), enumerador
        // vivo.
        unsafe {
            abrir(
                &self.enumerador,
                &self.origen,
                self.canales,
                self.eco,
                self.solo_proceso,
            )
        }
    }

    fn avisos(&self) -> u64 {
        self.vigilancia.as_ref().map_or(0, Vigilancia::cambios)
    }

    fn id_por_defecto(&mut self) -> Option<String> {
        let flujo = self.flujo?;
        // SAFETY: COM iniciado en este hilo, enumerador vivo. Fuera de los
        // avisos (que es donde no se puede llamar al enumerador).
        unsafe {
            por_defecto(&self.enumerador, flujo)
                .map(|a| id_de(&a))
                .filter(|id| !id.is_empty())
        }
    }
}

/// **Un flujo de silencio** en unos altavoces, para que su loopback no se
/// pare (ver el comentario del modulo). Se para al soltarlo.
struct Latido {
    cliente: IAudioClient,
    render: IAudioRenderClient,
    marcos: u32,
}

impl Latido {
    /// Lo abre en `aparato` y lo arranca lleno de silencio. `None` si no se
    /// puede (no pasa nada: el relleno por el reloj lo cubre).
    ///
    /// # Safety
    ///
    /// COM iniciado en el hilo; `aparato` vivo.
    unsafe fn abrir(aparato: &IMMDevice) -> Option<Latido> {
        // SAFETY: llamadas COM sobre objetos vivos de este hilo; el formato
        // de `GetMixFormat` vive bajo `MemoriaCom` hasta despues del
        // `Initialize` que lo lee. `GetBuffer` devuelve un puntero que no se
        // toca: se suelta con `AUDCLNT_BUFFERFLAGS_SILENT`, que le dice a
        // Windows que lo trate como ceros.
        unsafe {
            let cliente: IAudioClient = aparato.Activate(CLSCTX_ALL, None).ok()?;
            let mesa = MemoriaCom(cliente.GetMixFormat().ok()?);
            if mesa.0.is_null() {
                return None;
            }
            cliente
                .Initialize(
                    AUDCLNT_SHAREMODE_SHARED,
                    0,
                    ANILLO_LATIDO_100NS,
                    0,
                    mesa.0,
                    None,
                )
                .ok()?;
            let marcos = cliente.GetBufferSize().ok()?;
            let render: IAudioRenderClient = cliente.GetService().ok()?;
            render.GetBuffer(marcos).ok()?;
            render
                .ReleaseBuffer(marcos, AUDCLNT_BUFFERFLAGS_SILENT.0 as u32)
                .ok()?;
            cliente.Start().ok()?;
            Some(Latido {
                cliente,
                render,
                marcos,
            })
        }
    }

    /// Rellena de silencio lo que ya sono.
    ///
    /// # Safety
    ///
    /// En el hilo que lo abrio.
    unsafe fn alimentar(&self) {
        // SAFETY: cliente y render vivos y arrancados; el puntero de
        // `GetBuffer` no se toca (ver `abrir`).
        unsafe {
            let Ok(ocupados) = self.cliente.GetCurrentPadding() else {
                return;
            };
            let libres = self.marcos.saturating_sub(ocupados);
            if libres > 0 && self.render.GetBuffer(libres).is_ok() {
                let _ = self
                    .render
                    .ReleaseBuffer(libres, AUDCLNT_BUFFERFLAGS_SILENT.0 as u32);
            }
        }
    }
}

impl Drop for Latido {
    fn drop(&mut self) {
        // SAFETY: cliente vivo; parar uno ya parado o perdido es legal.
        unsafe {
            let _ = self.cliente.Stop();
        }
    }
}

/// Un aparato abierto y arrancado.
///
/// El orden de los campos es el orden en que se sueltan: primero el
/// latido y el cliente (ya parados en `Drop`), despues el evento que tenia
/// enganchado.
pub(crate) struct Abierta {
    latido: Option<Latido>,
    cliente: IAudioClient,
    lector: IAudioCaptureClient,
    evento: Evento,
    conversor: Conversor,
    /// Lo convertido de un paquete, intercalado en los canales de la salida.
    crudo: Vec<f32>,
    max: usize,
    canales: usize,
    id: String,
    por_proceso: bool,
    eco: u32,
}

impl Drop for Abierta {
    fn drop(&mut self) {
        // SAFETY: cliente vivo; parar uno ya parado (o perdido) es legal y
        // solo devuelve un error que no importa.
        unsafe {
            let _ = self.cliente.Stop();
        }
    }
}

impl Aparato for Abierta {
    fn tasa(&self) -> u32 {
        self.conversor.formato().tasa
    }

    fn max_marcos(&self) -> usize {
        self.max
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn por_proceso(&self) -> bool {
        self.por_proceso
    }

    fn eco(&self) -> u32 {
        self.eco
    }

    fn latido(&self) -> bool {
        self.latido.is_some()
    }

    fn esperar(&mut self, ms: u32) {
        // SAFETY: el evento esta enganchado al cliente vivo; el tope hace que
        // el reloj corra aunque la tarjeta no avise.
        let _ = unsafe { WaitForSingleObject(self.evento.0, ms) };
    }

    fn vaciar(&mut self, toma: &mut Toma) -> Result<(), Perdido> {
        if let Some(l) = &self.latido {
            // SAFETY: se abrio en este hilo.
            unsafe { l.alimentar() };
        }
        // SAFETY: cliente y lector vivos y arrancados en este hilo.
        unsafe { vaciar(self, toma) }.map_err(|e| Perdido {
            esperado: e.code() == AUDCLNT_E_DEVICE_INVALIDATED,
            motivo: format!("{e:?}"),
        })
    }
}

/// Abre el aparato de `origen`. `solo_proceso`: con un proceso pedido, no
/// caer a todo el sonido del equipo si no se puede.
///
/// # Safety
///
/// COM iniciado en el hilo; `enumerador` vivo.
unsafe fn abrir(
    enumerador: &IMMDeviceEnumerator,
    origen: &Origen,
    canales: usize,
    eco: bool,
    solo_proceso: bool,
) -> Result<Abierta, ErrorMotor> {
    // SAFETY: las precondiciones de esta funcion son las de las que llama.
    unsafe {
        match origen {
            Origen::Microfono(Microfono::Defecto) => {
                let a = por_defecto(enumerador, eCapture).ok_or(ErrorMotor::SinMicrofono)?;
                abrir_aparato(&a, origen, false, canales, eco)
            }
            Origen::Microfono(Microfono::Aparato(id)) => {
                let a = enumerador
                    .GetDevice(&HSTRING::from(id.as_str()))
                    .ok()
                    .or_else(|| {
                        tracing::warn!("el microfono elegido no esta; se usa el de por defecto");
                        por_defecto(enumerador, eCapture)
                    })
                    .ok_or(ErrorMotor::SinMicrofono)?;
                abrir_aparato(&a, origen, false, canales, eco)
            }
            Origen::Equipo(Equipo::Todo) => {
                let a = por_defecto(enumerador, eRender).ok_or(ErrorMotor::SinSonidoDelEquipo)?;
                abrir_aparato(&a, origen, true, canales, false)
            }
            Origen::Equipo(Equipo::Proceso(pid)) => match abrir_proceso(*pid, canales) {
                Ok(a) => Ok(a),
                Err(e) if solo_proceso => Err(e),
                Err(e) => {
                    // Windows anterior a 2004, o el proceso ya no esta: todo
                    // el sonido del equipo, mejor que nada.
                    tracing::warn!(?e, "sin sonido por proceso; se graba todo el equipo");
                    let a =
                        por_defecto(enumerador, eRender).ok_or(ErrorMotor::SinSonidoDelEquipo)?;
                    abrir_aparato(&a, origen, true, canales, false)
                }
            },
        }
    }
}

/// El aparato por defecto de `flujo` en el papel de consola.
///
/// # Safety
///
/// COM iniciado en el hilo; `enumerador` vivo.
unsafe fn por_defecto(enumerador: &IMMDeviceEnumerator, flujo: EDataFlow) -> Option<IMMDevice> {
    // SAFETY: llamada COM sobre un objeto vivo.
    unsafe { enumerador.GetDefaultAudioEndpoint(flujo, eConsole) }.ok()
}

/// El microfono como de comunicaciones, para que Windows le ponga su
/// cancelador de eco. Antes de `Initialize`. Devuelve si se pudo.
///
/// # Safety
///
/// `cliente` vivo, sin inicializar.
unsafe fn pedir_comunicaciones(cliente: &IAudioClient) -> bool {
    let propiedades = AudioClientProperties {
        cbSize: size_of::<AudioClientProperties>() as u32,
        bIsOffload: false.into(),
        eCategory: AudioCategory_Communications,
        Options: AUDCLNT_STREAMOPTIONS_NONE,
    };
    // SAFETY: `IAudioClient2` es Windows 8+; si no esta, `cast` falla y no se
    // pide nada. `propiedades` es un local vivo durante la llamada.
    unsafe {
        cliente
            .cast::<IAudioClient2>()
            .and_then(|c| c.SetClientProperties(&propiedades))
            .is_ok()
    }
}

/// Lo de despues de `Initialize` para el eco: la referencia de los
/// altavoces (Windows 11 22H2+; `NULL` deja que Windows elija la de por
/// defecto) y que abrir un flujo de comunicaciones no baje el volumen de lo
/// demas (la reunion que se esta grabando). Devuelve el nivel de
/// [`crate::medidas::Cuentas::eco`].
///
/// # Safety
///
/// `cliente` vivo e inicializado, sin arrancar.
unsafe fn ajustar_eco(cliente: &IAudioClient) -> u32 {
    // SAFETY: servicios del cliente vivo; si no estan, no se hace nada.
    unsafe {
        if let Ok(sesion) = cliente.GetService::<IAudioSessionControl>()
            && let Ok(s2) = sesion.cast::<IAudioSessionControl2>()
        {
            let _ = s2.SetDuckingPreference(true);
        }
        match cliente.GetService::<IAcousticEchoCancellationControl>() {
            Ok(aec)
                if aec
                    .SetEchoCancellationRenderEndpoint(PCWSTR::null())
                    .is_ok() =>
            {
                2
            }
            _ => 1,
        }
    }
}

/// Abre un aparato fisico con su formato de mesa.
///
/// # Safety
///
/// COM iniciado en el hilo; `aparato` vivo.
unsafe fn abrir_aparato(
    aparato: &IMMDevice,
    origen: &Origen,
    loopback: bool,
    canales: usize,
    eco: bool,
) -> Result<Abierta, ErrorMotor> {
    let falta = origen.falta();
    // SAFETY: llamadas COM sobre objetos vivos de este hilo; el unico
    // puntero crudo es el formato de `GetMixFormat`, bajo `MemoriaCom`, que
    // vive hasta despues del `Initialize` que lo lee.
    unsafe {
        let cliente: IAudioClient = aparato.Activate(CLSCTX_ALL, None).map_err(|e| {
            tracing::warn!(?e, "no se pudo activar el aparato de sonido");
            falta.clone()
        })?;
        let comunicaciones = eco && !loopback && pedir_comunicaciones(&cliente);
        let mesa = MemoriaCom(
            cliente
                .GetMixFormat()
                .map_err(en("preguntar el formato de la tarjeta"))?,
        );
        if mesa.0.is_null() {
            return Err(falta);
        }
        let formato = leer_formato(mesa.0)?;
        let banderas = if loopback {
            AUDCLNT_STREAMFLAGS_EVENTCALLBACK | AUDCLNT_STREAMFLAGS_LOOPBACK
        } else {
            AUDCLNT_STREAMFLAGS_EVENTCALLBACK
        };
        cliente
            .Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                banderas,
                ANILLO_TARJETA_100NS,
                0,
                mesa.0,
                None,
            )
            .map_err(|e| de_initialize(e, &falta, loopback))?;
        let eco = if comunicaciones {
            ajustar_eco(&cliente)
        } else {
            0
        };
        let latido = if loopback {
            let l = Latido::abrir(aparato);
            if l.is_none() {
                tracing::debug!("sin latido en los altavoces; el silencio lo pone el reloj");
            }
            l
        } else {
            None
        };
        let mut a = terminar_de_abrir(cliente, formato, canales, false, id_de(aparato))?;
        a.eco = eco;
        a.latido = latido;
        Ok(a)
    }
}

/// Lo que se le dice al usuario de un `Initialize` que fallo.
fn de_initialize(e: windows::core::Error, falta: &ErrorMotor, loopback: bool) -> ErrorMotor {
    let codigo = e.code();
    if codigo == E_ACCESSDENIED {
        // Sin permiso de microfono en los ajustes de privacidad de Windows:
        // es lo mismo que no tener microfono, y el mensaje de `SinMicrofono`
        // ya lo dice.
        return falta.clone();
    }
    let paso = if codigo == AUDCLNT_E_DEVICE_IN_USE {
        if loopback {
            "abrir el sonido del equipo: otra aplicacion usa los altavoces en exclusiva"
        } else {
            "abrir el microfono: otra aplicacion lo usa en exclusiva"
        }
    } else if codigo == AUDCLNT_E_SERVICE_NOT_RUNNING {
        "abrir el sonido: el servicio de audio de Windows no esta en marcha"
    } else if loopback {
        "abrir el sonido del equipo"
    } else {
        "abrir el microfono"
    };
    en(paso)(e)
}

/// Abre la captura de un proceso, probando los formatos de
/// [`proceso::formatos`]. Este cliente no tiene formato de mesa
/// (`GetMixFormat` da `E_NOTIMPL`): hay que darle uno, y Windows convierte.
///
/// # Safety
///
/// COM (multihilo) iniciado en el hilo.
unsafe fn abrir_proceso(pid: u32, canales: usize) -> Result<Abierta, ErrorMotor> {
    let mut ultimo = ErrorMotor::SinSonidoDelEquipo;
    for f in proceso::formatos(48_000) {
        // SAFETY: COM multihilo iniciado (precondicion de esta funcion); `f`
        // es un local vivo durante el `Initialize`, que lo copia.
        unsafe {
            let cliente = proceso::cliente(pid)?;
            let formato = leer_formato(&f)?;
            let banderas = AUDCLNT_STREAMFLAGS_LOOPBACK
                | AUDCLNT_STREAMFLAGS_EVENTCALLBACK
                | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM;
            match cliente.Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                banderas,
                ANILLO_TARJETA_100NS,
                0,
                &f,
                None,
            ) {
                Ok(()) => {
                    return terminar_de_abrir(cliente, formato, canales, true, String::new());
                }
                Err(e) => ultimo = en("abrir el sonido del proceso")(e),
            }
        }
    }
    Err(ultimo)
}

/// Lee un `WAVEFORMATEX*` (o su forma extensible) a traves de sus bytes.
///
/// # Safety
///
/// `p` apunta a una cabecera valida de `18 + cbSize` bytes.
unsafe fn leer_formato(p: *const WAVEFORMATEX) -> Result<Formato, ErrorMotor> {
    // SAFETY: los 18 bytes de la cabecera estan siempre; `cbSize` (bytes 16
    // y 17) dice cuantos siguen, y la precondicion garantiza que estan. Se
    // lee como bytes porque la estructura esta empaquetada y no alineada.
    unsafe {
        let cabecera = std::slice::from_raw_parts(p as *const u8, 18);
        let extra = u16::from_le_bytes([cabecera[16], cabecera[17]]) as usize;
        let todo = std::slice::from_raw_parts(p as *const u8, 18 + extra);
        Ok(Formato::de_bytes(todo)?)
    }
}

/// Lo comun despues de `Initialize`: el evento, el lector, los buffers y el
/// `Start`.
///
/// # Safety
///
/// `cliente` inicializado en este hilo con `formato` y
/// `AUDCLNT_STREAMFLAGS_EVENTCALLBACK`.
unsafe fn terminar_de_abrir(
    cliente: IAudioClient,
    formato: Formato,
    canales: usize,
    por_proceso: bool,
    id: String,
) -> Result<Abierta, ErrorMotor> {
    let evento = Evento::nuevo()?;
    // SAFETY: cliente inicializado (precondicion). El evento vive en la
    // `Abierta` despues del cliente (ver el orden de sus campos).
    unsafe {
        cliente
            .SetEventHandle(evento.0)
            .map_err(en("enganchar el aviso de la tarjeta"))?;
        let lector: IAudioCaptureClient = cliente
            .GetService()
            .map_err(en("pedir el lector de la tarjeta"))?;
        let max = marcos_de_paquete(cliente.GetBufferSize().ok(), formato.tasa);
        let abierta = Abierta {
            latido: None,
            conversor: Conversor::nuevo(formato, canales as u16),
            crudo: vec![0.0; max * canales],
            max,
            canales,
            id,
            por_proceso,
            eco: 0,
            lector,
            evento,
            cliente,
        };
        abierta
            .cliente
            .Start()
            .map_err(en("arrancar la tarjeta de sonido"))?;
        Ok(abierta)
    }
}

/// Vacia todos los paquetes que haya en la tarjeta.
///
/// # Safety
///
/// Cliente y lector de `a` vivos y arrancados en este hilo.
unsafe fn vaciar(a: &mut Abierta, toma: &mut Toma) -> windows::core::Result<()> {
    let c = a.canales;
    let por_marco = a.conversor.formato().bytes_por_marco();
    loop {
        // SAFETY: consulta sobre el lector vivo.
        let siguiente = unsafe { a.lector.GetNextPacketSize() }?;
        if siguiente == 0 {
            return Ok(());
        }
        let mut datos: *mut u8 = std::ptr::null_mut();
        let mut marcos = 0u32;
        let mut banderas = 0u32;
        let mut qpc = 0u64;
        // SAFETY: los punteros son a locales vivos. Desde aqui `datos` vale
        // `marcos * por_marco` bytes hasta el `ReleaseBuffer` de abajo, que se
        // hace siempre, con los mismos marcos, antes de la vuelta siguiente.
        unsafe {
            a.lector.GetBuffer(
                &mut datos,
                &mut marcos,
                &mut banderas,
                None,
                Some(&raw mut qpc),
            )
        }?;
        if marcos == 0 {
            // `AUDCLNT_S_BUFFER_EMPTY`: no hay paquete que soltar.
            return Ok(());
        }
        let n = marcos as usize;
        if a.crudo.len() < n * c {
            // Un paquete mas grande de lo reservado: fuera de lo normal, se
            // hace sitio.
            a.crudo.resize(n * c, 0.0);
        }
        let silencio = banderas & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || datos.is_null();
        if silencio {
            a.crudo[..n * c].fill(0.0);
        } else {
            // SAFETY: `GetBuffer` garantiza `marcos * por_marco` bytes
            // validos en `datos` hasta el `ReleaseBuffer`; se leen como bytes
            // (sin suponer alineacion) y no se guardan.
            let bytes = unsafe { std::slice::from_raw_parts(datos, n * por_marco) };
            a.conversor.convertir(bytes, &mut a.crudo[..n * c]);
        }
        // SAFETY: devuelve exactamente los marcos que entrego `GetBuffer`. A
        // partir de aqui `datos` no vale y no se vuelve a usar.
        let soltado = unsafe { a.lector.ReleaseBuffer(marcos) };
        let fecha = (banderas & AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR.0 as u32 == 0).then_some(qpc);
        let discontinuidad = banderas & AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY.0 as u32 != 0;
        toma.paquete(&a.crudo[..n * c], fecha, qpc_100ns(), discontinuidad);
        soltado?;
    }
}
