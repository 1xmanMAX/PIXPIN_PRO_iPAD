//! **Las herramientas de documentos del asistente, del lado de la app**
//! (E1; `asistente/documentos_acciones.rs` de Windows, la tabla esta en
//! `pixpin_ia::acciones_documentos`).
//!
//! - `tabla_de_imagen`: las palabras con su sitio (la capa de texto del PDF
//!   o Vision desde Swift) → filas y columnas (`pixpin_ia::tabla_escaneada`)
//!   → el pedido `tabla_nueva` (la tabla del chat, que viaja al movil) o un
//!   `.xlsx` (`pixpin_proyecto::tabla_xlsx`) que entra al chat con el pedido
//!   `chat`.
//! - `anotar_documento`: busca el texto en las paginas (PDF, o Word y
//!   PowerPoint maquetados como en el lector, con las letras de Core Text)
//!   o en los textos del plano, y escribe en **las mismas capas** que el
//!   lector ([`Nucleo::anotar_hoja`], la `anot-<uid>-p<n>` con su `.hoja`)
//!   y el visor de planos ([`crate::planos::Plano::anotar`]), con las
//!   figuras del motor comun. Asi se ve al abrir y viaja al movil.
//! - `buscar_en_documento`: el pedido `abrir` con `buscar` (PDF/Office) o
//!   `zona` (plano), para que el visor lleve ahi.
//! - `abrir_por_nombre`: el archivo por su nombre o lo que trata, de los
//!   adjuntos de todos los chats; si hay varios, se le dicen al modelo para
//!   que pregunte con una lista corta. Nunca se le pide un codigo al usuario.
//! - `exportar_documento`: la hoja de compartir comun de Swift
//!   (`PrepararCompartir` + `generar`, [`AppIa::exportar`]), y lo hecho entra
//!   al chat.
//!
//! Lo que solo sabe hacer el Mac desde Swift (las medidas y el dibujo de una
//! hoja de PDF con PDFKit, el OCR de Vision, exportar) se pide por
//! [`AppIa`]; lo demas es el mismo codigo que Windows. Todo corre en el hilo
//! del turno.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use pixpin_ia::acciones::{AccionApp, normalizar};
use pixpin_ia::acciones_documentos::{ABRIR, ANOTAR, BUSCAR_EN, EXPORTAR, TABLA};
use pixpin_ia::tabla_escaneada::{self, Palabra, TablaLeida};
use pixpin_motor2d::{ColorRgba, Elemento, Figura, Punto2};
use pixpin_pdf::texto::PaginaDeTexto;
use pixpin_proyecto::almacen;
use pixpin_proyecto::cuaderno::{Clase, Cuaderno};
use serde_json::{Value, json};

use super::turno::{AdjuntoIa, AppIa, PalabraLeida};
use crate::Nucleo;

/// Lo que necesitan estas herramientas del chat que las pide.
pub struct Contexto<'a> {
    pub nucleo: &'a Nucleo,
    /// El chat desde el que se pide (su id); vacio: «Mensajes guardados».
    pub proyecto: &'a str,
    pub adjuntos: &'a [AdjuntoIa],
    /// Las manos de Swift (pedidos, PDFKit, Vision); sin ellas solo se
    /// puede lo que no las necesita.
    pub app: Option<&'a dyn AppIa>,
    /// Si el usuario pulso «Parar».
    pub parar: &'a dyn Fn() -> bool,
    /// Hora local menos UTC (las fechas de lo que se le cuenta).
    pub desfase_ms: i64,
}

impl Contexto<'_> {
    pub fn raiz(&self) -> &Path {
        self.nucleo.ruta()
    }

    pub fn app(&self) -> Result<&dyn AppIa, String> {
        self.app.ok_or_else(|| "aqui no se puede hacer eso (sin la app)".to_string())
    }
}

/// **Hace una herramienta de documentos.** `Ok` lleva lo que se le cuenta
/// al modelo; `Err`, el motivo (que el modelo dira o corregira).
pub fn hacer(a: &AccionApp, cx: &Contexto) -> Result<String, String> {
    let args = &a.pedido;
    match a.herramienta {
        TABLA => tabla(args, cx),
        ANOTAR => anotar(args, cx),
        BUSCAR_EN => buscar_en(args, cx),
        ABRIR => abrir(args, cx),
        EXPORTAR => exportar(args, cx),
        otra => Err(format!("la herramienta «{otra}» no es de documentos")),
    }
}

pub(crate) fn texto_de<'a>(args: &'a Value, campo: &str) -> Option<&'a str> {
    args.get(campo).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty())
}

fn numeros_de(args: &Value, campo: &str) -> Vec<f64> {
    args.get(campo)
        .and_then(Value::as_array)
        .map(|l| l.iter().filter_map(Value::as_f64).collect())
        .unwrap_or_default()
}

/// El `proyecto` de los pedidos: el chat de ahora (sin el, «Mensajes
/// guardados»).
fn proyecto_del_pedido(cx: &Contexto) -> Value {
    if cx.proyecto.trim().is_empty() { Value::Null } else { json!(cx.proyecto) }
}

/// Manda un pedido del protocolo a la app (`AppIa::pedido`).
pub(crate) fn mandar(cx: &Contexto, pedido: Value) -> Result<(), String> {
    match cx.app()?.pedido(pedido.to_string()) {
        None => Ok(()),
        Some(motivo) => Err(motivo),
    }
}

/// La carpeta de lo que el asistente hace antes de que entre al chat.
pub(crate) fn carpeta_temporal() -> PathBuf {
    std::env::temp_dir().join("pixpin-asistente")
}

// ---------------------------------------------------------------------------
// Que archivo: por su nombre o lo que trata

/// Un archivo de un chat.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidato {
    pub nombre: String,
    pub ruta: PathBuf,
    pub proyecto: String,
    pub proyecto_nombre: String,
    pub cuando: i64,
}

/// Que clase de fichero sirve para una herramienta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sirve {
    /// PDF o imagen (tabla escaneada).
    Escaneado,
    /// PDF, Word, PowerPoint o plano (anotar, buscar dentro).
    Documento,
    /// Cualquiera (abrir, exportar).
    Cualquiera,
    /// Lo que se anota con cabeza (`anotar_inteligente`): PDF, Word,
    /// PowerPoint, Excel, foto o plano.
    Anotable,
}

fn extension(nombre: &str) -> String {
    pixpin_docs::extension(nombre)
}

/// Si es un plano (DWG o DXF, lo que abre el visor de planos).
pub fn es_plano(nombre: &str) -> bool {
    matches!(extension(nombre).as_str(), "dwg" | "dxf")
}

/// Si es un Word o un PowerPoint de los que se paginan como Office.
pub fn es_office_de_paginas(nombre: &str) -> bool {
    matches!(extension(nombre).as_str(), "docx" | "docm" | "dotx" | "dotm" | "pptx" | "pptm" | "ppsx" | "ppsm" | "potx" | "potm")
}

pub fn es_imagen(ruta: &Path) -> bool {
    super::turno::es_imagen(&ruta.to_string_lossy())
}

pub fn es_pdf(ruta: &Path) -> bool {
    extension(&pixpin_docs::nombre(ruta)) == "pdf"
}

fn sirve(nombre: &str, s: Sirve) -> bool {
    let e = extension(nombre);
    match s {
        Sirve::Escaneado => e == "pdf" || es_imagen(Path::new(nombre)),
        Sirve::Documento => e == "pdf" || es_office_de_paginas(nombre) || es_plano(nombre),
        Sirve::Cualquiera => !e.is_empty(),
        Sirve::Anotable => {
            e == "pdf" || es_office_de_paginas(nombre) || es_plano(nombre) || es_imagen(Path::new(nombre))
                || matches!(e.as_str(), "xlsx" | "xls" | "ods" | "csv")
        }
    }
}

/// **Los archivos de todos los chats** (los adjuntos que estan en este Mac),
/// sin el chat del asistente.
pub fn candidatos(raiz: &Path) -> Vec<Candidato> {
    let mut v = Vec::new();
    for ficha in almacen::Indice::leer(raiz).proyectos {
        if ficha.es_asistente() {
            continue;
        }
        let pnombre = if ficha.es_guardados() { "Mensajes guardados".to_string() } else { ficha.nombre.clone() };
        let Ok(c) = Cuaderno::leer_de(&almacen::carpeta(raiz, &ficha.id)) else { continue };
        for m in c.mensajes.iter().filter(|m| m.clase == Some(Clase::Archivo) && !m.en_buzon) {
            let Some(ruta) = m.ruta.as_deref().and_then(|r| pixpin_proyecto::vista::ruta_real(raiz, &ficha.id, r)) else {
                continue;
            };
            let ruta: PathBuf = ruta.components().collect();
            if !ruta.is_file() {
                continue;
            }
            let nombre = if m.nombre.trim().is_empty() { pixpin_docs::nombre(&ruta) } else { m.nombre.trim().to_string() };
            v.push(Candidato { nombre, ruta, proyecto: ficha.id.clone(), proyecto_nombre: pnombre.clone(), cuando: m.cuando });
        }
    }
    v
}

