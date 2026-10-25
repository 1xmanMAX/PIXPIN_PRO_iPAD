//! **`buscar_en_mis_cosas` en el Mac**: lo que el asistente sabe de los
//! documentos del usuario («¿que dice la tesis sobre…?», «¿donde mande el
//! plano de cunetas?»), como Windows (`apps/pixpin/src/asistente/documentos.rs`).
//!
//! El indice y la busqueda (BM25, trozos con su origen, citas) son los de
//! `pixpin_ia::documentos`. Aqui va lo del Mac:
//!
//! - **Que se indexa** ([`fuentes`]): de cada chat, sus mensajes (notas,
//!   voz pasada a texto, listas, los nombres de los adjuntos), los PDF,
//!   Office, `.md`, `.txt` y `.csv` que se adjuntaron y sus notas Markdown.
//!   El chat «Asistente» no: son sus propias respuestas.
//! - **Como se lee** ([`LectorDelMac`]): un PDF, con el texto que ya leyo
//!   «Buscar en todo» (`cache/textos`) o, si aun no, con PDFKit desde Swift
//!   ([`AppIa::leer_texto`]); Office, OpenDocument y lo demas con el lector
//!   de «Buscar en todo» (`buscar::textos::extraer`); notas y textos tal cual.
//! - **Al dia sin hilo propio**: cada busqueda lee solo lo que cambio (por
//!   tamano y fecha); la ventana del asistente lo pone al dia al abrirse
//!   ([`ia_documentos_al_dia`]). Vive en `<datos>/cache/documentos` (no se
//!   sincroniza) y se suelta de la memoria tras [`REPOSO`].
//! - **La nube** ([`permiso_nube`]): con Claude o DeepSeek, los trozos
//!   encontrados salen a internet. Con `documentos_a_la_nube` vacio se
//!   pregunta la primera vez y la respuesta queda en los ajustes.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use pixpin_ia::documentos::{Abrir, Alcance, Firma, Fuente, Hallazgo, Indice, Lector, PeticionBusqueda, Tipo, Unidad, texto};
use pixpin_proyecto::almacen;
use pixpin_proyecto::cuaderno::{Clase, Cuaderno, Mensaje};

use super::turno::AppIa;
use crate::Nucleo;

/// Tras cuanto sin buscar se suelta el indice de la memoria.
pub const REPOSO: Duration = Duration::from_secs(5 * 60);
/// Lo mas grande que se lee de un fichero.
pub const MAX_BYTES: u64 = 200 * 1024 * 1024;
/// El aparato de los mensajes que escribe la IA en Windows (su chat).
const APARATO_IA: &str = "IA";
/// Lo que deja el OCR de Windows al lado de un PDF escaneado.
const MARCA_HOJA: &str = "--- Hoja ";
const SUFIJO_OCR: &str = " (texto).txt";

fn tipo_de(nombre: &str) -> Option<Tipo> {
    Some(match pixpin_docs::extension(nombre).as_str() {
        "pdf" => Tipo::Pdf,
        "docx" | "docm" | "dotx" | "dotm" | "odt" => Tipo::Word,
        "xlsx" | "xlsm" | "ods" => Tipo::Excel,
        "pptx" | "pptm" | "ppsx" | "ppsm" | "potx" | "potm" | "odp" => Tipo::PowerPoint,
        "md" | "markdown" => Tipo::Nota,
        "txt" | "csv" => Tipo::Texto,
        _ => return None,
    })
}

fn proyecto_de_pedido(f: &almacen::Ficha) -> Option<String> {
    (!f.es_guardados()).then(|| f.id.clone())
}

fn nombre_de(f: &almacen::Ficha) -> String {
    if f.es_guardados() { "Mensajes guardados".into() } else { f.nombre.clone() }
}

fn es_de_la_ia(m: &Mensaje) -> bool {
    m.aparato.as_deref() == Some(APARATO_IA)
}

