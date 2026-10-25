//! **El reconocedor que ya trae Windows: SAPI 5, sin red y sin descargar
//! nada.**
//!
//! Es la alternativa de Windows al «Google» del movil (`MotorGoogle.kt`,
//! que usa el reconocedor del propio telefono y sin red). Aqui el
//! equivalente es el **Microsoft Speech Recognizer 8.0** de escritorio, que
//! Windows instala con el paquete de voz de cada idioma (en un Windows en
//! castellano ya esta: `MS-3082-80-DESK`, «Spanish - Spain»).
//!
//! ## Por que SAPI y no `Windows.Media.SpeechRecognition`
//!
//! El `SpeechRecognizer` de WinRT solo oye el microfono, y su dictado libre
//! exige la «voz en linea» de la privacidad de Windows: manda el audio a
//! Microsoft. SAPI en cambio:
//!
//! - **lee un audio guardado**: el reconocedor «en proceso»
//!   (`SpInprocRecognizer`) acepta cualquier flujo como entrada, asi que una
//!   nota de voz se le da ya decodificada, en memoria, y la muele **mucho
//!   mas rapido que en tiempo real** (medido el 2026-09-22 en el equipo de
//!   desarrollo: 9 s de audio en 0,9 s);
//! - **corre entero en el equipo**: dictado sin conexion;
//! - y da el **momento de cada frase** (`GetResultTimes`), que es justo lo
//!   que hace falta para escribir `[1:23]` en la transcripcion.
//!
//! Lo que se pierde frente a Whisper: escribe sin signos de puntuacion
//! (Vosk tampoco los pone) y es algo menos fino con el habla rapida. A
//! cambio no pide nada y no tarda nada, que es lo que hace que «pasar a
//! texto» se sienta instantaneo.
//!
//! ## Dos usos
//!
//! - [`transcribir_pcm`]: una nota de voz ya grabada (PCM 16 kHz mono).
//! - [`Dictado`]: dictar en vivo por el microfono, frase a frase, con el
//!   texto provisional mientras se habla. Es lo que usa el microfono de la
//!   caja de escribir del chat.
//!
//! Todo lo de aqui es COM: se llama desde un hilo que ya haya iniciado COM
//! (el de transcribir lo hace con `ComDelHilo`, y [`Dictado`] lanza el suyo).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;

use windows::Win32::Media::Audio::{WAVE_FORMAT_PCM, WAVEFORMATEX};
use windows::Win32::Media::Speech::{
    IEnumSpObjectTokens, ISpObjectToken, ISpObjectTokenCategory, ISpRecoContext, ISpRecoGrammar,
    ISpRecoResult, ISpRecognizer, ISpStream, SPCAT_AUDIOIN, SPCAT_RECOGNIZERS, SPEI_END_SR_STREAM,
    SPEI_HYPOTHESIS, SPEI_RECOGNITION, SPET_LPARAM_IS_OBJECT, SPET_LPARAM_IS_POINTER,
    SPET_LPARAM_IS_STRING, SPET_LPARAM_IS_TOKEN, SPEVENT, SPLO_STATIC, SPRECORESULTTIMES,
    SPRS_ACTIVE, SPRST_ACTIVE, SpInprocRecognizer, SpObjectToken, SpObjectTokenCategory, SpStream,
};
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance, CoTaskMemFree};
use windows::Win32::UI::Shell::SHCreateMemStream;
use windows::core::{GUID, IUnknown, Interface, PCWSTR, PWSTR, w};

use crate::credibilidad::creible;
use crate::idiomas::lengua_de;
use crate::motor::Transcripcion;
use crate::pcm;
use crate::tiempos::{Segmento, con_tiempos, estado_de};
use crate::{ErrorVoz, HERCIOS, en};

/// `SPDFID_WaveFormatEx` de `sapi.h`: «el formato va en el `WAVEFORMATEX`
/// que acompana». El crate `windows` no lo trae.
const SPDFID_WAVE_FORMAT_EX: GUID = GUID::from_u128(0xc31adbae_527f_4ff5_a230_f62bb61ff70c);

/// `SP_GETWHOLEPHRASE`: «la frase entera», para `GetText`.
const FRASE_ENTERA: u32 = 0xFFFF_FFFF;

/// Cada cuanto se despierta el bucle de eventos para mirar si le han
/// cancelado. Corto a proposito: cancelar tiene que sentirse inmediato.
const ESPERA_MS: u32 = 100;

/// La mascara de eventos de SAPI (`SPFEI` de `sapi.h`): el bit del evento
/// mas los dos bits reservados que el reconocedor exige ver puestos.
fn interes(eventos: &[i32]) -> u64 {
    const RESERVADOS: u64 = (1 << 30) | (1 << 33);
    eventos.iter().fold(RESERVADOS, |m, e| m | (1u64 << *e))
}

