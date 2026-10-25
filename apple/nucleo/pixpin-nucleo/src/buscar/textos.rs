//! **El texto de dentro de los documentos de los chats** (PDF, Word,
//! PowerPoint, Excel, OpenDocument, EPUB, HTML, Markdown y texto), para
//! que «Buscar en todo» encuentre lo que pone dentro y no solo su nombre.
//!
//! Se lee una vez y se guarda en `<datos>/cache/textos/`, con un nombre que
//! sale de la ruta, la fecha y el tamano del fichero: si el documento
//! cambia, cambia el nombre y se vuelve a leer (como el texto de las
//! capturas en `cache/galeria/`). Es solo de este aparato: no se sincroniza.
//!
//! Rust lee lo que sabe leer (los ZIP de Office y OpenDocument, EPUB, HTML,
//! Markdown y texto). Los PDF los lee Swift con PDFKit y los apunta con
//! [`Nucleo::poner_texto_de_documento`], igual que el OCR de las capturas.
//!
//! Buscar no lee nunca un documento: solo mira el texto ya guardado (y lo
//! tiene en memoria, ya plegado, mientras no cambie).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use sha2::{Digest, Sha256};

use super::puntos::normalizar;

/// La carpeta de los textos, dentro de los datos.
const CARPETA: &str = "cache/textos";

/// Lo mas que se guarda de un documento, en letras: un libro entero cabe,
/// una base de datos exportada a CSV no.
pub const TOPE_DE_LETRAS: usize = 2_000_000;

/// Lo mas que se descomprime de una parte de un ZIP.
const TOPE_DE_PARTE: u64 = 32 * 1024 * 1024;

/// Cuantos textos se tienen en memoria a la vez.
const TOPE_EN_MEMORIA: usize = 400;

/// Quien lee un documento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lector {
    /// Swift, con PDFKit.
    Pdf,
    /// `pixpin_docs::abrir` (Word, EPUB, HTML, Markdown, texto).
    Docs,
    /// Las partes de texto de un ZIP de Office u OpenDocument.
    Zip,
}

/// Si el fichero es un documento cuyo texto se busca, y quien lo lee.
pub fn lector_de(nombre: &str) -> Option<Lector> {
    let ext = pixpin_docs::extension(nombre);
    match ext.as_str() {
        "pdf" => Some(Lector::Pdf),
        "docx" | "docm" | "dotx" | "epub" | "html" | "htm" | "xhtml" | "md" | "markdown"
        | "txt" | "log" | "csv" => Some(Lector::Docs),
        "pptx" | "ppsx" | "potx" | "xlsx" | "xlsm" | "odt" | "ods" | "odp" => Some(Lector::Zip),
        _ => None,
    }
}

/// Lo que se sabe de un texto guardado: el de verdad (para ensenar el
/// trozo) y el plegado (para buscar).
#[derive(Debug)]
pub struct Texto {
    pub original: String,
    pub plegado: String,
}

fn memoria() -> &'static Mutex<HashMap<PathBuf, Arc<Texto>>> {
    static M: OnceLock<Mutex<HashMap<PathBuf, Arc<Texto>>>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Donde se guarda el texto de `fichero` en su version de ahora. `None` si
/// el fichero no esta.
pub fn ruta_del_texto(raiz: &Path, fichero: &Path) -> Option<PathBuf> {
    let meta = std::fs::metadata(fichero).ok()?;
    if !meta.is_file() {
        return None;
    }
    let segundos = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    let huella = Sha256::digest(fichero.to_string_lossy().as_bytes());
    let corta: String = huella.iter().take(8).map(|b| format!("{b:02x}")).collect();
    Some(
        raiz.join(CARPETA)
            .join(format!("{corta}-{segundos}-{}.txt", meta.len())),
    )
}

/// El texto ya leido de `fichero`, si lo hay (de memoria o del disco).
pub fn guardado(raiz: &Path, fichero: &Path) -> Option<Arc<Texto>> {
    let ruta = ruta_del_texto(raiz, fichero)?;
    if let Some(t) = memoria().lock().ok()?.get(&ruta) {
        return Some(Arc::clone(t));
    }
    let original = std::fs::read_to_string(&ruta).ok()?;
    let t = Arc::new(Texto {
        plegado: normalizar(&original),
        original,
    });
    let mut m = memoria().lock().ok()?;
    if m.len() >= TOPE_EN_MEMORIA {
        m.clear();
    }
    m.insert(ruta, Arc::clone(&t));
    Some(t)
}

