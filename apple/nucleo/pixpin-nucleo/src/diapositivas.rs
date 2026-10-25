//! **Lo que pide presentar un PowerPoint** (tanda D9): las notas del orador,
//! la transicion de cada diapositiva, sus videos y audios y los fotogramas
//! de cada transicion. Es lo que `diapositivas.rs` de Windows lee de
//! `pixpin_office::pptx::pase`, puesto al alcance de Swift
//! (`Documentos/Diapositivas.swift`).
//!
//! Nada se lee al abrir: cada cosa se pide cuando hace falta (las notas al
//! ensenarlas, la transicion al pasar a esa diapositiva), como alla. Las
//! cuentas del fotograma ([`capas_de_transicion`]) son las mismas de
//! Windows: quien pinta solo mezcla dos imagenes ya hechas.

use pixpin_office::Documento;
use pixpin_office::pptx::pase::{self as pp, Efecto, Fuente, Sentido};

use crate::office::DocumentoOffice;

/// Como entra una diapositiva (`pixpin_office::pptx::pase::Efecto`). `dx` y
/// `dy` valen -1, 0 o 1: hacia donde se mueve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum EfectoDiapositiva {
    /// De golpe (o tras un instante en negro).
    Corte {
        por_negro: bool,
    },
    Fundido {
        por_negro: bool,
    },
    /// La vieja sale y la nueva entra empujandola.
    Empuje {
        dx: i8,
        dy: i8,
    },
    /// La nueva aparece en su sitio tras un borde que barre.
    Barrido {
        dx: i8,
        dy: i8,
    },
    /// La nueva entra por encima de la vieja.
    Cubrir {
        dx: i8,
        dy: i8,
    },
    /// La vieja se va y deja ver la nueva debajo.
    Descubrir {
        dx: i8,
        dy: i8,
    },
}

impl From<Efecto> for EfectoDiapositiva {
    fn from(e: Efecto) -> Self {
        match e {
            Efecto::Corte { por_negro } => EfectoDiapositiva::Corte { por_negro },
            Efecto::Fundido { por_negro } => EfectoDiapositiva::Fundido { por_negro },
            Efecto::Empuje(s) => EfectoDiapositiva::Empuje { dx: s.dx, dy: s.dy },
            Efecto::Barrido(s) => EfectoDiapositiva::Barrido { dx: s.dx, dy: s.dy },
            Efecto::Cubrir(s) => EfectoDiapositiva::Cubrir { dx: s.dx, dy: s.dy },
            Efecto::Descubrir(s) => EfectoDiapositiva::Descubrir { dx: s.dx, dy: s.dy },
        }
    }
}

impl From<EfectoDiapositiva> for Efecto {
    fn from(e: EfectoDiapositiva) -> Self {
        // Un sentido que no es -1, 0 o 1 se aprieta; uno quieto, hacia la
        // izquierda (el de por defecto de PowerPoint).
        let s = |dx: i8, dy: i8| {
            let s = Sentido {
                dx: dx.signum(),
                dy: dy.signum(),
            };
            if s.dx == 0 && s.dy == 0 {
                Sentido::IZQUIERDA
            } else {
                s
            }
        };
        match e {
            EfectoDiapositiva::Corte { por_negro } => Efecto::Corte { por_negro },
            EfectoDiapositiva::Fundido { por_negro } => Efecto::Fundido { por_negro },
            EfectoDiapositiva::Empuje { dx, dy } => Efecto::Empuje(s(dx, dy)),
            EfectoDiapositiva::Barrido { dx, dy } => Efecto::Barrido(s(dx, dy)),
            EfectoDiapositiva::Cubrir { dx, dy } => Efecto::Cubrir(s(dx, dy)),
            EfectoDiapositiva::Descubrir { dx, dy } => Efecto::Descubrir(s(dx, dy)),
        }
    }
}

