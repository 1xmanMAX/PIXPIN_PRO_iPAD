//! **Leer y anotar un documento como un pin** (PDF, y Word o PowerPoint
//! paginados como Office): lo que en Windows hacen `lector_pdf.rs`,
//! `lector_pdf_proyecto.rs` (`DondeVa`) y `anotado_del_adjunto.rs`, sin
//! ventana. Las hojas las pinta Swift (PDFKit, o [`crate::office`]); aqui
//! solo se decide **donde va lo anotado y en que unidades**, con los mismos
//! nombres que Windows y Android para que viaje:
//!
//! | Que | Adjunto del chat | PDF de un proyecto | Otro documento |
//! |---|---|---|---|
//! | tinta de la hoja `n` | `anot-<uid>-p<n>` + su marco `.hoja` | el dibujo de su hoja del proyecto | `<doc>.pixpin-anotado/hoja-<n+1>.excalidraw` |
//! | marcadores (PDF) | `anot-<uid>.marcas` (`id:x:y:emoji`) | `anot-<uid del proyecto>.marcas` | `<doc>.pixpin-lectura` |
//! | espacios para anotar (PDF) | `anot-<uid>.espacios` (`0`..`3`) | `anot-<uid del proyecto>.espacios` | `<doc>.pixpin-lectura` |
//! | aumento y pagina | `<doc>.pixpin-lectura` (de esta pantalla, no viaja) | igual | igual |
//!
//! # Las unidades
//!
//! Cada hoja mide `ANCHO_HOJA` (1400) de ancho con el cero en su esquina de
//! arriba a la izquierda y la Y hacia abajo; su alto es `1400 * alto/ancho`
//! de la pagina (la caja que se ve, ya girada). Los margenes para anotar son
//! las X negativas y las de mas alla de 1400. La tinta se abre con
//! [`Nucleo::anotar_hoja`] como un [`EditorLienzo`] en esas unidades; el que
//! la escribio en otras (el movil, segun sus espacios) lo dice su marco, o
//! la regla de antes si no lo trae. Guardar la escribe siempre en las de la
//! hoja con el marco `0,0,1400,alto`, como Windows y Android v0.98.
//!
//! # Rendimiento
//!
//! Nada se abre por adelantado: Swift pide la tinta de las hojas que se ven
//! ([`Nucleo::hojas_con_tinta`] dice cuales tienen, sin leerlas) y cada
//! editor pinta solo lo que cae en pantalla.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use pixpin_docs::lectura;
use pixpin_motor2d::marcas;
use pixpin_proyecto::anotado::{self as an, Base};
use pixpin_proyecto::capas_del_pdf;
use pixpin_sincro::anotado::{MarcoDeLaHoja, huella_coincide};

use crate::editor::{EditorLienzo, HojaDoc};
use crate::unidades::{Unidades, hoja_propia};
use crate::{ErrorNucleo, Nucleo};

// Cambiar el nombre, juntar la tinta con el disco y la pagina que sube
// sola (tanda B7).
mod lector;
pub use lector::DocumentoRenombrado;
// El visor reflujado de Word y libros, con su tinta (tanda E7).
mod reflujo;

/// Un marcador de un PDF: `x` es la fraccion del ancho de la hoja y `y` la
/// pagina (desde 0) mas la fraccion de su alto (`marcas::en_la_pagina`).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct MarcaDocumento {
    /// Cuando se puso, en milisegundos: es su identidad.
    pub id: i64,
    pub x: f64,
    pub y: f64,
    pub emoji: String,
}

/// Adonde va lo anotado de un documento.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum DondeAnota {
    /// Un adjunto de un chat: viaja con su mensaje (`anot-<uid>…`).
    Chat,
    /// El PDF de un proyecto: la tinta va en las hojas del proyecto.
    Proyecto,
    /// Un documento de fuera del almacen: junto a el, no viaja.
    JuntoAlDocumento,
}

/// Lo que se recuerda de un documento al leerlo.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct LecturaDocumento {
    /// El aumento propio (1 = la hoja a lo ancho de la ventana).
    pub zoom: f32,
    /// Por donde se iba: la pagina (desde 0) mas la fraccion de su alto.
    pub pagina: f64,
    /// Los espacios para anotar, en bits: 1 izquierda, 2 derecha.
    pub espacios: u8,
    pub marcas: Vec<MarcaDocumento>,
    pub donde: DondeAnota,
}