/// El identificador de idioma principal de Windows (`PRIMARYLANGID`) para
/// una etiqueta como `es`, `es-PE` o `en-US`. Los reconocedores de SAPI
/// declaran su idioma con el LCID completo (`c0a` = es-ES), y cualquier
/// variante del mismo idioma sirve: quien habla castellano de Peru se
/// entiende mejor con el de Espana que con ninguno.
pub fn idioma_principal(idioma: &str) -> Option<u16> {
    Some(match lengua_de(idioma).as_str() {
        "es" => 0x0A,
        "en" => 0x09,
        "zh" => 0x04,
        "fr" => 0x0C,
        "de" => 0x07,
        "ja" => 0x11,
        "pt" => 0x16,
        "it" => 0x10,
        _ => return None,
    })
}

/// Si una lista de LCID como la del registro de SAPI (`"409;9"`, `"c0a"`)
/// incluye el idioma principal pedido.
pub fn lista_incluye(lcids: &str, principal: u16) -> bool {
    lcids
        .split(';')
        .filter_map(|l| u32::from_str_radix(l.trim(), 16).ok())
        .any(|l| (l & 0x3FF) as u16 == principal)
}

/// Lee una cadena devuelta por COM y libera su memoria.
///
/// # Safety
///
/// `p` tiene que venir de una llamada COM que la reserva con
/// `CoTaskMemAlloc` y pasa la propiedad al llamante (o ser nula).
unsafe fn cadena_com(p: PWSTR) -> String {
    if p.is_null() {
        return String::new();
    }
    // SAFETY: lo garantiza el contrato: una cadena ancha terminada en cero
    // que ahora es nuestra; se copia y se libera una sola vez.
    let s = unsafe { p.to_string() }.unwrap_or_default();
    // SAFETY: la reservo COM con su asignador y nadie mas la libera.
    unsafe { CoTaskMemFree(Some(p.0 as *const _)) };
    s
}

/// Busca el reconocedor de escritorio de un idioma.
fn reconocedor_de(idioma: &str) -> Result<Option<ISpObjectToken>, ErrorVoz> {
    ficha_de(SPCAT_RECOGNIZERS, idioma)
}

/// La primera ficha de una categoria de SAPI (reconocedores o voces) que
/// habla ese idioma.
fn ficha_de(categoria_id: PCWSTR, idioma: &str) -> Result<Option<ISpObjectToken>, ErrorVoz> {
    let Some(principal) = idioma_principal(idioma) else {
        return Ok(None);
    };
    // SAFETY: COM ya iniciado en este hilo (contrato del modulo); cada
    // objeto se crea y se usa aqui mismo.
    unsafe {
        let categoria: ISpObjectTokenCategory =
            CoCreateInstance(&SpObjectTokenCategory, None, CLSCTX_ALL)
                .map_err(en("abrir la lista de reconocedores de voz"))?;
        categoria
            .SetId(categoria_id, false)
            .map_err(en("abrir la lista de reconocedores de voz"))?;
        let lista: IEnumSpObjectTokens = categoria
            .EnumTokens(PCWSTR::null(), PCWSTR::null())
            .map_err(en("enumerar los reconocedores de voz"))?;
        let mut cuantos = 0u32;
        lista
            .GetCount(&mut cuantos)
            .map_err(en("enumerar los reconocedores de voz"))?;
        for i in 0..cuantos {
            let Ok(ficha) = lista.Item(i) else { continue };
            let Ok(atributos) = ficha.OpenKey(w!("Attributes")) else {
                continue;
            };
            let idiomas = atributos
                .GetStringValue(w!("Language"))
                .map(|p| cadena_com(p))
                .unwrap_or_default();
            if lista_incluye(&idiomas, principal) {
                return Ok(Some(ficha));
            }
        }
    }
    Ok(None)
}

/// **Si Windows sabe pasar a texto este idioma sin descargar nada.**
///
/// Es lo que mira el chat antes de ofrecer «Pasar a texto» con el motor de
/// Windows. Barato: lee el registro, no carga el reconocedor.
pub fn disponible(idioma: &str) -> bool {
    matches!(reconocedor_de(idioma), Ok(Some(_)))
}

/// El reconocedor montado: motor, contexto y gramatica de dictado.
///
/// Se guardan los tres porque soltar cualquiera antes de tiempo apaga el
/// reconocimiento sin decir nada.
struct Montado {
    reconocedor: ISpRecognizer,
    contexto: ISpRecoContext,
    _gramatica: ISpRecoGrammar,
}

