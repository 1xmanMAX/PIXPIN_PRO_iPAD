//! **Un hilo por fuente**: WASAPI en modo compartido, avisado por evento.
//!
//! Cada hilo abre su aparato con el formato que ya tiene la mesa de Windows
//! (`GetMixFormat`, sin pedirle conversiones), le pone prioridad de audio
//! (MMCSS «Pro Audio») y hace siempre lo mismo:
//!
//! 1. Espera el evento de la tarjeta, **como mucho 10 ms**: el sonido del
//!    equipo no avisa mientras no suena nada, y el reloj tiene que seguir.
//! 2. Vacia **todos** los paquetes que haya (`GetNextPacketSize` hasta cero),
//!    cada uno con su `GetBuffer`/`ReleaseBuffer` emparejados y la copia
//!    hecha entre los dos. Un paquete marcado `AUDCLNT_BUFFERFLAGS_SILENT` es
//!    silencio aunque el buffer traiga basura.
//! 3. Convierte a coma flotante en los canales de la salida, coloca cada
//!    paquete por su fecha ([`crate::reloj::Relleno`]) y lo deja en el anillo.
//! 4. Si no llega nada, pone silencio al paso del reloj.
//!
//! **Si el aparato se va** (`AUDCLNT_E_DEVICE_INVALIDATED`: se desenchufo, o
//! Windows cambio de controlador) o el usuario cambia el de por defecto, el
//! hilo cierra, sigue poniendo silencio por el reloj y vuelve a abrir cada
//! medio segundo hasta que puede. El anillo conserva su tasa para toda la
//! grabacion: si el aparato nuevo va a otra, se remuestrea aqui antes de
//! escribir.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{SyncSender, sync_channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::E_ACCESSDENIED;
use windows::Win32::Media::Audio::{
    AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY, AUDCLNT_BUFFERFLAGS_SILENT,
    AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR, AUDCLNT_E_DEVICE_INVALIDATED, AUDCLNT_SHAREMODE_SHARED,
    AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM, AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
    AUDCLNT_STREAMFLAGS_LOOPBACK, EDataFlow, IAudioCaptureClient, IAudioClient, IMMDevice,
    IMMDeviceEnumerator, WAVEFORMATEX, eCapture, eConsole, eRender,
};
use windows::Win32::System::Com::CLSCTX_ALL;
use windows::Win32::System::Threading::WaitForSingleObject;
use windows::core::HSTRING;

use super::aparatos::{self, Vigilancia};
use super::com::{ComDelHilo, Evento, MemoriaCom, Mmcss, qpc_100ns};
use super::proceso;
use crate::anillo::{Consumidor, Productor, anillo};
use crate::formato::{Conversor, Formato};
use crate::medidas::Cuentas;
use crate::nucleo::Compartido;
use crate::opciones::{Equipo, Microfono};
use crate::reloj::{Relleno, duracion_de};
use crate::remuestreo::Remuestreador;
use crate::{ErrorMotor, en};

/// El anillo de la tarjeta: 200 ms en unidades de 100 ns. Es holgura, no
/// retraso: se vacia en cuanto avisa. Con menos, un tiron del sistema se
/// come un trozo; con mas, no se gana nada.
const ANILLO_TARJETA_100NS: i64 = 2_000_000;
/// Lo mas que se espera al evento de la tarjeta.
const ESPERA_MS: u32 = 10;
/// Cada cuanto se intenta volver a abrir un aparato que se fue.
const REINTENTO: Duration = Duration::from_millis(500);
/// Segundos que caben en el anillo hacia la mezcla: de sobra para que un
/// atasco del hilo de mezcla no pierda nada.
const SEGUNDOS_DE_ANILLO: usize = 2;

/// De donde lee un hilo.
#[derive(Debug, Clone)]
pub(crate) enum Origen {
    Microfono(Microfono),
    Equipo(Equipo),
}

impl Origen {
    fn falta(&self) -> ErrorMotor {
        match self {
            Origen::Microfono(_) => ErrorMotor::SinMicrofono,
            Origen::Equipo(_) => ErrorMotor::SinSonidoDelEquipo,
        }
    }

    /// Si sigue al aparato por defecto (y por tanto hay que vigilar sus
    /// cambios).
    fn sigue_al_de_defecto(&self) -> Option<EDataFlow> {
        match self {
            Origen::Microfono(Microfono::Defecto) => Some(eCapture),
            Origen::Equipo(Equipo::Todo) => Some(eRender),
            _ => None,
        }
    }
}

