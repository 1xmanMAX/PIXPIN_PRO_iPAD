//! **Bajar el asistente cuando el usuario lo pide**, nunca en el instalador.
//!
//! Todo va a `%LOCALAPPDATA%\PixPinMax\modelos` (no a `%APPDATA%`: son
//! gigas que no deben viajar con el perfil movil de Windows):
//!
//! ```text
//! modelos\
//!   llama.cpp\b11580\llama-server.exe, *.dll   (~70 MB, del zip de 32 MB)
//!   qwen3.5-2b\Qwen3.5-2B-Q4_K_M.gguf          (1,28 GB)
//!   qwen3.5-2b\mmproj-F16.gguf                 (0,67 GB)
//! ```
//!
//! Cada fichero se baja a `.parte` (lo hace `pixpin_voz::whisper::bajar`,
//! que ya sigue las redirecciones de Hugging Face y GitHub y se deja
//! cancelar), se le comprueba la **huella SHA-256** y solo entonces toma su
//! nombre. Al lado queda `<fichero>.ok` con la huella: las siguientes veces
//! basta con mirar el tamano, sin volver a leer 2 GB.
//!
//! **Solo Windows baja desde aqui** (WinHTTP). En Mac, iPad e iPhone baja
//! URLSession desde Swift a la carpeta que elija la app (Application
//! Support, no `LOCALAPPDATA`: [`carpeta_base`] da `None`) y despues usa
//! [`verificar`], [`fichero_listo`] y [`modelo_listo`], que son portables.
//! Pedir a [`asegurar`] algo que falte devuelve un error claro.

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use sha2::{Digest, Sha256};

use crate::ErrorIa;
use crate::modelos::{Fichero, Modelo, Runtime, se_extrae};

/// `%LOCALAPPDATA%\PixPinMax\modelos`.
pub fn carpeta_base() -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA")?;
    Some(PathBuf::from(local).join("PixPinMax").join("modelos"))
}

pub fn carpeta_modelo(raiz: &Path, m: &Modelo) -> PathBuf {
    raiz.join(m.id)
}

pub fn carpeta_runtime(raiz: &Path, r: &Runtime) -> PathBuf {
    raiz.join("llama.cpp").join(r.version)
}

pub fn ruta_servidor(raiz: &Path, r: &Runtime) -> PathBuf {
    carpeta_runtime(raiz, r).join("llama-server.exe")
}

fn marca(ruta: &Path) -> PathBuf {
    let mut n = ruta.file_name().unwrap_or_default().to_os_string();
    n.push(".ok");
    ruta.with_file_name(n)
}

/// ¿Esta el fichero, con su tamano y ya verificado alguna vez?
pub fn fichero_listo(carpeta: &Path, f: &Fichero) -> bool {
    let ruta = carpeta.join(f.nombre);
    std::fs::metadata(&ruta).is_ok_and(|m| m.len() == f.bytes)
        && std::fs::read_to_string(marca(&ruta)).is_ok_and(|h| h.trim() == f.sha256)
}

pub fn modelo_listo(raiz: &Path, m: &Modelo) -> bool {
    let c = carpeta_modelo(raiz, m);
    m.ficheros().iter().all(|f| fichero_listo(&c, f))
}

pub fn runtime_listo(raiz: &Path, r: &Runtime) -> bool {
    let c = carpeta_runtime(raiz, r);
    c.join("llama-server.exe").is_file()
        && std::fs::read_to_string(c.join("zip.ok")).is_ok_and(|h| h.trim() == r.zip.sha256)
}

/// La huella SHA-256 de un fichero, en hexadecimal.
pub fn huella(ruta: &Path, cancelado: &AtomicBool) -> Result<String, ErrorIa> {
    let mut lector = BufReader::with_capacity(1 << 20, File::open(ruta)?);
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        if cancelado.load(Ordering::Relaxed) {
            return Err(ErrorIa::Cancelado);
        }
        let n = lector.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Comprueba la huella y deja la marca. Si no coincide, **borra** el
/// fichero: uno danado no debe quedarse con el nombre bueno.
pub fn verificar(ruta: &Path, f: &Fichero, cancelado: &AtomicBool) -> Result<(), ErrorIa> {
    let h = huella(ruta, cancelado)?;
    if h != f.sha256 {
        let _ = std::fs::remove_file(ruta);
        let _ = std::fs::remove_file(marca(ruta));
        return Err(ErrorIa::HuellaDistinta {
            que: f.nombre.to_string(),
        });
    }
    std::fs::write(marca(ruta), &h)?;
    Ok(())
}

/// Por donde va la descarga, para la barra.
#[derive(Debug, Clone, PartialEq)]
pub struct Progreso {
    pub hecho: u64,
    pub total: u64,
    pub fichero: String,
    /// Bajando o comprobando la huella.
    pub verificando: bool,
}

impl Progreso {
    pub fn fraccion(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            (self.hecho as f64 / self.total as f64).min(1.0) as f32
        }
    }
}