/// **Todas las fuentes** de los datos de `raiz`. Solo mira el disco (firmas
/// y cuadernos), no lee ningun documento.
pub fn fuentes(raiz: &Path) -> Vec<Fuente> {
    let mut v = Vec::new();
    for ficha in almacen::Indice::leer(raiz).proyectos {
        if ficha.es_asistente() {
            continue;
        }
        let carpeta = almacen::carpeta(raiz, &ficha.id);
        let pedido = proyecto_de_pedido(&ficha);
        let pnombre = nombre_de(&ficha);
        let base = |clave: String, nombre: String, tipo: Tipo, ruta: PathBuf, firma: Firma, abrir: Abrir| Fuente {
            clave,
            proyecto: ficha.id.clone(),
            proyecto_nombre: pnombre.clone(),
            nombre,
            tipo,
            ruta,
            firma,
            abrir,
        };
        // 1. El chat entero.
        let jsonl = carpeta.join("guardados.jsonl");
        let Some(firma) = Firma::de(&jsonl) else { continue };
        v.push(base(
            format!("chat:{}", ficha.id),
            format!("Chat de {pnombre}"),
            Tipo::Chat,
            jsonl.clone(),
            firma,
            Abrir::Mensaje {
                proyecto: pedido.clone(),
                codigo: String::new(),
            },
        ));
        // 2. Sus adjuntos que se pueden leer.
        let cuaderno = Cuaderno::leer_de(&carpeta).unwrap_or_default();
        let adjuntos: Vec<(String, PathBuf)> = cuaderno
            .mensajes
            .iter()
            .filter(|m| m.clase == Some(Clase::Archivo) && !m.en_buzon)
            .filter_map(|m| {
                let ruta = pixpin_proyecto::vista::ruta_real(raiz, &ficha.id, m.ruta.as_deref()?)?;
                let nombre = if m.nombre.trim().is_empty() { pixpin_docs::nombre(&ruta) } else { m.nombre.trim().to_string() };
                Some((nombre, ruta))
            })
            .collect();
        let mut vistas = std::collections::HashSet::new();
        for (nombre, ruta) in &adjuntos {
            let Some(mut tipo) = tipo_de(nombre).or_else(|| tipo_de(&pixpin_docs::nombre(ruta))) else { continue };
            let Some(firma) = Firma::de(ruta) else { continue };
            if firma.bytes > MAX_BYTES || !vistas.insert(ruta.clone()) {
                continue;
            }
            let mut mostrado = nombre.clone();
            let mut abrir = Abrir::Fichero { ruta: ruta.clone(), pagina: None };
            // El texto del OCR de un PDF escaneado (Windows): cuenta como el PDF.
            if let Some(base_pdf) = nombre.strip_suffix(SUFIJO_OCR)
                && let Some((pdf_nombre, pdf)) = adjuntos
                    .iter()
                    .find(|(n, _)| pixpin_docs::extension(n) == "pdf" && pixpin_docs::sin_extension(n) == base_pdf)
            {
                tipo = Tipo::Pdf;
                mostrado = pdf_nombre.clone();
                abrir = Abrir::Fichero { ruta: pdf.clone(), pagina: None };
            }
            v.push(base(format!("f:{}", ruta.display()), mostrado, tipo, ruta.clone(), firma, abrir));
        }
        // 3. Sus notas Markdown.
        if let Ok(dir) = std::fs::read_dir(carpeta.join("notas")) {
            for e in dir.flatten() {
                let ruta = e.path();
                if tipo_de(&pixpin_docs::nombre(&ruta)) != Some(Tipo::Nota) || !vistas.insert(ruta.clone()) {
                    continue;
                }
                let Some(firma) = Firma::de(&ruta) else { continue };
                let titulo = std::fs::read_to_string(&ruta)
                    .ok()
                    .and_then(|t| t.lines().map(|l| l.trim().trim_start_matches('#').trim().to_string()).find(|l| !l.is_empty()))
                    .map(|t| t.chars().take(80).collect::<String>())
                    .unwrap_or_else(|| pixpin_docs::sin_extension(&pixpin_docs::nombre(&ruta)));
                v.push(base(
                    format!("f:{}", ruta.display()),
                    format!("Nota «{titulo}»"),
                    Tipo::Nota,
                    ruta.clone(),
                    firma,
                    Abrir::Fichero { ruta, pagina: None },
                ));
            }
        }
    }
    v
}

