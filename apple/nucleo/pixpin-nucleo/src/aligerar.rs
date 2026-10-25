//! **Aligerar un PDF** con pdfsqueeze (`pdfsqueeze-core`), como Windows
//! (`por-traducir/pixpin/src/aligerar.rs`) y el movil
//! (`pdf/PdfSqueezeService.kt`).
//!
//! - [`aligerar_pdf`]: comprime un fichero en otro, sin mas.
//! - [`Nucleo::aligerar_en_su_sitio`]: lo que hace Windows con un PDF del
//!   chat (`procesar`): solo se cambia si baja al menos un 15 % (un 2 % sin
//!   perdida), si sale con las mismas paginas y si mientras tanto nadie
//!   cambio el fichero; se cambia de un golpe (`rename`) junto con sus
//!   copias identicas de la misma carpeta, y cada PDF ya visto se apunta por
//!   su huella en `cache/pdf-aligerados.txt` para no repetir la pasada.
//!
//! En Mac no hace falta el ejecutable aparte de Windows: el xcframework se
//! compila con `panic = "unwind"` y un panico de lopdf llega a Swift como
//! error. **Bloqueante**: Swift lo llama desde una `Task` con prioridad
//! `.utility`. pdfsqueeze corre en su propio grupo de hilos (la mitad de los
//! nucleos, al menos uno), que se suelta al acabar: no queda nada vivo.
//! Plan B de Windows (`pixpin_pdf::aligerar`, con WIC) no hay: va tras
//! `cfg(windows)`.

use std::io::Write;
use std::path::{Path, PathBuf};

use pixpin_store::ajustes::{self, NivelPdf};

use crate::{ErrorNucleo, Nucleo};

/// Los cuatro niveles de PixPin (los perfiles de pdfsqueeze).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum NivelAligerar {
    /// Ni un pixel cambia.
    SinPerdida,
    /// No se nota a la vista (fotos a 200 ppp).
    Equilibrado,
    /// Para leer en pantalla.
    Pequeno,
    /// Lo minimo que se sigue leyendo.
    Extremo,
}

impl From<NivelPdf> for NivelAligerar {
    fn from(n: NivelPdf) -> Self {
        match n {
            NivelPdf::SinPerdida => NivelAligerar::SinPerdida,
            NivelPdf::Equilibrado => NivelAligerar::Equilibrado,
            NivelPdf::Pequeno => NivelAligerar::Pequeno,
            NivelPdf::Extremo => NivelAligerar::Extremo,
        }
    }
}

impl From<NivelAligerar> for NivelPdf {
    fn from(n: NivelAligerar) -> Self {
        match n {
            NivelAligerar::SinPerdida => NivelPdf::SinPerdida,
            NivelAligerar::Equilibrado => NivelPdf::Equilibrado,
            NivelAligerar::Pequeno => NivelPdf::Pequeno,
            NivelAligerar::Extremo => NivelPdf::Extremo,
        }
    }
}

fn perfil(n: NivelAligerar) -> pdfsqueeze_core::Profile {
    use pdfsqueeze_core::Profile;
    match n {
        NivelAligerar::SinPerdida => Profile::Lossless,
        NivelAligerar::Equilibrado => Profile::Balanced,
        NivelAligerar::Pequeno => Profile::Small,
        NivelAligerar::Extremo => Profile::Extreme,
    }
}

/// **El nivel con que se aligera al entrar al chat** (`nivel_para_entrar`):
/// sin perdida y equilibrado tal cual; los otros dos gastan mucha memoria y
/// degradan a proposito, asi que al entrar se usa equilibrado.
#[uniffi::export]
pub fn nivel_para_entrar(elegido: NivelAligerar) -> NivelAligerar {
    match elegido {
        NivelAligerar::SinPerdida => NivelAligerar::SinPerdida,
        _ => NivelAligerar::Equilibrado,
    }
}

/// Lo que tiene que quedar como mucho para cambiar el fichero
/// (`lo_que_puede_quedar`): un 98 % sin perdida, un 85 % con perdida.
pub fn lo_que_puede_quedar(n: NivelAligerar) -> f64 {
    if n == NivelAligerar::SinPerdida {
        0.98
    } else {
        0.85
    }
}

/// El avance de un aligerado, desde los hilos de trabajo.
#[uniffi::export(callback_interface)]
pub trait AvanceAligerar: Send + Sync {
    /// `por_ciento` de 0 a 100 (solo cuando cambia). Devolver `false`
    /// cancela.
    fn avance(&self, por_ciento: u8) -> bool;
}