/// Baja `url` a `destino` con el WinHTTP de la voz.
#[cfg(windows)]
fn bajar(
    url: &str,
    destino: &Path,
    base: u64,
    total: u64,
    avance: &mut dyn FnMut(Progreso),
    cancelado: &AtomicBool,
) -> Result<(), ErrorIa> {
    let nombre = destino
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    pixpin_voz::whisper::bajar::bajar(
        url,
        destino,
        &mut |leidos, _| {
            avance(Progreso {
                hecho: base + leidos,
                total,
                fichero: nombre.clone(),
                verificando: false,
            })
        },
        cancelado,
    )
    .map(|_| ())
    .map_err(|e| match e {
        pixpin_voz::ErrorVoz::Cancelada => ErrorIa::Cancelado,
        otro => ErrorIa::Descarga {
            que: nombre.clone(),
            detalle: otro.to_string(),
        },
    })
}

/// Fuera de Windows no se baja nada desde Rust: en Apple lo hace URLSession
/// desde Swift (con el proxy, los certificados y las descargas en segundo
/// plano del sistema), que deja cada fichero en su sitio y luego llama a
/// [`verificar`] (huella y marca `.ok`) o a [`extraer`]. Asi el ejecutable
/// no lleva un cliente HTTPS ni TLS propios (§4bis del plan de Apple).
#[cfg(not(windows))]
fn bajar(
    url: &str,
    destino: &Path,
    _base: u64,
    _total: u64,
    _avance: &mut dyn FnMut(Progreso),
    _cancelado: &AtomicBool,
) -> Result<(), ErrorIa> {
    Err(ErrorIa::Descarga {
        que: destino
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        detalle: format!(
            "en este sistema la descarga ({url}) la hace la app (en Apple, URLSession desde Swift), no el nucleo"
        ),
    })
}

/// **Deja listo el motor y el modelo**, bajando solo lo que falte. Tarda
/// (minutos la primera vez): hilo de fondo. `cancelado` corta en el acto.
pub fn asegurar(
    raiz: &Path,
    modelo: &Modelo,
    runtime: &Runtime,
    avance: &mut dyn FnMut(Progreso),
    cancelado: &AtomicBool,
) -> Result<(), ErrorIa> {
    let faltan_runtime = !runtime_listo(raiz, runtime);
    let carpeta_m = carpeta_modelo(raiz, modelo);
    let faltan: Vec<Fichero> = modelo
        .ficheros()
        .into_iter()
        .filter(|f| !fichero_listo(&carpeta_m, f))
        .collect();
    let total = faltan.iter().map(|f| f.bytes).sum::<u64>()
        + if faltan_runtime { runtime.zip.bytes } else { 0 };
    let mut hecho = 0;
    if faltan_runtime {
        instalar_runtime(raiz, runtime, hecho, total, avance, cancelado)?;
        hecho += runtime.zip.bytes;
    }
    std::fs::create_dir_all(&carpeta_m)?;
    for f in faltan {
        let ruta = carpeta_m.join(f.nombre);
        // Un fichero del tamano justo pero sin marca (se corto al verificar):
        // se verifica sin volver a bajarlo.
        let ya_esta = std::fs::metadata(&ruta).is_ok_and(|m| m.len() == f.bytes);
        if !ya_esta {
            bajar(f.url, &ruta, hecho, total, avance, cancelado)?;
        }
        avance(Progreso {
            hecho: hecho + f.bytes,
            total,
            fichero: f.nombre.to_string(),
            verificando: true,
        });
        verificar(&ruta, &f, cancelado)?;
        hecho += f.bytes;
    }
    Ok(())
}

fn instalar_runtime(
    raiz: &Path,
    r: &Runtime,
    hecho: u64,
    total: u64,
    avance: &mut dyn FnMut(Progreso),
    cancelado: &AtomicBool,
) -> Result<(), ErrorIa> {
    let carpeta = carpeta_runtime(raiz, r);
    std::fs::create_dir_all(&carpeta)?;
    let zip = carpeta.join(r.zip.nombre);
    if !std::fs::metadata(&zip).is_ok_and(|m| m.len() == r.zip.bytes) {
        bajar(r.zip.url, &zip, hecho, total, avance, cancelado)?;
    }
    verificar(&zip, &r.zip, cancelado)?;
    extraer(&zip, &carpeta, r.extraer)?;
    std::fs::write(carpeta.join("zip.ok"), r.zip.sha256)?;
    let _ = std::fs::remove_file(&zip);
    let _ = std::fs::remove_file(marca(&zip));
    Ok(())
}