// ---------------------------------------------------------------------------
// Como se lee cada cosa

/// Lee cada formato con lo que el Mac ya tiene.
pub struct LectorDelMac {
    pub raiz: PathBuf,
    /// Para los PDF que «Buscar en todo» aun no leyo (PDFKit desde Swift).
    pub app: Option<Arc<dyn AppIa>>,
}

impl Lector for LectorDelMac {
    fn leer(&self, f: &Fuente) -> Result<Vec<Unidad>, String> {
        if f.tipo == Tipo::Chat {
            return leer_chat(f);
        }
        if !f.ruta.is_file() {
            return Err("el fichero ya no esta".into());
        }
        let nombre = pixpin_docs::nombre(&f.ruta);
        match tipo_de(&nombre) {
            Some(Tipo::Pdf) => self.leer_pdf(&f.ruta),
            Some(Tipo::Nota) | Some(Tipo::Texto) => leer_texto(&f.ruta),
            Some(_) => crate::buscar::textos::extraer(&f.ruta)
                .filter(|t| !t.trim().is_empty())
                .map(|texto| vec![Unidad { texto, ..Default::default() }])
                .ok_or_else(|| format!("«{nombre}» no se deja leer")),
            None => Err(format!("PixPin no sabe leer «{nombre}»")),
        }
    }
}

impl LectorDelMac {
    /// Un PDF por paginas: lo que ya leyo «Buscar en todo» o, si no, Swift.
    fn leer_pdf(&self, ruta: &Path) -> Result<Vec<Unidad>, String> {
        if let Some(t) = crate::buscar::textos::guardado(&self.raiz, ruta) {
            let texto = t.original.clone();
            if !texto.trim().is_empty() {
                return Ok(vec![Unidad { texto, ..Default::default() }]);
            }
        }
        let app = self.app.as_ref().ok_or("sin quien lea el PDF")?;
        let texto = app.leer_texto(ruta.display().to_string(), Vec::new()).map_err(|e| e.to_string())?;
        // «[página N]» al empezar cada una (`ManosDelAsistente.textoDePdf`).
        let mut v = Vec::new();
        let mut pagina = None;
        let mut actual = String::new();
        for linea in texto.lines() {
            if let Some(n) = linea.trim().strip_prefix("[página ").and_then(|r| r.trim_end_matches(']').parse::<u32>().ok()) {
                if !actual.trim().is_empty() {
                    v.push(Unidad { texto: std::mem::take(&mut actual), pagina, ..Default::default() });
                }
                actual.clear();
                pagina = Some(n);
                continue;
            }
            actual.push_str(linea);
            actual.push('\n');
        }
        if !actual.trim().is_empty() {
            v.push(Unidad { texto: actual, pagina, ..Default::default() });
        }
        Ok(v)
    }
}

/// Cada mensaje de un chat, su unidad, que abre el chat en el.
fn leer_chat(f: &Fuente) -> Result<Vec<Unidad>, String> {
    let texto = std::fs::read_to_string(&f.ruta).map_err(|e| format!("no se pudo leer el chat: {e}"))?;
    let proyecto = match &f.abrir {
        Abrir::Mensaje { proyecto, .. } => proyecto.clone(),
        _ => Some(f.proyecto.clone()),
    };
    let mut v = Vec::new();
    for m in Cuaderno::leer(&texto).mensajes {
        if m.en_buzon || es_de_la_ia(&m) || m.id.is_empty() {
            continue;
        }
        let (texto, seccion) = match &m.clase {
            None | Some(Clase::Nota) => (m.texto.clone(), None),
            Some(Clase::Voz) => (
                m.transcripcion.clone().filter(|t| !t.trim().is_empty()).unwrap_or_default(),
                Some("nota de voz".to_string()),
            ),
            Some(Clase::MiniApp) => (m.texto.clone(), Some("lista".to_string())),
            Some(Clase::Archivo) | Some(Clase::Imagen) => {
                if m.ruta.as_deref().is_some_and(|r| r.ends_with(".leccion")) {
                    continue;
                }
                let nombre = if m.nombre.trim().is_empty() {
                    m.ruta.as_deref().map(|r| r.rsplit(['/', '\\']).next().unwrap_or(r).to_string()).unwrap_or_default()
                } else {
                    m.nombre.clone()
                };
                (format!("{nombre} {}", m.texto), Some("adjunto".to_string()))
            }
            Some(Clase::Dibujo) | Some(Clase::Pagina) => (format!("{} {}", m.nombre, m.texto), Some("lienzo".to_string())),
            _ => (m.resumen(), None),
        };
        if texto.trim().is_empty() {
            continue;
        }
        v.push(Unidad {
            texto,
            pagina: None,
            seccion,
            abrir: Some(Abrir::Mensaje { proyecto: proyecto.clone(), codigo: m.id.clone() }),
            cuando: Some(m.cuando).filter(|c| *c > 0),
        });
    }
    Ok(v)
}