/// Lo que devuelve un hilo que arranco bien.
pub(crate) struct Arranque {
    pub(crate) consumidor: Consumidor,
    pub(crate) tasa: u32,
    pub(crate) por_proceso: bool,
}

/// Un aparato abierto y arrancado.
///
/// El orden de los campos es el orden en que se sueltan: primero el
/// cliente (ya parado en `Drop`), despues el evento que tenia enganchado.
struct Abierta {
    cliente: IAudioClient,
    lector: IAudioCaptureClient,
    evento: Evento,
    conversor: Conversor,
    /// Lo convertido de un paquete, intercalado en los canales de la salida.
    crudo: Vec<f32>,
    /// Si el aparato no va a la tasa del anillo (cambio de aparato a mitad).
    a_anillo: Option<Remuestreador>,
    remuestreado: Vec<f32>,
    por_proceso: bool,
}

impl Abierta {
    fn tasa(&self) -> u32 {
        self.conversor.formato().tasa
    }
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

/// Arranca el hilo de `origen` y espera a que el aparato este abierto de
/// verdad: que no se pueda grabar (no hay microfono, falta el permiso) se
/// sabe aqui, no a mitad.
pub(crate) fn lanzar(
    origen: Origen,
    canales: u16,
    parar: Arc<AtomicBool>,
    compartido: Arc<Compartido>,
) -> Result<(JoinHandle<()>, Arranque), ErrorMotor> {
    let (avisar, arranque) = sync_channel(1);
    let nombre = match origen {
        Origen::Microfono(_) => "pixpin-microfono",
        Origen::Equipo(_) => "pixpin-equipo",
    };
    let hilo = std::thread::Builder::new()
        .name(nombre.into())
        .spawn(move || {
            hilo(
                origen,
                canales.clamp(1, 2) as usize,
                &parar,
                &compartido,
                avisar,
            )
        })
        .map_err(|_| ErrorMotor::HiloCaido)?;
    match arranque.recv() {
        Ok(Ok(a)) => Ok((hilo, a)),
        Ok(Err(e)) => {
            let _ = hilo.join();
            Err(e)
        }
        Err(_) => {
            let _ = hilo.join();
            Err(ErrorMotor::HiloCaido)
        }
    }
}

/// Lo que el hilo lleva de una vuelta a otra.
struct Estado<'a> {
    productor: Productor,
    relleno: Relleno,
    cuentas: &'a Cuentas,
    canales: usize,
}

