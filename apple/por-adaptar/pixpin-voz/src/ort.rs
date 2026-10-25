//! **Enlace propio y minimo a la API en C de ONNX Runtime, cargada al vuelo.**
//!
//! Windows 11 (y los Windows 10 recientes) traen `onnxruntime.dll` en
//! `System32`: es el que usa el propio sistema para sus funciones de IA. Aqui
//! se aprovecha ese mismo fichero para correr Whisper, sin anadir ni un
//! crate ni un byte al ejecutable: igual que con Vosk (`motor.rs`), la DLL
//! **se carga con `LoadLibraryW`** y no se enlaza.
//!
//! Por que no el crate `ort`: baja su propio runtime al compilar (o exige
//! uno instalado para enlazar), mete decenas de dependencias en el arbol de
//! `cargo-deny` y trae mucho mas de lo que hace falta. Whisper usa veinte
//! funciones de la API; caben aqui.
//!
//! ## Como se habla con ONNX Runtime
//!
//! La DLL exporta **un solo simbolo**, `OrtGetApiBase`. Devuelve una
//! estructura con `GetApi(version)`, que a su vez devuelve **una tabla de
//! punteros a funcion** (`struct OrtApi`). Esa tabla solo crece por el
//! final entre versiones —es la promesa de estabilidad de ONNX Runtime—, asi
//! que la posicion de cada funcion es fija: basta con saber el indice.
//!
//! **Los indices no estan adivinados.** Salen de contar, en orden, las
//! entradas de `struct OrtApi` en el `onnxruntime_c_api.h` oficial del tag
//! `v1.17.3` de `microsoft/onnxruntime`
//! (`include/onnxruntime/core/session/onnxruntime_c_api.h`, lineas 719 a
//! 4569). Se contaron con un guion que quita los comentarios y numera cada
//! `ORT_API2_STATUS(Nombre, …)`, cada `ORT_CLASS_RELEASE(X)` (que es
//! `ReleaseX`) y cada `(ORT_API_CALL* Nombre)`: 276 entradas, de
//! `CreateStatus` (0) a `SessionOptionsAppendExecutionProvider_OpenVINO_V2`
//! (275). La prueba `los_indices_de_la_tabla_estan_en_orden` guarda que no
//! se descoloquen al tocarlos.
//!
//! Todas las funciones son `ORT_API_CALL`, que en Windows es `__stdcall`:
//! `extern "system"` en Rust (en x86-64 es la misma convencion que C).
//!
//! Este modulo habla con una libreria en C: cada `unsafe` lleva su
//! `// SAFETY:`.

use std::ffi::{CStr, CString, c_char, c_void};
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use windows::Win32::System::LibraryLoader::{
    GetProcAddress, LOAD_WITH_ALTERED_SEARCH_PATH, LoadLibraryExW,
};
use windows::core::{PCSTR, PCWSTR};

use crate::ErrorVoz;

/// El nombre del fichero, para buscarlo y para decir cual falta.
pub const NOMBRE_DE_LA_DLL: &str = "onnxruntime.dll";

/// La version de la tabla que se pide primero: la del `.h` del que salen los
/// indices (`ORT_API_VERSION 17`, linea 41).
const VERSION_DE_LA_API: u32 = 17;

/// La mas vieja que se acepta. Todas las funciones de aqui estan por debajo
/// del indice 119, que ya existian en la API 4 (ONNX Runtime 1.4); se pide
/// hasta la 7 por margen, y por debajo se dice que el runtime es demasiado
/// viejo en vez de leer una tabla mas corta que los indices.
const VERSION_MINIMA: u32 = 7;

/// Posiciones en `struct OrtApi` (ver la cabecera del modulo).
mod indice {
    pub const GET_ERROR_MESSAGE: usize = 2;
    pub const CREATE_ENV: usize = 3;
    pub const CREATE_SESSION: usize = 7;
    pub const RUN: usize = 9;
    pub const CREATE_SESSION_OPTIONS: usize = 10;
    pub const DISABLE_MEM_PATTERN: usize = 17;
    pub const DISABLE_CPU_MEM_ARENA: usize = 19;
    pub const SET_SESSION_GRAPH_OPTIMIZATION_LEVEL: usize = 23;
    pub const SET_INTRA_OP_NUM_THREADS: usize = 24;
    pub const SESSION_GET_INPUT_COUNT: usize = 30;
    pub const SESSION_GET_OUTPUT_COUNT: usize = 31;
    pub const SESSION_GET_INPUT_NAME: usize = 36;
    pub const SESSION_GET_OUTPUT_NAME: usize = 37;
    pub const CREATE_TENSOR_WITH_DATA_AS_ORT_VALUE: usize = 49;
    pub const GET_TENSOR_MUTABLE_DATA: usize = 51;
    pub const GET_TENSOR_ELEMENT_TYPE: usize = 60;
    pub const GET_DIMENSIONS_COUNT: usize = 61;
    pub const GET_DIMENSIONS: usize = 62;
    pub const GET_TENSOR_TYPE_AND_SHAPE: usize = 65;
    pub const CREATE_CPU_MEMORY_INFO: usize = 69;
    pub const ALLOCATOR_FREE: usize = 76;
    pub const GET_ALLOCATOR_WITH_DEFAULT_OPTIONS: usize = 78;
    pub const RELEASE_STATUS: usize = 93;
    pub const RELEASE_SESSION: usize = 95;
    pub const RELEASE_VALUE: usize = 96;
    pub const RELEASE_TENSOR_TYPE_AND_SHAPE_INFO: usize = 99;
    pub const RELEASE_SESSION_OPTIONS: usize = 100;
    pub const SESSION_GET_MODEL_METADATA: usize = 111;
    pub const MODEL_METADATA_LOOKUP_CUSTOM_METADATA_MAP: usize = 116;
    pub const RELEASE_MODEL_METADATA: usize = 118;

