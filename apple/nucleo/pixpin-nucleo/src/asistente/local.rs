//! **El modelo local del Mac**: que se baja, donde vive y su `llama-server`.
//!
//! Es el mismo GGUF que Windows y Android (`pixpin_ia::modelos`: Qwen3.5 2B
//! y 0.8B, Gemma 4 E2B para «mejor vista») con el `llama-server` de
//! llama.cpp **para macOS** (Metal), de la misma version que el de Windows.
//! Lo baja URLSession desde Swift (`DescargaModelos.swift`) a la carpeta que
//! elige la app (`~/Library/Application Support/PixPinMax/modelos`), y aqui
//! se dice adonde va cada fichero, se comprueba su huella SHA-256 y se dice
//! si ya esta todo:
//!
//! ```text
//! modelos/
//!   llama.cpp/b11580-macos/llama-server, lib*.dylib   (del .tar.gz de 12 MB)
//!   qwen3.5-2b/Qwen3.5-2B-Q4_K_M.gguf, mmproj-F16.gguf
//!   llama-server.pid                                  (el que esta vivo)
//! ```
//!
//! **Que no quede un `llama-server` huerfano.** Un motor por proceso (el
//! relevo de `pixpin_ia::relevo`: un solo modelo cargado a la vez), que se
//! cierra solo a los 5 min sin uso y al salir de la app
//! ([`ia_soltar_motores`]). Si PixPin muere de golpe, el proceso seguiria
//! vivo: por eso se apunta su pid y su ruta en `llama-server.pid`, y al
//! arrancar [`ia_limpiar_huerfanos`] mata el que siga ahi (solo si su orden
//! es nuestro `llama-server`: un pid reutilizado por otro programa no se
//! toca). Va en su grupo de procesos (`servidor::en_segundo_plano`), y se
//! mata el grupo entero.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, OnceLock};

use pixpin_ia::ErrorIa;
use pixpin_ia::descarga;
use pixpin_ia::modelos::{self, Fichero, Modelo, QWEN35_2B};
use pixpin_ia::motor::{Motor, Peticion, Respuesta, Seguir, Trozo};
use pixpin_ia::relevo::Relevo;
use pixpin_ia::servidor::{Config, MotorLlama, elegir_dispositivo, listar_dispositivos};

use crate::ErrorNucleo;

/// **El motor de llama.cpp para macOS**: la misma version que el de
/// Windows (`modelos::LLAMA_CPP`), con Metal. Huellas de la API de GitHub
/// (`digest` de cada fichero de la version), comprobadas el 16-oct-2026
/// bajando el de arm64.
pub const VERSION_MOTOR: &str = "b11580";

#[cfg(target_arch = "aarch64")]
pub const MOTOR_MAC: Fichero = Fichero {
    nombre: "llama-b11580-bin-macos-arm64.tar.gz",
    url: "https://github.com/ggml-org/llama.cpp/releases/download/b11580/llama-b11580-bin-macos-arm64.tar.gz",
    bytes: 12_112_217,
    sha256: "ca2ba63aa7c18e06b0dcd339273c605cba8fdc1b607521e6ea598841ec5462c5",
};

#[cfg(not(target_arch = "aarch64"))]
pub const MOTOR_MAC: Fichero = Fichero {
    nombre: "llama-b11580-bin-macos-x64.tar.gz",
    url: "https://github.com/ggml-org/llama.cpp/releases/download/b11580/llama-b11580-bin-macos-x64.tar.gz",
    bytes: 11_607_948,
    sha256: "882c2620879d8b2c79fad1db63160bc8a556be0a7fb135a09e8cd12bafa636b8",
};

/// Lo que se pide a [`ia_ruta_descarga`] para el motor (los modelos van
/// por su id).
pub const QUE_MOTOR: &str = "motor";

/// Un fichero que se baja.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FicheroIa {
    pub nombre: String,
    pub url: String,
    pub bytes: u64,
    pub sha256: String,
    /// Si ya esta en su sitio y verificado.
    pub listo: bool,
}