/// Lo que hizo [`aligerar_pdf`].
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ResumenAligerar {
    pub antes: u64,
    pub despues: u64,
    pub paginas: u32,
    /// Si se escribio `destino` (solo si salio mas pequeno).
    pub escrito: bool,
}

/// Como acabo [`Nucleo::aligerar_en_su_sitio`].
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum DesenlaceAligerar {
    /// Cambiado en su sitio.
    Aligerado {
        antes: u64,
        despues: u64,
    },
    /// No bajo lo bastante: se queda como estaba.
    SinMejora,
    /// Ya se habia pasado (por su huella): no se repite.
    YaEstaba,
    /// Cifrado: no se toca.
    Cifrado,
    Cancelado,
    Fallo {
        detalle: String,
    },
}

/// Los ajustes de `[pdf]` de `pixpinmax.toml` (los de Windows).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct AjustesPdf {
    /// Al entrar un PDF al chat, se aligera despues, en segundo plano.
    pub aligerar_al_entrar: bool,
    /// Con que nivel (el de «Aligerar el PDF» a mano; al entrar, ver
    /// [`nivel_para_entrar`]).
    pub nivel: NivelAligerar,
}

/// Cuantos hilos: la mitad de los nucleos, al menos uno (`hilos_para`).
fn hilos() -> usize {
    let n = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    (n / 2).max(1)
}

enum Salida {
    Menor(Vec<u8>, pdfsqueeze_core::Report),
    NoMenor(pdfsqueeze_core::Report),
    Cifrado,
    Cancelado,
    Fallo(String),
}

/// pdfsqueeze sobre `bytes`, en su propio grupo de hilos.
fn comprimir(bytes: &[u8], nivel: NivelAligerar, avance: &(dyn Fn(u8) -> bool + Sync)) -> Salida {
    let mut opciones = pdfsqueeze_core::Options::from_profile(perfil(nivel));
    // 0: el grupo en el que se corre, el de aqui abajo (con un numero
    // intentaria montar el global de rayon).
    opciones.threads = 0;
    let grupo = match rayon::ThreadPoolBuilder::new()
        .num_threads(hilos())
        .thread_name(|i| format!("pdfsqueeze-{i}"))
        .build()
    {
        Ok(g) => g,
        Err(e) => return Salida::Fallo(e.to_string()),
    };
    let ultimo = std::sync::atomic::AtomicU8::new(u8::MAX);
    let r = grupo.install(|| {
        pdfsqueeze_core::pipeline::compress_with(bytes, &opciones, &|p| {
            // Solo cuando cambia: cada foto avisa.
            if ultimo.swap(p.percent, std::sync::atomic::Ordering::Relaxed) != p.percent {
                avance(p.percent)
            } else {
                true
            }
        })
    });
    match r {
        Ok((datos, informe)) => {
            if informe.returned_original || datos.len() >= bytes.len() {
                Salida::NoMenor(informe)
            } else {
                Salida::Menor(datos, informe)
            }
        }
        Err(pdfsqueeze_core::Error::Encrypted) => Salida::Cifrado,
        Err(pdfsqueeze_core::Error::Cancelled) => Salida::Cancelado,
        Err(e) => Salida::Fallo(e.to_string()),
    }
}

fn aligerar_con(
    origen: &str,
    destino: &str,
    nivel: NivelAligerar,
    avance: &(dyn Fn(u8) -> bool + Sync),
) -> Result<ResumenAligerar, ErrorNucleo> {
    let bytes = std::fs::read(origen)?;
    match comprimir(&bytes, nivel, avance) {
        Salida::Menor(datos, informe) => {
            pixpin_sincro::disco::escribir_atomico(Path::new(destino), &datos)?;
            Ok(ResumenAligerar {
                antes: bytes.len() as u64,
                despues: datos.len() as u64,
                paginas: informe.pages,
                escrito: true,
            })
        }
        Salida::NoMenor(informe) => Ok(ResumenAligerar {
            antes: bytes.len() as u64,
            despues: bytes.len() as u64,
            paginas: informe.pages,
            escrito: false,
        }),
        Salida::Cifrado => Err(ErrorNucleo::Argumento {
            detalle: "PDF cifrado: no se aligera".into(),
        }),
        Salida::Cancelado => Err(ErrorNucleo::Argumento {
            detalle: "cancelado".into(),
        }),
        Salida::Fallo(detalle) => Err(ErrorNucleo::Disco { detalle }),
    }
}