/// Los emoticonos de los marcadores (`marcas::EMOJIS`, los del movil).
#[uniffi::export]
pub fn emojis_de_marcas() -> Vec<String> {
    marcas::EMOJIS.iter().map(|e| e.to_string()).collect()
}

/// El ancho de una hoja en unidades de la tinta (1400).
#[uniffi::export]
pub fn ancho_de_hoja() -> f32 {
    pixpin_docs::vista::ANCHO_HOJA
}

/// Lo que se abre a cada lado con los espacios `espacios`, en unidades de
/// la hoja (`vista::espacios_del_pdf`: tres cuartos de la hoja por lado).
#[uniffi::export]
pub fn espacios_de_hoja(espacios: u8) -> Vec<f32> {
    let (i, d) = pixpin_docs::vista::espacios_del_pdf(espacios);
    vec![i, d]
}

fn es_pdf(doc: &Path) -> bool {
    doc.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

/// **Donde va lo anotado** de un documento abierto (`DondeVa` de Windows).
pub(crate) struct Donde {
    raiz: PathBuf,
    doc: PathBuf,
    proyecto: Option<String>,
    adjunto: Option<an::Adjunto>,
    huella: String,
}

/// Cuanto vale lo ya mirado de si un PDF es de un proyecto: mirarlo lee el
/// `proyecto.json` de cada chat, y el lector lo pregunta al abrir cada hoja.
const VIGENCIA_DEL_PROYECTO: std::time::Duration = std::time::Duration::from_secs(30);

type CacheProyectos = HashMap<PathBuf, (std::time::Instant, Option<String>)>;

fn proyectos_vistos() -> &'static Mutex<CacheProyectos> {
    static C: std::sync::OnceLock<Mutex<CacheProyectos>> = std::sync::OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// La ficha del proyecto de este PDF (`capas_del_pdf::de_este_pdf`), con
/// lo mirado hace poco guardado (tope de 64 documentos).
fn proyecto_de(raiz: &Path, doc: &Path) -> Option<String> {
    let clave = doc.to_path_buf();
    if let Ok(c) = proyectos_vistos().lock()
        && let Some((cuando, ficha)) = c.get(&clave)
        && cuando.elapsed() < VIGENCIA_DEL_PROYECTO
    {
        return ficha.clone();
    }
    let ficha = capas_del_pdf::de_este_pdf(raiz, doc).map(|p| p.ficha);
    if let Ok(mut c) = proyectos_vistos().lock() {
        if c.len() >= 64 {
            c.retain(|_, (cuando, _)| cuando.elapsed() < VIGENCIA_DEL_PROYECTO);
            if c.len() >= 64 {
                c.clear();
            }
        }
        c.insert(clave, (std::time::Instant::now(), ficha.clone()));
    }
    ficha
}

impl Donde {
    pub(crate) fn de(raiz: &Path, doc: &Path) -> Donde {
        let proyecto = if es_pdf(doc) {
            proyecto_de(raiz, doc)
        } else {
            None
        };
        let adjunto = if proyecto.is_none() {
            an::adjunto_de(raiz, doc)
        } else {
            None
        };
        Donde {
            raiz: raiz.to_path_buf(),
            doc: doc.to_path_buf(),
            huella: capas_del_pdf::huella(&doc.to_string_lossy()),
            proyecto,
            adjunto,
        }
    }

    fn donde(&self) -> DondeAnota {
        if self.proyecto.is_some() {
            DondeAnota::Proyecto
        } else if self.adjunto.is_some() {
            DondeAnota::Chat
        } else {
            DondeAnota::JuntoAlDocumento
        }
    }

    /// La base de los marcadores y espacios de un PDF (`base_del_pdf`).
    fn base_del_pdf(&self) -> Option<Base> {
        if !es_pdf(&self.doc) {
            return None;
        }
        an::base_del_pdf(&self.raiz, &self.doc, self.proyecto.as_deref())
    }

    /// La base de la tinta de la hoja `i` de un adjunto del chat.
    fn base_de_la_hoja(&self, i: u32) -> Option<Base> {
        if self.proyecto.is_some() {
            return None;
        }
        self.adjunto
            .as_ref()
            .map(|x| Base::de_pagina(&self.raiz, x, i))
    }

    /// El dibujo de la hoja `i` en el proyecto (sin apuntarlo).
    fn dibujo_del_proyecto(&self, i: u32) -> Option<String> {
        let ficha = self.proyecto.as_deref()?;
        capas_del_pdf::dibujo_de_la_pagina(&self.raiz, ficha, i, &self.huella, false, 0)
    }

    /// **El fichero de la tinta de la hoja `i`** (`DondeVa::ruta`).
    fn ruta(&self, i: u32) -> PathBuf {
        if let (Some(ficha), Some(d)) = (self.proyecto.as_deref(), self.dibujo_del_proyecto(i)) {
            return capas_del_pdf::fichero(&self.raiz, ficha, &d);
        }
        match self.base_de_la_hoja(i) {
            Some(b) => b.tinta(),
            None => junto_al_documento(&self.doc, i),
        }
    }

    /// En que unidades estan las tintas del movil sin marco.
    fn unidades(&self, espacios: u8) -> Unidades {
        if self.adjunto.is_some() && self.proyecto.is_none() {
            Unidades::de_la_capa_del_movil(espacios)
        } else {
            Unidades::DEL_PC
        }
    }
}

/// `<doc>.pixpin-anotado/hoja-<i+1>.excalidraw` (`lector_tinta::ruta_de_hoja`).
fn junto_al_documento(doc: &Path, i: u32) -> PathBuf {
    let mut nombre = doc
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "documento".into());
    nombre.push_str(".pixpin-anotado");
    doc.with_file_name(nombre)
        .join(format!("hoja-{}.excalidraw", i + 1))
}