    /// Todos, en el orden de la tabla, para la prueba de orden.
    #[cfg(test)]
    pub const TODOS: [usize; 30] = [
        GET_ERROR_MESSAGE,
        CREATE_ENV,
        CREATE_SESSION,
        RUN,
        CREATE_SESSION_OPTIONS,
        DISABLE_MEM_PATTERN,
        DISABLE_CPU_MEM_ARENA,
        SET_SESSION_GRAPH_OPTIMIZATION_LEVEL,
        SET_INTRA_OP_NUM_THREADS,
        SESSION_GET_INPUT_COUNT,
        SESSION_GET_OUTPUT_COUNT,
        SESSION_GET_INPUT_NAME,
        SESSION_GET_OUTPUT_NAME,
        CREATE_TENSOR_WITH_DATA_AS_ORT_VALUE,
        GET_TENSOR_MUTABLE_DATA,
        GET_TENSOR_ELEMENT_TYPE,
        GET_DIMENSIONS_COUNT,
        GET_DIMENSIONS,
        GET_TENSOR_TYPE_AND_SHAPE,
        CREATE_CPU_MEMORY_INFO,
        ALLOCATOR_FREE,
        GET_ALLOCATOR_WITH_DEFAULT_OPTIONS,
        RELEASE_STATUS,
        RELEASE_SESSION,
        RELEASE_VALUE,
        RELEASE_TENSOR_TYPE_AND_SHAPE_INFO,
        RELEASE_SESSION_OPTIONS,
        SESSION_GET_MODEL_METADATA,
        MODEL_METADATA_LOOKUP_CUSTOM_METADATA_MAP,
        RELEASE_MODEL_METADATA,
    ];
}

// Valores de los `enum` de la cabecera que se usan aqui.
/// `ORT_LOGGING_LEVEL_WARNING`: solo avisos, como con Vosk.
const NIVEL_AVISOS: i32 = 2;
/// `ORT_ENABLE_ALL`.
const OPTIMIZAR_TODO: i32 = 99;
/// `OrtArenaAllocator` y `OrtMemTypeDefault`.
const ASIGNADOR_ARENA: i32 = 1;
const MEMORIA_POR_DEFECTO: i32 = 0;
/// `ONNX_TENSOR_ELEMENT_DATA_TYPE_FLOAT` e `…_INT64`.
const TIPO_F32: i32 = 1;
const TIPO_I64: i32 = 7;

// Firmas de la cabecera. `OrtStatus*` es nulo si fue bien.
type Estado = *mut c_void;
type FnGetApi = unsafe extern "system" fn(u32) -> *const *const c_void;
type FnVersion = unsafe extern "system" fn() -> *const c_char;
#[repr(C)]
struct OrtApiBase {
    get_api: FnGetApi,
    get_version_string: FnVersion,
}
type FnGetApiBase = unsafe extern "system" fn() -> *const OrtApiBase;

type FnMensaje = unsafe extern "system" fn(*const c_void) -> *const c_char;
type FnSoltar = unsafe extern "system" fn(*mut c_void);
type FnCrearEntorno = unsafe extern "system" fn(i32, *const c_char, *mut *mut c_void) -> Estado;
type FnCrearOpciones = unsafe extern "system" fn(*mut *mut c_void) -> Estado;
type FnOpcionEntera = unsafe extern "system" fn(*mut c_void, i32) -> Estado;
/// `DisableCpuMemArena` y `DisableMemPattern`: solo las opciones.
type FnOpcionSola = unsafe extern "system" fn(*mut c_void) -> Estado;
type FnCrearSesion =
    unsafe extern "system" fn(*const c_void, *const u16, *const c_void, *mut *mut c_void) -> Estado;
type FnCuenta = unsafe extern "system" fn(*const c_void, *mut usize) -> Estado;
type FnNombre =
    unsafe extern "system" fn(*const c_void, usize, *mut c_void, *mut *mut c_char) -> Estado;
