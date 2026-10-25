//! **El reconocedor: Vosk, cargado al vuelo.**
//!
//! Puerto de `guardados/MotorVosk.kt`. La diferencia grande con el movil
//! —alli Vosk es una dependencia de Gradle— es **como entra aqui**:
//!
//! `libvosk.dll` no se enlaza al compilar, **se carga con `LoadLibraryW` y
//! se le piden nueve funciones con `GetProcAddress`**. La API de Vosk en C
//! son nueve simbolos y ninguno tiene estructuras: caben en treinta lineas
//! de declaraciones. A cambio:
//!
//! - El repositorio no gana ninguna dependencia de compilacion. Nada de
//!   cmake, nada de bindgen, nada de un `.lib` que tenga que estar presente
//!   para que `cargo test` compile. Un clon limpio en una maquina sin nada
//!   instalado compila y pasa las pruebas igual que hoy.
//! - `cargo-deny` no tiene un solo paquete nuevo que mirar.
//! - Nada se distribuye con la aplicacion: el usuario pone la DLL cuando
//!   quiera transcribir, y si no la pone, el chat lo dice y no pasa nada.
//!
//! Este modulo habla con una libreria en C: cada bloque `unsafe` lleva su
//! `// SAFETY:`.

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::Foundation::HMODULE;
use windows::Win32::Storage::FileSystem::GetShortPathNameW;
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::core::{PCSTR, PCWSTR};

use crate::credibilidad::creible;
use crate::idiomas;
use crate::pcm;
use crate::tiempos::{EstadoDelTexto, Segmento, con_tiempos, estado_de};
use crate::{ErrorVoz, HERCIOS};

/// Cuantas muestras se le dan al reconocedor de una vez.
///
/// El movil le da bloques de 8000 **bytes**, «un cuarto de segundo»
/// (`MotorVosk.kt:166`). Son 4000 muestras de 16 bits; se conserva el mismo
/// tamano porque es tambien el ritmo al que se puede avisar del avance y
/// mirar si el usuario ha cancelado.
const MUESTRAS_POR_BLOQUE: usize = 4_000;

/// Lo que Vosk escribe por su cuenta: solo los avisos, como en el movil
/// (`LogLevel.WARNINGS`, `MotorVosk.kt:151`). Con el nivel de por defecto
/// llena la consola con la estructura del grafo de Kaldi.
const NIVEL_DE_AVISOS: c_int = -1;

// Las nueve funciones de la API en C de Vosk (`vosk_api.h`). Los punteros
// opacos van como `*mut c_void` a proposito: aqui no se toca por dentro ni
// un campo de sus estructuras.
type FnNivel = unsafe extern "C" fn(c_int);
type FnModeloNuevo = unsafe extern "C" fn(*const c_char) -> *mut c_void;
type FnModeloLibre = unsafe extern "C" fn(*mut c_void);
type FnRecNuevo = unsafe extern "C" fn(*mut c_void, f32) -> *mut c_void;
type FnRecPalabras = unsafe extern "C" fn(*mut c_void, c_int);
type FnRecComer = unsafe extern "C" fn(*mut c_void, *const i16, c_int) -> c_int;
type FnRecTexto = unsafe extern "C" fn(*mut c_void) -> *const c_char;
type FnRecLibre = unsafe extern "C" fn(*mut c_void);

/// La libreria ya cargada, con sus funciones resueltas.
struct Libreria {
    /// Se guarda aunque no se use: documenta que el modulo sigue cargado
    /// mientras viva esta estructura.
    #[allow(dead_code)]
    modulo: HMODULE,
    nivel: FnNivel,
    modelo_nuevo: FnModeloNuevo,
    modelo_libre: FnModeloLibre,
    rec_nuevo: FnRecNuevo,
    rec_palabras: FnRecPalabras,
    rec_comer: FnRecComer,
    rec_resultado: FnRecTexto,
    rec_final: FnRecTexto,
    rec_libre: FnRecLibre,
}