/// Un modelo que se puede bajar.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ModeloIa {
    /// `qwen3.5-2b`, `qwen3.5-0.8b`, `gemma-4-e2b` (el ajuste `modelo`).
    pub id: String,
    pub nombre: String,
    pub licencia: String,
    /// Lo que ocupa en memoria cargado.
    pub ram_mb: u32,
    pub ficheros: Vec<FicheroIa>,
    /// El de «mejor vista»: no es nunca «el de siempre».
    pub opcional: bool,
    pub listo: bool,
}

/// Lo que se puede bajar y como esta.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CatalogoLocal {
    pub version_motor: String,
    pub motor: FicheroIa,
    pub modelos: Vec<ModeloIa>,
}

fn ficha(f: &Fichero, listo: bool) -> FicheroIa {
    FicheroIa {
        nombre: f.nombre.into(),
        url: f.url.into(),
        bytes: f.bytes,
        sha256: f.sha256.into(),
        listo,
    }
}

pub fn carpeta_motor(raiz: &Path) -> PathBuf {
    raiz.join("llama.cpp").join(format!("{VERSION_MOTOR}-macos"))
}

pub fn ruta_servidor(raiz: &Path) -> PathBuf {
    carpeta_motor(raiz).join("llama-server")
}

fn marca_motor(raiz: &Path) -> PathBuf {
    carpeta_motor(raiz).join("motor.ok")
}

/// Si el `llama-server` de macOS esta sacado y era el de la huella.
pub fn motor_listo(raiz: &Path) -> bool {
    ruta_servidor(raiz).is_file() && std::fs::read_to_string(marca_motor(raiz)).is_ok_and(|h| h.trim() == MOTOR_MAC.sha256)
}

/// Si el modelo y el motor estan para usarse.
pub fn listo(raiz: &Path, m: &Modelo) -> bool {
    motor_listo(raiz) && descarga::modelo_listo(raiz, m)
}

fn todos() -> impl Iterator<Item = (Modelo, bool)> {
    modelos::MODELOS
        .iter()
        .map(|m| (*m, false))
        .chain(modelos::OPCIONALES.iter().map(|m| (*m, true)))
}

/// El modelo de los ajustes; sin ajuste o uno que no se conoce, el 2B. El
/// de mejor vista nunca es «el de siempre».
pub fn modelo_elegido(id: &str) -> Modelo {
    modelos::MODELOS.iter().copied().find(|m| m.id == id.trim()).unwrap_or(QWEN35_2B)
}

/// **Lo que se puede bajar**, y que esta ya en `carpeta`.
#[uniffi::export]
pub fn ia_catalogo_local(carpeta: String) -> CatalogoLocal {
    let raiz = PathBuf::from(carpeta);
    CatalogoLocal {
        version_motor: VERSION_MOTOR.into(),
        motor: ficha(&MOTOR_MAC, motor_listo(&raiz)),
        modelos: todos()
            .map(|(m, opcional)| {
                let c = descarga::carpeta_modelo(&raiz, &m);
                ModeloIa {
                    id: m.id.into(),
                    nombre: m.nombre.into(),
                    licencia: m.licencia.into(),
                    ram_mb: m.ram_mb,
                    ficheros: m.ficheros().iter().map(|f| ficha(f, descarga::fichero_listo(&c, f))).collect(),
                    opcional,
                    listo: descarga::modelo_listo(&raiz, &m),
                }
            })
            .collect(),
    }
}

/// El fichero `nombre` de `que` (un id de modelo o [`QUE_MOTOR`]) y su
/// carpeta.
fn buscar(raiz: &Path, que: &str, nombre: &str) -> Result<(PathBuf, Fichero), ErrorNucleo> {
    let no = || ErrorNucleo::Argumento {
        detalle: format!("no se baja {nombre:?} de {que:?}"),
    };
    if que == QUE_MOTOR {
        return (MOTOR_MAC.nombre == nombre)
            .then(|| (raiz.join("llama.cpp"), MOTOR_MAC))
            .ok_or_else(no);
    }
    let m = modelos::por_id(que).ok_or_else(no)?;
    let f = m.ficheros().into_iter().find(|f| f.nombre == nombre).ok_or_else(no)?;
    Ok((descarga::carpeta_modelo(raiz, &m), f))
}