/// Palabras que no dicen que archivo es.
const VACIAS: &[&str] = &[
    "de", "del", "la", "el", "los", "las", "en", "y", "mi", "mis", "que", "con", "por", "para", "un", "una", "abre",
    "abrir", "abreme", "archivo", "fichero", "documento", "muestrame", "ensename", "ese", "esa", "este", "esta", "al", "a", "lo",
];

fn palabras(s: &str) -> Vec<String> {
    normalizar(s)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect()
}

/// Lo que se parecen una palabra pedida y una del nombre: igual, o una
/// empieza por la otra («senal» y «senalizacion»).
fn parecido(q: &str, n: &str) -> f64 {
    if q == n {
        2.0
    } else if q.len().min(n.len()) >= 4 && (q.starts_with(n) || n.starts_with(q)) {
        1.5
    } else {
        0.0
    }
}

/// **Cuanto encaja un archivo con la descripcion.** Cada palabra suma lo
/// que mas se parezca en el nombre (o, a medias, en el nombre del chat); el
/// tipo dicho («plano», «pdf», «excel», «foto») suma si es el suyo.
pub fn puntuar(c: &Candidato, descripcion: &str) -> f64 {
    let nombre = palabras(&c.nombre);
    let chat = palabras(&c.proyecto_nombre);
    let ext = extension(&c.nombre);
    let mut s = 0.0;
    for q in palabras(descripcion) {
        let tipo = match q.as_str() {
            "plano" | "planos" | "dwg" | "dxf" | "cad" => Some(es_plano(&c.nombre)),
            "pdf" => Some(ext == "pdf"),
            "excel" | "xlsx" | "libro" => Some(matches!(ext.as_str(), "xlsx" | "xls" | "xlsm" | "csv")),
            "word" | "docx" => Some(matches!(ext.as_str(), "docx" | "doc")),
            "foto" | "imagen" | "captura" => Some(es_imagen(&c.ruta)),
            "presentacion" | "diapositivas" | "pptx" => Some(matches!(ext.as_str(), "pptx" | "ppt")),
            _ => None,
        };
        if let Some(es) = tipo {
            s += if es { 1.0 } else { -0.5 };
        }
        if VACIAS.contains(&q.as_str()) {
            continue;
        }
        let en_nombre = nombre.iter().map(|n| parecido(&q, n)).fold(0.0, f64::max);
        let en_chat = chat.iter().map(|n| parecido(&q, n)).fold(0.0, f64::max) * 0.5;
        s += en_nombre.max(en_chat);
    }
    s
}

/// **Elige entre `todos`**: el nombrado o descrito; sin nada, el adjunto
/// mas reciente que sirva (del turno, luego del chat). Si hay varios que
/// encajan casi igual, el `Err` los lista para que el modelo pregunte cual.
/// `que`: como se dice lo que se busca en el error.
pub(crate) fn elegir_entre(
    cx: &Contexto,
    todos: Vec<Candidato>,
    archivo: Option<&str>,
    vale: &dyn Fn(&str) -> bool,
    que: &str,
) -> Result<Candidato, String> {
    let Some(descripcion) = archivo.map(str::trim).filter(|a| !a.is_empty()) else {
        // Lo adjunto en este turno, el ultimo.
        if let Some(a) = cx.adjuntos.iter().rev().find(|a| vale(&a.nombre) && Path::new(&a.ruta).is_file()) {
            let ruta = PathBuf::from(&a.ruta);
            return Ok(todos.iter().find(|c| c.ruta == ruta).cloned().unwrap_or(Candidato {
                nombre: a.nombre.clone(),
                ruta,
                proyecto: cx.proyecto.to_string(),
                proyecto_nombre: String::new(),
                cuando: 0,
            }));
        }
        return todos
            .iter()
            .filter(|c| c.proyecto == cx.proyecto)
            .max_by_key(|c| c.cuando)
            .cloned()
            .ok_or_else(|| format!("en este chat no hay {que}; pregunta al usuario cual (por su nombre)"));
    };
    // Una ruta escrita tal cual.
    let como_ruta = PathBuf::from(descripcion.trim_matches(['"', '\'', '«', '»']));
    if como_ruta.is_absolute() && como_ruta.is_file() {
        let nombre = pixpin_docs::nombre(&como_ruta);
        if !vale(&nombre) {
            return Err(format!("«{nombre}» no es {que}"));
        }
        return Ok(Candidato { nombre, ruta: como_ruta, proyecto: String::new(), proyecto_nombre: String::new(), cuando: 0 });
    }
    // Lo adjunto en el turno cuenta como de este chat.
    let mut puntuados: Vec<(f64, Candidato)> = todos
        .into_iter()
        .map(|c| {
            let mut p = puntuar(&c, descripcion);
            if c.proyecto == cx.proyecto {
                p += 0.25;
            }
            (p, c)
        })
        .filter(|(p, _)| *p >= 1.5)
        .collect();
    // El mas nuevo antes a igual puntuacion; el mismo fichero, una vez.
    puntuados.sort_by(|a, b| b.0.total_cmp(&a.0).then(b.1.cuando.cmp(&a.1.cuando)));
    puntuados.dedup_by(|a, b| a.1.ruta == b.1.ruta);
    let Some((mejor, _)) = puntuados.first().cloned() else {
        return Err(format!("no hay {que} en los chats que se llame o trate de «{descripcion}»; pregunta al usuario el nombre"));
    };
    let parecidos: Vec<&Candidato> = puntuados.iter().filter(|(p, _)| *p >= mejor - 0.3).map(|(_, c)| c).collect();
    if parecidos.len() > 1 {
        let lista: Vec<String> = parecidos
            .iter()
            .take(5)
            .enumerate()
            .map(|(i, c)| format!("{}) {} (chat {})", i + 1, c.nombre, c.proyecto_nombre))
            .collect();
        // La pregunta ya hecha: un modelo pequeno la copia tal cual (Windows,
        // medido el 14-oct: con solo «pregunta cual», el 2B preguntaba sin
        // la lista).
        return Err(format!(
            "no hagas nada todavia; contesta al usuario EXACTAMENTE esto: «Hay {} que encajan: {}. ¿Cual?» y, cuando elija, vuelve a llamar con su nombre exacto",
            parecidos.len(),
            lista.join("; ")
        ));
    }
    Ok(puntuados.swap_remove(0).1)
}

/// **Elige el archivo** de los adjuntos de los chats que sirvan a `s`.
pub fn elegir(cx: &Contexto, archivo: Option<&str>, s: Sirve) -> Result<Candidato, String> {
    let todos: Vec<Candidato> = candidatos(cx.raiz()).into_iter().filter(|c| sirve(&c.nombre, s)).collect();
    let sin_nada = todos.iter().all(|c| c.proyecto != cx.proyecto)
        && !cx.adjuntos.iter().any(|a| sirve(&a.nombre, s));
    if archivo.is_none_or(|a| a.trim().is_empty()) && sin_nada {
        return Err(no_sirve(cx, s));
    }
    elegir_entre(cx, todos, archivo, &|n| sirve(n, s), "ningun archivo que sirva")
}

/// **Por que no hay archivo que sirva**, dicho para que el modelo elija
/// bien (Windows, 15-oct: con un Excel adjunto, anotar_documento contestaba
/// «no hay ningun archivo» y el modelo pedia al usuario que lo adjuntara
/// otra vez). Dice que formatos sirven, que hay en el chat y que
/// herramienta va con eso.
fn no_sirve(cx: &Contexto, s: Sirve) -> String {
    let que = match s {
        Sirve::Escaneado => "una foto o un PDF",
        Sirve::Documento => "un PDF, Word, PowerPoint o plano",
        Sirve::Cualquiera => "ningun archivo",
        Sirve::Anotable => "un PDF, Word, PowerPoint, Excel, foto o plano",
    };
    let mut hay: Vec<Candidato> = candidatos(cx.raiz()).into_iter().filter(|c| c.proyecto == cx.proyecto).collect();
    let de_turno: Vec<Candidato> = cx
        .adjuntos
        .iter()
        .filter(|a| !hay.iter().any(|c| c.ruta == Path::new(&a.ruta)))
        .map(|a| Candidato {
            nombre: a.nombre.clone(),
            ruta: PathBuf::from(&a.ruta),
            proyecto: cx.proyecto.to_string(),
            proyecto_nombre: String::new(),
            cuando: 0,
        })
        .collect();
    hay.extend(de_turno);
    if hay.is_empty() {
        return format!("en este chat no hay {que}; pregunta al usuario cual (por su nombre)");
    }
    hay.sort_by_key(|c| std::cmp::Reverse(c.cuando));
    let nombres: Vec<String> = hay.iter().take(6).map(|c| format!("«{}»", c.nombre)).collect();
    let consejo = match s {
        Sirve::Documento => "; para leer un Excel usa leer_excel",
        Sirve::Escaneado => "; un Excel ya es una tabla (leer_excel), y un Word o PowerPoint se lee con leer_documento",
        _ => "",
    };
    format!("esta herramienta necesita {que} y en este chat solo hay {}{consejo}", nombres.join(", "))
}

