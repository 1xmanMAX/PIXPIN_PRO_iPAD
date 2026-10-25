//! **Pasar una nota de voz a texto, en este ordenador y sin red.**
//!
//! Es el puerto de `guardados/Transcriptor.kt` del movil, y tambien el del
//! telepronter (`guardados/TelepronterActivity.kt`), que es lo que el movil
//! llama «leer en voz alta». Las dos cosas viven juntas porque comparten lo
//! unico que de verdad tienen en comun: el cuerpo con marcas de tiempo
//! (`[1:23] lo que se dijo`) que acaba en `Mensaje.transcripcion` y viaja
//! por la sincronizacion.
//!
//! ## Dos correcciones que hay que tener presentes
//!
//! 1. **La transcripcion del movil NO es la nube.** Los tres motores corren
//!    en el aparato; la red solo baja modelos. Hasta el llamado «Google»
//!    usa `createOnDeviceSpeechRecognizer` con `EXTRA_PREFER_OFFLINE`
//!    (`MotorGoogle.kt:78, 170`).
//! 2. **«Leer en voz alta» no es voz sintetica.** En todo PixPin Android no
//!    hay ni un `TextToSpeech`. Significa que el usuario lee mientras el
//!    telefono graba: telepronter y pronunciar. La unica voz sintetica es
//!    un extra del PC en Pronunciar, «Oir el texto» (`sapi::Lector`), y la
//!    pone Windows.
//!
//! ## Tres motores: Whisper, el de Windows y Vosk
//!
//! **Actualizado otra vez el 2026-09-22, por decision del usuario.** Con sus
//! propias notas, SAPI escribio frases que nadie dijo. Pasar a texto va
//! ahora con **Whisper *base*** (el mismo modelo que el movil) sobre el
//! **ONNX Runtime que ya trae Windows** en `System32`, cargado al vuelo (ver
//! [`ort`] y [`whisper`]). El modelo (unos 160 MB) se baja una vez, cuando
//! el usuario lo pide. SAPI se queda para dictar en vivo por el microfono.
//!
//! **Actualizado el 2026-09-22.** El motor por omision es ahora el que ya
//! trae Windows, SAPI 5 (ver `sapi`): el analisis de abajo descarto
//! `Windows.Media.SpeechRecognition` —y con razon: no lee ficheros—, pero
//! no miro SAPI, cuyo reconocedor «en proceso» **si** acepta un flujo de
//! audio, corre sin red y da el momento de cada frase. No pide descargar
//! nada, muele diez veces mas rapido que el tiempo real y sirve tambien
//! para dictar en vivo. Vosk se queda para los idiomas que Windows no trae
//! (ver [`motor_para`]).
//!
//! Lo que sigue es el analisis original, que sigue valiendo para Vosk.
//!
//! Se miraron los tres caminos:
//!
//! - **`Windows.Media.SpeechRecognition`** (viene con el sistema, gratis).
//!   **Queda descartado por una razon tecnica, no de gusto: no sabe leer un
//!   fichero.** Su `SpeechRecognizer` oye el microfono por defecto y nada
//!   mas —ni `RecognizeAsync` ni `SpeechContinuousRecognitionSession`
//!   admiten una fuente de audio—, asi que transcribir un `.m4a` guardado
//!   obligaria a reproducirlo por los altavoces y volverlo a oir, o a
//!   montar un cable de audio virtual. Ademas exige que el usuario instale
//!   el paquete de voz del idioma desde los ajustes de Windows, y devuelve
//!   la frase sin el tiempo de cada palabra, que es justo lo que hace falta
//!   para escribir `[1:23]`.
//! - **sherpa-onnx o whisper.cpp** (Apache-2.0 / MIT). Dan buen texto y con
//!   puntuacion, pero: son C++ que hay que **enlazar al compilar**, o sea
//!   cmake, bindgen y onnxruntime en el arbol, y sin ese aparejo no
//!   compilaria ni `cargo test`; los modelos pesan de 104 a 375 MB frente a
//!   los 40 de Vosk; y Whisper **se inventa frases** cuando le llega
//!   silencio —el movil arrastra una lista negra entera contra eso
//!   (`Transcriptor.kt:284-324`)—.
//! - **Vosk** (Kaldi). Es **el motor por omision del movil**
//!   (`Transcriptor.kt:231`), con **los mismos modelos**
//!   (`vosk-model-small-es-0.42` y hermanos, `MotorVosk.kt:32-40`), asi que
//!   el mismo audio da **el mismo texto en los dos aparatos**, que era el
//!   objetivo. Da el tiempo de cada palabra de serie (`setWords`). Y trae
//!   un `libvosk.dll` ya compilado, de modo que aqui **se carga al vuelo
//!   con `LoadLibraryW`** en vez de enlazarlo: el repositorio no gana ni
//!   una dependencia de compilacion, `cargo build` y `cargo test` siguen
//!   funcionando en una maquina donde no haya nada de esto instalado, y no
//!   viaja un solo byte de modelo dentro del ejecutable.
//!
//! Licencias, que es la otra puerta: `vosk-api` es **Apache-2.0**, Kaldi es
//! **Apache-2.0** y los modelos *small* que usa el movil son **Apache-2.0**.
//! Nada de GPL ni AGPL entra por aqui, y como no hay dependencia de cargo
//! nueva, `cargo-deny` no tiene nada nuevo que mirar.
//!
//! Lo que se pierde con Vosk y conviene saber: los modelos pequenos
//! escriben **sin mayusculas ni puntuacion**. El movil vive con ello desde
//! el primer dia; el texto se lee igual y se busca mejor.
//!
//! ## Lo que hay que instalar (una vez, y lo hace el usuario)
//!
//! Ver [`motor`] y [`idiomas`]: un `libvosk.dll` y la carpeta del modelo
//! del idioma, las dos dentro de `<datos>/vosk/`. Este crate **no descarga
//! nada**: dice lo que falta y el enlace exacto, y quien lo baja es el
//! usuario.
//!
//! ## Reparto del crate
//!
//! - [`tiempos`], [`credibilidad`], [`idiomas`] y [`telepronter`]:
//!   aritmetica y texto, sin Windows y sin sonido. Es donde estan las
//!   pruebas de verdad.
//! - [`pcm`] y [`motor`]: lo que toca el sistema operativo y la libreria en
//!   C, con su `unsafe` acotado y comentado.
#![deny(clippy::undocumented_unsafe_blocks)]