/// **Adonde va un fichero** que baja Swift (ya con su carpeta creada).
/// Swift baja a otro sitio y lo mueve aqui al acabar.
#[uniffi::export]
pub fn ia_ruta_descarga(carpeta: String, que: String, nombre: String) -> Result<String, ErrorNucleo> {
    let (c, f) = buscar(Path::new(&carpeta), &que, &nombre)?;
    std::fs::create_dir_all(&c)?;
    Ok(c.join(f.nombre).display().to_string())
}

/// **Comprueba la huella** de lo bajado y deja su marca. Si no coincide, lo
/// borra (uno danado no se queda con el nombre bueno) y da error. Tarda
/// (lee el fichero entero): fuera del hilo principal.
#[uniffi::export]
pub fn ia_verificar_descarga(carpeta: String, que: String, nombre: String) -> Result<(), ErrorNucleo> {
    let (c, f) = buscar(Path::new(&carpeta), &que, &nombre)?;
    let ruta = c.join(f.nombre);
    if std::fs::metadata(&ruta).map(|m| m.len()).unwrap_or(0) != f.bytes {
        let _ = std::fs::remove_file(&ruta);
        return Err(ErrorNucleo::Asistente {
            detalle: format!("{} llego incompleto; vuelve a bajarlo", f.nombre),
        });
    }
    descarga::verificar(&ruta, &f, &AtomicBool::new(false)).map_err(|e| ErrorNucleo::Asistente {
        detalle: match e {
            ErrorIa::HuellaDistinta { que } => format!("{que} no es el que se esperaba (huella distinta); se borro"),
            otro => otro.to_string(),
        },
    })
}

/// La carpeta donde Swift saca el `.tar.gz` del motor (con `tar`), ya
/// creada.
#[uniffi::export]
pub fn ia_carpeta_motor(carpeta: String) -> Result<String, ErrorNucleo> {
    let c = carpeta_motor(Path::new(&carpeta));
    std::fs::create_dir_all(&c)?;
    Ok(c.display().to_string())
}

/// **El motor ya sacado**: comprueba que esta `llama-server`, deja la marca
/// y borra el `.tar.gz`.
#[uniffi::export]
pub fn ia_terminar_motor(carpeta: String) -> Result<(), ErrorNucleo> {
    let raiz = PathBuf::from(carpeta);
    if !ruta_servidor(&raiz).is_file() {
        return Err(ErrorNucleo::Asistente {
            detalle: "el paquete del motor no trae llama-server".into(),
        });
    }
    std::fs::write(marca_motor(&raiz), MOTOR_MAC.sha256)?;
    let tgz = raiz.join("llama.cpp").join(MOTOR_MAC.nombre);
    let _ = std::fs::remove_file(&tgz);
    let _ = std::fs::remove_file(tgz.with_file_name(format!("{}.ok", MOTOR_MAC.nombre)));
    Ok(())
}

// ---------------------------------------------------------------------------
// El motor del proceso

/// **El `llama-server` que se monta al primer uso** (elegir donde corre
/// tambien, en el hilo del turno). Antes de contestar apunta su pid para
/// [`ia_limpiar_huerfanos`].
struct MotorPerezoso {
    raiz: PathBuf,
    modelo: Modelo,
    dentro: OnceLock<MotorLlama>,
}

impl MotorPerezoso {
    fn motor(&self) -> &MotorLlama {
        self.dentro.get_or_init(|| {
            let carpeta = descarga::carpeta_modelo(&self.raiz, &self.modelo);
            let servidor = ruta_servidor(&self.raiz);
            let mut c = Config::nueva(
                servidor.clone(),
                carpeta.join(self.modelo.pesos.nombre),
                self.modelo.vision.map(|v| carpeta.join(v.nombre)),
            );
            c.dispositivo = elegir_dispositivo(&listar_dispositivos(&servidor), self.modelo.pesos.bytes);
            MotorLlama::nuevo(c)
        })
    }

    fn cargar(&self) -> Result<(), ErrorIa> {
        if !listo(&self.raiz, &self.modelo) {
            return Err(ErrorIa::Falta(self.modelo.nombre.into()));
        }
        let m = self.motor();
        m.precargar()?;
        if let Some(pid) = m.pid() {
            apuntar_pid(&self.raiz, pid);
        }
        Ok(())
    }
}