/// La transicion con la que entra una diapositiva (`p:transition`).
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct TransicionDiapositiva {
    pub efecto: EfectoDiapositiva,
    /// Lo que dura, en milisegundos (0 si es un corte).
    pub ms: u32,
    /// Si pasa sola: tras cuantos milisegundos de verse (`advTm`).
    pub avance_ms: Option<u32>,
    /// Si un clic la pasa (`advClick`).
    pub avance_clic: bool,
}

/// De donde sale lo que pinta una capa de un fotograma.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FuenteCapa {
    Vieja,
    Nueva,
    Negro,
}

/// Una capa de un fotograma de transicion: el trozo `origen` de la fuente
/// en `destino`, con su opacidad. En fracciones de la diapositiva (0..1:
/// izquierda, arriba, derecha, abajo; la Y hacia abajo). Se pintan en orden
/// sobre negro, recortadas a la diapositiva.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct CapaDiapositiva {
    pub fuente: FuenteCapa,
    pub origen: Vec<f32>,
    pub destino: Vec<f32>,
    pub alfa: f32,
}

/// Un video o un audio de una diapositiva, con su caja en puntos.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct MedioDiapositiva {
    pub x: f32,
    pub y: f32,
    pub ancho: f32,
    pub alto: f32,
    /// La parte del paquete con el fichero (`None` si es un enlace a fuera).
    pub parte: Option<String>,
    pub audio: bool,
}

/// **Las capas del fotograma** de `efecto` en `t` (0..1): en 0 solo la
/// vieja, en 1 solo la nueva. Las de Windows (`pase::capas`), con su
/// arrancar y frenar suave.
#[uniffi::export]
pub fn capas_de_transicion(efecto: EfectoDiapositiva, t: f32) -> Vec<CapaDiapositiva> {
    pp::capas(efecto.into(), t)
        .into_iter()
        .map(|c| CapaDiapositiva {
            fuente: match c.fuente {
                Fuente::Vieja => FuenteCapa::Vieja,
                Fuente::Nueva => FuenteCapa::Nueva,
                Fuente::Negro => FuenteCapa::Negro,
            },
            origen: c.origen.to_vec(),
            destino: c.destino.to_vec(),
            alfa: c.alfa,
        })
        .collect()
}

#[uniffi::export]
impl DocumentoOffice {
    /// Las notas del orador de la diapositiva `n` (desde 0), un parrafo por
    /// linea. `None` si no tiene, si estan vacias o si no es un PowerPoint.
    pub fn notas(&self, n: u32) -> Option<String> {
        match &mut *self.tomar() {
            Documento::PowerPoint(p) => p.notas(n as usize),
            Documento::Word(_) => None,
        }
    }

    /// La transicion con la que entra la diapositiva `n`, si tiene.
    pub fn transicion(&self, n: u32) -> Option<TransicionDiapositiva> {
        match &mut *self.tomar() {
            Documento::PowerPoint(p) => p.transicion(n as usize).map(|t| TransicionDiapositiva {
                efecto: t.efecto.into(),
                ms: t.ms,
                avance_ms: t.avance_ms,
                avance_clic: t.avance_clic,
            }),
            Documento::Word(_) => None,
        }
    }

    /// Los videos y audios de la diapositiva `n`.
    pub fn medios(&self, n: u32) -> Vec<MedioDiapositiva> {
        match &mut *self.tomar() {
            Documento::PowerPoint(p) => p
                .medios(n as usize)
                .into_iter()
                .map(|m| MedioDiapositiva {
                    x: m.rect.x,
                    y: m.rect.y,
                    ancho: m.rect.ancho,
                    alto: m.rect.alto,
                    parte: m.parte,
                    audio: m.audio,
                })
                .collect(),
            Documento::Word(_) => Vec::new(),
        }
    }