type FnCorrer = unsafe extern "system" fn(
    *mut c_void,
    *const c_void,
    *const *const c_char,
    *const *const c_void,
    usize,
    *const *const c_char,
    usize,
    *mut *mut c_void,
) -> Estado;
type FnCrearTensor = unsafe extern "system" fn(
    *const c_void,
    *mut c_void,
    usize,
    *const i64,
    usize,
    i32,
    *mut *mut c_void,
) -> Estado;
type FnDatos = unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> Estado;
type FnTipoDeElemento = unsafe extern "system" fn(*const c_void, *mut i32) -> Estado;
type FnDimensiones = unsafe extern "system" fn(*const c_void, *mut i64, usize) -> Estado;
type FnObtener = unsafe extern "system" fn(*const c_void, *mut *mut c_void) -> Estado;
type FnCrearMemoria = unsafe extern "system" fn(i32, i32, *mut *mut c_void) -> Estado;
type FnLiberar = unsafe extern "system" fn(*mut c_void, *mut c_void) -> Estado;
type FnAsignador = unsafe extern "system" fn(*mut *mut c_void) -> Estado;
type FnBuscarMetadato = unsafe extern "system" fn(
    *const c_void,
    *mut c_void,
    *const c_char,
    *mut *mut c_char,
) -> Estado;

/// **Donde se busca el runtime**, en orden: el de Windows, el que haya
/// puesto el usuario en `<datos>/whisper/` y el de junto al ejecutable.
///
/// El de `System32` va primero porque es el que se mantiene solo con
/// Windows Update; los otros dos son para los Windows 10 viejos que no lo
/// traen.
pub fn candidatos(raiz: &Path, junto_al_exe: Option<&Path>) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(sistema) = carpeta_del_sistema() {
        v.push(sistema.join(NOMBRE_DE_LA_DLL));
    }
    v.push(raiz.join("whisper").join(NOMBRE_DE_LA_DLL));
    if let Some(exe) = junto_al_exe {
        v.push(exe.join(NOMBRE_DE_LA_DLL));
    }
    v
}

/// El primer candidato que existe, sin cargarlo: es lo que se mira para
/// decidir el motor, y cargar 20 MB de DLL para eso no tiene sentido.
pub fn buscar(raiz: &Path, junto_al_exe: Option<&Path>) -> Option<PathBuf> {
    candidatos(raiz, junto_al_exe)
        .into_iter()
        .find(|p| p.is_file())
}

fn carpeta_del_sistema() -> Option<PathBuf> {
    let mut buf = [0u16; 260];
    // SAFETY: el buffer es propio y se pasa con su longitud; devuelve
    // cuantos u16 escribio (sin el cero) o 0 si fallo, o mas que el buffer
    // si no cabia, que se trata como fallo.
    let n =
        unsafe { windows::Win32::System::SystemInformation::GetSystemDirectoryW(Some(&mut buf)) }
            as usize;
    (n > 0 && n < buf.len()).then(|| PathBuf::from(String::from_utf16_lossy(&buf[..n])))
}