/// Un texto llano o una nota. Si es lo que dejo el OCR (`--- Hoja N ---`),
/// por paginas.
fn leer_texto(ruta: &Path) -> Result<Vec<Unidad>, String> {
    let bytes = std::fs::read(ruta).map_err(|e| format!("no se pudo leer: {e}"))?;
    let texto = pixpin_docs::texto_de(&bytes);
    if !texto.contains(MARCA_HOJA) {
        return Ok(vec![Unidad { texto, ..Default::default() }]);
    }
    let mut v = Vec::new();
    let mut pagina = None;
    let mut actual = String::new();
    for linea in texto.lines() {
        if let Some(n) = linea.trim().strip_prefix(MARCA_HOJA).and_then(|r| r.trim_end_matches('-').trim().parse::<u32>().ok()) {
            if !actual.trim().is_empty() {
                v.push(Unidad { texto: std::mem::take(&mut actual), pagina, ..Default::default() });
            }
            actual.clear();
            pagina = Some(n);
            continue;
        }
        actual.push_str(linea);
        actual.push('\n');
    }
    if !actual.trim().is_empty() {
        v.push(Unidad { texto: actual, pagina, ..Default::default() });
    }
    Ok(v)
}

// ---------------------------------------------------------------------------
// El indice del proceso

struct Abierto {
    raiz: PathBuf,
    indice: Indice,
    usado: Instant,
}

static INDICE: Mutex<Option<Abierto>> = Mutex::new(None);
/// Solo una puesta al dia a la vez.
static PONIENDO_AL_DIA: Mutex<()> = Mutex::new(());

/// Donde vive el indice.
pub fn carpeta_cache(raiz: &Path) -> PathBuf {
    raiz.join("cache").join("documentos")
}

/// Con el indice de `raiz` en la mano (cargado si hace falta).
fn con_indice<R>(raiz: &Path, f: impl FnOnce(&mut Indice) -> R) -> R {
    let mut g = INDICE.lock().unwrap_or_else(|e| e.into_inner());
    if g.as_ref().is_none_or(|a| a.raiz != raiz) {
        *g = Some(Abierto {
            raiz: raiz.to_path_buf(),
            indice: Indice::cargar(&carpeta_cache(raiz)),
            usado: Instant::now(),
        });
    }
    let a = g.as_mut().expect("recien puesto");
    a.usado = Instant::now();
    f(&mut a.indice)
}

/// **Pone el indice al dia** con lo que hay ahora: lee solo lo que cambio,
/// sin tener el indice cogido mientras lee (una busqueda no espera a un PDF
/// largo). Devuelve cuantas fuentes leyo.
pub fn poner_al_dia(raiz: &Path, lector: &LectorDelMac) -> usize {
    let _uno = PONIENDO_AL_DIA.lock().unwrap_or_else(|e| e.into_inner());
    let todas = fuentes(raiz);
    let leer = con_indice(raiz, |ix| {
        let (leer, quitar) = ix.pendientes(&todas);
        for c in &quitar {
            ix.quitar(c);
        }
        leer
    });
    let n = leer.len();
    for f in leer {
        // Un fichero raro no tumba el turno ni el indice.
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| lector.leer(&f)))
            .unwrap_or_else(|_| Err("el lector se rompio leyendolo".into()));
        con_indice(raiz, |ix| ix.aplicar(f, r));
    }
    con_indice(raiz, Indice::guardar);
    n
}

