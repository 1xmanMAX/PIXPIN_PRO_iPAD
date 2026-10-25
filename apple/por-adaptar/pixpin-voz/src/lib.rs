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
//! ## Cuatro motores: Whisper, Qwen3-ASR, el de Windows y Vosk
//!
//! **Actualizado el 2026-10-25.** Para pasar notas y grabaciones (clases,
//! reuniones) a texto hay un motor **opcional**, **Qwen3-ASR-0.6B**
//! (Apache-2.0, ver [`qwen3`] y `docs/voz-qwen3.md`): se elige en Ajustes ›
//! Voz, se baja entonces (unos 988 MB, con sha256) y corre sobre **el mismo
//! ONNX Runtime** que Whisper e Indra, con la exportacion int8 de
//! sherpa-onnx. En el banco de FLEURS es_419 deja el WER en la mitad o menos
//! que Whisper base (limpio, reunion y aula); a cambio pide ~1,5 GB de
//! memoria mientras transcribe y es unas 4 veces mas lento. **Whisper base
//! sigue siendo el de fabrica y el de reserva**: con Qwen3-ASR elegido pero
//! sin bajar, con «todo en el primero» (traducir, que Qwen no hace) o si
//! falla, se pasa con Whisper ([`toca_qwen3`], [`transcribir_con`]).
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
//! - [`tiempos`], [`credibilidad`], [`idiomas`], [`cortes`] y
//!   [`telepronter`]: aritmetica y texto, sin Windows y sin sonido. Es donde
//!   estan las pruebas de verdad. Tambien las partes puras de [`qwen3`]
//!   (mel, tokens, guion, catalogo).
//! - [`ort`] y [`qwen3::motor`]: hablan con ONNX Runtime; compilan tambien
//!   en Linux (con `libonnxruntime.so`) para probar Qwen3-ASR con el modelo
//!   de verdad en la maquina de desarrollo.
//! - [`pcm`] y [`motor`]: lo que toca el sistema operativo y la libreria en
//!   C, con su `unsafe` acotado y comentado.
#![deny(clippy::undocumented_unsafe_blocks)]

pub mod cortes;
pub mod credibilidad;
pub mod dos_idiomas;
/// Las voces de Microsoft Edge para leer en voz alta (los lectores).
pub mod edge;
pub mod idiomas;
/// Qwen3-ASR-0.6B, el motor opcional de las grabaciones (ver `qwen3`).
pub mod qwen3;
pub mod telepronter;
pub mod tiempos;
pub mod turnos;

#[cfg(windows)]
pub mod motor;
/// El enlace a ONNX Runtime (Whisper, Indra y Qwen3-ASR). Compila tambien
/// fuera de Windows, solo para las pruebas (ver `ort`).
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
pub use tiempos::{
    EstadoDelTexto, Segmento, Transcripcion, con_tiempos, marca_de_tiempo, tiempo_de,
};

#[cfg(windows)]
pub use sapi::{AvisoDeDictado, Dictado};

/// Con que se pasa a texto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotorDeVoz {
    /// Whisper *base* sobre el ONNX Runtime que trae Windows; sin red una
    /// vez bajado el modelo. Ver `whisper`.
    Whisper,
    /// Qwen3-ASR-0.6B sobre el mismo runtime, si el usuario lo eligio y lo
    /// bajo. Ver [`qwen3`].
    Qwen3,
    /// El reconocedor que ya trae Windows (SAPI 5, sin red). Ver `sapi`.
    Windows,
    /// Vosk, con los mismos modelos que el movil. Ver `motor`.
    Vosk,
}

/// **El motor que eligio el usuario** para pasar notas y grabaciones a
/// texto (Ajustes › Voz, `[voz] motor`). Whisper es el de fabrica y el de
/// reserva: con Qwen3-ASR elegido pero sin bajar, o si falla, se pasa con
/// Whisper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MotorDeNotas {
    #[default]
    Whisper,
    Qwen3,
}

/// Lo elegido en Ajustes. Lo fija la app al arrancar y al cerrar Ajustes.
static ELEGIDO: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Lo fija la app (ver [`MotorDeNotas`]).
pub fn fijar_motor_de_notas(m: MotorDeNotas) {
    ELEGIDO.store(
        u8::from(m == MotorDeNotas::Qwen3),
        std::sync::atomic::Ordering::Relaxed,
    );
}