/// Monta el reconocedor en proceso con `entrada` como fuente de audio y el
/// dictado libre activado.
fn montar(idioma: &str, entrada: &IUnknown, parciales: bool) -> Result<Montado, ErrorVoz> {
    let ficha = reconocedor_de(idioma)?.ok_or_else(|| ErrorVoz::SinReconocedorDeWindows {
        idioma: idioma.to_string(),
    })?;
    let mut eventos = vec![SPEI_RECOGNITION.0, SPEI_END_SR_STREAM.0];
    if parciales {
        eventos.push(SPEI_HYPOTHESIS.0);
    }
    // SAFETY: COM iniciado en el hilo; todos los objetos viven en `Montado`
    // o mueren al salir de este bloque.
    unsafe {
        let reconocedor: ISpRecognizer = CoCreateInstance(&SpInprocRecognizer, None, CLSCTX_ALL)
            .map_err(en("crear el reconocedor de voz de Windows"))?;
        reconocedor
            .SetRecognizer(&ficha)
            .map_err(en("elegir el reconocedor del idioma"))?;
        reconocedor
            .SetInput(entrada, true)
            .map_err(en("darle el audio al reconocedor"))?;
        let contexto = reconocedor
            .CreateRecoContext()
            .map_err(en("crear el contexto de reconocimiento"))?;
        contexto
            .SetNotifyWin32Event()
            .map_err(en("preparar los avisos del reconocedor"))?;
        let mascara = interes(&eventos);
        contexto
            .SetInterest(mascara, mascara)
            .map_err(en("preparar los avisos del reconocedor"))?;
        let gramatica = contexto
            .CreateGrammar(0)
            .map_err(en("crear la gramatica de dictado"))?;
        gramatica
            .LoadDictation(PCWSTR::null(), SPLO_STATIC)
            .map_err(en("cargar el dictado"))?;
        gramatica
            .SetDictationState(SPRS_ACTIVE)
            .map_err(en("activar el dictado"))?;
        // Con un fichero hay que arrancarlo a mano. Con el microfono no: ya
        // nace activo, y pedirselo otra vez lo dejo colgado mas de 15 s
        // una de cada pocas veces (medido el 2026-09-22).
        if !parciales {
            reconocedor
                .SetRecoState(SPRST_ACTIVE)
                .map_err(en("poner en marcha el reconocedor"))?;
        }
        Ok(Montado {
            reconocedor,
            contexto,
            _gramatica: gramatica,
        })
    }
}

/// Lo que interesa de un evento de SAPI, ya copiado y con su memoria
/// liberada.
enum Suceso {
    /// Una frase cerrada: el texto y cuando empezo, en milisegundos desde
    /// el principio del audio.
    Frase { texto: String, desde_ms: i64 },
    /// Lo que va entendiendo mientras se habla.
    Provisional(String),
    /// Se acabo el audio.
    FinDelAudio,
    /// Otro evento: solo cuenta para el avance.
    Otro,
}

/// Saca los eventos pendientes y los traduce.
///
/// `avance_bytes` recibe el desplazamiento en bytes del audio por donde va
/// el reconocedor.
fn recoger(contexto: &ISpRecoContext, avance_bytes: &mut u64) -> Vec<Suceso> {
    let mut sucesos = Vec::new();
    loop {
        let mut e = SPEVENT::default();
        let mut sacados = 0u32;
        // SAFETY: un hueco para un evento y su contador, los dos nuestros.
        let r = unsafe { contexto.GetEvents(1, &mut e, &mut sacados) };
        if r.is_err() || sacados == 0 {
            break;
        }
        *avance_bytes = (*avance_bytes).max(e.ullAudioStreamOffset);
        let id = e._bitfield & 0xFFFF;
        let tipo = (e._bitfield >> 16) & 0xFFFF;
        let mut suceso = Suceso::Otro;
        if tipo == SPET_LPARAM_IS_OBJECT.0 && e.lParam.0 != 0 {
            // SAFETY: con este tipo, `lParam` es un `IUnknown*` con una
            // referencia que nos pertenece (`SpClearEvent` la suelta con
            // `Release`); `from_raw` se queda con ella y la suelta al caer.
            let objeto = unsafe { IUnknown::from_raw(e.lParam.0 as *mut _) };
            if id == SPEI_RECOGNITION.0 || id == SPEI_HYPOTHESIS.0 {
                if let Ok(resultado) = objeto.cast::<ISpRecoResult>() {
                    let texto = texto_de(&resultado);
                    suceso = if id == SPEI_RECOGNITION.0 {
                        Suceso::Frase {
                            texto,
                            desde_ms: inicio_ms(&resultado),
                        }
                    } else {
                        Suceso::Provisional(texto)
                    };
                }
            }
        } else if (tipo == SPET_LPARAM_IS_POINTER.0 || tipo == SPET_LPARAM_IS_STRING.0)
            && e.lParam.0 != 0
        {
            // SAFETY: con estos tipos la memoria la reservo SAPI con
            // `CoTaskMemAlloc` para nosotros (`SpClearEvent`).
            unsafe { CoTaskMemFree(Some(e.lParam.0 as *const _)) };
        } else if tipo == SPET_LPARAM_IS_TOKEN.0 && e.lParam.0 != 0 {
            // SAFETY: igual que el objeto: una referencia que nos toca soltar.
            drop(unsafe { IUnknown::from_raw(e.lParam.0 as *mut _) });
        }
        if id == SPEI_END_SR_STREAM.0 {
            suceso = Suceso::FinDelAudio;
        }
        sucesos.push(suceso);
    }
    sucesos
}