impl Motor for MotorPerezoso {
    fn completar(&self, p: &Peticion, al_trozo: &mut dyn FnMut(&Trozo) -> Seguir) -> Result<Respuesta, ErrorIa> {
        self.cargar()?;
        self.motor().completar(p, al_trozo)
    }

    fn soltar(&self) {
        if let Some(m) = self.dentro.get() {
            m.soltar();
        }
    }

    fn precalentar(&self) {
        // Si falla, la primera pregunta lo dira.
        let _ = self.cargar();
    }
}

fn relevo() -> &'static Relevo {
    static RELEVO: OnceLock<Relevo> = OnceLock::new();
    RELEVO.get_or_init(Relevo::nuevo)
}

/// El motor del proceso para este modelo (uno cargado a la vez). `None` si
/// falta bajar algo.
pub fn del_proceso(raiz: &Path, m: &Modelo) -> Option<Arc<dyn Motor>> {
    if !listo(raiz, m) {
        return None;
    }
    let (raiz, modelo) = (raiz.to_path_buf(), *m);
    let llave = format!("{}@{}", m.id, raiz.display());
    Some(relevo().motor(&llave, move || {
        Arc::new(MotorPerezoso {
            raiz,
            modelo,
            dentro: OnceLock::new(),
        })
    }))
}

/// Carga el modelo ya, sin esperar a la primera pregunta (al abrir la
/// ventana del asistente). Bloquea hasta que carga: desde un hilo de fondo.
#[uniffi::export]
pub fn ia_precargar_local(carpeta: String, modelo: String) {
    let raiz = PathBuf::from(carpeta);
    if let Some(m) = del_proceso(&raiz, &modelo_elegido(&modelo)) {
        m.precalentar();
    }
}

/// **Suelta los modelos ya**: mata cada `llama-server` de PixPin y
/// devuelve la memoria (al salir de la app y al cambiar de modelo).
#[uniffi::export]
pub fn ia_soltar_motores() {
    relevo().soltar_todo();
    if let Some(r) = PIDS.get() {
        let _ = std::fs::remove_file(r);
    }
}

static PIDS: OnceLock<PathBuf> = OnceLock::new();

fn fichero_pid(raiz: &Path) -> PathBuf {
    raiz.join("llama-server.pid")
}

fn apuntar_pid(raiz: &Path, pid: u32) {
    let f = fichero_pid(raiz);
    let _ = PIDS.set(f.clone());
    let _ = std::fs::write(&f, format!("{pid}\n{}\n", ruta_servidor(raiz).display()));
}

/// **Mata el `llama-server` que dejo una PixPin que murio de golpe** (al
/// arrancar). Solo si el pid apuntado sigue siendo nuestro `llama-server`
/// (su orden lleva la ruta apuntada, que es de nuestra carpeta). Devuelve si mato alguno.
#[uniffi::export]
pub fn ia_limpiar_huerfanos(carpeta: String) -> bool {
    let raiz = PathBuf::from(carpeta);
    let f = fichero_pid(&raiz);
    let Ok(texto) = std::fs::read_to_string(&f) else { return false };
    let _ = std::fs::remove_file(&f);
    let mut lineas = texto.lines();
    let (Some(pid), Some(ruta)) = (lineas.next().and_then(|p| p.trim().parse::<u32>().ok()), lineas.next()) else {
        return false;
    };
    if pid <= 1 || pid == std::process::id() || !es_nuestro(pid, ruta.trim()) {
        return false;
    }
    // Su grupo entero (se lanzo con `process_group(0)`: su pgid es su pid).
    let _ = std::process::Command::new("/bin/kill")
        .args(["-TERM", "--", &format!("-{pid}")])
        .status();
    true
}