impl Libreria {
    fn cargar(dll: &Path) -> Result<Libreria, ErrorVoz> {
        let ancha: Vec<u16> = {
            use std::os::windows::ffi::OsStrExt;
            dll.as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect()
        };
        // SAFETY: `ancha` es una cadena UTF-16 terminada en cero viva
        // durante la llamada. Cargar una DLL ejecuta su `DllMain`, que es
        // por definicion lo que se quiere aqui; la ruta la eligio el
        // usuario poniendo el fichero en su carpeta de datos.
        let modulo =
            unsafe { LoadLibraryW(PCWSTR(ancha.as_ptr())) }.map_err(|_| ErrorVoz::SinMotor {
                donde: dll.display().to_string(),
            })?;

        // SAFETY: el modulo esta cargado y cada nombre es una constante
        // terminada en cero. `transmute` de `FARPROC` al tipo de la
        // funcion es lo que `GetProcAddress` obliga a hacer siempre; las
        // firmas estan copiadas de `vosk_api.h` y son todas `extern "C"`,
        // que en x86-64 de Windows es la misma convencion que `system`.
        // Si falta un simbolo, `GetProcAddress` devuelve nulo y se sale
        // con «el motor no vale» en vez de saltar a la nada.
        let libreria = unsafe {
            macro_rules! pedir {
                ($nombre:literal, $tipo:ty) => {{
                    let simbolo = GetProcAddress(modulo, PCSTR(concat!($nombre, "\0").as_ptr()));
                    match simbolo {
                        Some(f) => {
                            std::mem::transmute::<unsafe extern "system" fn() -> isize, $tipo>(f)
                        }
                        None => {
                            return Err(ErrorVoz::SinMotor {
                                donde: dll.display().to_string(),
                            });
                        }
                    }
                }};
            }
            Libreria {
                modulo,
                nivel: pedir!("vosk_set_log_level", FnNivel),
                modelo_nuevo: pedir!("vosk_model_new", FnModeloNuevo),
                modelo_libre: pedir!("vosk_model_free", FnModeloLibre),
                rec_nuevo: pedir!("vosk_recognizer_new", FnRecNuevo),
                rec_palabras: pedir!("vosk_recognizer_set_words", FnRecPalabras),
                rec_comer: pedir!("vosk_recognizer_accept_waveform_s", FnRecComer),
                rec_resultado: pedir!("vosk_recognizer_result", FnRecTexto),
                rec_final: pedir!("vosk_recognizer_final_result", FnRecTexto),
                rec_libre: pedir!("vosk_recognizer_free", FnRecLibre),
            }
        };

        // SAFETY: la funcion acaba de resolverse del modulo cargado y solo
        // toma un entero.
        unsafe { (libreria.nivel)(NIVEL_DE_AVISOS) };
        Ok(libreria)
    }
}

/// **El motor cargado, con su modelo de un idioma.**
///
/// Cargar un modelo tarda un par de segundos, asi que quien vaya a
/// transcribir varias notas seguidas se queda con el suyo y lo reutiliza,
/// igual que el movil (`MotorVosk.cargado`, `MotorVosk.kt:44`).
///
/// **No es `Send`**, y no por descuido: el modelo es un puntero a una
/// estructura de Kaldi y el crate no promete nada sobre pasarla entre
/// hilos. Se carga en el hilo que transcribe y se muere ahi.
pub struct Motor {
    libreria: Libreria,
    modelo: *mut c_void,
    idioma: String,
}

impl Drop for Motor {
    fn drop(&mut self) {
        // SAFETY: `self.modelo` lo devolvio `vosk_model_new` y no se ha
        // liberado antes: es el unico sitio que lo libera y `Motor` no es
        // copiable. La DLL sigue cargada a proposito —no se llama a
        // `FreeLibrary`— porque descargar una libreria con estado global
        // propio a mitad de la vida del proceso no gana nada y puede
        // costar un cuelgue.
        unsafe { (self.libreria.modelo_libre)(self.modelo) };
    }
}