pub mod credibilidad;
pub mod dos_idiomas;
/// Las voces de Microsoft Edge para leer en voz alta (los lectores).
pub mod edge;
pub mod idiomas;
pub mod telepronter;
pub mod tiempos;
pub mod turnos;

#[cfg(windows)]
pub mod motor;
#[cfg(windows)]
pub mod ort;
#[cfg(windows)]
pub mod pcm;
#[cfg(windows)]
pub mod sapi;
#[cfg(windows)]
pub mod whisper;

pub use credibilidad::creible;
pub use dos_idiomas::{Idiomas, ModoDeIdiomas};
pub use idiomas::{Disponibilidad, IDIOMAS, carpeta_de_modelos, enlace_del_modelo, modelo_de};
pub use telepronter::{Desfile, parrafos_de};
pub use tiempos::{EstadoDelTexto, Segmento, con_tiempos, marca_de_tiempo, tiempo_de};

#[cfg(windows)]
pub use motor::Transcripcion;
#[cfg(windows)]
pub use sapi::{AvisoDeDictado, Dictado};

/// Con que se pasa a texto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotorDeVoz {
    /// Whisper *base* sobre el ONNX Runtime que trae Windows; sin red una
    /// vez bajado el modelo. Ver `whisper`.
    Whisper,
    /// El reconocedor que ya trae Windows (SAPI 5, sin red). Ver `sapi`.
    Windows,
    /// Vosk, con los mismos modelos que el movil. Ver `motor`.
    Vosk,
}