/// **Comprime `origen` en `destino`** con el nivel dado. Solo escribe si sale
/// mas pequeno (`escrito`). Bloqueante.
#[uniffi::export]
pub fn aligerar_pdf(
    origen: String,
    destino: String,
    nivel: NivelAligerar,
) -> Result<ResumenAligerar, ErrorNucleo> {
    aligerar_con(&origen, &destino, nivel, &|_| true)
}

/// [`aligerar_pdf`] diciendo por donde va (y que se puede cancelar).
#[uniffi::export]
pub fn aligerar_pdf_con_avance(
    origen: String,
    destino: String,
    nivel: NivelAligerar,
    avance: Box<dyn AvanceAligerar>,
) -> Result<ResumenAligerar, ErrorNucleo> {
    aligerar_con(&origen, &destino, nivel, &|p| avance.avance(p))
}

fn parece_pdf(bytes: &[u8]) -> bool {
    bytes[..bytes.len().min(1024)]
        .windows(5)
        .any(|v| v == b"%PDF-")
}

fn huella(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn fichero_de_vistos(raiz: &Path) -> PathBuf {
    raiz.join("cache").join("pdf-aligerados.txt")
}

fn ya_visto(raiz: &Path, h: &str) -> bool {
    std::fs::read_to_string(fichero_de_vistos(raiz)).is_ok_and(|t| t.lines().any(|l| l.trim() == h))
}

fn apuntar(raiz: &Path, huellas: &[&str]) {
    let ruta = fichero_de_vistos(raiz);
    if let Some(p) = ruta.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&ruta)
        .and_then(|mut f| {
            for h in huellas {
                writeln!(f, "{h}")?;
            }
            Ok(())
        });
}

fn sigue_igual(ruta: &Path, original: &[u8]) -> bool {
    std::fs::metadata(ruta).is_ok_and(|m| m.len() == original.len() as u64)
        && std::fs::read(ruta).is_ok_and(|b| b == original)
}

/// Los `.pdf` de la misma carpeta con los mismos bytes (`gemelos`).
fn gemelos(ruta: &Path, original: &[u8]) -> Vec<PathBuf> {
    let Some(carpeta) = ruta.parent() else {
        return Vec::new();
    };
    let Ok(l) = std::fs::read_dir(carpeta) else {
        return Vec::new();
    };
    l.flatten()
        .map(|e| e.path())
        .filter(|g| {
            g.as_path() != ruta
                && g.extension().is_some_and(|x| x.eq_ignore_ascii_case("pdf"))
                && std::fs::metadata(g)
                    .is_ok_and(|m| m.is_file() && m.len() == original.len() as u64)
        })
        .filter(|g| sigue_igual(g, original))
        .collect()
}

/// Escribe al lado y cambia de un golpe (`poner_en_su_sitio`).
fn poner_en_su_sitio(destino: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut nombre = destino.file_name().unwrap_or_default().to_os_string();
    nombre.push(".ligero");
    let temporal = destino.with_file_name(nombre);
    std::fs::write(&temporal, bytes)?;
    std::fs::rename(&temporal, destino).inspect_err(|_| {
        let _ = std::fs::remove_file(&temporal);
    })
}

/// Cuantas paginas dice el PDF (la misma lectura tolerante de pdfsqueeze).
fn paginas_de(bytes: &[u8]) -> Option<usize> {
    pdfsqueeze_core::pipeline::load_lenient(bytes)
        .ok()
        .map(|(d, _)| d.get_pages().len())
}