/// Si ya se leyo (aunque saliera vacio).
pub fn esta_leido(raiz: &Path, fichero: &Path) -> bool {
    ruta_del_texto(raiz, fichero).is_some_and(|r| r.exists())
}

/// Apunta el texto de `fichero` (vacio tambien: «no tiene texto», y no se
/// vuelve a intentar). Recortado a [`TOPE_DE_LETRAS`].
pub fn guardar(raiz: &Path, fichero: &Path, texto: &str) -> std::io::Result<()> {
    let Some(ruta) = ruta_del_texto(raiz, fichero) else {
        return Ok(());
    };
    if let Some(p) = ruta.parent() {
        std::fs::create_dir_all(p)?;
    }
    let recortado: String = texto.chars().take(TOPE_DE_LETRAS).collect();
    let tmp = ruta.with_extension("txt.tmp");
    std::fs::write(&tmp, recortado)?;
    std::fs::rename(&tmp, &ruta)?;
    if let Ok(mut m) = memoria().lock() {
        m.remove(&ruta);
    }
    Ok(())
}

/// **Lee el texto** de un documento que Rust sabe leer. `None` si no es de
/// esos o no se pudo abrir (se apunta vacio igual: no se reintenta).
pub fn extraer(fichero: &Path) -> Option<String> {
    let nombre = pixpin_docs::nombre(fichero);
    match lector_de(&nombre)? {
        Lector::Pdf => None,
        Lector::Docs => {
            let d = pixpin_docs::abrir(fichero).ok()?;
            let mut s = String::new();
            for b in &d.bloques {
                let t = b.texto();
                if t.trim().is_empty() || t == pixpin_docs::documento::MARCA_IMAGEN {
                    continue;
                }
                s.push_str(t.trim());
                s.push('\n');
                if s.len() > TOPE_DE_LETRAS * 4 {
                    break;
                }
            }
            Some(s)
        }
        Lector::Zip => texto_de_zip(fichero),
    }
}

/// Las partes con texto de un PowerPoint, un Excel o un OpenDocument, sin
/// las etiquetas.
fn texto_de_zip(fichero: &Path) -> Option<String> {
    let mut p = pixpin_docs::Paquete::abrir(fichero).ok()?;
    let mut partes: Vec<String> = p
        .nombres()
        .into_iter()
        .filter(|n| {
            (n.starts_with("ppt/slides/slide") || n.starts_with("ppt/notesSlides/notesSlide"))
                && n.ends_with(".xml")
                || n == "xl/sharedStrings.xml"
                || n == "content.xml"
        })
        .collect();
    // Las diapositivas en su orden (slide2 antes que slide10).
    partes.sort_by_key(|n| {
        let cifras: String = n.chars().filter(char::is_ascii_digit).collect();
        (cifras.parse::<u64>().unwrap_or(0), n.clone())
    });
    let mut s = String::new();
    for parte in partes {
        if let Some(xml) = p.texto(&parte, TOPE_DE_PARTE) {
            s.push_str(&sin_etiquetas(&xml));
            if s.len() > TOPE_DE_LETRAS * 4 {
                break;
            }
        }
    }
    Some(s)
}

/// El texto de un XML de Office: lo de fuera de las etiquetas, con un
/// salto al cerrar cada parrafo y las entidades basicas traducidas.
pub fn sin_etiquetas(xml: &str) -> String {
    let mut s = String::with_capacity(xml.len() / 4);
    let mut resto = xml;
    while let Some(i) = resto.find('<') {
        s.push_str(&entidades(&resto[..i]));
        let Some(f) = resto[i..].find('>') else {
            resto = "";
            break;
        };
        let etiqueta = &resto[i + 1..i + f];
        // Fin de parrafo, de celda o de diapositiva: un salto.
        if matches!(
            etiqueta,
            "/a:p" | "/w:p" | "/si" | "/text:p" | "/text:h" | "/p:sld" | "/table:table-cell"
        ) {
            s.push('\n');
        } else if etiqueta.starts_with("a:br") || etiqueta.starts_with("text:tab") {
            s.push(' ');
        }
        resto = &resto[i + f + 1..];
    }
    s.push_str(&entidades(resto));
    s
}