fn ancha(p: &Path) -> Vec<u16> {
    p.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// **El runtime cargado**, con su tabla, su entorno y su asignador.
///
/// Uno por proceso: ONNX Runtime pide un solo `OrtEnv` y la DLL no se
/// descarga nunca (igual que `libvosk.dll`: descargar una libreria con
/// hilos propios a mitad de vida no gana nada).
pub struct Runtime {
    tabla: *const *const c_void,
    entorno: *mut c_void,
    asignador: *mut c_void,
    memoria: *mut c_void,
    /// «1.17.260311-…»: para el registro.
    pub version: String,
    pub dll: PathBuf,
}

// SAFETY: la tabla es de solo lectura y vive lo que el proceso. El `OrtEnv`,
// el asignador por defecto y la `OrtMemoryInfo` de CPU son, segun la
// documentacion de ONNX Runtime, seguros entre hilos; ninguno se toca por
// dentro desde Rust.
unsafe impl Send for Runtime {}
// SAFETY: ver arriba.
unsafe impl Sync for Runtime {}

static RUNTIME: OnceLock<Result<Runtime, String>> = OnceLock::new();

impl Runtime {
    /// El runtime del proceso, cargandolo la primera vez.
    ///
    /// Si falla, el fallo tambien se guarda: volver a intentar cargar una
    /// DLL que no esta o que es de otra version da lo mismo cada vez.
    pub fn del_proceso(
        raiz: &Path,
        junto_al_exe: Option<&Path>,
    ) -> Result<&'static Runtime, ErrorVoz> {
        let r = RUNTIME.get_or_init(|| {
            let Some(dll) = buscar(raiz, junto_al_exe) else {
                return Err(String::new());
            };
            Runtime::cargar(&dll).map_err(|e| format!("{}: {e}", dll.display()))
        });
        match r {
            Ok(rt) => Ok(rt),
            Err(e) if e.is_empty() => Err(ErrorVoz::SinOnnxRuntime {
                donde: raiz.join("whisper").display().to_string(),
            }),
            Err(e) => Err(ErrorVoz::Onnx {
                paso: "cargar el runtime",
                detalle: e.clone(),
            }),
        }
    }

    fn cargar(dll: &Path) -> Result<Runtime, String> {
        let ruta = ancha(dll);
        // SAFETY: `ruta` es UTF-16 terminada en cero y viva durante la
        // llamada. `LOAD_WITH_ALTERED_SEARCH_PATH` hace que las dependencias
        // de una DLL puesta a mano se busquen a su lado y no junto al exe.
        let modulo =
            unsafe { LoadLibraryExW(PCWSTR(ruta.as_ptr()), None, LOAD_WITH_ALTERED_SEARCH_PATH) }
                .map_err(|e| format!("no se pudo cargar: {e}"))?;

        // SAFETY: el modulo esta cargado y el nombre es una constante
        // terminada en cero. La firma de `OrtGetApiBase` es la de la
        // cabecera (linea 674): sin argumentos, devuelve `const OrtApiBase*`.
        let base = unsafe {
            let f = GetProcAddress(modulo, PCSTR(c"OrtGetApiBase".as_ptr().cast()))
                .ok_or("la DLL no exporta OrtGetApiBase: no es ONNX Runtime")?;
            let f: FnGetApiBase = std::mem::transmute(f);
            f()
        };
        if base.is_null() {
            return Err("OrtGetApiBase devolvio nulo".into());
        }
        // SAFETY: `base` no es nulo y apunta a la estructura estatica de la
        // DLL (dos punteros a funcion, `repr(C)` igual que en la cabecera).
        let (get_api, get_version) = unsafe { ((*base).get_api, (*base).get_version_string) };
        // SAFETY: devuelve una cadena estatica de la DLL, terminada en cero.
        let version = unsafe {
            let p = get_version();
            if p.is_null() {
                String::new()
            } else {
                CStr::from_ptr(p).to_string_lossy().into_owned()
            }
        };

        // La mas nueva que conozca la DLL, sin pasar de la 17: una DLL mas
        // nueva sirve la tabla vieja tal cual (solo crece por el final), y
        // una mas vieja devuelve nulo para las versiones que no sabe.
        let mut tabla = std::ptr::null();
        for v in (VERSION_MINIMA..=VERSION_DE_LA_API).rev() {
            // SAFETY: `GetApi` solo lee el entero y devuelve nulo si no
            // conoce la version.
            tabla = unsafe { get_api(v) };
            if !tabla.is_null() {
                break;
            }
        }
        if tabla.is_null() {
            return Err(format!("ONNX Runtime {version} es demasiado viejo"));
        }

        let mut rt = Runtime {
            tabla,
            entorno: std::ptr::null_mut(),
            asignador: std::ptr::null_mut(),
            memoria: std::ptr::null_mut(),
            version,
            dll: dll.to_path_buf(),
        };
        let mut entorno = std::ptr::null_mut();
        // SAFETY: firma de `CreateEnv`; el identificador es una constante
        // terminada en cero y la salida una variable local.
        let st = unsafe {
            rt.f::<FnCrearEntorno>(indice::CREATE_ENV)(
                NIVEL_AVISOS,
                c"pixpin-voz".as_ptr(),
                &mut entorno,
            )
        };
        rt.comprobar(st, "crear el entorno")
            .map_err(|e| e.to_string())?;
        rt.entorno = entorno;

        let mut asignador = std::ptr::null_mut();
        // SAFETY: firma de `GetAllocatorWithDefaultOptions`; el asignador
        // devuelto es de la DLL y no se libera nunca (lo dice la cabecera).
        let st = unsafe {
            rt.f::<FnAsignador>(indice::GET_ALLOCATOR_WITH_DEFAULT_OPTIONS)(&mut asignador)
        };
        rt.comprobar(st, "pedir el asignador")
            .map_err(|e| e.to_string())?;
        rt.asignador = asignador;

        let mut memoria = std::ptr::null_mut();
        // SAFETY: firma de `CreateCpuMemoryInfo` con los valores de sus
        // `enum`; la salida es una variable local.
        let st = unsafe {
            rt.f::<FnCrearMemoria>(indice::CREATE_CPU_MEMORY_INFO)(
                ASIGNADOR_ARENA,
                MEMORIA_POR_DEFECTO,
                &mut memoria,
            )
        };
        rt.comprobar(st, "describir la memoria")
            .map_err(|e| e.to_string())?;
        rt.memoria = memoria;
        Ok(rt)
    }

    /// La funcion de la tabla en `i`, con su firma.
    ///
    /// # Safety
    ///
    /// `F` tiene que ser exactamente la firma de la entrada `i` de
    /// `struct OrtApi` en la cabecera v1.17.3, e `i` uno de los de
    /// [`indice`], todos por debajo de lo que la version pedida garantiza.
    unsafe fn f<F: Copy>(&self, i: usize) -> F {
        // SAFETY: lo promete el llamante; la tabla tiene al menos `i + 1`
        // entradas y cada una es un puntero a funcion no nulo.
        unsafe {
            let p = *self.tabla.add(i);
            std::mem::transmute_copy::<*const c_void, F>(&p)
        }
    }

    /// Convierte un `OrtStatus*` en `Result`, liberandolo.
    fn comprobar(&self, st: Estado, paso: &'static str) -> Result<(), ErrorVoz> {
        if st.is_null() {
            return Ok(());
        }
        // SAFETY: `st` no es nulo y lo acaba de devolver la API;
        // `GetErrorMessage` devuelve una cadena propiedad del estado, que se
        // copia antes de liberarlo con `ReleaseStatus`.
        let detalle = unsafe {
            let m = self.f::<FnMensaje>(indice::GET_ERROR_MESSAGE)(st);
            let texto = if m.is_null() {
                String::new()
            } else {
                CStr::from_ptr(m).to_string_lossy().into_owned()
            };
            self.f::<FnSoltar>(indice::RELEASE_STATUS)(st);
            texto
        };
        Err(ErrorVoz::Onnx { paso, detalle })
    }

    /// **Abre un modelo `.onnx`**, en CPU, con `hilos` hilos por operacion.
    pub fn abrir(&self, modelo: &Path, hilos: usize) -> Result<Sesion<'_>, ErrorVoz> {
        self.abrir_con(modelo, hilos, true)
    }

    /// Como [`abrir`](Runtime::abrir), diciendo si ONNX Runtime usa su
    /// arena de memoria (mas rapido, mucha mas memoria).
    pub fn abrir_con(
        &self,
        modelo: &Path,
        hilos: usize,
        arena: bool,
    ) -> Result<Sesion<'_>, ErrorVoz> {
        let mut opciones = std::ptr::null_mut();
        // SAFETY: firma de `CreateSessionOptions`.
        let st =
            unsafe { self.f::<FnCrearOpciones>(indice::CREATE_SESSION_OPTIONS)(&mut opciones) };
        self.comprobar(st, "crear las opciones")?;
        let ruta = ancha(modelo);
        let mut sesion = std::ptr::null_mut();
        // SAFETY: `opciones` esta viva hasta el `ReleaseSessionOptions` de
        // abajo, que se hace en todos los caminos (entre medias no hay `?`).
        // La ruta es UTF-16 terminada en cero: en Windows `ORTCHAR_T` es
        // `wchar_t`, asi que las tildes de la carpeta del usuario no son un
        // problema como lo son con Vosk.
        let st = unsafe {
            let a = self.f::<FnOpcionEntera>(indice::SET_INTRA_OP_NUM_THREADS)(
                opciones,
                hilos.clamp(1, 64) as i32,
            );
            let b = self.f::<FnOpcionEntera>(indice::SET_SESSION_GRAPH_OPTIMIZATION_LEVEL)(
                opciones,
                OPTIMIZAR_TODO,
            );
            // Un fallo en una opcion no impide abrir: se suelta y se sigue
            // con los valores por defecto.
            // **Sin arena ni patron de memoria en modo cuidadoso**: los dos
            // guardan lo mas grande que se pidio para no volver a pedirlo, y
            // con el decoder de 130 MB eso deja cientos de megas cogidos.
            // Medido con una nota de 22,7 s: pico de 720 MB con arena, 350 MB
            // sin ella, a cambio de 1,5-2 veces mas CPU. En 4 GB manda la
            // memoria (decision del usuario, como el modo cuidadoso de
            // pdfsqueeze).
            let (c, d) = if arena {
                (std::ptr::null_mut(), std::ptr::null_mut())
            } else {
                (
                    self.f::<FnOpcionSola>(indice::DISABLE_CPU_MEM_ARENA)(opciones),
                    self.f::<FnOpcionSola>(indice::DISABLE_MEM_PATTERN)(opciones),
                )
            };
            for s in [a, b, c, d] {
                if !s.is_null() {
                    self.f::<FnSoltar>(indice::RELEASE_STATUS)(s);
                }
            }
            let st = self.f::<FnCrearSesion>(indice::CREATE_SESSION)(
                self.entorno,
                ruta.as_ptr(),
                opciones,
                &mut sesion,
            );
            self.f::<FnSoltar>(indice::RELEASE_SESSION_OPTIONS)(opciones);
            st
        };
        self.comprobar(st, "abrir el modelo")?;
        let mut s = Sesion {
            rt: self,
            ptr: sesion,
            entradas: Vec::new(),
            salidas: Vec::new(),
        };
        s.entradas = s.nombres(
            indice::SESSION_GET_INPUT_COUNT,
            indice::SESSION_GET_INPUT_NAME,
        )?;
        s.salidas = s.nombres(
            indice::SESSION_GET_OUTPUT_COUNT,
            indice::SESSION_GET_OUTPUT_NAME,
        )?;
        Ok(s)
    }

    /// Libera una cadena que la API asigno con el asignador por defecto.
    ///
    /// # Safety
    ///
    /// `p` la tuvo que devolver la API con `self.asignador` y no estar ya
    /// liberada.
    unsafe fn soltar_cadena(&self, p: *mut c_char) -> String {
        if p.is_null() {
            return String::new();
        }
        // SAFETY: lo promete el llamante; se copia antes de liberar.
        unsafe {
            let s = CStr::from_ptr(p).to_string_lossy().into_owned();
            let st = self.f::<FnLiberar>(indice::ALLOCATOR_FREE)(self.asignador, p.cast());
            if !st.is_null() {
                self.f::<FnSoltar>(indice::RELEASE_STATUS)(st);
            }
            s
        }
    }

    /// Un tensor de `f32` que **se queda con** `datos`: ONNX Runtime no los
    /// copia, asi que el vector tiene que vivir lo que viva el tensor.
    pub fn tensor_f32(&self, datos: Vec<f32>, forma: &[i64]) -> Result<Valor<'_>, ErrorVoz> {
        self.tensor(Respaldo::F32(datos), forma)
    }

    /// Lo mismo con `i64` (los tokens y el desplazamiento de Whisper).
    pub fn tensor_i64(&self, datos: Vec<i64>, forma: &[i64]) -> Result<Valor<'_>, ErrorVoz> {
        self.tensor(Respaldo::I64(datos), forma)
    }

    fn tensor(&self, mut respaldo: Respaldo, forma: &[i64]) -> Result<Valor<'_>, ErrorVoz> {
        let elementos: i64 = forma.iter().product();
        let (puntero, bytes, tipo, largo) = match &mut respaldo {
            Respaldo::F32(v) => (
                v.as_mut_ptr().cast::<c_void>(),
                v.len() * 4,
                TIPO_F32,
                v.len(),
            ),
            Respaldo::I64(v) => (
                v.as_mut_ptr().cast::<c_void>(),
                v.len() * 8,
                TIPO_I64,
                v.len(),
            ),
            Respaldo::Suyo => (std::ptr::null_mut(), 0, TIPO_F32, 0),
        };
        if elementos < 0 || elementos as usize != largo {
            return Err(ErrorVoz::Onnx {
                paso: "crear un tensor",
                detalle: format!("forma {forma:?} para {largo} elementos"),
            });
        }
        let mut valor = std::ptr::null_mut();
        // SAFETY: el puntero y los bytes son los del vector, cuyo buffer en
        // el monton no se mueve aunque se mueva el `Vec` (queda dentro del
        // `Valor` y se suelta despues del tensor, en `Drop`). La forma es un
        // slice vivo durante la llamada; la API la copia.
        let st = unsafe {
            self.f::<FnCrearTensor>(indice::CREATE_TENSOR_WITH_DATA_AS_ORT_VALUE)(
                self.memoria,
                puntero,
                bytes,
                forma.as_ptr(),
                forma.len(),
                tipo,
                &mut valor,
            )
        };
        self.comprobar(st, "crear un tensor")?;
        Ok(Valor {
            rt: self,
            ptr: valor,
            _respaldo: respaldo,
        })
    }
}