/// **Que motor usar hoy**, o `None` si no hay ninguno para ese idioma.
///
/// **Actualizado el 2026-09-22**: primero Whisper, si hay ONNX Runtime y el
/// modelo ya esta bajado. El usuario comparo con sus propias notas: el
/// reconocedor de Windows entiende mal el habla natural y Whisper no. Luego
/// el de Windows, que no pide nada. Vosk queda para los idiomas que ninguno
/// de los dos trae; con `preferir_vosk` manda Vosk si esta listo (quien lo
/// puso a proposito).
#[cfg(windows)]
pub fn motor_para(
    raiz: &std::path::Path,
    junto_al_exe: Option<&std::path::Path>,
    idioma: &str,
    preferir_vosk: bool,
) -> Option<MotorDeVoz> {
    let vosk = matches!(
        idiomas::disponibilidad(raiz, junto_al_exe, idioma),
        Disponibilidad::Listo(_)
    );
    if preferir_vosk && vosk {
        return Some(MotorDeVoz::Vosk);
    }
    if whisper_posible(raiz, junto_al_exe, idioma) && whisper::modelo_listo(raiz) {
        return Some(MotorDeVoz::Whisper);
    }
    if sapi::disponible(idioma) {
        return Some(MotorDeVoz::Windows);
    }
    vosk.then_some(MotorDeVoz::Vosk)
}

/// Si Whisper puede correr aqui: hay ONNX Runtime y sabe el idioma. El
/// modelo puede faltar todavia; eso se baja (ver [`ErrorVoz::SinModeloWhisper`]).
#[cfg(windows)]
pub fn whisper_posible(
    raiz: &std::path::Path,
    junto_al_exe: Option<&std::path::Path>,
    idioma: &str,
) -> bool {
    whisper::sabe(idioma) && whisper::runtime_disponible(raiz, junto_al_exe)
}

/// **Pasa una nota de voz a texto con el mejor motor que haya.**
///
/// Ver [`motor_para`]. Con ONNX Runtime pero **sin el modelo de Whisper**
/// no se cae al reconocedor de Windows: sale
/// [`ErrorVoz::SinModeloWhisper`], para que la app ofrezca bajarlo (una vez)
/// en vez de dar en silencio el texto malo. Si no hay ningun motor, el
/// error es el de Vosk (dice que descargar y de donde).
#[cfg(windows)]
pub fn transcribir(
    audio: &std::path::Path,
    raiz: &std::path::Path,
    junto_al_exe: Option<&std::path::Path>,
    idioma: &str,
    avance: &mut dyn FnMut(f32),
    cancelado: &std::sync::atomic::AtomicBool,
) -> Result<Transcripcion, ErrorVoz> {
    let motor = motor_para(raiz, junto_al_exe, idioma, false);
    if motor != Some(MotorDeVoz::Whisper) && whisper_posible(raiz, junto_al_exe, idioma) {
        return Err(ErrorVoz::SinModeloWhisper {
            donde: whisper::carpeta(raiz).display().to_string(),
            megas: whisper::MEGAS,
        });
    }
    match motor {
        Some(MotorDeVoz::Whisper) => {
            whisper::transcribir(audio, raiz, junto_al_exe, idioma, avance, cancelado)
        }
        Some(MotorDeVoz::Windows) => sapi::transcribir(audio, idioma, avance, cancelado),
        _ => motor::transcribir(audio, raiz, junto_al_exe, idioma, avance, cancelado),
    }
}

/// **Lo mismo con dos idiomas y con turnos** (B6, B10). Solo Whisper sabe
/// de dos idiomas y de turnos con nombre: con otro motor se transcribe como
/// siempre, en el primero y de corrido, que es lo que haria el movil con
/// Vosk.
#[cfg(windows)]
pub fn transcribir_con(
    audio: &std::path::Path,
    raiz: &std::path::Path,
    junto_al_exe: Option<&std::path::Path>,
    idiomas: &Idiomas,
    turnos: &[turnos::Turno],
    avance: &mut dyn FnMut(f32),
    cancelado: &std::sync::atomic::AtomicBool,
) -> Result<Transcripcion, ErrorVoz> {
    let idioma = idiomas.principal.as_str();
    if motor_para(raiz, junto_al_exe, idioma, false) == Some(MotorDeVoz::Whisper) {
        return whisper::transcribir_con(
            audio,
            raiz,
            junto_al_exe,
            idiomas,
            turnos,
            avance,
            cancelado,
        );
    }
    transcribir(audio, raiz, junto_al_exe, idioma, avance, cancelado)
}