/// Suelta el indice si lleva [`REPOSO`] sin usarse.
pub fn soltar_si_reposa() {
    let mut g = INDICE.lock().unwrap_or_else(|e| e.into_inner());
    if g.as_ref().is_some_and(|a| a.usado.elapsed() >= REPOSO) {
        *g = None;
    }
}

/// Los proyectos que se llaman como dice el modelo (vale un trozo, sin
/// mayusculas ni tildes).
fn proyectos_llamados(raiz: &Path, nombre: &str) -> Vec<String> {
    let buscado = texto::plano(nombre.trim());
    almacen::Indice::leer(raiz)
        .proyectos
        .into_iter()
        .filter(|f| {
            let n = texto::plano(&nombre_de(f));
            !buscado.is_empty() && (n == buscado || n.contains(&buscado))
        })
        .map(|f| f.id)
        .collect()
}

/// **Busca** lo que pide el modelo en todo lo del usuario (el asistente del
/// Mac no vive en un chat compartido). Devuelve lo encontrado y un aviso
/// para el modelo si el proyecto que nombro no esta.
pub fn buscar(raiz: &Path, p: &PeticionBusqueda, lector: &LectorDelMac) -> (Vec<Hallazgo>, Option<String>) {
    let mut aviso = None;
    let alcance = match &p.proyecto {
        None => Alcance::Todo,
        Some(nombre) => {
            let ids = proyectos_llamados(raiz, nombre);
            if ids.is_empty() {
                aviso = Some(format!("No hay ningun proyecto llamado «{nombre}»; se busco en todos."));
                Alcance::Todo
            } else {
                Alcance::Proyectos(ids.into_iter().collect())
            }
        }
    };
    poner_al_dia(raiz, lector);
    let r = con_indice(raiz, |ix| ix.buscar(&p.consulta, &alcance, pixpin_ia::documentos::herramienta::RESULTADOS));
    (r, aviso)
}

// ---------------------------------------------------------------------------
// La nube

/// Lo que se pregunta la primera vez (Windows).
pub const PREGUNTA_NUBE: &str = "El asistente está usando un modelo de la nube. Para contestar sobre tus \
documentos le mandaría trozos de ellos (lo que encuentre en tus PDF, notas y chats) por internet.\n\n\
¿Lo permites? Lo que elijas se recuerda y se cambia en Ajustes › Asistente.";

/// Lo que se le dice al modelo si no hay permiso.
pub const SIN_PERMISO_NUBE: &str = "el usuario no permite mandar sus documentos a la nube; dile que puede \
permitirlo en Ajustes › Asistente o usar el modelo local, que busca sin salir del Mac";

/// **¿Pueden salir los trozos a la nube?** `si`/`no` en los ajustes manda;
/// vacio, se pregunta y la respuesta se guarda (sin carpeta de datos, no).
pub fn permiso_nube(nucleo: Option<&Nucleo>, confirmar: &dyn Fn(&str) -> bool) -> bool {
    let ajustes = nucleo.and_then(|n| n.ajustes().ok());
    match ajustes.as_ref().map(|a| a.asistente_documentos_a_la_nube.trim()) {
        Some("si") => true,
        Some("no") => false,
        _ => {
            let si = confirmar(PREGUNTA_NUBE);
            if let (Some(n), Some(mut a)) = (nucleo, ajustes) {
                a.asistente_documentos_a_la_nube = if si { "si" } else { "no" }.into();
                let _ = n.guardar_ajustes(a);
            }
            si
        }
    }
}

/// **Pone al dia el indice de los documentos** (al abrir la ventana del
/// asistente, desde un hilo de fondo: lee lo que cambio). Devuelve cuantas
/// fuentes leyo.
#[uniffi::export]
pub fn ia_documentos_al_dia(nucleo: Arc<Nucleo>, app: Arc<dyn AppIa>) -> u32 {
    let raiz = nucleo.ruta().to_path_buf();
    let lector = LectorDelMac { raiz: raiz.clone(), app: Some(app) };
    let n = poner_al_dia(&raiz, &lector) as u32;
    soltar_si_reposa();
    n
}