fn hilo(
    origen: Origen,
    canales: usize,
    parar: &AtomicBool,
    compartido: &Compartido,
    avisar: SyncSender<Result<Arranque, ErrorMotor>>,
) {
    // El primero: se suelta el ultimo, cuando ya no queda nada de COM.
    let _com = ComDelHilo::nuevo();
    let _prioridad = Mmcss::pro_audio();
    let enumerador = match aparatos::enumerador() {
        Ok(e) => e,
        Err(e) => {
            let _ = avisar.send(Err(e));
            return;
        }
    };
    // SAFETY: COM iniciado arriba en este hilo, enumerador vivo.
    let primera = unsafe { abrir(&enumerador, &origen, canales, None, false) };
    let abierta = match primera {
        Ok(a) => a,
        Err(e) => {
            let _ = avisar.send(Err(e));
            return;
        }
    };
    // Si se pidio un proceso y se pudo, al volver a abrir se insiste en el
    // proceso (con silencio mientras tanto): pasar a grabar todo el equipo a
    // mitad de una reunion seria colar justo lo que se queria dejar fuera.
    let solo_proceso = abierta.por_proceso;
    let flujo = origen.sigue_al_de_defecto().or_else(|| {
        matches!(origen, Origen::Equipo(Equipo::Proceso(_)) if !solo_proceso).then_some(eRender)
    });
    let vigilancia = flujo.and_then(|f| Vigilancia::nueva(&enumerador, f));
    let tasa = abierta.tasa();
    let (productor, consumidor) = anillo(tasa as usize * SEGUNDOS_DE_ANILLO, canales);
    let cuentas = &compartido.cuentas;
    cuentas.tasa_aparato.store(tasa, Ordering::Relaxed);
    let _ = avisar.send(Ok(Arranque {
        consumidor,
        tasa,
        por_proceso: abierta.por_proceso,
    }));
    drop(avisar);

    let mut estado = Estado {
        productor,
        relleno: Relleno::nuevo(tasa, qpc_100ns()),
        cuentas,
        canales,
    };
    let mut abierta = Some(abierta);
    let mut visto = vigilancia.as_ref().map_or(0, Vigilancia::cambios);
    let mut reintentar = Instant::now();

    while !parar.load(Ordering::Acquire) {
        let perdida = match abierta.as_mut() {
            Some(a) => {
                // SAFETY: el evento esta enganchado al cliente vivo; el tope
                // hace que el reloj corra aunque la tarjeta no avise.
                let _ = unsafe { WaitForSingleObject(a.evento.0, ESPERA_MS) };
                // SAFETY: cliente y lector vivos y arrancados.
                match unsafe { vaciar(a, &mut estado) } {
                    Ok(()) => false,
                    Err(e) => {
                        if e.code() == AUDCLNT_E_DEVICE_INVALIDATED {
                            tracing::info!("el aparato de sonido se fue; se volvera a abrir");
                        } else {
                            tracing::warn!(?e, "fallo leyendo la tarjeta; se volvera a abrir");
                        }
                        true
                    }
                }
            }
            None => {
                std::thread::sleep(Duration::from_millis(u64::from(ESPERA_MS)));
                if Instant::now() >= reintentar {
                    reintentar = Instant::now() + REINTENTO;
                    // SAFETY: COM iniciado en este hilo, enumerador vivo.
                    let otra =
                        unsafe { abrir(&enumerador, &origen, canales, Some(tasa), solo_proceso) };
                    match otra {
                        Ok(a) => {
                            Cuentas::sumar(&cuentas.reaperturas, 1);
                            cuentas.tasa_aparato.store(a.tasa(), Ordering::Relaxed);
                            abierta = Some(a);
                        }
                        Err(e) => tracing::debug!(?e, "aun no se puede volver a abrir"),
                    }
                }
                false
            }
        };
        if perdida {
            abierta = None;
            // Se intenta en la siguiente vuelta, sin esperar el medio segundo:
            // lo normal es que Windows ya tenga otro aparato por defecto.
            reintentar = Instant::now();
        }
        if let Some(v) = &vigilancia {
            let c = v.cambios();
            if c != visto {
                visto = c;
                tracing::info!("cambio el aparato de sonido por defecto; se pasa al nuevo");
                abierta = None;
                reintentar = Instant::now();
            }
        }
        // Lo que no llego, por el reloj.
        let n = estado.relleno.espera(qpc_100ns());
        if n > 0 {
            let puestos = estado.productor.escribir_ceros(n as usize);
            Cuentas::sumar(&cuentas.silencio, puestos as u64);
            Cuentas::sumar(&cuentas.perdidos, n - puestos as u64);
        }
    }
    // `abierta` se suelta aqui (Stop), antes que la vigilancia y el COM.
    drop(abierta);
}