/// **El marco de la tinta de `b`, si esta, se entiende y es de esta tinta**
/// (`anotado_del_adjunto::leer_marco`).
fn leer_marco(b: &Base) -> Option<MarcoDeLaHoja> {
    let texto = an::leer(&b.marco())?;
    let (m, huella) = MarcoDeLaHoja::de_texto_con_huella(&texto)?;
    if let Some(h) = huella {
        let resumen = b.resumen_de_la_tinta();
        if !resumen.as_deref().is_some_and(|r| huella_coincide(&h, r)) {
            return None;
        }
    }
    Some(m)
}

fn marcas_a_texto(l: &[MarcaDocumento]) -> String {
    let v: Vec<marcas::Marca> = l
        .iter()
        .filter(|m| m.x.is_finite() && m.y.is_finite())
        .map(|m| marcas::Marca {
            id: m.id,
            emoji: m.emoji.replace(['|', '\n', '\r'], ""),
            x: m.x,
            y: m.y,
        })
        .collect();
    marcas::a_texto(&v)
}

fn marcas_de_texto(t: &str) -> Vec<MarcaDocumento> {
    marcas::de_texto(t.trim())
        .into_iter()
        .filter(|m| m.id != 0 || !m.emoji.is_empty())
        .map(|m| MarcaDocumento {
            id: m.id,
            x: m.x,
            y: m.y,
            emoji: m.emoji,
        })
        .collect()
}

/// Lo de junto al documento y, encima, lo del mensaje (`leer` de
/// `anotado_del_adjunto` con un PDF): manda lo que viaja.
fn leer_ajustes(d: &Donde) -> lectura::Ajustes {
    let mut a = lectura::leer(&d.doc);
    if let Some(b) = d.base_del_pdf() {
        // La primera vez, lo de aqui pasa al mensaje (`migrar`).
        an::poner_si_falta(
            &b.fichero(".marcas"),
            Some(&a.marcas)
                .filter(|m| !m.is_empty())
                .map(String::as_str),
        );
        an::poner_si_falta(
            &b.fichero(".espacios"),
            (a.espacios != 0).then(|| a.espacios.to_string()).as_deref(),
        );
        if let Some(t) = an::leer(&b.fichero(".marcas")) {
            a.marcas = t.replace(['\n', '\r'], "").trim().to_string();
        }
        // Lo que no se entiende no pisa lo de aqui.
        if let Some(e) =
            an::leer(&b.fichero(".espacios")).and_then(|t| t.trim().parse::<i64>().ok())
        {
            a.espacios = (e & 3) as u8;
        }
    }
    a
}