/// `procesar` de Windows, sin el proceso aparte.
fn procesar(
    raiz: &Path,
    ruta: &Path,
    nivel: NivelAligerar,
    a_mano: bool,
    avance: &(dyn Fn(u8) -> bool + Sync),
) -> DesenlaceAligerar {
    let original = match std::fs::read(ruta) {
        Ok(b) => b,
        Err(e) => {
            return DesenlaceAligerar::Fallo {
                detalle: format!("no se lee: {e}"),
            };
        }
    };
    if !parece_pdf(&original) {
        return DesenlaceAligerar::Fallo {
            detalle: "no es un PDF".into(),
        };
    }
    let h = huella(&original);
    if !a_mano && ya_visto(raiz, &h) {
        return DesenlaceAligerar::YaEstaba;
    }
    let (ligero, informe) = match comprimir(&original, nivel, avance) {
        Salida::Menor(d, i) => (d, i),
        Salida::NoMenor(_) => {
            apuntar(raiz, &[&h]);
            return DesenlaceAligerar::SinMejora;
        }
        Salida::Cifrado => {
            apuntar(raiz, &[&h]);
            return DesenlaceAligerar::Cifrado;
        }
        Salida::Cancelado => return DesenlaceAligerar::Cancelado,
        Salida::Fallo(detalle) => {
            apuntar(raiz, &[&h]);
            return DesenlaceAligerar::Fallo { detalle };
        }
    };
    let tope = (original.len() as f64 * lo_que_puede_quedar(nivel)) as usize;
    if ligero.is_empty() || ligero.len() > tope {
        apuntar(raiz, &[&h]);
        return DesenlaceAligerar::SinMejora;
    }
    // Que se abra con las mismas paginas (en Windows ademas se dibuja con
    // `Windows.Data.Pdf`; aqui lo comprueba la verificacion de pdfsqueeze y
    // PDFKit al ensenarlo).
    let antes = paginas_de(&original);
    let despues = paginas_de(&ligero);
    if despues.is_none_or(|d| d == 0 || antes.is_some_and(|a| a != d))
        || (informe.pages as usize != despues.unwrap_or(0))
    {
        apuntar(raiz, &[&h]);
        return DesenlaceAligerar::Fallo {
            detalle: "el aligerado no se abre igual".into(),
        };
    }
    let gemelos = gemelos(ruta, &original);
    // Si mientras tanto alguien lo cambio, no se pisa.
    if !sigue_igual(ruta, &original) {
        return DesenlaceAligerar::Fallo {
            detalle: "cambio mientras se aligeraba".into(),
        };
    }
    if let Err(e) = poner_en_su_sitio(ruta, &ligero) {
        return DesenlaceAligerar::Fallo {
            detalle: format!("no se pudo cambiar: {e}"),
        };
    }
    for g in gemelos {
        if sigue_igual(&g, &original) {
            let _ = poner_en_su_sitio(&g, &ligero);
        }
    }
    apuntar(raiz, &[&h, &huella(&ligero)]);
    DesenlaceAligerar::Aligerado {
        antes: original.len() as u64,
        despues: ligero.len() as u64,
    }
}

#[uniffi::export]
impl Nucleo {
    /// **Aligera un PDF del chat en su sitio** (`ruta`: la de
    /// [`Nucleo::ruta_absoluta`]), como Windows. `a_mano`: pedido desde el
    /// menu del mensaje (se repite aunque ya se hubiera pasado). Bloqueante.
    pub fn aligerar_en_su_sitio(
        &self,
        ruta: String,
        nivel: NivelAligerar,
        a_mano: bool,
        avance: Box<dyn AvanceAligerar>,
    ) -> DesenlaceAligerar {
        procesar(self.ruta(), Path::new(&ruta), nivel, a_mano, &|p| {
            avance.avance(p)
        })
    }

    /// Los ajustes `[pdf]` de `pixpinmax.toml`.
    pub fn ajustes_pdf(&self) -> Result<AjustesPdf, ErrorNucleo> {
        let a = ajustes::cargar(&self.ubicacion_pdf())?;
        Ok(AjustesPdf {
            aligerar_al_entrar: a.pdf.aligerar_al_entrar,
            nivel: a.pdf.nivel.into(),
        })
    }

    /// Guarda `[pdf]` sin tocar lo demas del fichero.
    pub fn guardar_ajustes_pdf(&self, nuevos: AjustesPdf) -> Result<(), ErrorNucleo> {
        let u = self.ubicacion_pdf();
        let mut a = ajustes::cargar(&u)?;
        a.pdf.aligerar_al_entrar = nuevos.aligerar_al_entrar;
        a.pdf.nivel = nuevos.nivel.into();
        ajustes::guardar_conservando(&u, &a)?;
        Ok(())
    }
}