/// Los datos de un tensor creado desde Rust. Solo esta para mantenerlos
/// vivos: ONNX Runtime los lee por su puntero.
enum Respaldo {
    F32(Vec<f32>),
    I64(Vec<i64>),
    /// Tensores que asigno la propia API (las salidas).
    Suyo,
}

/// Un `OrtValue` propio: se libera al soltarlo.
pub struct Valor<'r> {
    rt: &'r Runtime,
    ptr: *mut c_void,
    _respaldo: Respaldo,
}

impl Drop for Valor<'_> {
    fn drop(&mut self) {
        // SAFETY: `ptr` lo devolvio la API (al crear el tensor o en `Run`),
        // es de este `Valor` y no se ha liberado; el respaldo, si lo hay, se
        // suelta despues, cuando ya nadie lo mira.
        unsafe { self.rt.f::<FnSoltar>(indice::RELEASE_VALUE)(self.ptr) };
    }
}

impl Valor<'_> {
    /// La forma del tensor: `[1, 80, 3000]`.
    pub fn forma(&self) -> Result<Vec<i64>, ErrorVoz> {
        let rt = self.rt;
        let mut info = std::ptr::null_mut();
        // SAFETY: `ptr` es un tensor vivo; `info` se libera en todos los
        // caminos antes de salir.
        unsafe {
            let st = rt.f::<FnObtener>(indice::GET_TENSOR_TYPE_AND_SHAPE)(self.ptr, &mut info);
            rt.comprobar(st, "leer la forma de un tensor")?;
            let mut n = 0usize;
            let st = rt.f::<FnCuenta>(indice::GET_DIMENSIONS_COUNT)(info, &mut n);
            let mut dims = vec![0i64; n];
            let st2 = if st.is_null() {
                rt.f::<FnDimensiones>(indice::GET_DIMENSIONS)(info, dims.as_mut_ptr(), n)
            } else {
                std::ptr::null_mut()
            };
            let mut tipo = 0i32;
            let st3 = rt.f::<FnTipoDeElemento>(indice::GET_TENSOR_ELEMENT_TYPE)(info, &mut tipo);
            rt.f::<FnSoltar>(indice::RELEASE_TENSOR_TYPE_AND_SHAPE_INFO)(info);
            rt.comprobar(st, "contar las dimensiones")?;
            rt.comprobar(st2, "leer las dimensiones")?;
            rt.comprobar(st3, "leer el tipo de un tensor")?;
            Ok(dims)
        }
    }

    /// Los datos de un tensor de `f32`, sin copiarlos.
    pub fn como_f32(&self) -> Result<&[f32], ErrorVoz> {
        let n: i64 = self.forma()?.iter().product();
        let mut datos = std::ptr::null_mut();
        // SAFETY: `ptr` es un tensor vivo de `f32` (lo son todas las salidas
        // de Whisper, y las entradas se crearon aqui con su tipo); el
        // puntero vale mientras viva `self`, y el slice toma prestado `self`.
        unsafe {
            let st = self.rt.f::<FnDatos>(indice::GET_TENSOR_MUTABLE_DATA)(self.ptr, &mut datos);
            self.rt.comprobar(st, "leer un tensor")?;
            if datos.is_null() || n <= 0 {
                return Ok(&[]);
            }
            Ok(std::slice::from_raw_parts(datos.cast::<f32>(), n as usize))
        }
    }
}