/// Lo elegido ahora.
pub fn motor_de_notas() -> MotorDeNotas {
    if ELEGIDO.load(std::sync::atomic::Ordering::Relaxed) == 1 {
        MotorDeNotas::Qwen3
    } else {
        MotorDeNotas::Whisper
    }
}

/// **Si toca Qwen3-ASR** con lo elegido y lo que hay (puro, para probarlo):
/// elegido, bajado, con runtime y con el idioma en su lista. «Todo en el
/// primero» con dos idiomas pide traducir, que Qwen3-ASR no hace: eso
/// sigue con Whisper.
pub fn toca_qwen3(elegido: MotorDeNotas, bajado: bool, runtime: bool, idiomas: &Idiomas) -> bool {
    let traducir = idiomas.otro.is_some() && idiomas.modo == ModoDeIdiomas::TodoEnUno;
    elegido == MotorDeNotas::Qwen3
        && bajado
        && runtime
        && qwen3::sabe(&idiomas.principal)
        && !traducir
}

/// **Que motor usar hoy**, o `None` si no hay ninguno para ese idioma.
///
/// **Actualizado el 2026-09-22**: primero Whisper, si hay ONNX Runtime y el
/// modelo ya esta bajado. El usuario comparo con sus propias notas: el
/// reconocedor de Windows entiende mal el habla natural y Whisper no. Luego
/// el de Windows, que no pide nada. Vosk queda para los idiomas que ninguno
/// de los dos trae; con `preferir_vosk` manda Vosk si esta listo (quien lo
/// puso a proposito).
///
/// **Actualizado el 2026-10-25**: antes que Whisper, Qwen3-ASR si el usuario
/// lo eligio en Ajustes y esta bajado (ver [`toca_qwen3`]).
#[cfg(windows)]
pub fn motor_para(
    raiz: &std::path::Path,
    junto_al_exe: Option<&std::path::Path>,
    idioma: &str,
    preferir_vosk: bool,
) -> Option<MotorDeVoz> {
    motor_para_con(raiz, junto_al_exe, &Idiomas::uno(idioma), preferir_vosk)
}