#[uniffi::export]
impl Nucleo {
    /// **Lo que se recuerda de un documento** (`documento`: su ruta en este
    /// Mac, la de [`Nucleo::ruta_absoluta`] si es un adjunto): aumento,
    /// pagina, espacios para anotar y marcadores, y adonde va lo anotado.
    pub fn lectura_de_documento(&self, documento: String) -> LecturaDocumento {
        let d = Donde::de(self.ruta(), Path::new(&documento));
        let a = leer_ajustes(&d);
        LecturaDocumento {
            zoom: if a.zoom.is_finite() && a.zoom > 0.0 {
                a.zoom
            } else {
                1.0
            },
            pagina: if a.pagina.is_finite() {
                a.pagina.max(0.0)
            } else {
                0.0
            },
            espacios: a.espacios & 3,
            marcas: marcas_de_texto(&a.marcas),
            donde: d.donde(),
        }
    }

    /// **Guarda lo que se recuerda**: aumento y pagina junto al documento
    /// (de esta pantalla), y marcadores y espacios donde viajan (los del PDF
    /// de un adjunto o de un proyecto). Solo se escribe lo que cambia.
    ///
    /// `altos`: el alto de cada hoja en unidades de la tinta. Si los espacios
    /// cambian, a la tinta del chat que aun no tiene marco se le apunta antes
    /// el de la regla de ahora (`fijar_lo_viejo`), para que no se corra.
    pub fn guardar_lectura_de_documento(
        &self,
        documento: String,
        lectura: LecturaDocumento,
        altos: Vec<f32>,
    ) -> Result<(), ErrorNucleo> {
        let doc = PathBuf::from(&documento);
        let d = Donde::de(self.ruta(), &doc);
        let mut a = leer_ajustes(&d);
        let espacios = lectura.espacios & 3;
        if espacios != a.espacios {
            fijar_lo_viejo(&d, a.espacios, &altos);
        }
        if lectura.zoom.is_finite() && lectura.zoom > 0.0 {
            a.zoom = lectura.zoom;
        }
        if lectura.pagina.is_finite() {
            a.pagina = lectura.pagina.max(0.0);
        }
        a.espacios = espacios;
        a.marcas = marcas_a_texto(&lectura.marcas);
        lectura::escribir(&doc, &a)?;
        if let Some(b) = d.base_del_pdf() {
            // Lo que no esta y no dice nada no se crea.
            let poner = |t: &str, texto: String, nada: bool| -> std::io::Result<()> {
                let f = b.fichero(t);
                if nada && !f.exists() {
                    return Ok(());
                }
                an::escribir(&f, &texto)
            };
            poner(".marcas", a.marcas.clone(), a.marcas.is_empty())?;
            poner(".espacios", a.espacios.to_string(), a.espacios == 0)?;
        }
        Ok(())
    }

    /// **Abre la tinta de la hoja `pagina`** (desde 0) de `documento` para
    /// verla y anotarla, en las unidades de la hoja (ver la doc del modulo).
    /// `alto`: el de la hoja en esas unidades (`1400 * alto / ancho` de la
    /// pagina tal como se ve). Si no hay tinta, nace vacia y no se escribe
    /// nada hasta que se anote. Guardar con [`EditorLienzo::guardar`] (fuera
    /// del hilo principal) escribe la tinta y su marco.
    pub fn anotar_hoja(
        &self,
        documento: String,
        pagina: u32,
        alto: f32,
    ) -> Result<Arc<EditorLienzo>, ErrorNucleo> {
        if !(alto.is_finite() && alto > 0.0) {
            return Err(ErrorNucleo::Argumento {
                detalle: format!("alto de hoja no valido: {alto}"),
            });
        }
        let doc = PathBuf::from(&documento);
        let d = Donde::de(self.ruta(), &doc);
        let espacios = leer_ajustes(&d).espacios;
        Ok(Arc::new(abrir_hoja(&d, pagina, alto, espacios, true)?))
    }

    /// **Que hojas tienen tinta** de las `paginas` primeras (sin leerlas:
    /// solo si su fichero esta). Para no abrir un editor por hoja vacia.
    pub fn hojas_con_tinta(&self, documento: String, paginas: u32) -> Vec<u32> {
        let d = Donde::de(self.ruta(), Path::new(&documento));
        // En un proyecto, cada hoja leeria `proyecto.json`: se lee una vez.
        (0..paginas.min(100_000))
            .filter(|i| d.ruta(*i).is_file())
            .collect()
    }

    /// Adonde va lo anotado de `documento`.
    pub fn donde_anota(&self, documento: String) -> DondeAnota {
        Donde::de(self.ruta(), Path::new(&documento)).donde()
    }
}