// ---------------------------------------------------------------------------
// El texto de las paginas, con donde cae cada letra

/// El ancho de una hoja del lector (`pixpin_docs::vista::ANCHO_HOJA`): las
/// capas de un PDF o un Word se escriben en estas unidades.
pub const ANCHO_HOJA: f32 = pixpin_docs::vista::ANCHO_HOJA;

/// Las paginas de un PDF, Word o PowerPoint: su texto con la caja de cada
/// letra (fracciones de la hoja) y el alto de cada hoja en unidades del
/// lector. Un PDF escaneado pasa por el OCR (como mucho [`MAX_OCR`] hojas).
pub struct Paginas {
    pub textos: Vec<PaginaDeTexto>,
    pub altos: Vec<f32>,
}

const MAX_OCR: usize = 12;
/// El ancho al que se pinta una hoja para el OCR (~200 ppp en A4).
pub const ANCHO_OCR: u32 = 1700;

/// **El alto de cada hoja de un PDF** en unidades del lector (1400 de
/// ancho), con las medidas de PDFKit (la caja que se ve, ya girada: las
/// mismas que usa el lector para su tinta).
pub fn altos_de_pdf(app: &dyn AppIa, ruta: &Path) -> Result<Vec<f32>, String> {
    let m = app.paginas_de_pdf(ruta.display().to_string());
    if m.is_empty() {
        return Err(format!("no se pudo abrir el PDF «{}»", pixpin_docs::nombre(ruta)));
    }
    Ok(m.iter().map(|p| ANCHO_HOJA * p.alto / p.ancho.max(1.0)).collect())
}

pub fn paginas(app: Option<&dyn AppIa>, ruta: &Path, parar: &dyn Fn() -> bool) -> Result<Paginas, String> {
    let nombre = pixpin_docs::nombre(ruta);
    if es_office_de_paginas(&nombre) {
        return paginas_de_office(ruta);
    }
    let app = app.ok_or("aqui no se pueden abrir PDF")?;
    let altos = altos_de_pdf(app, ruta)?;
    let mut textos = pixpin_pdf::texto::de_fichero(ruta).unwrap_or_default();
    textos.resize_with(altos.len(), || PaginaDeTexto { texto: String::new(), cajas: Vec::new() });
    let mut leidas = 0;
    for (i, t) in textos.iter_mut().enumerate() {
        if t.texto.trim().chars().count() >= 20 || leidas >= MAX_OCR {
            continue;
        }
        if parar() {
            return Err("el usuario lo paro".into());
        }
        leidas += 1;
        let Some((palabras, ancho, alto)) = ocr_de_pagina(app, ruta, i, ANCHO_OCR) else { continue };
        *t = pagina_de_ocr(&palabras, ancho, alto);
    }
    Ok(Paginas { textos, altos })
}

/// La hoja `i` pintada por PDFKit a `ancho` pixeles y leida por Vision:
/// sus palabras y lo que mide la imagen.
pub(crate) fn ocr_de_pagina(app: &dyn AppIa, ruta: &Path, i: usize, ancho: u32) -> Option<(Vec<PalabraLeida>, u32, u32)> {
    let png = app.pintar_pagina(ruta.display().to_string(), i as u32, ancho)?;
    let (w, h) = crate::editor_imagenes::medidas_png(&png)?;
    Some((app.palabras_de_imagen(png), w, h))
}

/// El texto de una imagen por el OCR, como una pagina: cada letra con la
/// caja de su palabra repartida a partes iguales.
pub fn pagina_de_ocr(palabras: &[PalabraLeida], ancho: u32, alto: u32) -> PaginaDeTexto {
    let (w, h) = (ancho.max(1) as f32, alto.max(1) as f32);
    let mut t = PaginaDeTexto { texto: String::new(), cajas: Vec::new() };
    let mut renglon = palabras.first().map(|p| p.renglon);
    for p in palabras {
        if Some(p.renglon) != renglon {
            renglon = Some(p.renglon);
            t.texto.push('\n');
            t.cajas.push(None);
        }
        let n = p.texto.chars().count().max(1) as f32;
        let paso = p.ancho / n;
        for (k, c) in p.texto.chars().enumerate() {
            let x0 = p.x + paso * k as f32;
            t.texto.push(c);
            t.cajas.push(Some([x0 / w, p.y / h, (x0 + paso) / w, (p.y + p.alto) / h]));
        }
        t.texto.push(' ');
        t.cajas.push(None);
    }
    if !palabras.is_empty() {
        t.texto.push('\n');
        t.cajas.push(None);
    }
    t
}

/// Un Word o un PowerPoint maquetado como en el lector (mismas letras de
/// Core Text, mismas hojas).
fn paginas_de_office(ruta: &Path) -> Result<Paginas, String> {
    let doc = super::letras::abrir_office(ruta)?;
    let mut textos = Vec::new();
    let mut altos = Vec::new();
    for i in 0..400 {
        let Some((alto, t)) = doc.con_pagina(i, |p, medidor| (ANCHO_HOJA * p.alto / p.ancho.max(1.0), texto_de_pagina(p, medidor))) else {
            break;
        };
        altos.push(alto);
        textos.push(t);
    }
    if textos.is_empty() {
        return Err("el documento no tiene paginas".into());
    }
    Ok(Paginas { textos, altos })
}

/// **El texto de una hoja de Office con la caja de cada letra** (en
/// fracciones de la hoja), como `lector_pdf::office::texto_de_pagina` de
/// Windows: los saltos de renglon y los espacios entre trozos, y cada letra
/// con su avance medido.
pub fn texto_de_pagina(pagina: &pixpin_office::Pagina, medidor: &dyn pixpin_office::Medidor) -> PaginaDeTexto {
    let (w, h) = (pagina.ancho.max(1.0), pagina.alto.max(1.0));
    let mut texto = String::new();
    let mut cajas: Vec<Option<[f32; 4]>> = Vec::new();
    // La linea base, el tamano y donde acabo el trozo anterior.
    let mut antes: Option<(f32, f32, f32)> = None;
    let mut avances = Vec::new();
    for (t, r) in pagina.textos() {
        if t.texto.is_empty() {
            continue;
        }
        if let Some((y0, tam0, fin)) = antes {
            let tam = tam0.max(t.letra.tam);
            let blanco = texto.ends_with(char::is_whitespace) || t.texto.starts_with(char::is_whitespace);
            if (t.y - y0).abs() > tam * 0.5 {
                let salto = if (t.y - y0).abs() > tam * 1.9 || t.y < y0 { '\n' } else { ' ' };
                if !blanco || salto == '\n' {
                    texto.push(salto);
                    cajas.push(None);
                }
            } else if t.x - fin > tam * 0.2 && !blanco {
                texto.push(' ');
                cajas.push(None);
            }
        }
        avances.clear();
        avances_de(t, medidor, &mut avances);
        let mut x = r.x;
        for (c, a) in t.texto.chars().zip(avances.iter()) {
            texto.push(c);
            cajas.push(Some([x / w, r.y / h, (x + a) / w, (r.y + r.alto) / h]));
            x += a;
        }
        antes = Some((t.y, t.letra.tam, r.x + t.ancho));
    }
    PaginaDeTexto { texto, cajas }
}

/// Lo que avanza cada letra de un trozo, con el interletrado y el
/// justificado. Si el medidor no da una por letra, el ancho del trozo
/// repartido a partes iguales.
fn avances_de(t: &pixpin_office::pagina::Texto, medidor: &dyn pixpin_office::Medidor, salida: &mut Vec<f32>) {
    let n = t.texto.chars().count();
    medidor.avances(&t.texto, &t.letra, salida);
    if salida.len() != n {
        salida.clear();
        salida.extend(std::iter::repeat_n(t.ancho / n.max(1) as f32, n));
        return;
    }
    for (a, c) in salida.iter_mut().zip(t.texto.chars()) {
        *a += t.espaciado;
        if c == ' ' {
            *a += t.extra_espacio;
        }
    }
}

/// Un sitio encontrado: la hoja (desde 0) y las cajas de sus renglones en
/// unidades del lector (1400 de ancho).
#[derive(Debug, Clone, PartialEq)]
pub struct Sitio {
    pub pagina: usize,
    pub cajas: Vec<[f32; 4]>,
    /// Un trozo del texto de alrededor, para contarselo al modelo.
    pub contexto: String,
}