impl Nucleo {
    fn ubicacion_pdf(&self) -> pixpin_store::Ubicacion {
        pixpin_store::Ubicacion::Instalado {
            raiz: self.ruta().to_path_buf(),
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use pdfsqueeze_core::testgen::{self, ImgEnc, Synth};

    /// Un PDF pequeno con una foto guardada sin comprimir: hay mucho que
    /// ganar sin perder nada.
    fn pdf_pesado() -> Vec<u8> {
        let mut s = Synth::new();
        let foto = testgen::photo(320, 240);
        s.image_page(&foto, ImgEnc::Raw, 40.0, 300.0, 320.0, 240.0);
        s.text_page(20);
        s.finish()
    }

    fn carpeta(n: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("pixpin-nucleo-aligerar-{n}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn aligerar_un_pdf_pequeno_lo_deja_menor_y_con_sus_paginas() {
        let d = carpeta("suelto");
        let origen = d.join("pesado.pdf");
        let bytes = pdf_pesado();
        std::fs::write(&origen, &bytes).unwrap();
        let destino = d.join("ligero.pdf");
        let avisos = std::sync::Mutex::new(Vec::new());
        let r = aligerar_con(
            &origen.display().to_string(),
            &destino.display().to_string(),
            NivelAligerar::SinPerdida,
            &|p| {
                avisos.lock().unwrap().push(p);
                true
            },
        )
        .unwrap();
        assert!(r.escrito);
        assert!(r.despues < r.antes, "{r:?}");
        assert_eq!(r.paginas, 2);
        let ligero = std::fs::read(&destino).unwrap();
        assert_eq!(paginas_de(&ligero), Some(2));
        assert!(!avisos.lock().unwrap().is_empty());
        // Cancelar desde el avance no escribe nada.
        let otro = d.join("cancelado.pdf");
        let e = aligerar_con(
            &origen.display().to_string(),
            &otro.display().to_string(),
            NivelAligerar::Equilibrado,
            &|_| false,
        );
        assert!(e.is_err());
        assert!(!otro.exists());
        // Caso negativo: lo que no es un PDF no se aligera.
        let falso = d.join("falso.pdf");
        std::fs::write(&falso, b"hola").unwrap();
        assert!(
            aligerar_pdf(
                falso.display().to_string(),
                otro.display().to_string(),
                NivelAligerar::SinPerdida
            )
            .is_err()
        );
    }

    #[test]
    fn en_su_sitio_cambia_el_fichero_y_sus_gemelos_una_sola_vez() {
        let raiz = carpeta("sitio");
        let docs = raiz.join("proyectos").join("p1").join("archivos");
        std::fs::create_dir_all(&docs).unwrap();
        let bytes = pdf_pesado();
        let pdf = docs.join("informe.pdf");
        let copia = docs.join("informe-limpio.pdf");
        std::fs::write(&pdf, &bytes).unwrap();
        std::fs::write(&copia, &bytes).unwrap();
        let hecho = procesar(&raiz, &pdf, NivelAligerar::SinPerdida, false, &|_| true);
        let DesenlaceAligerar::Aligerado { antes, despues } = hecho else {
            panic!("{hecho:?}");
        };
        assert!(despues as f64 <= antes as f64 * 0.98);
        let nuevo = std::fs::read(&pdf).unwrap();
        assert_eq!(nuevo.len() as u64, despues);
        assert_eq!(std::fs::read(&copia).unwrap(), nuevo, "el gemelo tambien");
        // Ya visto: no se repite al entrar, pero si a mano.
        assert_eq!(
            procesar(&raiz, &pdf, NivelAligerar::SinPerdida, false, &|_| true),
            DesenlaceAligerar::YaEstaba
        );
        assert!(fichero_de_vistos(&raiz).is_file());
        assert_eq!(
            nivel_para_entrar(NivelAligerar::Extremo),
            NivelAligerar::Equilibrado
        );
    }

    #[test]
    fn los_ajustes_pdf_van_en_pixpinmax_toml() {
        let d = carpeta("ajustes");
        let n = Nucleo::abrir(d.display().to_string(), "Mac".into()).unwrap();
        let a = n.ajustes_pdf().unwrap();
        assert!(a.aligerar_al_entrar);
        assert_eq!(a.nivel, NivelAligerar::Equilibrado);
        n.guardar_ajustes_pdf(AjustesPdf {
            aligerar_al_entrar: false,
            nivel: NivelAligerar::Pequeno,
        })
        .unwrap();
        let b = n.ajustes_pdf().unwrap();
        assert!(!b.aligerar_al_entrar);
        assert_eq!(b.nivel, NivelAligerar::Pequeno);
        let toml = std::fs::read_to_string(d.join("pixpinmax.toml")).unwrap();
        assert!(toml.contains("pequeno"), "{toml}");
    }
}