/// Un modelo abierto, con los nombres de sus entradas y salidas.
pub struct Sesion<'r> {
    rt: &'r Runtime,
    ptr: *mut c_void,
    pub entradas: Vec<CString>,
    pub salidas: Vec<CString>,
}

impl Drop for Sesion<'_> {
    fn drop(&mut self) {
        // SAFETY: `ptr` lo devolvio `CreateSession`, es de esta sesion y no
        // se ha liberado.
        unsafe { self.rt.f::<FnSoltar>(indice::RELEASE_SESSION)(self.ptr) };
    }
}

impl<'r> Sesion<'r> {
    fn nombres(&self, cuenta: usize, nombre: usize) -> Result<Vec<CString>, ErrorVoz> {
        let mut n = 0usize;
        // SAFETY: la sesion esta viva; la salida es una variable local.
        let st = unsafe { self.rt.f::<FnCuenta>(cuenta)(self.ptr, &mut n) };
        self.rt.comprobar(st, "contar las entradas del modelo")?;
        let mut v = Vec::with_capacity(n);
        for i in 0..n {
            let mut p = std::ptr::null_mut();
            // SAFETY: `i < n`; el nombre lo asigna la API con el asignador
            // por defecto y se libera en `soltar_cadena` tras copiarlo.
            let texto = unsafe {
                let st = self.rt.f::<FnNombre>(nombre)(self.ptr, i, self.rt.asignador, &mut p);
                self.rt.comprobar(st, "leer el nombre de una entrada")?;
                self.rt.soltar_cadena(p)
            };
            v.push(CString::new(texto).unwrap_or_default());
        }
        Ok(v)
    }