/// **Busca `consulta` en las paginas** (sin mirar mayusculas ni tildes,
/// como ⌘F). `pagina`: solo esa (desde 0).
pub fn buscar_en_paginas(p: &Paginas, consulta: &str, pagina: Option<usize>, max: usize) -> Vec<Sitio> {
    let mut v = Vec::new();
    for k in pixpin_docs::buscar::buscar(p.textos.iter().map(|t| t.texto.as_str()), consulta) {
        if pagina.is_some_and(|q| q != k.texto) {
            continue;
        }
        let t = &p.textos[k.texto];
        let alto = p.altos.get(k.texto).copied().unwrap_or(ANCHO_HOJA * 1.414);
        let cajas: Vec<[f32; 4]> = t
            .cajas_de(k.inicio, k.largo)
            .into_iter()
            .map(|c| [c[0] * ANCHO_HOJA, c[1] * alto, c[2] * ANCHO_HOJA, c[3] * alto])
            .collect();
        if cajas.is_empty() {
            continue;
        }
        let letras: Vec<char> = t.texto.chars().collect();
        let desde = k.inicio.saturating_sub(40);
        let hasta = (k.inicio + k.largo + 40).min(letras.len());
        let contexto: String = letras[desde..hasta].iter().collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ");
        v.push(Sitio { pagina: k.texto, cajas, contexto });
        if v.len() >= max {
            break;
        }
    }
    v
}

// ---------------------------------------------------------------------------
// Las figuras: del motor comun

/// Como se anota.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComoAnotar {
    Resaltar,
    Subrayar,
    Tachar,
    Nota,
    Encerrar,
    Marco,
    Dibujar,
}

impl ComoAnotar {
    pub fn de(s: &str) -> Option<ComoAnotar> {
        Some(match s {
            "resaltar" => ComoAnotar::Resaltar,
            "subrayar" => ComoAnotar::Subrayar,
            "tachar" => ComoAnotar::Tachar,
            "nota" => ComoAnotar::Nota,
            "encerrar" => ComoAnotar::Encerrar,
            "marco" => ComoAnotar::Marco,
            "dibujar" => ComoAnotar::Dibujar,
            _ => return None,
        })
    }
    fn color_de_siempre(self) -> (u8, u8, u8) {
        match self {
            ComoAnotar::Resaltar => (0xff, 0xd4, 0x3b),
            ComoAnotar::Subrayar | ComoAnotar::Tachar | ComoAnotar::Encerrar | ComoAnotar::Dibujar => (0xe0, 0x31, 0x31),
            ComoAnotar::Marco => (0x3d, 0x8b, 0xff),
            ComoAnotar::Nota => (0x34, 0x3a, 0x40),
        }
    }
}

/// El grupo de lo que pone el asistente (para encontrarlo y borrarlo junto).
pub const GRUPO: &str = "asistente-anotado";

fn rgb(color: Option<&str>, como: ComoAnotar) -> (u8, u8, u8) {
    color
        .and_then(pixpin_ia::herramientas::color)
        .and_then(|c| {
            let h = c.0.trim_start_matches('#');
            let n = u32::from_str_radix(h.get(..6)?, 16).ok()?;
            Some(((n >> 16) as u8, (n >> 8) as u8, n as u8))
        })
        .unwrap_or_else(|| como.color_de_siempre())
}

fn color_motor((r, g, b): (u8, u8, u8)) -> ColorRgba {
    ColorRgba::opaco(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}

/// La caja que abarca todas.
fn union(cajas: &[[f32; 4]]) -> [f32; 4] {
    cajas.iter().fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |u, c| [u[0].min(c[0]), u[1].min(c[1]), u[2].max(c[2]), u[3].max(c[3])])
}

/// Opacidad del resaltado: deja leer lo de debajo (`pines::marcas_texto`).
const OPACIDAD_RESALTADO: f32 = 0.40;
/// Cuanto se agranda el resaltado alrededor de las letras.
const AIRE_RESALTADO: f32 = 0.12;

/// **Las marcas sobre el texto** (`pines::marcas_texto::elementos` de
/// Windows: las mismas del pin): resaltar, subrayar o tachar cada caja,
/// redondeadas a pixeles enteros como alli.
fn marcas_texto(como: ComoAnotar, (r, g, b): (u8, u8, u8), cajas: &[[f32; 4]]) -> Vec<Elemento> {
    let color = ColorRgba::opaco(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    let base = Elemento {
        trazo: color,
        rugosidad: 0.0,
        grupos: vec![GRUPO.to_string()],
        ..Default::default()
    };
    cajas
        .iter()
        .map(|b| (b[0].round(), b[1].round(), (b[2] - b[0]).round().max(1.0), (b[3] - b[1]).round().max(1.0)))
        .map(|(x, y, w, h)| {
            let grosor = (h * 0.09).max(1.5);
            match como {
                ComoAnotar::Resaltar => {
                    let aire = h * AIRE_RESALTADO;
                    Elemento {
                        figura: Figura::Rectangulo,
                        x: x - aire,
                        y: y - aire,
                        ancho: w + 2.0 * aire,
                        alto: h + 2.0 * aire,
                        relleno: Some(color),
                        estilo_relleno: pixpin_motor2d::relleno::EstiloRelleno::Solido,
                        trazo: ColorRgba { a: 0.0, ..color },
                        grosor: 1.0,
                        opacidad: OPACIDAD_RESALTADO,
                        ..base.clone()
                    }
                }
                _ => {
                    let altura = if como == ComoAnotar::Tachar { y + h * 0.55 } else { y + h + grosor };
                    Elemento {
                        figura: Figura::Linea { puntos: vec![Punto2::nuevo(x, altura), Punto2::nuevo(x + w, altura)] },
                        x,
                        y: altura,
                        ancho: w,
                        alto: 0.0,
                        grosor,
                        ..base.clone()
                    }
                }
            }
        })
        .collect()
}

/// **Las figuras de una anotacion** sobre `cajas` (`[x0, y0, x1, y1]` en
/// las unidades de la capa, y hacia abajo): las mismas del lienzo y de las
/// marcas de texto de los pines. `letra`: el alto de letra de referencia
/// (el del texto marcado, o uno comodo para la hoja).
pub fn figuras(como: ComoAnotar, cajas: &[[f32; 4]], color: (u8, u8, u8), nota: Option<&str>, puntos: &[(f32, f32)], letra: f32) -> Vec<Elemento> {
    let grupo = vec![GRUPO.to_string()];
    let c = color_motor(color);
    let grosor = (letra * 0.12).max(1.0);
    match como {
        ComoAnotar::Resaltar | ComoAnotar::Subrayar | ComoAnotar::Tachar => marcas_texto(como, color, cajas),
        ComoAnotar::Encerrar | ComoAnotar::Marco => {
            if cajas.is_empty() {
                return Vec::new();
            }
            let u = union(cajas);
            let aire = letra * 0.6;
            vec![Elemento {
                figura: if como == ComoAnotar::Encerrar { Figura::Elipse } else { Figura::Rectangulo },
                x: u[0] - aire * 1.5,
                y: u[1] - aire,
                ancho: (u[2] - u[0]) + aire * 3.0,
                alto: (u[3] - u[1]) + aire * 2.0,
                trazo: c,
                grosor: grosor * 1.5,
                rugosidad: 0.0,
                grupos: grupo,
                ..Default::default()
            }]
        }
        ComoAnotar::Nota => {
            let texto = nota.unwrap_or("").trim();
            if texto.is_empty() {
                return Vec::new();
            }
            // A la derecha de lo marcado, o donde diga la zona.
            let u = if cajas.is_empty() { [0.0, 0.0, 0.0, letra] } else { union(cajas) };
            let tam = letra.max(1.0);
            let lineas = partir(texto, 28);
            let ancho = lineas.iter().map(|l| l.chars().count()).max().unwrap_or(1) as f32 * tam * 0.55 + tam;
            let alto = lineas.len() as f32 * tam * 1.3 + tam * 0.6;
            let (x, y) = (u[2] + tam * 0.8, u[1] - tam * 0.3);
            let papel = Elemento {
                figura: Figura::Rectangulo,
                x,
                y,
                ancho,
                alto,
                trazo: ColorRgba { a: 0.0, ..color_motor((0xe8, 0xc5, 0x47)) },
                relleno: Some(color_motor((0xff, 0xf3, 0xa3))),
                estilo_relleno: pixpin_motor2d::relleno::EstiloRelleno::Solido,
                grosor: 1.0,
                rugosidad: 0.0,
                grupos: grupo.clone(),
                ..Default::default()
            };
            let letras = Elemento {
                figura: Figura::Texto { texto: lineas.join("\n"), tam, familia: "sans".into() },
                x: x + tam * 0.5,
                y: y + tam * 0.3,
                ancho: ancho - tam,
                alto: alto - tam * 0.6,
                trazo: c,
                grosor: 1.0,
                rugosidad: 0.0,
                grupos: grupo,
                ..Default::default()
            };
            vec![papel, letras]
        }
        ComoAnotar::Dibujar => {
            if puntos.len() < 2 {
                return Vec::new();
            }
            let pts: Vec<Punto2> = puntos.iter().map(|(x, y)| Punto2::nuevo(*x, *y)).collect();
            let (x0, y0, x1, y1) = pts.iter().fold((f32::MAX, f32::MAX, f32::MIN, f32::MIN), |a, p| (a.0.min(p.x), a.1.min(p.y), a.2.max(p.x), a.3.max(p.y)));
            vec![Elemento {
                figura: Figura::Linea { puntos: pts },
                x: x0,
                y: y0,
                ancho: x1 - x0,
                alto: y1 - y0,
                trazo: c,
                grosor: grosor * 1.5,
                rugosidad: 0.0,
                grupos: grupo,
                ..Default::default()
            }]
        }
    }
}

/// Parte un texto en renglones de unas `n` letras, por palabras.
fn partir(texto: &str, n: usize) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for parrafo in texto.lines() {
        let mut l = String::new();
        for w in parrafo.split_whitespace() {
            if !l.is_empty() && l.chars().count() + 1 + w.chars().count() > n {
                v.push(std::mem::take(&mut l));
            }
            if !l.is_empty() {
                l.push(' ');
            }
            l.push_str(w);
        }
        v.push(l);
    }
    v
}