    /// Los bytes de un video o un audio incrustado (`MedioDiapositiva::parte`).
    pub fn bytes_de_medio(&self, parte: String) -> Option<Vec<u8>> {
        match &mut *self.tomar() {
            Documento::PowerPoint(p) => p.bytes_de_parte(&parte),
            Documento::Word(_) => None,
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    use crate::office::{MedidorLetras, MetricasLetra};
    use std::io::Write;

    struct Fijo;

    impl MedidorLetras for Fijo {
        fn avances(&self, caracteres: String, _: String, _: bool, _: bool) -> Vec<f32> {
            caracteres.chars().map(|_| 500.0).collect()
        }
        fn metricas(&self, _: String, _: bool, _: bool) -> MetricasLetra {
            MetricasLetra {
                ascenso: 800.0,
                descenso: 200.0,
                hueco: 0.0,
            }
        }
        fn existe(&self, _: String) -> bool {
            true
        }
    }

    const NS: &str = r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main""#;
    const RELS: &str = r#"xmlns="http://schemas.openxmlformats.org/package/2006/relationships""#;
    const T: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

    /// Un `.pptx` de dos diapositivas: la primera con notas, la segunda
    /// entra empujando hacia arriba y pasa sola a los 3 s.
    fn pptx(ruta: &std::path::Path) {
        let diapositiva = |transicion: &str, notas: bool| {
            let rels = if notas {
                format!(
                    r#"<Relationship Id="rN" Type="{T}/notesSlide" Target="../notesSlides/notesSlide1.xml"/>"#
                )
            } else {
                String::new()
            };
            (
                format!(r#"<p:sld {NS}><p:cSld><p:spTree/></p:cSld>{transicion}</p:sld>"#),
                format!(
                    r#"<Relationships {RELS}><Relationship Id="rL" Type="{T}/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>{rels}</Relationships>"#
                ),
            )
        };
        let (s1, r1) = diapositiva("", true);
        let (s2, r2) = diapositiva(
            r#"<p:transition spd="med" advTm="3000"><p:push dir="u"/></p:transition>"#,
            false,
        );
        let partes = [
            (
                "_rels/.rels".to_string(),
                format!(
                    r#"<Relationships {RELS}><Relationship Id="r1" Type="{T}/officeDocument" Target="ppt/presentation.xml"/></Relationships>"#
                ),
            ),
            (
                "ppt/presentation.xml".into(),
                format!(
                    r#"<p:presentation {NS}><p:sldIdLst><p:sldId id="256" r:id="rS1"/><p:sldId id="257" r:id="rS2"/></p:sldIdLst><p:sldSz cx="9144000" cy="5143500"/></p:presentation>"#
                ),
            ),
            (
                "ppt/_rels/presentation.xml.rels".into(),
                format!(
                    r#"<Relationships {RELS}><Relationship Id="rS1" Type="{T}/slide" Target="slides/slide1.xml"/><Relationship Id="rS2" Type="{T}/slide" Target="slides/slide2.xml"/></Relationships>"#
                ),
            ),
            ("ppt/slides/slide1.xml".into(), s1),
            ("ppt/slides/_rels/slide1.xml.rels".into(), r1),
            ("ppt/slides/slide2.xml".into(), s2),
            ("ppt/slides/_rels/slide2.xml.rels".into(), r2),
            (
                "ppt/slideLayouts/slideLayout1.xml".into(),
                format!(r#"<p:sldLayout {NS}><p:cSld><p:spTree/></p:cSld></p:sldLayout>"#),
            ),
            (
                "ppt/slideLayouts/_rels/slideLayout1.xml.rels".into(),
                format!(
                    r#"<Relationships {RELS}><Relationship Id="rM" Type="{T}/slideMaster" Target="../slideMasters/slideMaster1.xml"/></Relationships>"#
                ),
            ),
            (
                "ppt/slideMasters/slideMaster1.xml".into(),
                format!(r#"<p:sldMaster {NS}><p:cSld><p:spTree/></p:cSld></p:sldMaster>"#),
            ),
            (
                "ppt/notesSlides/notesSlide1.xml".into(),
                format!(
                    r#"<p:notes {NS}><p:cSld><p:spTree><p:sp><p:nvSpPr><p:cNvPr id="3" name="N"/><p:cNvSpPr/><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr><p:txBody><a:bodyPr/><a:p><a:r><a:t>Saludar al publico</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:notes>"#
                ),
            ),
        ];
        let f = std::fs::File::create(ruta).unwrap();
        let mut z = zip::ZipWriter::new(f);
        let o: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default();
        for (nombre, xml) in &partes {
            z.start_file(nombre.as_str(), o).unwrap();
            z.write_all(xml.as_bytes()).unwrap();
        }
        z.finish().unwrap();
    }

    #[test]
    fn un_powerpoint_da_sus_notas_y_sus_transiciones() {
        let d =
            std::env::temp_dir().join(format!("pixpin-nucleo-diapositivas-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let ruta = d.join("charla.pptx");
        pptx(&ruta);
        let doc = DocumentoOffice::abrir(ruta.display().to_string(), Box::new(Fijo)).unwrap();
        assert!(doc.es_presentacion());
        assert_eq!(doc.total(), 2);
        assert_eq!(doc.notas(0).as_deref(), Some("Saludar al publico"));
        assert_eq!(doc.notas(1), None, "sin notas");
        assert_eq!(doc.notas(9), None, "una que no existe");
        assert_eq!(doc.transicion(0), None);
        let t = doc.transicion(1).expect("la segunda entra empujando");
        assert_eq!(t.efecto, EfectoDiapositiva::Empuje { dx: 0, dy: -1 });
        assert!(t.ms > 0);
        assert_eq!(t.avance_ms, Some(3000));
        assert!(doc.medios(0).is_empty());
        assert_eq!(doc.bytes_de_medio("ppt/media/no-esta.mp4".into()), None);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn el_efecto_va_y_vuelve_sin_perder_nada() {
        for e in [
            Efecto::Corte { por_negro: true },
            Efecto::Fundido { por_negro: false },
            Efecto::Empuje(Sentido { dx: 0, dy: 1 }),
            Efecto::Barrido(Sentido { dx: 1, dy: 0 }),
            Efecto::Cubrir(Sentido { dx: -1, dy: -1 }),
            Efecto::Descubrir(Sentido::IZQUIERDA),
        ] {
            assert_eq!(Efecto::from(EfectoDiapositiva::from(e)), e);
        }
        // Un sentido que no se entiende no rompe las cuentas.
        assert_eq!(
            Efecto::from(EfectoDiapositiva::Empuje { dx: 0, dy: 0 }),
            Efecto::Empuje(Sentido::IZQUIERDA)
        );
        assert_eq!(
            Efecto::from(EfectoDiapositiva::Cubrir { dx: 7, dy: 0 }),
            Efecto::Cubrir(Sentido { dx: 1, dy: 0 })
        );
    }

    #[test]
    fn las_capas_empiezan_en_la_vieja_y_acaban_en_la_nueva() {
        let e = EfectoDiapositiva::Empuje { dx: -1, dy: 0 };
        let c0 = capas_de_transicion(e, 0.0);
        assert_eq!(c0.len(), 1);
        assert_eq!(c0[0].fuente, FuenteCapa::Vieja);
        let c1 = capas_de_transicion(e, 1.0);
        assert_eq!(c1.len(), 1);
        assert_eq!(c1[0].fuente, FuenteCapa::Nueva);
        assert_eq!(c1[0].destino, vec![0.0, 0.0, 1.0, 1.0]);
        // A medias: las dos, movidas.
        let medio = capas_de_transicion(e, 0.5);
        assert_eq!(medio.len(), 2);
        assert!(
            medio
                .iter()
                .all(|c| c.origen.len() == 4 && c.destino.len() == 4)
        );
        // Un fundido a medias deja la nueva medio transparente.
        let f = capas_de_transicion(EfectoDiapositiva::Fundido { por_negro: false }, 0.5);
        assert!(
            f.iter()
                .any(|c| c.fuente == FuenteCapa::Nueva && (c.alfa - 0.5).abs() < 1e-4)
        );
    }
}