fn entidades(t: &str) -> String {
    if !t.contains('&') {
        return t.to_string();
    }
    t.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn temporal(nombre: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "pixpin-buscar-textos-{nombre}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn cada_documento_tiene_su_lector() {
        assert_eq!(lector_de("Informe.PDF"), Some(Lector::Pdf));
        assert_eq!(lector_de("acta.docx"), Some(Lector::Docs));
        assert_eq!(lector_de("notas.md"), Some(Lector::Docs));
        assert_eq!(lector_de("obra.pptx"), Some(Lector::Zip));
        assert_eq!(lector_de("cuentas.xlsx"), Some(Lector::Zip));
        // Caso negativo: una foto, un audio o algo sin extension no.
        assert_eq!(lector_de("foto.png"), None);
        assert_eq!(lector_de("voz.m4a"), None);
        assert_eq!(lector_de("LEEME"), None);
    }

    #[test]
    fn el_xml_de_office_queda_en_texto_con_sus_saltos() {
        let xml = r#"<p:sld><a:p><a:r><a:t>Grieta &amp; fisura</a:t></a:r></a:p><a:p><a:t>muro norte</a:t></a:p></p:sld>"#;
        assert_eq!(sin_etiquetas(xml), "Grieta & fisura\nmuro norte\n\n");
        // Caso negativo: sin etiquetas, el texto tal cual.
        assert_eq!(sin_etiquetas("solo texto"), "solo texto");
    }

    #[test]
    fn se_guarda_se_lee_y_si_el_fichero_cambia_se_vuelve_a_leer() {
        let raiz = temporal("guardar");
        let doc = raiz.join("acta.txt");
        std::fs::write(&doc, "la reunión del lunes").unwrap();
        assert!(!esta_leido(&raiz, &doc));
        let leido = extraer(&doc).unwrap();
        assert!(leido.contains("reunión"), "{leido}");
        guardar(&raiz, &doc, &leido).unwrap();
        assert!(esta_leido(&raiz, &doc));
        let t = guardado(&raiz, &doc).unwrap();
        assert!(t.plegado.contains("reunion del lunes"));
        assert!(t.original.contains("reunión"));
        // Otro tamano: otra version, sin leer.
        std::fs::write(&doc, "la reunión del lunes y del martes").unwrap();
        assert!(!esta_leido(&raiz, &doc));
        // Caso negativo: un fichero que no esta no tiene texto ni lo apunta.
        let no = raiz.join("no.txt");
        assert!(guardado(&raiz, &no).is_none());
        guardar(&raiz, &no, "x").unwrap();
        assert!(!esta_leido(&raiz, &no));
        let _ = std::fs::remove_dir_all(&raiz);
    }

    #[test]
    fn un_pptx_se_lee_por_sus_diapositivas_en_orden() {
        use std::io::Write;
        let raiz = temporal("pptx");
        let ruta = raiz.join("obra.pptx");
        let f = std::fs::File::create(&ruta).unwrap();
        let mut z = zip::ZipWriter::new(f);
        let o = zip::write::SimpleFileOptions::default();
        for (n, t) in [
            ("ppt/slides/slide10.xml", "diez"),
            ("ppt/slides/slide2.xml", "dos"),
        ] {
            z.start_file(n, o).unwrap();
            z.write_all(format!("<p:sld><a:p><a:t>{t}</a:t></a:p></p:sld>").as_bytes())
                .unwrap();
        }
        z.finish().unwrap();
        let t = extraer(&ruta).unwrap();
        assert!(t.find("dos").unwrap() < t.find("diez").unwrap(), "{t}");
        // Caso negativo: un PDF no lo lee Rust (lo lee Swift).
        let pdf = raiz.join("a.pdf");
        std::fs::write(&pdf, b"%PDF-1.4").unwrap();
        assert!(extraer(&pdf).is_none());
        let _ = std::fs::remove_dir_all(&raiz);
    }
}