/// Una zona `[x, y, ancho, alto]` en fracciones a una caja `[x0, y0, x1, y1]`
/// dentro de `marco` (las mismas unidades, y hacia abajo).
fn zona_en(zona: &[f64], marco: [f32; 4]) -> Option<[f32; 4]> {
    let [x, y, w, h] = <[f64; 4]>::try_from(zona.get(..4)?).ok()?;
    if !(0.0..=1.0).contains(&x) || !(0.0..=1.0).contains(&y) || w <= 0.0 || h <= 0.0 {
        return None;
    }
    let (mw, mh) = (marco[2] - marco[0], marco[3] - marco[1]);
    Some([
        marco[0] + x as f32 * mw,
        marco[1] + y as f32 * mh,
        marco[0] + ((x + w).min(1.0)) as f32 * mw,
        marco[1] + ((y + h).min(1.0)) as f32 * mh,
    ])
}

fn puntos_en(puntos: &[f64], marco: [f32; 4]) -> Vec<(f32, f32)> {
    let (mw, mh) = (marco[2] - marco[0], marco[3] - marco[1]);
    puntos
        .chunks_exact(2)
        .map(|p| (marco[0] + p[0].clamp(0.0, 1.0) as f32 * mw, marco[1] + p[1].clamp(0.0, 1.0) as f32 * mh))
        .collect()
}

// ---------------------------------------------------------------------------
// 1. Tabla escaneada

/// Las palabras de una imagen o de la hoja `pagina` (desde 0; `None`: la
/// primera con texto) de un PDF: la capa de texto si la tiene, si no OCR.
pub fn palabras_de(app: Option<&dyn AppIa>, ruta: &Path, pagina: Option<usize>) -> Result<(Vec<Palabra>, usize), String> {
    if es_imagen(ruta) {
        let app = app.ok_or("aqui no se puede leer una imagen")?;
        let bytes = std::fs::read(ruta).map_err(|e| format!("no se pudo abrir la imagen: {e}"))?;
        return Ok((palabras_de_ocr(&app.palabras_de_imagen(bytes)), 0));
    }
    // La capa de texto del PDF (exacta y al instante).
    if let Ok(trozos) = pixpin_pdf::texto::trozos_de_fichero(ruta) {
        let elegida = match pagina {
            Some(p) => Some(p),
            None => trozos.iter().position(|t| t.len() >= 4),
        };
        if let Some(p) = elegida
            && let Some(t) = trozos.get(p)
            && t.len() >= 4
        {
            let ps = t
                .iter()
                .map(|t| Palabra::nueva(&t.texto, t.x as f64, (t.y - t.alto * 0.8) as f64, (t.fin - t.x).max(0.0) as f64, t.alto as f64))
                .collect();
            return Ok((ps, p));
        }
    }
    // Escaneado: la hoja pintada, por el OCR.
    let app = app.ok_or("aqui no se puede leer un PDF escaneado")?;
    let total = app.paginas_de_pdf(ruta.display().to_string()).len();
    if total == 0 {
        return Err("no se pudo abrir el PDF".into());
    }
    let p = pagina.unwrap_or(0);
    if p >= total {
        return Err(format!("el PDF tiene {total} paginas"));
    }
    let (palabras, _, _) = ocr_de_pagina(app, ruta, p, ANCHO_OCR).ok_or("no se pudo pintar la hoja para leerla")?;
    Ok((palabras_de_ocr(&palabras), p))
}

/// Las rayas tenues marcadas (una tabla de tema oscuro leida en negativo):
/// cada canal, a la cuarta respecto al fondo (Windows, 18-oct).
fn rayas_marcadas(px: &mut [u8]) {
    let mut g: Vec<u8> = px
        .chunks_exact(4)
        .step_by(5)
        .map(|p| ((p[0] as u32 * 299 + p[1] as u32 * 587 + p[2] as u32 * 114) / 1000) as u8)
        .collect();
    g.sort_unstable();
    let fondo = g.get(g.len() / 2).copied().unwrap_or(255).max(1) as f32;
    for p in px.chunks_exact_mut(4) {
        for c in &mut p[..3] {
            *c = (255.0 * (*c as f32 / fondo).min(1.0).powi(4)).round() as u8;
        }
    }
}

/// Las palabras de unos pixeles por Vision (la app), pasandolos a PNG.
fn palabras_de_pixeles(app: &dyn AppIa, rgba: &[u8], ancho: u32, alto: u32) -> Option<Vec<Palabra>> {
    let png = pixpin_codec::imagen::codificar_png(&pixpin_codec::ImagenRgba { ancho, alto, pixeles: rgba.to_vec() }).ok()?;
    Some(palabras_de_ocr(&app.palabras_de_imagen(png)))
}

/// **El aspecto de la tabla** (`pixpin_ia::tabla_aspecto`): en la foto, o
/// en la hoja del PDF pintada por PDFKit, con las palabras de Vision en
/// sus mismos pixeles. Con el ancho de esa imagen.
pub fn aspecto_de_tabla(app: &dyn AppIa, ruta: &Path, pagina: Option<usize>) -> Option<pixpin_ia::tabla_aspecto::Medida> {
    let img = if es_imagen(ruta) {
        pixpin_codec::imagen::cargar(ruta).ok()?
    } else {
        let p = match pagina {
            Some(p) => p,
            None => palabras_de(Some(app), ruta, None).ok()?.1,
        };
        let png = app.pintar_pagina(ruta.display().to_string(), p as u32, ANCHO_OCR)?;
        let tmp = carpeta_temporal().join(format!("aspecto-{}.png", crate::ahora_ms()));
        std::fs::create_dir_all(tmp.parent()?).ok()?;
        std::fs::write(&tmp, png).ok()?;
        let i = pixpin_codec::imagen::cargar(&tmp).ok();
        let _ = std::fs::remove_file(&tmp);
        i?
    };
    let ps = palabras_de_pixeles(app, &img.pixeles, img.ancho, img.alto)?;
    let mut m = pixpin_ia::tabla_aspecto::medir(&img.pixeles, img.ancho, img.alto, &ps)?;
    if m.columnas.is_empty() {
        let mut fuerte = img.pixeles.clone();
        rayas_marcadas(&mut fuerte);
        if let Some(mut m2) = pixpin_ia::tabla_aspecto::medir(&fuerte, img.ancho, img.alto, &ps)
            && !m2.columnas.is_empty()
        {
            m2.texto = m.texto;
            m = m2;
        }
    }
    // Torcida: Vision otra vez sobre la foto enderezada, y la medida con esas.
    if m.inclinacion != 0.0
        && !m.columnas.is_empty()
        && let derecha = pixpin_ia::tabla_aspecto::enderezar(&img.pixeles, img.ancho, img.alto, m.inclinacion)
        && let Some(ps2) = palabras_de_pixeles(app, &derecha, img.ancho, img.alto)
        && let Some(mut m2) = pixpin_ia::tabla_aspecto::medir(&derecha, img.ancho, img.alto, &ps2)
    {
        m2.inclinacion = m.inclinacion;
        return Some(m2);
    }
    Some(m)
}

/// Las celdas que se usan: las de la rejilla medida en una foto o, si no
/// hay rejilla o es un PDF que ya cuadra con ella, las leidas (Windows).
pub fn filas_de(leida: &TablaLeida, m: &pixpin_ia::tabla_aspecto::Medida, ruta: &Path) -> Vec<Vec<String>> {
    let rejilla = !m.celdas.is_empty() && m.celdas.iter().flatten().any(|c| !c.is_empty());
    let pdf_que_cuadra = !es_imagen(ruta) && m.cuadra(leida.filas.len(), leida.columnas());
    if rejilla && !pdf_que_cuadra { m.celdas.clone() } else { leida.filas.clone() }
}

/// La tabla con otras celdas: si cambian, sus combinadas ya no valen.
fn con_filas(leida: TablaLeida, filas: Vec<Vec<String>>) -> TablaLeida {
    if filas == leida.filas { leida } else { TablaLeida { filas, combinadas: Vec::new() } }
}