impl Motor {
    /// Carga el motor y el modelo del idioma que haya en `raiz`.
    ///
    /// `junto_al_exe` es por donde se busca la DLL en segundo lugar; en la
    /// aplicacion es el directorio del ejecutable.
    pub fn cargar(
        raiz: &Path,
        junto_al_exe: Option<&Path>,
        idioma: &str,
    ) -> Result<Motor, ErrorVoz> {
        let modelo_nombre =
            idiomas::modelo_de(idioma).ok_or_else(|| ErrorVoz::IdiomaSinModelo {
                idioma: idioma.to_string(),
            })?;
        let dll = idiomas::buscar_motor(raiz, junto_al_exe).ok_or_else(|| ErrorVoz::SinMotor {
            donde: idiomas::carpeta_de_modelos(raiz).display().to_string(),
        })?;
        let carpeta = idiomas::buscar_modelo(raiz, idioma).ok_or_else(|| ErrorVoz::SinModelo {
            idioma: idioma.to_string(),
            modelo: modelo_nombre.to_string(),
            donde: idiomas::carpeta_de_modelos(raiz).display().to_string(),
        })?;

        let libreria = Libreria::cargar(&dll)?;
        let ruta = ruta_para_kaldi(&carpeta)?;

        // SAFETY: `ruta` es una cadena terminada en cero viva durante toda
        // la llamada; Vosk se queda con una copia, no con el puntero.
        // Devuelve nulo cuando la carpeta no es un modelo, que es el caso
        // que se comprueba justo debajo.
        let modelo = unsafe { (libreria.modelo_nuevo)(ruta.as_ptr()) };
        if modelo.is_null() {
            return Err(ErrorVoz::ModeloIlegible {
                donde: carpeta.display().to_string(),
            });
        }
        Ok(Motor {
            libreria,
            modelo,
            idioma: idioma.to_string(),
        })
    }

    /// Pasa a texto un audio ya decodificado a PCM de 16 bits, mono, a
    /// 16 kHz.
    ///
    /// `avance` se llama con una fraccion de 0 a 1 cada cuarto de segundo
    /// de audio; `cancelado` se mira con la misma frecuencia, para que
    /// cerrar la ventana no deje un hilo royendo cinco minutos de audio.
    pub fn transcribir_pcm(
        &self,
        muestras: &[i16],
        avance: &mut dyn FnMut(f32),
        cancelado: &AtomicBool,
    ) -> Result<Transcripcion, ErrorVoz> {
        if muestras.is_empty() {
            return Err(ErrorVoz::AudioVacio);
        }

        // SAFETY: `self.modelo` sigue vivo (lo libera `Drop`, y `self` esta
        // prestado). Devuelve nulo si no puede crear el reconocedor.
        let rec = unsafe { (self.libreria.rec_nuevo)(self.modelo, HERCIOS as f32) };
        if rec.is_null() {
            return Err(ErrorVoz::ModeloIlegible {
                donde: self.idioma.clone(),
            });
        }
        // A partir de aqui hay un puntero que hay que liberar en todos los
        // caminos: el trabajo se hace en una funcion aparte y se libera al
        // volver, pase lo que pase.
        let salida = self.moler(rec, muestras, avance, cancelado);
        // SAFETY: `rec` lo devolvio `vosk_recognizer_new`, no se ha
        // liberado y nadie mas se quedo con el.
        unsafe { (self.libreria.rec_libre)(rec) };
        salida
    }