/// Abre el aparato de `origen`. `tasa_anillo`: la del anillo, si ya existe
/// (al volver a abrir). `solo_proceso`: con un proceso pedido, no caer a todo
/// el sonido del equipo si no se puede.
///
/// # Safety
///
/// COM iniciado en el hilo; `enumerador` vivo.
unsafe fn abrir(
    enumerador: &IMMDeviceEnumerator,
    origen: &Origen,
    canales: usize,
    tasa_anillo: Option<u32>,
    solo_proceso: bool,
) -> Result<Abierta, ErrorMotor> {
    // SAFETY: las precondiciones de esta funcion son las de las que llama.
    unsafe {
        match origen {
            Origen::Microfono(Microfono::Defecto) => {
                let a = por_defecto(enumerador, eCapture).ok_or(ErrorMotor::SinMicrofono)?;
                abrir_aparato(&a, origen, false, canales, tasa_anillo)
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
                abrir_aparato(&a, origen, false, canales, tasa_anillo)
            }
            Origen::Equipo(Equipo::Todo) => {
                let a = por_defecto(enumerador, eRender).ok_or(ErrorMotor::SinSonidoDelEquipo)?;
                abrir_aparato(&a, origen, true, canales, tasa_anillo)
            }
            Origen::Equipo(Equipo::Proceso(pid)) => {
                match abrir_proceso(*pid, canales, tasa_anillo) {
                    Ok(a) => Ok(a),
                    Err(e) if solo_proceso => Err(e),
                    Err(e) => {
                        // Windows anterior a 2004, o el proceso ya no esta:
                        // todo el sonido del equipo, mejor que nada.
                        tracing::warn!(?e, "sin sonido por proceso; se graba todo el equipo");
                        let a = por_defecto(enumerador, eRender)
                            .ok_or(ErrorMotor::SinSonidoDelEquipo)?;
                        abrir_aparato(&a, origen, true, canales, tasa_anillo)
                    }
                }
            }
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
    tasa_anillo: Option<u32>,
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
            .map_err(|e| {
                // Sin permiso de microfono en los ajustes de privacidad de
                // Windows: es lo mismo que no tener microfono, y el mensaje
                // de `SinMicrofono` ya lo dice.
                if e.code() == E_ACCESSDENIED {
                    falta.clone()
                } else {
                    en(if loopback {
                        "abrir el sonido del equipo"
                    } else {
                        "abrir el microfono"
                    })(e)
                }
            })?;
        terminar_de_abrir(cliente, formato, canales, tasa_anillo, false)
    }
}

/// Abre la captura de un proceso, probando los formatos de
/// [`proceso::formatos`].
///
/// # Safety
///
/// COM (multihilo) iniciado en el hilo.
unsafe fn abrir_proceso(
    pid: u32,
    canales: usize,
    tasa_anillo: Option<u32>,
) -> Result<Abierta, ErrorMotor> {
    let tasa = tasa_anillo.unwrap_or(48_000);
    let mut ultimo = ErrorMotor::SinSonidoDelEquipo;
    for f in proceso::formatos(tasa) {
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
                Ok(()) => return terminar_de_abrir(cliente, formato, canales, tasa_anillo, true),
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
    tasa_anillo: Option<u32>,
    por_proceso: bool,
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
        // Lo mas grande que puede traer un paquete es el anillo entero.
        let marcos = cliente
            .GetBufferSize()
            .map(|n| n as usize)
            .unwrap_or(formato.tasa as usize / 2)
            .max(64);
        let a_anillo = match tasa_anillo {
            Some(t) if t != formato.tasa => {
                Some(Remuestreador::nuevo(formato.tasa, t, canales, marcos))
            }
            _ => None,
        };
        let abierta = Abierta {
            conversor: Conversor::nuevo(formato, canales as u16),
            crudo: vec![0.0; marcos * canales],
            // Lo que da un paquete entero a la tasa del anillo, con margen:
            // si fuera menos, lo que no cupiera se iria quedando en el
            // remuestreador y creceria sin parar.
            remuestreado: match tasa_anillo {
                Some(t) if a_anillo.is_some() => {
                    let salen = (marcos as u64 * u64::from(t)).div_ceil(u64::from(formato.tasa));
                    vec![0.0; (salen as usize * 2 + 64) * canales]
                }
                _ => Vec::new(),
            },
            a_anillo,
            por_proceso,
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
unsafe fn vaciar(a: &mut Abierta, e: &mut Estado) -> windows::core::Result<()> {
    let c = e.canales;
    let por_marco = a.conversor.formato().bytes_por_marco();
    let tasa_aparato = a.tasa();
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
            // Un paquete mas grande que el anillo de la tarjeta no deberia
            // existir; si existe, se hace sitio (fuera de lo normal).
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

        let cuentas = e.cuentas;
        Cuentas::sumar(&cuentas.capturados, n as u64);
        if banderas & AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY.0 as u32 != 0 {
            Cuentas::sumar(&cuentas.discontinuidades, 1);
        }
        let ahora = qpc_100ns();
        let dura = duracion_de(n as u64, tasa_aparato);
        let fecha_mala =
            banderas & AUDCLNT_BUFFERFLAGS_TIMESTAMP_ERROR.0 as u32 != 0 || qpc == 0 || qpc > ahora;
        let inicio = if fecha_mala {
            ahora.saturating_sub(dura)
        } else {
            qpc
        };
        let hueco = e.relleno.paquete(inicio, dura);
        if hueco > 0 {
            let puestos = e.productor.escribir_ceros(hueco as usize);
            Cuentas::sumar(&cuentas.silencio, puestos as u64);
            Cuentas::sumar(&cuentas.perdidos, hueco - puestos as u64);
        }
        let bloque: &[f32] = match a.a_anillo.as_mut() {
            Some(r) => {
                r.meter(&a.crudo[..n * c]);
                let k = r.sacar(&mut a.remuestreado);
                &a.remuestreado[..k * c]
            }
            None => &a.crudo[..n * c],
        };
        let total = bloque.len() / c;
        let saltar = e.relleno.recortar(total as u64) as usize;
        Cuentas::sumar(&cuentas.recortados, saltar as u64);
        let escritos = e.productor.escribir(&bloque[saltar * c..]);
        Cuentas::sumar(&cuentas.perdidos, (total - saltar - escritos) as u64);
        soltado?;
    }
}