fn palabras_de_ocr(leidas: &[PalabraLeida]) -> Vec<Palabra> {
    leidas
        .iter()
        .map(|p| Palabra::nueva(&p.texto, p.x as f64, p.y as f64, p.ancho as f64, p.alto as f64))
        .collect()
}

/// Lo que salio de una tabla, para el modelo.
#[derive(Debug, Clone)]
pub struct TablaHecha {
    pub leida: TablaLeida,
    pub nombre: String,
    pub cabecera: bool,
    pub excel: Option<PathBuf>,
}

/// **Lee la tabla** del archivo (sin mandar nada).
pub fn leer_tabla(app: Option<&dyn AppIa>, ruta: &Path, pagina: Option<usize>) -> Result<TablaLeida, String> {
    let (ps, p) = palabras_de(app, ruta, pagina)?;
    let t = tabla_escaneada::reconstruir(&ps);
    if t.filas.len() < 2 || t.columnas() < 2 {
        return Err(format!(
            "en la pagina {} no se ve una tabla (salieron {} filas y {} columnas)",
            p + 1,
            t.filas.len(),
            t.columnas()
        ));
    }
    Ok(t)
}

fn tabla(args: &Value, cx: &Contexto) -> Result<String, String> {
    let c = elegir(cx, texto_de(args, "archivo"), Sirve::Escaneado)?;
    let pagina = args.get("pagina").and_then(Value::as_u64).filter(|p| *p >= 1).map(|p| p as usize - 1);
    let leida = leer_tabla(cx.app, &c.ruta, pagina)?;
    // El aspecto de la tabla en la foto (Windows, 16/18-oct): la rejilla,
    // torcida o no, sus fondos y sus anchos.
    let aspecto = cx.app.and_then(|app| aspecto_de_tabla(app, &c.ruta, pagina));
    let leida = match &aspecto {
        Some(m) => {
            let filas = filas_de(&leida, m, &c.ruta);
            con_filas(leida, filas)
        }
        None => leida,
    };
    let cabecera = args.get("cabecera").and_then(Value::as_bool).unwrap_or_else(|| leida.parece_cabecera());
    let nombre = texto_de(args, "nombre").map(str::to_string).unwrap_or_else(|| pixpin_docs::sin_extension(&c.nombre));
    let excel = texto_de(args, "destino") == Some("excel");
    let hecha = if excel {
        let ruta = escribir_excel_con(&nombre, &leida.filas, cabecera, aspecto.as_ref())?;
        mandar(
            cx,
            json!({"pixpin": 1, "accion": "chat", "proyecto": proyecto_del_pedido(cx),
                   "texto": format!("[archivo 01] {nombre}.xlsx"), "archivos": [ruta]}),
        )?;
        TablaHecha { leida, nombre, cabecera, excel: Some(ruta) }
    } else {
        mandar(
            cx,
            json!({"pixpin": 1, "accion": "tabla_nueva", "proyecto": proyecto_del_pedido(cx), "nombre": nombre,
                   "filas": leida.filas, "cabecera": cabecera, "abrir": true}),
        )?;
        TablaHecha { leida, nombre, cabecera, excel: None }
    };
    Ok(resumen_de_tabla(&hecha, &c.nombre))
}

/// Lo que se le cuenta al modelo: el tamano y las primeras filas.
pub fn resumen_de_tabla(h: &TablaHecha, de: &str) -> String {
    let filas: Vec<String> = h.leida.filas.iter().take(4).map(|f| f.join(" | ")).collect();
    format!(
        "hecho: {} «{}» de «{}», {} filas x {} columnas{}{}. Primeras filas: {}",
        if h.excel.is_some() { "Excel" } else { "tabla del chat" },
        h.nombre,
        de,
        h.leida.filas.len(),
        h.leida.columnas(),
        if h.cabecera { ", con cabecera" } else { "" },
        if h.leida.combinadas.is_empty() {
            String::new()
        } else {
            format!(", {} celdas combinadas (aproximadas)", h.leida.combinadas.len())
        },
        filas.join(" / ")
    )
}

/// **La tabla de unas filas de texto** (`pedidos::tabla_de_filas` de
/// Windows): `filas[f][c]` va a la celda de la columna `c` y la fila `f`.
/// Con `cabecera`, la primera fila en negrita; las celdas que son numeros
/// escritos a la espanola o con miles se guardan como numero.
pub fn tabla_de_filas(nombre: &str, filas: &[Vec<String>], cabecera: bool, ahora: i64) -> pixpin_proyecto::tabla::Tabla {
    use pixpin_proyecto::tabla::{EstiloDeCelda, Ref, Tabla, ref_a};
    let nombre = match nombre.trim() {
        "" => "Tabla",
        n => n,
    };
    let mut t = Tabla {
        nombre: nombre.to_string(),
        tocado: ahora,
        ..Default::default()
    };
    for (f, fila) in filas.iter().enumerate().take(5000) {
        for (c, texto) in fila.iter().enumerate().take(200) {
            let r = Ref {
                columna: c as u32,
                fila: f as u32,
            };
            let texto = texto.trim();
            let valor = if f == 0 && cabecera {
                texto.to_string()
            } else {
                tabla_escaneada::como_numero(texto).unwrap_or_else(|| texto.to_string())
            };
            t.poner(r, &valor);
            if f == 0 && cabecera && !texto.is_empty() {
                t.estilos.insert(
                    ref_a(r),
                    EstiloDeCelda {
                        n: true,
                        ..Default::default()
                    },
                );
            }
        }
    }
    // Cada columna, del ancho de su texto mas largo (como al pegar una
    // tabla): sin esto un nombre de calle se corta en la rejilla.
    let columnas = filas.iter().map(Vec::len).max().unwrap_or(0).min(200);
    for c in 0..columnas {
        let letras = filas.iter().filter_map(|f| f.get(c)).map(|s| s.trim().chars().count()).max().unwrap_or(0);
        let ancho = (letras as u32 * 8 + 16).clamp(64, 320);
        if ancho > 88 {
            t.anchos.insert(
                ref_a(Ref { columna: c as u32, fila: 0 }).trim_end_matches(|c: char| c.is_ascii_digit()).to_string(),
                ancho,
            );
        }
    }
    t
}

/// El `.xlsx` de la tabla, en una carpeta temporal (el pedido `chat` lo
/// copia al chat).
pub fn escribir_excel(nombre: &str, filas: &[Vec<String>], cabecera: bool) -> Result<PathBuf, String> {
    escribir_excel_con(nombre, filas, cabecera, None)
}

/// Lo mismo con el aspecto medido en la foto: el fondo de cada celda y el
/// ancho de cada columna, si la rejilla cuadra con la tabla (Windows).
pub fn escribir_excel_con(
    nombre: &str,
    filas: &[Vec<String>],
    cabecera: bool,
    aspecto: Option<&pixpin_ia::tabla_aspecto::Medida>,
) -> Result<PathBuf, String> {
    let ahora = crate::ahora_ms();
    let mut t = tabla_de_filas(nombre, filas, cabecera, ahora);
    let columnas = filas.first().map_or(0, Vec::len);
    if let Some(m) = aspecto.filter(|m| m.cuadra(filas.len(), columnas)) {
        use pixpin_proyecto::tabla::{Ref, ref_a};
        for (f, fila) in m.fondos.iter().enumerate() {
            for (c, fondo) in fila.iter().enumerate() {
                if let Some(color) = fondo {
                    let r = ref_a(Ref { columna: c as u32, fila: f as u32 });
                    t.estilos.entry(r).or_default().f = Some(pixpin_ia::tabla_aspecto::hex(*color));
                }
            }
        }
        let k = 15.0 / m.tam_letra.max(1.0);
        for (c, a) in m.anchos().iter().enumerate() {
            let letra = ref_a(Ref { columna: c as u32, fila: 0 }).trim_end_matches('1').to_string();
            t.anchos.insert(letra, ((a * k).round() as u32).clamp(30, 600));
        }
    }
    let bytes = pixpin_proyecto::tabla_xlsx::a_xlsx(&[pixpin_proyecto::tabla_xlsx::HojaParaVer { nombre, tabla: &t, valores: None }], 0)
        .map_err(|e| e.to_string())?;
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let carpeta = carpeta_temporal().join(format!("{ahora}-{n}"));
    std::fs::create_dir_all(&carpeta).map_err(|e| e.to_string())?;
    let ruta = carpeta.join(format!("{}.xlsx", crate::compartir::nombre_de_fichero_compartido(nombre.to_string())));
    std::fs::write(&ruta, bytes).map_err(|e| e.to_string())?;
    Ok(ruta)
}

// ---------------------------------------------------------------------------
// 2. Anotar

/// Lo que quedo anotado: donde y que.
#[derive(Debug, Clone, Default)]
pub struct Anotado {
    /// Hoja (desde 0) de un PDF/Word; `None` en un plano.
    pub pagina: Option<usize>,
    pub figuras: usize,
    /// Donde se marco y cuantas veces aparecia el texto en total.
    pub sitios: usize,
    pub hallados: usize,
    /// La vista del plano donde quedo (para abrirlo ahi): centro y ancho.
    pub vista: Option<[f64; 3]>,
}