/// La tinta de la hoja `i` como editor (`DondeVa::leer_capa`). Con
/// `migrar`, a la del chat sin marco se le apunta el de la regla de antes.
fn abrir_hoja(
    d: &Donde,
    i: u32,
    alto: f32,
    espacios: u8,
    migrar: bool,
) -> Result<EditorLienzo, ErrorNucleo> {
    let propia = hoja_propia(alto);
    let ruta = d.ruta(i);
    let base = d.base_de_la_hoja(i);
    let del_marco = base
        .as_ref()
        .and_then(leer_marco)
        .and_then(|m| Unidades::del_marco(&propia, &m));
    let con_marco = del_marco.is_some();
    let u = del_marco.unwrap_or_else(|| d.unidades(espacios));
    let registrar = d.proyecto.as_ref().and_then(|ficha| {
        d.dibujo_del_proyecto(i)
            .map(|_| (d.raiz.clone(), ficha.clone(), i, d.huella.clone()))
    });
    let ed = EditorLienzo::de_hoja(
        ruta,
        u,
        HojaDoc {
            marco: base.clone().map(|b| (b, propia)),
            registrar,
        },
    )?;
    if migrar
        && !con_marco
        && let Some(b) = base
        && ed.cuantos_elementos() > 0
        && an::leer(&b.marco()).is_none_or(|t| MarcoDeLaHoja::de_texto_con_huella(&t).is_some())
    {
        b.escribir_marco(&u.marco_de(&propia))?;
    }
    Ok(ed)
}