#[cfg(windows)]
fn motor_para_con(
    raiz: &std::path::Path,
    junto_al_exe: Option<&std::path::Path>,
    idiomas: &Idiomas,
    preferir_vosk: bool,
) -> Option<MotorDeVoz> {
    let idioma = idiomas.principal.as_str();
    let vosk = matches!(
        idiomas::disponibilidad(raiz, junto_al_exe, idioma),
        Disponibilidad::Listo(_)
    );
    if preferir_vosk && vosk {
        return Some(MotorDeVoz::Vosk);
    }
    if toca_qwen3(
        motor_de_notas(),
        qwen3::modelo_listo(raiz),
        whisper::runtime_disponible(raiz, junto_al_exe),
        idiomas,
    ) {
        return Some(MotorDeVoz::Qwen3);
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

/// Si un fallo de Qwen3-ASR merece probar con Whisper: no si se cancelo,
/// ni si el audio no tiene nada (Whisper no lo haria mejor).
pub fn caer_a_whisper(e: &ErrorVoz) -> bool {
    !matches!(
        e,
        ErrorVoz::Cancelada
            | ErrorVoz::NoSeEntiendeNada
            | ErrorVoz::AudioVacio
            | ErrorVoz::NoEsAudio { .. }
    )
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
    transcribir_con(
        audio,
        raiz,
        junto_al_exe,
        &Idiomas::uno(idioma),
        &[],
        avance,
        cancelado,
    )
}

/// **Lo mismo con dos idiomas y con turnos** (B6, B10). Solo Whisper y
/// Qwen3-ASR saben de dos idiomas y de turnos con nombre: con otro motor se
/// transcribe como siempre, en el primero y de corrido, que es lo que haria
/// el movil con Vosk.
///
/// Con Qwen3-ASR elegido, si falla (un modelo roto, un runtime viejo…) se
/// dice en el registro y se pasa con Whisper, que es el de reserva.
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
    let motor = motor_para_con(raiz, junto_al_exe, idiomas, false);
    if motor == Some(MotorDeVoz::Qwen3) {
        let t = std::time::Instant::now();
        match qwen3::motor::transcribir_con(
            audio,
            raiz,
            junto_al_exe,
            idiomas,
            turnos,
            avance,
            cancelado,
        ) {
            Ok(r) => {
                tracing::info!(
                    segundos = t.elapsed().as_secs_f32(),
                    audio_s = r.duracion_ms as f32 / 1000.0,
                    "pasado a texto con Qwen3-ASR"
                );
                return Ok(r);
            }
            Err(e) if caer_a_whisper(&e) => {
                tracing::warn!(error = %e, "Qwen3-ASR fallo; se pasa con Whisper");
                avance(0.0);
            }
            Err(e) => return Err(e),
        }
    }
    let whisper_listo = whisper_posible(raiz, junto_al_exe, idioma) && whisper::modelo_listo(raiz);
    if !whisper_listo && whisper_posible(raiz, junto_al_exe, idioma) {
        return Err(ErrorVoz::SinModeloWhisper {
            donde: whisper::carpeta(raiz).display().to_string(),
            megas: whisper::MEGAS,
        });
    }
    match motor {
        Some(MotorDeVoz::Whisper | MotorDeVoz::Qwen3) => whisper::transcribir_con(
            audio,
            raiz,
            junto_al_exe,
            idiomas,
            turnos,
            avance,
            cancelado,
        ),
        Some(MotorDeVoz::Windows) => sapi::transcribir(audio, idioma, avance, cancelado),
        _ => motor::transcribir(audio, raiz, junto_al_exe, idioma, avance, cancelado),
    }
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
    /// Se eligio Qwen3-ASR en Ajustes pero sus ficheros (unos `megas` MB)
    /// no estan en `donde`. La app no lo baja sola: cae a Whisper y lo dice
    /// en Ajustes.
    #[error("falta el modelo de voz Qwen3-ASR: hay que bajarlo (unos {megas} MB) a {donde}")]
    SinModeloQwen3 { donde: String, megas: u64 },
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
    #[cfg(windows)]
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

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn qwen3_solo_si_se_eligio_esta_bajado_y_hay_runtime() {
        let es = Idiomas::uno("es");
        assert!(toca_qwen3(MotorDeNotas::Qwen3, true, true, &es));
        assert!(
            !toca_qwen3(MotorDeNotas::Whisper, true, true, &es),
            "de fabrica, Whisper"
        );
        assert!(
            !toca_qwen3(MotorDeNotas::Qwen3, false, true, &es),
            "sin bajar"
        );
        assert!(
            !toca_qwen3(MotorDeNotas::Qwen3, true, false, &es),
            "sin runtime"
        );
        assert!(!toca_qwen3(
            MotorDeNotas::Qwen3,
            true,
            true,
            &Idiomas::uno("eu")
        ));
        // Dos idiomas, cada trozo como se dijo: Qwen3 adivina por tanda.
        let dos = Idiomas::con("es", "en", ModoDeIdiomas::CadaUno);
        assert!(toca_qwen3(MotorDeNotas::Qwen3, true, true, &dos));
        // Todo en el primero pide traducir: Whisper.
        let traducir = Idiomas::con("en", "es", ModoDeIdiomas::TodoEnUno);
        assert!(!toca_qwen3(MotorDeNotas::Qwen3, true, true, &traducir));
    }

    #[test]
    fn un_fallo_de_qwen3_cae_a_whisper_salvo_cancelar_o_audio_sin_nada() {
        assert!(caer_a_whisper(&ErrorVoz::Onnx {
            paso: "abrir el modelo",
            detalle: String::new()
        }));
        assert!(caer_a_whisper(&ErrorVoz::SinModeloQwen3 {
            donde: String::new(),
            megas: 988
        }));
        assert!(!caer_a_whisper(&ErrorVoz::Cancelada));
        assert!(!caer_a_whisper(&ErrorVoz::NoSeEntiendeNada));
        assert!(!caer_a_whisper(&ErrorVoz::AudioVacio));
    }

    #[test]
    fn lo_elegido_va_y_vuelve() {
        assert_eq!(MotorDeNotas::default(), MotorDeNotas::Whisper);
        fijar_motor_de_notas(MotorDeNotas::Qwen3);
        assert_eq!(motor_de_notas(), MotorDeNotas::Qwen3);
        fijar_motor_de_notas(MotorDeNotas::Whisper);
        assert_eq!(motor_de_notas(), MotorDeNotas::Whisper);
    }
}