/// **Lo ultimo anotado en este hilo** (el del turno), para que la revision
/// de los ojos mire la hoja con lo anotado encima (`Zona::Documento`).
#[derive(Debug, Clone, PartialEq)]
pub struct UltimoAnotado {
    pub ruta: PathBuf,
    /// La hoja (desde 0) de un PDF o Word; `None` en un plano.
    pub hoja: Option<usize>,
    /// Lo que mide la hoja en unidades de la capa (ancho [`ANCHO_HOJA`]).
    pub alto: f32,
    /// Las cajas de lo que se acaba de anotar, en esas unidades.
    pub cajas: Vec<(f32, f32, f32, f32)>,
}

thread_local! {
    static ULTIMO_ANOTADO: RefCell<Option<UltimoAnotado>> = const { RefCell::new(None) };
}

pub(crate) fn apuntar_anotado(u: Option<UltimoAnotado>) {
    ULTIMO_ANOTADO.with_borrow_mut(|x| *x = u);
}

/// Lo ultimo anotado en este hilo, si hay.
pub fn ultimo_anotado() -> Option<UltimoAnotado> {
    ULTIMO_ANOTADO.with_borrow(Clone::clone)
}

/// **Anota sin mandar nada**: busca donde y escribe en la capa.
pub fn anotar_en(cx: &Contexto, ruta: &Path, args: &Value) -> Result<Anotado, String> {
    let como = texto_de(args, "tipo").and_then(ComoAnotar::de).ok_or("falta «tipo»")?;
    let texto = texto_de(args, "texto");
    let zona = numeros_de(args, "zona");
    let puntos = numeros_de(args, "puntos");
    let color = rgb(texto_de(args, "color"), como);
    let nota = texto_de(args, "nota").or(if como == ComoAnotar::Nota { texto } else { None });
    let pagina = args.get("pagina").and_then(Value::as_u64).filter(|p| *p >= 1).map(|p| p as usize - 1);
    if texto.is_none() && zona.len() < 4 && !(como == ComoAnotar::Dibujar && puntos.len() >= 4) && como != ComoAnotar::Nota {
        return Err("di sobre que texto del documento va («texto») o una «zona»".into());
    }
    if es_plano(&pixpin_docs::nombre(ruta)) {
        let r = anotar_plano(cx, ruta, como, texto, &zona, &puntos, color, nota);
        if r.is_ok() {
            apuntar_anotado(Some(UltimoAnotado { ruta: ruta.to_path_buf(), hoja: None, alto: 0.0, cajas: Vec::new() }));
        }
        return r;
    }
    let p = paginas(cx.app, ruta, cx.parar)?;
    // Donde: el texto (todas sus apariciones en la hoja elegida o en la
    // primera que lo tenga), la zona, o arriba a la derecha para una nota.
    let (hoja, cajas, sitios, hallados) = match texto {
        Some(t) if como != ComoAnotar::Nota || zona.len() < 4 => {
            let s = buscar_en_paginas(&p, t, pagina, 40);
            let Some(primera) = s.first() else {
                return Err(format!("«{t}» no aparece en «{}»", pixpin_docs::nombre(ruta)));
            };
            let hoja = primera.pagina;
            let de_la_hoja: Vec<&Sitio> = s.iter().filter(|x| x.pagina == hoja).take(20).collect();
            // Encerrar o una nota van a lo primero; marcar el texto, a todo.
            let marca = matches!(como, ComoAnotar::Resaltar | ComoAnotar::Subrayar | ComoAnotar::Tachar);
            let cajas: Vec<[f32; 4]> = if marca { de_la_hoja.iter().flat_map(|x| x.cajas.clone()).collect() } else { primera.cajas.clone() };
            let marcados = if marca { de_la_hoja.len() } else { 1 };
            (hoja, cajas, marcados, s.len())
        }
        _ => {
            let hoja = pagina.unwrap_or(0).min(p.altos.len().saturating_sub(1));
            let marco = [0.0, 0.0, ANCHO_HOJA, p.altos.get(hoja).copied().unwrap_or(ANCHO_HOJA * 1.414)];
            let caja = zona_en(&zona, marco).unwrap_or([ANCHO_HOJA * 0.6, 60.0, ANCHO_HOJA * 0.62, 90.0]);
            (hoja, vec![caja], 1, 0)
        }
    };
    let alto = p.altos.get(hoja).copied().unwrap_or(ANCHO_HOJA * 1.414);
    let letra = cajas.first().map_or(18.0, |c| (c[3] - c[1]).clamp(10.0, 40.0));
    let pts = puntos_en(&puntos, [0.0, 0.0, ANCHO_HOJA, alto]);
    let nuevas = figuras(como, &cajas, color, nota, &pts, letra);
    if nuevas.is_empty() {
        return Err("no salio ninguna figura (¿una nota sin texto?)".into());
    }
    let n = nuevas.len();
    let cajas_nuevas: Vec<(f32, f32, f32, f32)> = nuevas.iter().map(Elemento::caja).collect();
    escribir_en_la_hoja(cx.nucleo, ruta, hoja, alto, nuevas)?;
    apuntar_anotado(Some(UltimoAnotado { ruta: ruta.to_path_buf(), hoja: Some(hoja), alto, cajas: cajas_nuevas }));
    Ok(Anotado { pagina: Some(hoja), figuras: n, sitios, hallados, vista: None })
}

/// Escribe en la capa de la hoja `i` de un PDF o un Word: la MISMA que lee
/// y guarda el lector ([`Nucleo::anotar_hoja`]: la hoja de un PDF de
/// proyecto, la `anot-<uid>-p<i>` con su `.hoja` de un adjunto, o la de al
/// lado), en un paso de deshacer.
pub fn escribir_en_la_hoja(nucleo: &Nucleo, ruta: &Path, i: usize, alto: f32, nuevas: Vec<Elemento>) -> Result<(), String> {
    let ed = nucleo
        .anotar_hoja(ruta.display().to_string(), i as u32, alto)
        .map_err(|e| format!("no se pudo abrir lo anotado: {e}"))?;
    meter_y_guardar(&ed, nuevas)
}

/// Mete `nuevas` en la escena de `ed` en un paso de deshacer y la guarda.
pub(crate) fn meter_y_guardar(ed: &crate::editor::EditorLienzo, nuevas: Vec<Elemento>) -> Result<(), String> {
    ed.con_la_escena(|escena, _| {
        escena.abrir_paso();
        for e in nuevas {
            escena.anadir(e);
        }
        escena.cerrar_paso();
    });
    ed.guardar().map(|_| ()).map_err(|e| format!("no se pudo guardar lo anotado: {e}"))
}