    /// Un valor de los metadatos propios del modelo (`custom_metadata_map`),
    /// o `None` si no lo trae.
    pub fn metadato(&self, clave: &str) -> Result<Option<String>, ErrorVoz> {
        let clave = CString::new(clave).unwrap_or_default();
        let mut meta = std::ptr::null_mut();
        // SAFETY: la sesion esta viva; `meta` se libera en todos los caminos
        // tras buscar la clave; el valor lo asigna la API y lo libera
        // `soltar_cadena` tras copiarlo.
        unsafe {
            let st =
                self.rt.f::<FnObtener>(indice::SESSION_GET_MODEL_METADATA)(self.ptr, &mut meta);
            self.rt.comprobar(st, "leer los metadatos del modelo")?;
            let mut valor = std::ptr::null_mut();
            let st = self
                .rt
                .f::<FnBuscarMetadato>(indice::MODEL_METADATA_LOOKUP_CUSTOM_METADATA_MAP)(
                meta,
                self.rt.asignador,
                clave.as_ptr(),
                &mut valor,
            );
            self.rt.f::<FnSoltar>(indice::RELEASE_MODEL_METADATA)(meta);
            self.rt.comprobar(st, "buscar un metadato")?;
            if valor.is_null() {
                return Ok(None);
            }
            Ok(Some(self.rt.soltar_cadena(valor)))
        }
    }