/// A cuantas muestras por segundo oye el reconocedor.
///
/// No es negociable: los modelos de Vosk estan entrenados a 16 kHz
/// (`Transcriptor.HERCIOS`, `Transcriptor.kt:32`). Todo lo que entra se
/// remuestrea a esto antes de llegar al motor.
pub const HERCIOS: u32 = 16_000;

/// Bytes de PCM de 16 bits mono que ocupa un segundo de audio.
pub const BYTES_POR_SEGUNDO: usize = HERCIOS as usize * 2;

/// Lo que puede salir mal al pasar una nota a texto.
///
/// Cada variante dice **que hacer**, no solo que fallo, porque las dos
/// causas normales —falta el motor, falta el idioma— se arreglan con una
/// descarga y el usuario tiene que enterarse de cual. Misma regla que
/// `pixpin_audio::ErrorAudio`.
#[derive(Debug, thiserror::Error)]
pub enum ErrorVoz {
    #[error(
        "falta el reconocedor de voz: no se encontro `libvosk.dll` en {donde} \
         ni junto al ejecutable"
    )]
    SinMotor { donde: String },
    #[error(
        "no hay modelo de voz para «{idioma}»; hay que descomprimir {modelo} \
         dentro de {donde}"
    )]
    SinModelo {
        idioma: String,
        modelo: String,
        donde: String,
    },
    #[error("Vosk no tiene modelo de voz pequeno para «{idioma}»")]
    IdiomaSinModelo { idioma: String },
    #[error(
        "la carpeta del modelo {donde} lleva caracteres que el reconocedor no \
         sabe abrir; mueve la carpeta a una ruta sin tildes ni acentos"
    )]
    RutaImposible { donde: String },
    #[error("el modelo de {donde} esta incompleto o no se pudo cargar")]
    ModeloIlegible { donde: String },
    #[error("no se pudo leer {ruta}: no parece un audio que Windows sepa abrir")]
    NoEsAudio { ruta: String },
    #[error("el audio esta vacio: no hay nada que transcribir")]
    AudioVacio,
    /// El motor corrio entero y no saco ni una palabra creible. No es un
    /// fallo del programa: es una nota en la que no se entiende nada, y el
    /// movil tambien la distingue (`Transcriptor.NADA`).
    #[error("no se entendio nada de lo que se dice en el audio")]
    NoSeEntiendeNada,
    /// Windows no trae reconocedor de ese idioma. Se instala desde
    /// Configuracion > Hora e idioma > Voz, o se usa Vosk.
    #[error("Windows no tiene reconocedor de voz para «{idioma}»")]
    SinReconocedorDeWindows { idioma: String },
    #[error("no hay microfono: Windows no tiene ninguno por defecto")]
    SinMicrofono,
    #[error("se cancelo la transcripcion")]
    Cancelada,
    /// Hay ONNX Runtime pero falta el modelo de Whisper: se baja una vez
    /// (unos `megas` MB) a `donde`. La app lo ofrece al pulsar «Pasar a
    /// texto».
    #[error("falta el modelo de voz Whisper: hay que bajarlo (unos {megas} MB) a {donde}")]
    SinModeloWhisper { donde: String, megas: u64 },
    /// Ni `System32` ni `donde` tienen `onnxruntime.dll` (un Windows 10
    /// viejo): se puede poner a mano ahi o junto al ejecutable.
    #[error("este Windows no trae onnxruntime.dll; se puede copiar en {donde}")]
    SinOnnxRuntime { donde: String },
    /// No se pudo bajar un fichero del modelo.
    #[error("no se pudo bajar {que}: {detalle}")]
    Descarga { que: String, detalle: String },
    /// ONNX Runtime fallo en un paso (modelo roto, runtime muy viejo…).
    #[error("ONNX Runtime fallo al {paso}: {detalle}")]
    Onnx { paso: &'static str, detalle: String },
    #[error("Windows fallo al {paso}: {fuente}")]
    Windows {
        paso: &'static str,
        #[source]
        fuente: windows::core::Error,
    },
}

/// Envuelve un fallo de Windows diciendo en que paso ocurrio.
#[cfg(windows)]
pub(crate) fn en(paso: &'static str) -> impl Fn(windows::core::Error) -> ErrorVoz {
    move |fuente| ErrorVoz::Windows { paso, fuente }
}