fn texto_de(resultado: &ISpRecoResult) -> String {
    let mut p = PWSTR::null();
    // SAFETY: `p` recibe una cadena reservada por COM que `cadena_com`
    // copia y libera.
    unsafe {
        if resultado
            .GetText(FRASE_ENTERA, FRASE_ENTERA, true, &mut p, None)
            .is_err()
        {
            return String::new();
        }
        cadena_com(p).trim().to_string()
    }
}

/// Cuando empezo la frase, en milisegundos desde el principio del audio.
/// `ullStart` va en unidades de 100 ns.
fn inicio_ms(resultado: &ISpRecoResult) -> i64 {
    let mut t = SPRECORESULTTIMES::default();
    // SAFETY: una estructura nuestra que SAPI rellena.
    match unsafe { resultado.GetResultTimes(&mut t) } {
        Ok(()) => (t.ullStart / 10_000) as i64,
        Err(_) => 0,
    }
}

/// El formato que se le anuncia a SAPI: PCM de 16 bits, mono, a 16 kHz, que
/// es lo que ya deja `pcm::decodificar`.
fn formato() -> WAVEFORMATEX {
    WAVEFORMATEX {
        wFormatTag: WAVE_FORMAT_PCM as u16,
        nChannels: 1,
        nSamplesPerSec: HERCIOS,
        nAvgBytesPerSec: HERCIOS * 2,
        nBlockAlign: 2,
        wBitsPerSample: 16,
        cbSize: 0,
    }
}

/// **Pasa a texto un audio ya decodificado** (PCM 16 bits mono a 16 kHz)
/// con el reconocedor de Windows.
///
/// Mismo contrato que `Motor::transcribir_pcm` de Vosk: `avance` de 0 a 1 y
/// `cancelado` mirado cada decima de segundo. Se llama desde un hilo con
/// COM iniciado.
pub fn transcribir_pcm(
    muestras: &[i16],
    idioma: &str,
    avance: &mut dyn FnMut(f32),
    cancelado: &AtomicBool,
) -> Result<Transcripcion, ErrorVoz> {
    if muestras.is_empty() {
        return Err(ErrorVoz::AudioVacio);
    }
    if cancelado.load(Ordering::Relaxed) {
        return Err(ErrorVoz::Cancelada);
    }
    let bytes: Vec<u8> = muestras.iter().flat_map(|m| m.to_le_bytes()).collect();
    let total = bytes.len() as f32;

    // SAFETY: el flujo en memoria copia los bytes; el `ISpStream` lo
    // envuelve anunciando el formato, y los dos viven hasta el final.
    let flujo: ISpStream = unsafe {
        let memoria = SHCreateMemStream(Some(&bytes)).ok_or(ErrorVoz::AudioVacio)?;
        let flujo: ISpStream = CoCreateInstance(&SpStream, None, CLSCTX_ALL)
            .map_err(en("crear el flujo de audio para el reconocedor"))?;
        let f = formato();
        flujo
            .SetBaseStream(&memoria, &SPDFID_WAVE_FORMAT_EX, &f)
            .map_err(en("darle formato al audio"))?;
        flujo
    };
    let entrada: IUnknown = flujo.cast().map_err(en("darle el audio al reconocedor"))?;
    let montado = montar(idioma, &entrada, false)?;

    let mut segmentos: Vec<Segmento> = Vec::new();
    let mut avisos = 0usize;
    let mut por_donde = 0u64;
    'moler: loop {
        if cancelado.load(Ordering::Relaxed) {
            return Err(ErrorVoz::Cancelada);
        }
        // SAFETY: el contexto esta vivo; esperar con tope no toca nada mas.
        let _ = unsafe { montado.contexto.WaitForNotifyEvent(ESPERA_MS) };
        for suceso in recoger(&montado.contexto, &mut por_donde) {
            match suceso {
                Suceso::Frase { texto, desde_ms } => {
                    if texto.is_empty() {
                        continue;
                    }
                    if creible(&texto, idioma) {
                        segmentos.push(Segmento::nuevo(desde_ms, texto));
                    } else {
                        avisos += 1;
                    }
                }
                Suceso::FinDelAudio => break 'moler,
                Suceso::Provisional(_) | Suceso::Otro => {}
            }
        }
        avance((por_donde as f32 / total).min(0.99));
    }
    avance(1.0);
    // El reconocedor se queda con el flujo; se suelta primero la entrada.
    drop(montado.reconocedor);

    if segmentos.is_empty() {
        return Err(ErrorVoz::NoSeEntiendeNada);
    }
    Ok(Transcripcion {
        texto: con_tiempos(&segmentos, ""),
        estado: estado_de(segmentos.len(), avisos),
        duracion_ms: pcm::duracion_ms(muestras),
        segmentos,
    })
}