/// Si el proceso `pid` es el `llama-server` de `ruta`.
fn es_nuestro(pid: u32, ruta: &str) -> bool {
    if ruta.is_empty() {
        return false;
    }
    std::process::Command::new("/bin/ps")
        .args(["-p", &pid.to_string(), "-o", "command="])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .is_some_and(|o| String::from_utf8_lossy(&o.stdout).contains(ruta))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn carpeta() -> PathBuf {
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let d = std::env::temp_dir().join(format!("pixpin-modelos-{}-{n}-{}", std::process::id(), crate::ahora_ms()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn el_catalogo_dice_que_falta_y_adonde_va() {
        let d = carpeta();
        let c = ia_catalogo_local(d.display().to_string());
        assert_eq!(c.version_motor, modelos::LLAMA_CPP.version, "la misma version que Windows");
        assert!(c.motor.url.contains("macos") && c.motor.url.ends_with(".tar.gz"));
        assert!(!c.motor.listo);
        assert!(c.modelos.iter().any(|m| m.id == "qwen3.5-2b" && !m.opcional && !m.listo));
        assert!(c.modelos.iter().any(|m| m.id == "gemma-4-e2b" && m.opcional));
        let ruta = ia_ruta_descarga(d.display().to_string(), "qwen3.5-0.8b".into(), "mmproj-F16.gguf".into()).unwrap();
        assert!(ruta.ends_with("qwen3.5-0.8b/mmproj-F16.gguf"), "{ruta}");
        // Caso negativo: lo que no esta en el catalogo no tiene sitio.
        assert!(ia_ruta_descarga(d.display().to_string(), "qwen3.5-2b".into(), "../../x".into()).is_err());
        assert!(ia_ruta_descarga(d.display().to_string(), "otro".into(), "x".into()).is_err());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn lo_bajado_mal_se_borra_y_el_motor_pide_su_servidor() {
        let d = carpeta();
        let s = d.display().to_string();
        let ruta = ia_ruta_descarga(s.clone(), QUE_MOTOR.into(), MOTOR_MAC.nombre.into()).unwrap();
        std::fs::write(&ruta, b"no es el motor").unwrap();
        assert!(ia_verificar_descarga(s.clone(), QUE_MOTOR.into(), MOTOR_MAC.nombre.into()).is_err());
        assert!(!Path::new(&ruta).exists(), "lo incompleto no se queda");
        // Sin llama-server sacado no se da por listo.
        assert!(ia_terminar_motor(s.clone()).is_err());
        let c = ia_carpeta_motor(s.clone()).unwrap();
        std::fs::write(Path::new(&c).join("llama-server"), b"#!/bin/sh\n").unwrap();
        ia_terminar_motor(s.clone()).unwrap();
        assert!(motor_listo(&d));
        assert!(ia_catalogo_local(s).motor.listo);
        // Sin modelo, no hay motor del proceso.
        assert!(del_proceso(&d, &QWEN35_2B).is_none());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn el_huerfano_de_otro_programa_no_se_toca() {
        let d = carpeta();
        // Un proceso de verdad que NO es nuestro llama-server.
        let mut hijo = std::process::Command::new("/bin/sleep").arg("30").spawn().unwrap();
        std::fs::write(fichero_pid(&d), format!("{}\n{}\n", hijo.id(), ruta_servidor(&d).display())).unwrap();
        assert!(!ia_limpiar_huerfanos(d.display().to_string()));
        assert!(matches!(hijo.try_wait(), Ok(None)), "sigue vivo");
        assert!(!fichero_pid(&d).exists(), "la nota se quita igual");
        // El que si es nuestro (su orden lleva la ruta apuntada): un guion
        // con el nombre del motor.
        let falso = d.join("llama-server");
        std::fs::write(&falso, "#!/bin/sh\n/bin/sleep 30\n").unwrap();
        std::fs::set_permissions(&falso, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        let mut nuestro = {
            use std::os::unix::process::CommandExt;
            std::process::Command::new(&falso).arg("30").process_group(0).spawn().unwrap()
        };
        std::fs::write(fichero_pid(&d), format!("{}\n{}\n", nuestro.id(), falso.display())).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert!(ia_limpiar_huerfanos(d.display().to_string()));
        let fin = nuestro.wait().unwrap();
        assert!(!fin.success(), "lo mato la señal");
        let _ = hijo.kill();
        let _ = hijo.wait();
        // Sin nota, nada.
        assert!(!ia_limpiar_huerfanos(d.display().to_string()));
        let _ = std::fs::remove_dir_all(d);
    }
}