    /// **Corre el modelo**: una entrada por cada una del modelo, en su
    /// orden, y devuelve todas sus salidas, en su orden.
    pub fn correr(&self, entradas: &[&Valor<'r>]) -> Result<Vec<Valor<'r>>, ErrorVoz> {
        if entradas.len() != self.entradas.len() {
            return Err(ErrorVoz::Onnx {
                paso: "correr el modelo",
                detalle: format!(
                    "el modelo pide {} entradas y llegan {}",
                    self.entradas.len(),
                    entradas.len()
                ),
            });
        }
        let nombres_in: Vec<*const c_char> = self.entradas.iter().map(|c| c.as_ptr()).collect();
        let nombres_out: Vec<*const c_char> = self.salidas.iter().map(|c| c.as_ptr()).collect();
        let valores: Vec<*const c_void> = entradas.iter().map(|v| v.ptr as *const c_void).collect();
        let mut salidas: Vec<*mut c_void> = vec![std::ptr::null_mut(); self.salidas.len()];
        // SAFETY: los cuatro arreglos viven durante la llamada y tienen las
        // longitudes que se pasan; las salidas empiezan en nulo, que es como
        // se le dice a la API que las asigne ella.
        let st = unsafe {
            self.rt.f::<FnCorrer>(indice::RUN)(
                self.ptr,
                std::ptr::null(),
                nombres_in.as_ptr(),
                valores.as_ptr(),
                valores.len(),
                nombres_out.as_ptr(),
                nombres_out.len(),
                salidas.as_mut_ptr(),
            )
        };
        // Lo que se haya asignado se envuelve antes de mirar el error, para
        // que se libere pase lo que pase.
        let envueltas: Vec<Valor<'r>> = salidas
            .into_iter()
            .filter(|p| !p.is_null())
            .map(|ptr| Valor {
                rt: self.rt,
                ptr,
                _respaldo: Respaldo::Suyo,
            })
            .collect();
        self.rt.comprobar(st, "correr el modelo")?;
        Ok(envueltas)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_indices_de_la_tabla_estan_en_orden_y_sin_repetir() {
        // Si alguien toca uno a mano y lo descoloca, se nota aqui.
        assert!(indice::TODOS.windows(2).all(|p| p[0] < p[1]));
        // Y ninguno pasa de lo que ya traia la API 7 (ONNX Runtime 1.7).
        assert!(indice::TODOS.iter().all(|&i| i < 150));
    }

    #[test]
    fn el_runtime_se_busca_primero_en_windows_y_luego_en_los_datos_y_junto_al_exe() {
        let v = candidatos(Path::new("C:/datos"), Some(Path::new("C:/app")));
        assert_eq!(v.len(), 3);
        assert!(
            v[0].to_string_lossy()
                .to_lowercase()
                .ends_with("system32\\onnxruntime.dll")
        );
        assert_eq!(v[1], Path::new("C:/datos/whisper/onnxruntime.dll"));
        assert_eq!(v[2], Path::new("C:/app/onnxruntime.dll"));
    }

    #[test]
    fn una_dll_que_no_es_onnx_runtime_se_rechaza_sin_saltar_a_la_nada() {
        // `kernel32.dll` esta siempre y no exporta `OrtGetApiBase`.
        let sistema = carpeta_del_sistema().unwrap();
        let r = Runtime::cargar(&sistema.join("kernel32.dll"));
        assert!(r.is_err());
        assert!(r.err().unwrap().contains("OrtGetApiBase"));
    }

    #[test]
    fn una_dll_que_no_existe_se_dice() {
        let r = Runtime::cargar(Path::new("C:/no/existe/onnxruntime.dll"));
        assert!(r.is_err());
    }

    /// Con el runtime de Windows presente, se carga y un tensor va y vuelve.
    /// Sin el (Windows 10 viejo) se salta con aviso.
    #[test]
    fn el_runtime_de_windows_carga_y_un_tensor_conserva_su_forma() {
        let Some(sistema) = carpeta_del_sistema() else {
            return;
        };
        let dll = sistema.join(NOMBRE_DE_LA_DLL);
        if !dll.is_file() {
            eprintln!("aviso: este Windows no trae onnxruntime.dll");
            return;
        }
        let rt = Runtime::cargar(&dll).expect("el runtime de Windows carga");
        assert!(!rt.version.is_empty());
        let t = rt
            .tensor_f32(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &[1, 2, 3])
            .unwrap();
        assert_eq!(t.forma().unwrap(), [1, 2, 3]);
        assert_eq!(t.como_f32().unwrap(), [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        // Una forma que no cuadra con los datos no llega a la API.
        assert!(rt.tensor_i64(vec![1, 2], &[3]).is_err());
        let t = rt.tensor_i64(vec![7], &[1]).unwrap();
        assert_eq!(t.forma().unwrap(), [1]);
    }
}