/// Saca del zip solo los ficheros de la lista, a `destino`. Devuelve cuantos.
pub fn extraer(zip: &Path, destino: &Path, patrones: &[&str]) -> Result<usize, ErrorIa> {
    let mut archivo =
        zip::ZipArchive::new(BufReader::new(File::open(zip)?)).map_err(|e| ErrorIa::Descarga {
            que: "motor".into(),
            detalle: e.to_string(),
        })?;
    let mut n = 0;
    for i in 0..archivo.len() {
        let mut entrada = archivo.by_index(i).map_err(|e| ErrorIa::Descarga {
            que: "motor".into(),
            detalle: e.to_string(),
        })?;
        if entrada.is_dir() {
            continue;
        }
        let nombre = entrada.name().to_string();
        if !se_extrae(patrones, &nombre) {
            continue;
        }
        let ruta = destino.join(&nombre);
        let mut parte = ruta.clone().into_os_string();
        parte.push(".parte");
        let parte = PathBuf::from(parte);
        std::io::copy(&mut entrada, &mut File::create(&parte)?)?;
        let _ = std::fs::remove_file(&ruta);
        std::fs::rename(&parte, &ruta)?;
        n += 1;
    }
    if !destino.join("llama-server.exe").is_file() {
        return Err(ErrorIa::Descarga {
            que: "motor".into(),
            detalle: "el zip no trae llama-server.exe".into(),
        });
    }
    Ok(n)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::io::Write;

    fn carpeta(nombre: &str) -> PathBuf {
        let c =
            std::env::temp_dir().join(format!("pixpin-ia-pruebas-{nombre}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&c);
        std::fs::create_dir_all(&c).unwrap();
        c
    }

    static NO: AtomicBool = AtomicBool::new(false);

    #[test]
    fn la_huella_es_la_de_sha256() {
        let c = carpeta("huella");
        let r = c.join("abc.txt");
        std::fs::write(&r, b"abc").unwrap();
        assert_eq!(
            huella(&r, &NO).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn un_fichero_con_otra_huella_se_borra() {
        let c = carpeta("mala");
        let r = c.join("m.gguf");
        std::fs::write(&r, b"esto no es el modelo").unwrap();
        let f = Fichero {
            nombre: "m.gguf",
            url: "https://x/y",
            bytes: 20,
            sha256: "00",
        };
        assert!(matches!(
            verificar(&r, &f, &NO),
            Err(ErrorIa::HuellaDistinta { .. })
        ));
        assert!(!r.exists());
        assert!(!fichero_listo(&c, &f));
    }

    #[test]
    fn un_fichero_bueno_queda_marcado_y_listo() {
        let c = carpeta("buena");
        let r = c.join("m.gguf");
        std::fs::write(&r, b"abc").unwrap();
        let f = Fichero {
            nombre: "m.gguf",
            url: "https://x/y",
            bytes: 3,
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        };
        assert!(!fichero_listo(&c, &f), "sin verificar no esta listo");
        verificar(&r, &f, &NO).unwrap();
        assert!(fichero_listo(&c, &f));
        // Si cambia el tamano (fichero cortado), deja de estar listo.
        std::fs::write(&r, b"ab").unwrap();
        assert!(!fichero_listo(&c, &f));
    }

    #[test]
    fn cancelar_corta_la_huella() {
        let c = carpeta("cancelar");
        let r = c.join("x");
        std::fs::write(&r, b"abc").unwrap();
        assert!(matches!(
            huella(&r, &AtomicBool::new(true)),
            Err(ErrorIa::Cancelado)
        ));
    }

    #[test]
    fn del_zip_solo_sale_lo_de_la_lista() {
        let c = carpeta("zip");
        let z = c.join("r.zip");
        {
            let mut w = zip::ZipWriter::new(File::create(&z).unwrap());
            let o = zip::write::SimpleFileOptions::default();
            for n in [
                "llama-server.exe",
                "ggml-cpu-haswell.dll",
                "llama-cli.exe",
                "LICENSE-x",
            ] {
                w.start_file(n, o).unwrap();
                w.write_all(n.as_bytes()).unwrap();
            }
            w.finish().unwrap();
        }
        let destino = c.join("rt");
        std::fs::create_dir_all(&destino).unwrap();
        let n = extraer(&z, &destino, crate::modelos::LLAMA_CPP.extraer).unwrap();
        assert_eq!(n, 3);
        assert!(destino.join("ggml-cpu-haswell.dll").is_file());
        assert!(!destino.join("llama-cli.exe").exists());
    }

    #[test]
    fn un_zip_sin_servidor_es_error() {
        let c = carpeta("zipvacio");
        let z = c.join("r.zip");
        {
            let mut w = zip::ZipWriter::new(File::create(&z).unwrap());
            w.start_file("README.md", zip::write::SimpleFileOptions::default())
                .unwrap();
            w.finish().unwrap();
        }
        assert!(extraer(&z, &c, crate::modelos::LLAMA_CPP.extraer).is_err());
    }

    #[test]
    fn la_carpeta_es_la_de_localappdata() {
        if let Some(c) = carpeta_base() {
            assert!(c.ends_with("PixPinMax\\modelos") || c.ends_with("PixPinMax/modelos"));
        }
    }
}