/// **A la tinta del chat sin marco se le escribe el de la regla de antes con
/// los espacios de ahora, sin tocarla** (`fijar_lo_viejo`), antes de cambiar
/// los espacios. Devuelve cuantas hojas lo recibieron.
fn fijar_lo_viejo(d: &Donde, espacios: u8, altos: &[f32]) -> usize {
    if d.adjunto.is_none() || d.proyecto.is_some() {
        return 0;
    }
    let mut n = 0;
    for (i, alto) in altos.iter().enumerate() {
        let Ok(i) = u32::try_from(i) else { break };
        let Some(b) = d.base_de_la_hoja(i) else {
            continue;
        };
        if !alto.is_finite() || *alto <= 0.0 || !b.tinta().is_file() {
            continue;
        }
        if leer_marco(&b)
            .and_then(|m| Unidades::del_marco(&hoja_propia(*alto), &m))
            .is_some()
        {
            continue;
        }
        if abrir_hoja(d, i, *alto, espacios, true).is_ok_and(|e| e.cuantos_elementos() > 0) {
            n += 1;
        }
    }
    n
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::editor::FasePuntero;
    use pixpin_proyecto::almacen::{self, Ficha, Indice};
    use pixpin_proyecto::cuaderno::{self, Clase, Mensaje, Sello};

    fn raiz(etiqueta: &str) -> PathBuf {
        let r = std::env::temp_dir().join(format!(
            "pixpin-nucleo-doc-{etiqueta}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&r);
        std::fs::create_dir_all(&r).unwrap();
        r
    }

    /// Un almacen con un adjunto `nombre` en el chat «Tesis» (la misma
    /// receta que `anotado_del_adjunto::pruebas::con_adjunto` de Windows).
    fn con_adjunto(r: &Path, nombre: &str, bytes: &[u8]) -> (PathBuf, String) {
        let ficha = Ficha::nueva("Tesis", 5, "PC01");
        let mut i = Indice::default();
        i.proyectos.push(ficha.clone());
        i.guardar(r).unwrap();
        let ruta = almacen::guardar_adjunto(r, &ficha.id, nombre, bytes).unwrap();
        let sello = Sello {
            cuando: 10,
            numero: 1,
            aparato: "PC01".into(),
            proyecto: ficha.id.clone(),
        };
        let carpeta = almacen::carpeta(r, &ficha.id);
        let m = Mensaje::adjunto(Clase::Archivo, nombre, &ruta, 2, &sello);
        cuaderno::anadir(&carpeta, &m).unwrap();
        (carpeta.join(&ruta), m.codigo_unico())
    }

    fn nucleo(r: &Path) -> Arc<Nucleo> {
        Nucleo::abrir(r.display().to_string(), "Mac".into()).unwrap()
    }

    fn trazar(ed: &EditorLienzo, desde: (f32, f32), hasta: (f32, f32)) {
        ed.herramienta("lapiz".into()).unwrap();
        ed.puntero(FasePuntero::Pulsar, desde.0, desde.1, None, 0, 1.0);
        for k in 1..=8 {
            let t = k as f32 / 8.0;
            ed.puntero(
                FasePuntero::Mover,
                desde.0 + (hasta.0 - desde.0) * t,
                desde.1 + (hasta.1 - desde.1) * t,
                None,
                0,
                1.0,
            );
        }
        ed.puntero(FasePuntero::Soltar, hasta.0, hasta.1, None, 0, 1.0);
    }

    /// El fichero de la tinta, con el nombre del movil, donde lo pone el PC.
    fn tinta_del_mensaje(r: &Path, doc: &Path, i: u32) -> PathBuf {
        an::hoja_del_pdf(r, doc, i).expect("es un adjunto")
    }

    #[test]
    fn la_tinta_de_una_hoja_del_chat_se_guarda_como_windows_con_su_marco() {
        let r = raiz("pdf-chat");
        let (pdf, uid) = con_adjunto(&r, "informe.pdf", b"%PDF-1.4");
        let n = nucleo(&r);
        let doc = pdf.display().to_string();
        assert_eq!(n.donde_anota(doc.clone()), DondeAnota::Chat);
        assert!(n.hojas_con_tinta(doc.clone(), 3).is_empty());
        let ed = n.anotar_hoja(doc.clone(), 1, 1980.0).unwrap();
        assert!(ed.es_de_documento());
        // Abrir no escribe nada.
        assert!(!ed.guardar().unwrap());
        trazar(&ed, (100.0, 200.0), (500.0, 260.0));
        assert!(ed.guardar().unwrap());
        // El nombre del movil: `anot-<uid>-p1`, en `lienzos/` del chat.
        let tinta = tinta_del_mensaje(&r, &pdf, 1);
        assert!(tinta.is_file(), "{}", tinta.display());
        let nombre = tinta.file_name().unwrap().to_string_lossy().to_string();
        assert_eq!(nombre, format!("anot-{uid}-p1.excalidraw"));
        // El marco en las dos lineas de Android: la hoja a 1400 desde el cero.
        let marco = an::marco_del_pdf(&r, &pdf, 1).unwrap();
        assert_eq!(
            std::fs::read_to_string(&marco).unwrap(),
            "0,0,1400,1980\nv1\n"
        );
        // Los puntos del fichero, en unidades de la hoja tal cual.
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&tinta).unwrap()).unwrap();
        let e = &v["elements"][0];
        assert_eq!(e["type"], "freedraw");
        let x = e["x"].as_f64().unwrap();
        assert!((x - 100.0).abs() < 1.0, "x = {x}");
        assert_eq!(n.hojas_con_tinta(doc, 3), vec![1]);
    }

    #[test]
    fn la_tinta_del_movil_sin_marco_se_lee_con_sus_espacios_y_se_le_apunta_el_marco() {
        let r = raiz("pdf-movil");
        let (pdf, _) = con_adjunto(&r, "plano.pdf", b"%PDF-1.4");
        let n = nucleo(&r);
        let doc = pdf.display().to_string();
        // El movil, sin espacios, escribe en su capa: cada unidad de la hoja
        // son 2,5 del fichero y el cero corrido -1050 (`capa_del_movil(0)`).
        let tinta = tinta_del_mensaje(&r, &pdf, 0);
        std::fs::create_dir_all(tinta.parent().unwrap()).unwrap();
        let json = serde_json::json!({
            "type": "excalidraw", "version": 2, "source": "pixpin-android",
            "elements": [{
                "id": "trazoMovil1234567890a", "type": "freedraw",
                "x": 1450.0, "y": 500.0, "width": 250.0, "height": 0.0,
                "strokeColor": "#1e1e1e", "backgroundColor": "transparent",
                "strokeWidth": 2.5, "opacity": 100, "version": 3, "versionNonce": 9,
                "updated": 1, "isDeleted": false, "seed": 1,
                "points": [[0.0, 0.0], [125.0, 0.0], [250.0, 0.0]]
            }],
            "appState": {}, "files": {}
        });
        std::fs::write(&tinta, serde_json::to_string_pretty(&json).unwrap()).unwrap();
        let ed = n.anotar_hoja(doc.clone(), 0, 1980.0).unwrap();
        let caja = ed.caja().expect("el trazo del movil se ve");
        // 1450 del fichero = (1450 + 1050) / 2,5 = 1000 de la hoja.
        assert!((caja.x0 - 1000.0).abs() < 3.0, "x0 = {}", caja.x0);
        assert!((caja.y0 - 200.0).abs() < 3.0, "y0 = {}", caja.y0);
        // Al abrirla se le apunta el marco de esa regla, sin tocar la tinta.
        let marco = std::fs::read_to_string(an::marco_del_pdf(&r, &pdf, 0).unwrap()).unwrap();
        assert_eq!(marco, "-1050,0,2450,4950\nv1\n");
        let despues: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&tinta).unwrap()).unwrap();
        assert_eq!(despues["elements"][0]["x"], 1450.0);
        // Al anotar aqui, todo pasa a las unidades de la hoja y el marco lo dice.
        trazar(&ed, (10.0, 10.0), (60.0, 60.0));
        assert!(ed.guardar().unwrap());
        let marco = std::fs::read_to_string(an::marco_del_pdf(&r, &pdf, 0).unwrap()).unwrap();
        assert_eq!(marco, "0,0,1400,1980\nv1\n");
        let otra = n.anotar_hoja(doc, 0, 1980.0).unwrap();
        let caja = otra.caja().unwrap();
        assert!((caja.x0 - 10.0).abs() < 3.0);
        assert!(
            caja.x1 >= 1090.0,
            "el del movil sigue en su sitio: {}",
            caja.x1
        );
    }

    #[test]
    fn marcadores_y_espacios_viajan_y_el_aumento_se_queda() {
        let r = raiz("pdf-lectura");
        let (pdf, uid) = con_adjunto(&r, "libro.pdf", b"%PDF-1.4");
        let n = nucleo(&r);
        let doc = pdf.display().to_string();
        let mut l = n.lectura_de_documento(doc.clone());
        assert_eq!(l.espacios, 0);
        assert!(l.marcas.is_empty());
        // Abrir y guardar sin cambios no crea ficheros que viajen.
        n.guardar_lectura_de_documento(doc.clone(), l.clone(), vec![1980.0])
            .unwrap();
        let carpeta = an::marco_del_pdf(&r, &pdf, 0).unwrap();
        let carpeta = carpeta.parent().unwrap();
        assert!(!carpeta.join(format!("anot-{uid}.marcas")).exists());
        l.zoom = 1.5;
        l.pagina = 2.25;
        l.espacios = 2;
        l.marcas = vec![MarcaDocumento {
            id: 1700,
            x: 0.5,
            y: 2.25,
            emoji: "⭐".into(),
        }];
        n.guardar_lectura_de_documento(doc.clone(), l, vec![1980.0])
            .unwrap();
        let b = an::base_del_pdf(&r, &pdf, None).unwrap();
        assert_eq!(an::leer(&b.fichero(".marcas")).unwrap(), "1700:0.5:2.25:⭐");
        assert_eq!(an::leer(&b.fichero(".espacios")).unwrap(), "2");
        let otra = n.lectura_de_documento(doc);
        assert_eq!(otra.espacios, 2);
        assert!((otra.zoom - 1.5).abs() < 1e-6);
        assert!((otra.pagina - 2.25).abs() < 1e-9);
        assert_eq!(otra.marcas.len(), 1);
        // Lo del movil manda: un `.espacios` nuevo que llega se lee.
        an::escribir(&b.fichero(".espacios"), "1").unwrap();
        assert_eq!(
            n.lectura_de_documento(pdf.display().to_string()).espacios,
            1
        );
    }

    #[test]
    fn un_documento_de_fuera_del_almacen_anota_junto_a_el() {
        let r = raiz("fuera");
        let otro = raiz("fuera-doc");
        let pdf = otro.join("suelto.pdf");
        std::fs::write(&pdf, b"%PDF-1.4").unwrap();
        let n = nucleo(&r);
        let doc = pdf.display().to_string();
        assert_eq!(n.donde_anota(doc.clone()), DondeAnota::JuntoAlDocumento);
        let ed = n.anotar_hoja(doc, 0, 1000.0).unwrap();
        trazar(&ed, (0.0, 0.0), (90.0, 90.0));
        assert!(ed.guardar().unwrap());
        assert!(
            otro.join("suelto.pdf.pixpin-anotado/hoja-1.excalidraw")
                .is_file()
        );
        // Caso negativo: un alto sin sentido no abre nada.
        assert!(
            n.anotar_hoja(pdf.display().to_string(), 0, f32::NAN)
                .is_err()
        );
    }
}