    fn moler(
        &self,
        rec: *mut c_void,
        muestras: &[i16],
        avance: &mut dyn FnMut(f32),
        cancelado: &AtomicBool,
    ) -> Result<Transcripcion, ErrorVoz> {
        // SAFETY: `rec` esta vivo hasta que vuelva esta funcion.
        unsafe { (self.libreria.rec_palabras)(rec, 1) };

        let mut segmentos: Vec<Segmento> = Vec::new();
        let mut avisos = 0usize;
        let total = muestras.len() as f32;

        for (n, bloque) in muestras.chunks(MUESTRAS_POR_BLOQUE).enumerate() {
            if cancelado.load(Ordering::Relaxed) {
                return Err(ErrorVoz::Cancelada);
            }
            // SAFETY: `bloque` es un trozo vivo del vector del llamante y se
            // pasa su longitud real en muestras, que es lo que la version
            // `_s` de la funcion espera (la otra cuenta bytes). Vosk lee y
            // no guarda el puntero.
            let cerrada =
                unsafe { (self.libreria.rec_comer)(rec, bloque.as_ptr(), bloque.len() as c_int) };
            if cerrada == 1 {
                // SAFETY: devuelve un puntero a una cadena UTF-8 terminada
                // en cero **propiedad del reconocedor**, valida hasta la
                // siguiente llamada. Se copia a un `String` en el acto.
                let json = unsafe { texto_de(&(self.libreria.rec_resultado)(rec)) };
                apuntar(&json, &self.idioma, &mut segmentos, &mut avisos);
            }
            let hechas = ((n + 1) * MUESTRAS_POR_BLOQUE) as f32;
            avance((hechas / total).min(1.0));
        }

        // SAFETY: igual que arriba; el resultado final cierra lo que quede
        // en el reconocedor.
        let json = unsafe { texto_de(&(self.libreria.rec_final)(rec)) };
        apuntar(&json, &self.idioma, &mut segmentos, &mut avisos);
        avance(1.0);

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

    pub fn idioma(&self) -> &str {
        &self.idioma
    }
}

/// Copia a `String` una cadena en C devuelta por Vosk.
///
/// # Safety
///
/// `puntero` tiene que apuntar a una cadena UTF-8 terminada en cero y valida
/// durante la llamada, que es lo que devuelven `vosk_recognizer_result` y
/// `vosk_recognizer_final_result`.
unsafe fn texto_de(puntero: &*const c_char) -> String {
    if puntero.is_null() {
        return String::new();
    }
    // SAFETY: lo promete el llamante en el contrato de la funcion. Se copia
    // en el acto, asi que nada sobrevive a la siguiente llamada a Vosk.
    unsafe { CStr::from_ptr(*puntero) }
        .to_string_lossy()
        .into_owned()
}

/// Mete en la lista lo que diga un resultado de Vosk, si es creible.
///
/// El JSON es `{"text":"…","result":[{"word":"…","start":0.9,…}]}`. El
/// milisegundo del segmento es el `start` de su primera palabra, igual que
/// en el movil (`MotorVosk.apuntar`, `MotorVosk.kt:155-161`).
fn apuntar(json: &str, idioma: &str, segmentos: &mut Vec<Segmento>, avisos: &mut usize) {
    let Ok(valor) = serde_json::from_str::<serde_json::Value>(json) else {
        return;
    };
    let texto = valor
        .get("text")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .trim();
    if texto.is_empty() {
        // Un trozo callado no es un aviso: es un silencio, y los hay a
        // montones en cualquier nota.
        return;
    }
    if !creible(texto, idioma) {
        *avisos += 1;
        return;
    }
    let desde = valor
        .get("result")
        .and_then(|r| r.as_array())
        .and_then(|r| r.first())
        .and_then(|p| p.get("start"))
        .and_then(|s| s.as_f64())
        .unwrap_or(0.0);
    segmentos.push(Segmento::nuevo((desde * 1000.0) as i64, texto));
}

/// La ruta de la carpeta del modelo, en una cadena que el `fopen` de Kaldi
/// sepa abrir.
///
/// Vosk recibe la ruta como `char*` y por dentro la abre con las funciones
/// de C de toda la vida, que en Windows interpretan los bytes con la pagina
/// de codigos del sistema, no como UTF-8. Con una carpeta de usuario que
/// lleve tildes —«C:\Users\Iñigo\…»— eso falla sin decir por que. Se
/// intenta entonces el **nombre corto** de Windows (`C:\Users\IIGO~1\…`),
/// que es ASCII puro; si tampoco sale ASCII —hay equipos con los nombres
/// cortos desactivados— se dice que mueva la carpeta, que es lo unico
/// honesto.
fn ruta_para_kaldi(carpeta: &Path) -> Result<CString, ErrorVoz> {
    let texto = carpeta.display().to_string();
    if texto.is_ascii() {
        return CString::new(texto).map_err(|_| ErrorVoz::RutaImposible {
            donde: carpeta.display().to_string(),
        });
    }

    let ancha: Vec<u16> = {
        use std::os::windows::ffi::OsStrExt;
        carpeta
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    };
    let mut corta = [0u16; 512];
    // SAFETY: la cadena de entrada esta terminada en cero y viva; el buffer
    // de salida es propio y se pasa con su longitud. Devuelve cuantos u16
    // escribio, sin contar el cero, o 0 si no pudo.
    let escritos = unsafe { GetShortPathNameW(PCWSTR(ancha.as_ptr()), Some(&mut corta)) } as usize;
    let corta = if escritos > 0 && escritos < corta.len() {
        String::from_utf16_lossy(&corta[..escritos])
    } else {
        String::new()
    };
    if corta.is_ascii() && !corta.is_empty() {
        if let Ok(c) = CString::new(corta) {
            return Ok(c);
        }
    }
    Err(ErrorVoz::RutaImposible {
        donde: carpeta.display().to_string(),
    })
}

/// El resultado de pasar una nota a texto: justo lo que hay que escribir en
/// el mensaje del cuaderno.
#[derive(Debug, Clone, PartialEq)]
pub struct Transcripcion {
    /// El cuerpo con marcas, `[1:23] lo que se dijo`, separado por renglon
    /// en blanco. Va a `Mensaje.transcripcion`.
    pub texto: String,
    /// Los trozos sueltos, por si quien llama quiere pintarlos.
    pub segmentos: Vec<Segmento>,
    /// Va a `resto["estadoDelTexto"]`.
    pub estado: EstadoDelTexto,
    pub duracion_ms: i64,
}

/// **Pasa una nota de voz a texto, de principio a fin.**
///
/// Decodifica el fichero, carga el motor y el modelo del idioma y muele.
/// Es la puerta de entrada del crate para quien solo quiere una nota; quien
/// vaya a transcribir varias se queda con un [`Motor`] y lo reutiliza.
///
/// Tarda: se llama desde un hilo de fondo, nunca desde el de la interfaz.
pub fn transcribir(
    audio: &Path,
    raiz: &Path,
    junto_al_exe: Option<&Path>,
    idioma: &str,
    avance: &mut dyn FnMut(f32),
    cancelado: &AtomicBool,
) -> Result<Transcripcion, ErrorVoz> {
    // Primero el motor y el modelo: si falta algo, se entera el usuario en
    // el acto en vez de despues de un minuto decodificando.
    let motor = Motor::cargar(raiz, junto_al_exe, idioma)?;
    let muestras = pcm::decodificar(audio)?;
    if cancelado.load(Ordering::Relaxed) {
        return Err(ErrorVoz::Cancelada);
    }
    motor.transcribir_pcm(&muestras, avance, cancelado)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn un_resultado_con_palabras_se_apunta_con_el_segundo_de_la_primera() {
        let mut segmentos = Vec::new();
        let mut avisos = 0;
        apuntar(
            r#"{"text":"llamar al aparejador","result":[{"word":"llamar","start":1.5}]}"#,
            "es",
            &mut segmentos,
            &mut avisos,
        );
        assert_eq!(segmentos, [Segmento::nuevo(1_500, "llamar al aparejador")]);
        assert_eq!(avisos, 0);
    }

    #[test]
    fn un_trozo_callado_no_cuenta_como_aviso() {
        let mut segmentos = Vec::new();
        let mut avisos = 0;
        apuntar(
            r#"{"text":"","result":[]}"#,
            "es",
            &mut segmentos,
            &mut avisos,
        );
        apuntar(r#"{"text":"   "}"#, "es", &mut segmentos, &mut avisos);
        assert!(segmentos.is_empty());
        assert_eq!(avisos, 0, "el silencio no es un agujero en el texto");
    }

    #[test]
    fn un_trozo_que_no_se_entiende_suma_un_aviso_y_no_ensucia_el_texto() {
        let mut segmentos = Vec::new();
        let mut avisos = 0;
        apuntar(
            r#"{"text":"no no no no no no no no","result":[{"word":"no","start":0.0}]}"#,
            "es",
            &mut segmentos,
            &mut avisos,
        );
        assert!(segmentos.is_empty());
        assert_eq!(avisos, 1);
        assert_eq!(estado_de(segmentos.len(), avisos), EstadoDelTexto::Mal);
    }

    #[test]
    fn un_json_roto_de_una_version_futura_no_tumba_la_transcripcion() {
        let mut segmentos = Vec::new();
        let mut avisos = 0;
        apuntar("esto no es json", "es", &mut segmentos, &mut avisos);
        apuntar(r#"{"otra_cosa":7}"#, "es", &mut segmentos, &mut avisos);
        assert!(segmentos.is_empty());
        assert_eq!(avisos, 0);
    }

    #[test]
    fn un_resultado_sin_tiempos_empieza_en_cero_en_vez_de_perderse() {
        let mut segmentos = Vec::new();
        let mut avisos = 0;
        apuntar(r#"{"text":"hola"}"#, "es", &mut segmentos, &mut avisos);
        assert_eq!(segmentos, [Segmento::nuevo(0, "hola")]);
    }

    #[test]
    fn una_ruta_ascii_se_le_pasa_a_kaldi_tal_cual() {
        let c = ruta_para_kaldi(Path::new("C:/datos/vosk/vosk-model-small-es-0.42")).unwrap();
        assert_eq!(
            c.to_str().unwrap(),
            "C:/datos/vosk/vosk-model-small-es-0.42"
        );
    }

    #[test]
    fn sin_modelo_de_ese_idioma_no_se_llega_ni_a_cargar_la_dll() {
        let vacia = std::env::temp_dir().join("pixpin-voz-sin-nada");
        // `.err()`: `Motor` no es `Debug` —no hay nada legible que ensenar
        // de un puntero a Kaldi— y el mensaje del fallo si lo es.
        let salida = Motor::cargar(&vacia, None, "ja").err();
        assert!(
            matches!(salida, Some(ErrorVoz::IdiomaSinModelo { .. })),
            "se esperaba un idioma sin modelo, y salio {salida:?}"
        );
    }

    #[test]
    fn sin_la_dll_puesta_se_dice_donde_habia_que_ponerla() {
        let vacia = std::env::temp_dir().join("pixpin-voz-sin-dll");
        let salida = Motor::cargar(&vacia, None, "es").err();
        match salida {
            Some(ErrorVoz::SinMotor { donde }) => assert!(donde.ends_with("vosk")),
            otra => panic!("se esperaba que faltara el motor, y salio {otra:?}"),
        }
    }

    /// Necesita aparato de verdad: `libvosk.dll` y un modelo descomprimido
    /// en `%APPDATA%/PixPin Max/vosk/`. Se ejecuta a mano con
    /// `cargo test -p pixpin-voz -- --ignored transcribe_una_nota`.
    #[test]
    #[ignore = "pide libvosk.dll, un modelo descargado y un .m4a de verdad"]
    fn transcribe_una_nota_de_voz_de_verdad() {
        let raiz =
            std::env::var("PIXPIN_VOZ_RAIZ").expect("PIXPIN_VOZ_RAIZ con la carpeta de datos");
        let audio = std::env::var("PIXPIN_VOZ_AUDIO").expect("PIXPIN_VOZ_AUDIO con un .m4a");
        let salida = transcribir(
            Path::new(&audio),
            Path::new(&raiz),
            None,
            "es",
            &mut |_| {},
            &AtomicBool::new(false),
        )
        .expect("la transcripcion tiene que salir");
        assert!(!salida.texto.is_empty());
    }
}