/// **Pasa a texto un fichero de audio** con el reconocedor de Windows.
pub fn transcribir(
    audio: &std::path::Path,
    idioma: &str,
    avance: &mut dyn FnMut(f32),
    cancelado: &AtomicBool,
) -> Result<Transcripcion, ErrorVoz> {
    if !disponible(idioma) {
        return Err(ErrorVoz::SinReconocedorDeWindows {
            idioma: idioma.to_string(),
        });
    }
    let muestras = pcm::decodificar(audio)?;
    transcribir_pcm(&muestras, idioma, avance, cancelado)
}

/// Lo que va llegando mientras se dicta.
#[derive(Debug, Clone, PartialEq)]
pub enum AvisoDeDictado {
    /// Ya esta oyendo: se puede empezar a hablar.
    Oyendo,
    /// Lo que va entendiendo de la frase en curso; la siguiente
    /// `Provisional` o `Frase` la sustituye.
    Provisional(String),
    /// Una frase cerrada: ya no cambia.
    Frase(String),
    /// No se pudo dictar (sin microfono, sin reconocedor…).
    Fallo(String),
}

/// **Dictar en vivo por el microfono.**
///
/// Vive en su hilo, con su COM: la ventana solo mira [`Dictado::recoger`]
/// en su bucle, sin bloquear. Tirarlo (o [`Dictado::parar`]) cierra el
/// microfono.
pub struct Dictado {
    parar: Arc<AtomicBool>,
    avisos: Receiver<AvisoDeDictado>,
    hilo: Option<JoinHandle<()>>,
}

impl Dictado {
    /// Empieza a oir por el microfono por defecto de Windows.
    pub fn empezar(idioma: &str) -> Dictado {
        let parar = Arc::new(AtomicBool::new(false));
        let (enviar, avisos) = channel();
        let hilo = {
            let parar = Arc::clone(&parar);
            let idioma = idioma.to_string();
            std::thread::Builder::new()
                .name("pixpin-dictado".into())
                .spawn(move || {
                    if let Err(e) = dictar(&idioma, &parar, &enviar) {
                        tracing::info!(?e, "el dictado no pudo seguir");
                        let _ = enviar.send(AvisoDeDictado::Fallo(e.to_string()));
                    }
                })
                .ok()
        };
        Dictado {
            parar,
            avisos,
            hilo,
        }
    }

    /// Lo que haya llegado desde la ultima vez. No bloquea.
    pub fn recoger(&self) -> Vec<AvisoDeDictado> {
        let mut v = Vec::new();
        while let Ok(a) = self.avisos.try_recv() {
            v.push(a);
        }
        v
    }

    /// Si el hilo ya termino (por fallo o porque se le pidio parar).
    pub fn terminado(&self) -> bool {
        self.hilo.as_ref().is_none_or(|h| h.is_finished())
    }

    pub fn parar(&self) {
        self.parar.store(true, Ordering::Relaxed);
    }
}

impl Drop for Dictado {
    fn drop(&mut self) {
        self.parar();
        // No se espera al hilo: tarda como mucho una espera (100 ms) en
        // verlo, y la ventana no tiene por que quedarse quieta mientras.
    }
}

/// COM del hilo del dictado: se inicia al entrar y se cierra al salir.
struct Com;

impl Com {
    fn iniciar() -> Com {
        use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
        // SAFETY: se empareja con el `CoUninitialize` de `Drop` en este
        // mismo hilo.
        let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        Com
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        // SAFETY: pareja del `CoInitializeEx` de `iniciar`, mismo hilo.
        unsafe { windows::Win32::System::Com::CoUninitialize() };
    }
}

fn microfono() -> Result<IUnknown, ErrorVoz> {
    // SAFETY: COM iniciado en el hilo; la ficha del microfono vive lo que
    // la entrada del reconocedor.
    unsafe {
        let categoria: ISpObjectTokenCategory =
            CoCreateInstance(&SpObjectTokenCategory, None, CLSCTX_ALL)
                .map_err(en("abrir la lista de microfonos"))?;
        categoria
            .SetId(SPCAT_AUDIOIN, false)
            .map_err(en("abrir la lista de microfonos"))?;
        let id = categoria
            .GetDefaultTokenId()
            .map_err(|_| ErrorVoz::SinMicrofono)?;
        let id = cadena_com(id);
        let ancho: Vec<u16> = id.encode_utf16().chain(std::iter::once(0)).collect();
        let ficha: ISpObjectToken =
            CoCreateInstance(&SpObjectToken, None, CLSCTX_ALL).map_err(en("abrir el microfono"))?;
        ficha
            .SetId(PCWSTR::null(), PCWSTR(ancho.as_ptr()), false)
            .map_err(|_| ErrorVoz::SinMicrofono)?;
        ficha.cast().map_err(en("abrir el microfono"))
    }
}