#[allow(clippy::too_many_arguments)]
fn anotar_plano(
    cx: &Contexto,
    ruta: &Path,
    como: ComoAnotar,
    texto: Option<&str>,
    zona: &[f64],
    puntos: &[f64],
    color: (u8, u8, u8),
    nota: Option<&str>,
) -> Result<Anotado, String> {
    let plano = cx.nucleo.abrir_plano(ruta.display().to_string()).map_err(|e| format!("no se pudo abrir el plano: {e}"))?;
    let (origen, caja_m, encaje) = plano.para_el_asistente();
    // Las cajas en coordenadas del plano (relativas a su origen, y arriba).
    let (cajas_plano, hallados, letra_plano): (Vec<[f64; 4]>, usize, f64) = match texto {
        Some(t) if como != ComoAnotar::Nota || zona.len() < 4 => {
            let ts = crate::planos::textos::del_plano(cx.raiz(), ruta)?;
            let h = crate::planos::textos::buscar(&ts, t, 10);
            if h.is_empty() {
                return Err(format!("«{t}» no aparece en el plano «{}»", pixpin_docs::nombre(ruta)));
            }
            let uno = matches!(como, ComoAnotar::Encerrar | ComoAnotar::Marco | ComoAnotar::Nota);
            let cajas = h
                .iter()
                .take(if uno { 1 } else { 10 })
                .map(|h| {
                    let largo = h.texto.chars().count().max(1) as f64 * h.alto * 0.75;
                    let (x, y) = (h.x - origen[0], h.y - origen[1]);
                    [x, y, x + largo, y + h.alto]
                })
                .collect();
            (cajas, h.len(), h[0].alto)
        }
        _ => {
            let c = caja_m;
            let (x0, y0, x1, y1) = (c[0] as f64, c[1] as f64, c[2] as f64, c[3] as f64);
            let caja = match zona.get(..4) {
                Some(&[x, y, w, h]) if w > 0.0 && h > 0.0 => {
                    let (ax, ay) = (x1 - x0, y1 - y0);
                    [x0 + x * ax, y1 - (y + h) * ay, x0 + (x + w) * ax, y1 - y * ay]
                }
                _ => [x1 - (x1 - x0) * 0.2, y1 - (y1 - y0) * 0.1, x1 - (x1 - x0) * 0.05, y1 - (y1 - y0) * 0.05],
            };
            let letra = ((caja[3] - caja[1]) * 0.2).max((y1 - y0) * 0.01);
            (vec![caja], 0, letra)
        }
    };
    let n = if como == ComoAnotar::Marco {
        // El marco para imprimir de verdad, el del visor.
        let u = cajas_plano.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |u, c| [u[0].min(c[0]), u[1].min(c[1]), u[2].max(c[2]), u[3].max(c[3])]);
        let aire = letra_plano * 2.0;
        let mut a = plano.anotado();
        a.marcos.push(crate::planos::CajaPlano {
            x0: u[0] - aire + origen[0],
            y0: u[1] - aire + origen[1],
            x1: u[2] + aire + origen[0],
            y1: u[3] + aire + origen[1],
        });
        plano.guardar_anotado(a).map_err(|e| format!("no se pudo guardar lo anotado en el plano: {e}"))?;
        1
    } else {
        // Del plano (y arriba) a la capa (y abajo): la esquina de arriba.
        let a_capa = |c: &[f64; 4]| {
            let a = encaje.a_capa([c[0], c[3]]);
            let b = encaje.a_capa([c[2], c[1]]);
            [a[0] as f32, a[1] as f32, b[0] as f32, b[1] as f32]
        };
        let cajas: Vec<[f32; 4]> = cajas_plano.iter().map(a_capa).collect();
        let letra = (letra_plano / encaje.u) as f32;
        let todo = a_capa(&[caja_m[0] as f64, caja_m[1] as f64, caja_m[2] as f64, caja_m[3] as f64]);
        let pts = puntos_en(puntos, todo);
        let nuevas = figuras(como, &cajas, color, nota, &pts, letra.max(1.0));
        let n = nuevas.len();
        if n > 0 {
            let ed = plano.anotar().map_err(|e| format!("no se pudo abrir la capa del plano: {e}"))?;
            meter_y_guardar(&ed, nuevas)?;
            plano.poner_hoja(plano.encaje()).map_err(|e| format!("no se pudo guardar lo anotado en el plano: {e}"))?;
        }
        n
    };
    if n == 0 {
        return Err("no salio ninguna figura (¿una nota sin texto?)".into());
    }
    let c = cajas_plano[0];
    let ancho = ((c[2] - c[0]) * 6.0).max(letra_plano * 40.0);
    let vista = [(c[0] + c[2]) / 2.0 + origen[0], (c[1] + c[3]) / 2.0 + origen[1], ancho];
    Ok(Anotado { pagina: None, figuras: n, sitios: cajas_plano.len(), hallados, vista: Some(vista) })
}

fn anotar(args: &Value, cx: &Contexto) -> Result<String, String> {
    let c = elegir(cx, texto_de(args, "archivo"), Sirve::Documento)?;
    let hecho = anotar_en(cx, &c.ruta, args)?;
    // Se abre donde quedo, para que el usuario lo vea.
    let mut que = json!({"tipo": "fichero", "ruta": c.ruta});
    if let Some(p) = hecho.pagina {
        que["pagina"] = json!(p + 1);
    }
    if let Some(v) = hecho.vista {
        que["zona"] = json!(v);
    }
    mandar(cx, json!({"pixpin": 1, "accion": "abrir", "que": que}))?;
    Ok(format!(
        "hecho: {} en «{}»{}: marcado en {} sitio(s){} ({} figuras); ya esta abierto ahi y viaja al movil al sincronizar",
        texto_de(args, "tipo").unwrap_or("anotado"),
        c.nombre,
        hecho.pagina.map(|p| format!(", pagina {}", p + 1)).unwrap_or_default(),
        hecho.sitios,
        if hecho.hallados > hecho.sitios {
            format!(" (aparece {} veces en todo el documento; se marco lo de esta pagina o, al encerrar, lo primero)", hecho.hallados)
        } else {
            String::new()
        },
        hecho.figuras
    ))
}

// ---------------------------------------------------------------------------
// 3. Buscar dentro y llevar ahi

/// Lo encontrado, ya contado y con el pedido para abrir ahi.
#[derive(Debug, Clone)]
pub struct Encontrado {
    pub texto: String,
    pub pedido: Value,
    pub cuantos: usize,
}

/// **Busca sin mandar nada**: lo que se diria y el pedido `abrir`.
pub fn buscar_dentro(cx: &Contexto, ruta: &Path, consulta: &str) -> Result<Encontrado, String> {
    let nombre = pixpin_docs::nombre(ruta);
    if es_plano(&nombre) {
        let ts = crate::planos::textos::del_plano(cx.raiz(), ruta)?;
        let h = crate::planos::textos::buscar(&ts, consulta, 5);
        let Some(primero) = h.first() else {
            return Err(format!("«{consulta}» no aparece en el plano «{nombre}» ({} textos mirados)", ts.len()));
        };
        let vista = [primero.centro_x, primero.centro_y, primero.ancho];
        let lista: Vec<String> = h.iter().map(|h| format!("«{}» en ({:.1}, {:.1})", h.texto, h.x, h.y)).collect();
        return Ok(Encontrado {
            texto: format!("en «{nombre}»: {}", lista.join("; ")),
            pedido: json!({"pixpin": 1, "accion": "abrir", "que": {"tipo": "fichero", "ruta": ruta, "zona": vista}}),
            cuantos: h.len(),
        });
    }
    let p = paginas(cx.app, ruta, cx.parar)?;
    let s = buscar_en_paginas(&p, consulta, None, 8);
    let Some(primero) = s.first() else {
        return Err(format!("«{consulta}» no aparece en «{nombre}»"));
    };
    let lista: Vec<String> = s.iter().map(|x| format!("pagina {}: «…{}…»", x.pagina + 1, x.contexto)).collect();
    Ok(Encontrado {
        texto: format!("en «{nombre}»: {}", lista.join("; ")),
        pedido: json!({"pixpin": 1, "accion": "abrir", "que": {"tipo": "fichero", "ruta": ruta, "pagina": primero.pagina + 1, "buscar": consulta}}),
        cuantos: s.len(),
    })
}

fn buscar_en(args: &Value, cx: &Contexto) -> Result<String, String> {
    let consulta = texto_de(args, "texto").ok_or("falta «texto»")?;
    let c = elegir(cx, texto_de(args, "archivo"), Sirve::Documento)?;
    let e = buscar_dentro(cx, &c.ruta, consulta)?;
    let abrir = args.get("abrir").and_then(Value::as_bool).unwrap_or(true);
    if abrir {
        mandar(cx, e.pedido.clone())?;
    }
    Ok(format!("{} sitio(s) {}{}", e.cuantos, e.texto, if abrir { "; abierto en el primero" } else { "" }))
}

// ---------------------------------------------------------------------------
// 4. Abrir por su nombre

fn abrir(args: &Value, cx: &Contexto) -> Result<String, String> {
    let d = texto_de(args, "descripcion").ok_or("falta «descripcion»")?;
    let c = elegir(cx, Some(d), Sirve::Cualquiera)?;
    mandar(cx, json!({"pixpin": 1, "accion": "abrir", "que": {"tipo": "fichero", "ruta": c.ruta}}))?;
    Ok(format!("abierto «{}» (del chat {})", c.nombre, c.proyecto_nombre))
}

// ---------------------------------------------------------------------------
// 5. Exportar

fn exportar(args: &Value, cx: &Contexto) -> Result<String, String> {
    let formato = texto_de(args, "formato").ok_or("falta «formato»")?;
    let c = elegir(cx, texto_de(args, "archivo"), Sirve::Cualquiera)?;
    let carpeta = carpeta_temporal().join(format!("exportar-{}", crate::ahora_ms()));
    std::fs::create_dir_all(&carpeta).map_err(|e| format!("sin sitio para exportar: {e}"))?;
    let hechos = cx
        .app()?
        .exportar(c.ruta.display().to_string(), formato.to_string(), carpeta.display().to_string())
        .map_err(|e| format!("no se pudo exportar a {formato}: {e}"))?;
    if hechos.is_empty() {
        return Err(format!("exportar a {formato} no dejo ningun fichero"));
    }
    let fichas: String = (1..=hechos.len()).map(|i| format!("[archivo {i:02}]")).collect::<Vec<_>>().join(" ");
    mandar(
        cx,
        json!({"pixpin": 1, "accion": "chat", "proyecto": proyecto_del_pedido(cx),
               "texto": format!("{fichas} «{}» en {formato}", c.nombre), "archivos": hechos}),
    )?;
    let nombres: Vec<String> = hechos.iter().map(|r| pixpin_docs::nombre(Path::new(r))).collect();
    Ok(format!("hecho: «{}» exportado a {formato} ({}), ya esta en el chat", c.nombre, nombres.join(", ")))
}

#[cfg(test)]
#[path = "documentos_acciones_pruebas.rs"]
mod pruebas;