fn dictar(
    idioma: &str,
    parar: &AtomicBool,
    enviar: &Sender<AvisoDeDictado>,
) -> Result<(), ErrorVoz> {
    let _com = Com::iniciar();
    let entrada = microfono()?;
    let montado = montar(idioma, &entrada, true)?;
    let _ = enviar.send(AvisoDeDictado::Oyendo);
    let mut por_donde = 0u64;
    while !parar.load(Ordering::Relaxed) {
        // SAFETY: el contexto esta vivo hasta el final de la funcion.
        let _ = unsafe { montado.contexto.WaitForNotifyEvent(ESPERA_MS) };
        for suceso in recoger(&montado.contexto, &mut por_donde) {
            let aviso = match suceso {
                Suceso::Frase { texto, .. } if !texto.is_empty() => AvisoDeDictado::Frase(texto),
                Suceso::Provisional(t) if !t.is_empty() => AvisoDeDictado::Provisional(t),
                _ => continue,
            };
            if enviar.send(aviso).is_err() {
                // La ventana ya no escucha: no tiene sentido seguir oyendo.
                return Ok(());
            }
        }
    }
    Ok(())
}

// --- Oir un texto (B9, extra de Pronunciar) ---------------------------------

/// **La voz de Windows leyendo un texto en voz alta** (`ISpVoice`).
///
/// Es el extra de Pronunciar que el movil no tiene: oir como suena la frase
/// que se va a practicar antes de decirla. Sin red y sin descargar nada: las
/// voces las trae Windows (y las que el usuario haya puesto en
/// Configuracion > Hora e idioma > Voz). Habla **sin bloquear**
/// (`SPF_ASYNC`): quien llama sigue pintando mientras suena.
pub struct Lector {
    voz: windows::Win32::Media::Speech::ISpVoice,
    /// Como se llama la voz en Windows («Microsoft Helena Desktop»).
    nombre: String,
    /// Si esta en pausa (`Pause` de SAPI cuenta: dos pausas piden dos
    /// `Resume`, asi que se lleva aqui y se pide una sola).
    pausado: std::cell::Cell<bool>,
}

impl Lector {
    /// Una voz del idioma (`es`, `en-US`…), o `None` si Windows no tiene
    /// ninguna de ese idioma. COM tiene que estar iniciado en el hilo.
    pub fn nuevo(idioma: &str) -> Option<Lector> {
        use windows::Win32::Media::Speech::{ISpVoice, SPCAT_VOICES, SpVoice};
        let ficha = ficha_de(SPCAT_VOICES, idioma).ok()??;
        // SAFETY: COM iniciado en el hilo (contrato); la voz es nuestra y la
        // ficha se le da prestada.
        unsafe {
            let nombre = ficha
                .GetStringValue(PCWSTR::null())
                .map(|p| cadena_com(p))
                .unwrap_or_default();
            let voz: ISpVoice = CoCreateInstance(&SpVoice, None, CLSCTX_ALL).ok()?;
            voz.SetVoice(&ficha).ok()?;
            Some(Lector {
                voz,
                nombre,
                pausado: std::cell::Cell::new(false),
            })
        }
    }

    /// El nombre de la voz, para ensenarlo.
    pub fn nombre(&self) -> &str {
        &self.nombre
    }

    /// **La velocidad**, de −10 a 10 (0 es la normal; cada diez pasos, el
    /// triple). Vale para lo que se lea despues.
    pub fn poner_tasa(&self, tasa: i32) {
        // SAFETY: voz viva.
        unsafe {
            let _ = self.voz.SetRate(tasa.clamp(-10, 10));
        }
    }

    /// Se para donde va, a media palabra, y [`Lector::reanudar`] sigue ahi.
    pub fn pausar(&self) {
        if self.pausado.replace(true) {
            return;
        }
        // SAFETY: voz viva.
        unsafe {
            let _ = self.voz.Pause();
        }
    }

    pub fn reanudar(&self) {
        if !self.pausado.replace(false) {
            return;
        }
        // SAFETY: voz viva.
        unsafe {
            let _ = self.voz.Resume();
        }
    }

    /// **Si ya dijo todo lo que se le dio** (`SPRS_DONE`). En pausa no ha
    /// acabado.
    pub fn acabado(&self) -> bool {
        use windows::Win32::Media::Speech::{SPRS_DONE, SPVOICESTATUS};
        if self.pausado.get() {
            return false;
        }
        let mut estado = SPVOICESTATUS::default();
        // SAFETY: voz viva; el estado es nuestro y sin marcador que liberar
        // (puntero nulo: no se pide).
        let hecho = unsafe { self.voz.GetStatus(&mut estado, std::ptr::null_mut()) };
        hecho.is_ok() && estado.dwRunningState & SPRS_DONE.0 as u32 != 0
    }

    /// Lee `texto` desde el principio, cortando lo que estuviera leyendo.
    pub fn leer(&self, texto: &str) {
        // Purgar con la voz en pausa la dejaria esperando: antes se suelta.
        self.reanudar();
        use windows::Win32::Media::Speech::{SPF_ASYNC, SPF_IS_NOT_XML, SPF_PURGEBEFORESPEAK};
        let ancho: Vec<u16> = texto.encode_utf16().chain(std::iter::once(0)).collect();
        let banderas = (SPF_ASYNC.0 | SPF_PURGEBEFORESPEAK.0 | SPF_IS_NOT_XML.0) as u32;
        // SAFETY: voz viva; la cadena vive hasta que `Speak` la copia (lo
        // hace antes de volver, tambien en asincrono).
        unsafe {
            let _ = self.voz.Speak(PCWSTR(ancho.as_ptr()), banderas, None);
        }
    }

    /// Calla lo que estuviera leyendo.
    pub fn callar(&self) {
        self.reanudar();
        use windows::Win32::Media::Speech::{SPF_ASYNC, SPF_PURGEBEFORESPEAK};
        let banderas = (SPF_ASYNC.0 | SPF_PURGEBEFORESPEAK.0) as u32;
        // SAFETY: voz viva; una cadena nula con purga es «callate».
        unsafe {
            let _ = self.voz.Speak(PCWSTR::null(), banderas, None);
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn cada_idioma_va_con_su_identificador_principal_de_windows() {
        assert_eq!(idioma_principal("es"), Some(0x0A));
        assert_eq!(idioma_principal("es-PE"), Some(0x0A));
        assert_eq!(idioma_principal("en-US"), Some(0x09));
        assert_eq!(idioma_principal("tlh"), None, "klingon no esta en SAPI");
    }

    #[test]
    fn la_lista_del_registro_se_lee_por_idioma_principal_y_no_por_pais() {
        // «c0a» es es-ES; debe servir para quien pide «es-PE».
        assert!(lista_incluye("c0a", 0x0A));
        assert!(lista_incluye("409;9", 0x09));
        assert!(!lista_incluye("409;9", 0x0A));
        assert!(!lista_incluye("", 0x0A));
        assert!(!lista_incluye("basura", 0x0A));
    }

    #[test]
    fn la_mascara_de_eventos_lleva_los_bits_reservados() {
        let m = interes(&[SPEI_RECOGNITION.0]);
        assert_ne!(m & (1 << 30), 0);
        assert_ne!(m & (1 << 33), 0);
        assert_ne!(m & (1 << 38), 0);
        assert_eq!(m & (1 << 39), 0, "sin pedir provisionales no llegan");
    }

    /// Se sintetiza una frase con la voz de Windows y se reconoce de
    /// vuelta. Si el equipo no tiene voz o reconocedor en castellano, la
    /// prueba se salta con aviso: no es un fallo del codigo.
    #[test]
    fn reconoce_una_frase_dicha_por_la_propia_voz_de_windows() {
        let _com = Com::iniciar();
        if !disponible("es") {
            eprintln!("aviso: este equipo no tiene reconocedor de castellano");
            return;
        }
        let Some(muestras) = sintetizar("mañana tengo que revisar los planos del proyecto") else {
            eprintln!("aviso: este equipo no tiene voz de castellano");
            return;
        };
        let cancelado = AtomicBool::new(false);
        let mut ultimo = 0.0f32;
        let t = transcribir_pcm(&muestras, "es", &mut |f| ultimo = f, &cancelado)
            .expect("se entiende la frase");
        let texto = t.texto.to_lowercase();
        assert!(texto.contains("planos"), "salio: {texto}");
        assert!(texto.starts_with("[0:00]"), "salio: {texto}");
        eprintln!("reconocido: {}", t.texto);
        assert_eq!(ultimo, 1.0);
    }

    #[test]
    fn cancelar_antes_de_empezar_no_muele_nada() {
        let cancelado = AtomicBool::new(true);
        let r = transcribir_pcm(&[0i16; 16_000], "es", &mut |_| {}, &cancelado);
        assert!(matches!(r, Err(ErrorVoz::Cancelada)));
    }

    /// Abre el microfono de verdad un segundo y medio: exige escritorio y
    /// microfono, por eso va aparte (`--ignored`).
    #[test]
    #[ignore = "abre el microfono del equipo"]
    fn el_dictado_arranca_con_el_microfono_por_defecto() {
        if !disponible_en_hilo_nuevo("es") {
            return;
        }
        let d = Dictado::empezar("es");
        // Montar el dictado tarda de 0,6 a 1 s en el equipo de desarrollo.
        let mut avisos = Vec::new();
        let t = std::time::Instant::now();
        while !avisos.contains(&AvisoDeDictado::Oyendo) && t.elapsed().as_secs() < 10 {
            std::thread::sleep(std::time::Duration::from_millis(50));
            avisos.extend(d.recoger());
        }
        assert!(
            avisos.contains(&AvisoDeDictado::Oyendo),
            "no llego a oir: {avisos:?}"
        );
        assert!(
            !avisos.iter().any(|a| matches!(a, AvisoDeDictado::Fallo(_))),
            "{avisos:?}"
        );
        // Soltar el microfono tarda de 0,3 a 3,5 s; va en el hilo del
        // dictado, asi que la ventana no lo nota.
        d.parar();
        let t = std::time::Instant::now();
        while !d.terminado() && t.elapsed().as_secs() < 10 {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(d.terminado(), "parar tiene que cerrar el hilo");
    }

    fn disponible_en_hilo_nuevo(idioma: &str) -> bool {
        let idioma = idioma.to_string();
        std::thread::spawn(move || {
            let _com = Com::iniciar();
            disponible(&idioma)
        })
        .join()
        .unwrap_or(false)
    }

    #[test]
    fn un_idioma_sin_voz_no_da_lector_y_el_que_hay_lee_sin_bloquear() {
        let hecho = std::thread::spawn(|| {
            let _com = Com::iniciar();
            assert!(
                Lector::nuevo("xx").is_none(),
                "no hay voz de un idioma inventado"
            );
            // En este equipo hay voz castellana o inglesa (las trae Windows);
            // si no, no hay nada que comprobar.
            let Some(l) = Lector::nuevo("es").or_else(|| Lector::nuevo("en")) else {
                return;
            };
            let t = std::time::Instant::now();
            l.leer("hola");
            l.callar();
            assert!(
                t.elapsed() < std::time::Duration::from_millis(500),
                "leer no puede esperar a que acabe la frase"
            );
        });
        hecho.join().unwrap();
    }

    /// Habla por los altavoces del equipo: va aparte (`--ignored`).
    #[test]
    #[ignore = "suena por los altavoces"]
    fn en_pausa_no_acaba_y_al_reanudar_termina_la_frase() {
        let hecho = std::thread::spawn(|| {
            let _com = Com::iniciar();
            let Some(l) = Lector::nuevo("es").or_else(|| Lector::nuevo("en")) else {
                return;
            };
            assert!(!l.nombre().is_empty());
            l.poner_tasa(10);
            l.leer("uno dos tres");
            l.pausar();
            std::thread::sleep(std::time::Duration::from_millis(300));
            assert!(!l.acabado(), "en pausa no ha acabado");
            l.reanudar();
            let t = std::time::Instant::now();
            while !l.acabado() && t.elapsed().as_secs() < 10 {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            assert!(l.acabado(), "la frase tiene que acabar");
        });
        hecho.join().unwrap();
    }

    #[test]
    fn un_audio_vacio_se_dice_sin_montar_nada() {
        let cancelado = AtomicBool::new(false);
        let r = transcribir_pcm(&[], "es", &mut |_| {}, &cancelado);
        assert!(matches!(r, Err(ErrorVoz::AudioVacio)));
    }

    /// Voz sintetica de SAPI a PCM 16 kHz mono en memoria, para las pruebas.
    fn sintetizar(frase: &str) -> Option<Vec<i16>> {
        use windows::Win32::Media::Speech::{ISpVoice, SPCAT_VOICES, SPF_DEFAULT, SpVoice};
        use windows::Win32::System::Com::{STATFLAG_NONAME, STATSTG, STREAM_SEEK_SET};
        // SAFETY: COM iniciado por la prueba; todo vive en este bloque.
        unsafe {
            let memoria = SHCreateMemStream(None)?;
            let flujo: ISpStream = CoCreateInstance(&SpStream, None, CLSCTX_ALL).ok()?;
            let f = formato();
            flujo
                .SetBaseStream(&memoria, &SPDFID_WAVE_FORMAT_EX, &f)
                .ok()?;
            let voz: ISpVoice = CoCreateInstance(&SpVoice, None, CLSCTX_ALL).ok()?;
            let categoria: ISpObjectTokenCategory =
                CoCreateInstance(&SpObjectTokenCategory, None, CLSCTX_ALL).ok()?;
            categoria.SetId(SPCAT_VOICES, false).ok()?;
            let lista = categoria
                .EnumTokens(w!("Language=c0a"), PCWSTR::null())
                .ok()?;
            let ficha = lista.Item(0).ok()?;
            voz.SetVoice(&ficha).ok()?;
            voz.SetOutput(&flujo, true).ok()?;
            let ancho: Vec<u16> = frase.encode_utf16().chain(std::iter::once(0)).collect();
            voz.Speak(PCWSTR(ancho.as_ptr()), SPF_DEFAULT.0 as u32, None)
                .ok()?;
            let mut info = STATSTG::default();
            memoria.Stat(&mut info, STATFLAG_NONAME).ok()?;
            memoria.Seek(0, STREAM_SEEK_SET, None).ok()?;
            let mut bytes = vec![0u8; info.cbSize as usize];
            let mut leidos = 0u32;
            let _ = memoria.Read(
                bytes.as_mut_ptr() as *mut _,
                bytes.len() as u32,
                Some(&mut leidos),
            );
            bytes.truncate(leidos as usize);
            Some(
                bytes
                    .chunks_exact(2)
                    .map(|b| i16::from_le_bytes([b[0], b[1]]))
                    .collect(),
            )
        }
    }
}
