//! **Word y PowerPoint paginados como los imprime Office** (`pixpin-office`),
//! para el lector de documentos: las mismas hojas que el lector de Windows
//! (`lector_pdf/office.rs`), asi que la tinta de cada hoja
//! (`anot-<uid>-p<n>`, ver [`crate::documentos`]) cae en la misma pagina en
//! los dos sistemas.
//!
//! # Medir el texto
//!
//! La maquetacion pregunta cuanto avanza cada letra. En Windows contesta
//! DirectWrite; aqui, **Core Text desde Swift** por [`MedidorLetras`]. Para
//! no cruzar el puente por cada palabra, los avances se piden **una vez por
//! letra y por tipo de letra** (a tamano 1000) y se guardan: un documento
//! entero son unas pocas llamadas por familia.
//!
//! # La pagina, en un bufer
//!
//! Cada pagina cruza como [`PaginaOffice`]: un bufer plano de `f32` en
//! little-endian con las ordenes ya colocadas, en puntos y con el cero
//! arriba a la izquierda, mas las tablas de textos, letras e imagenes. Swift
//! lo recorre una vez y pinta con Core Graphics. Formato (version 1), cada
//! orden empieza por su codigo:
//!
//! - `1` texto: `x, y(base), texto, letra, tam, r, g, b, a, subrayado(0 no,
//!   1 simple, 2 doble, 3 grueso, 4 puntos, 5 rayas, 6 ondulado), tachado,
//!   espaciado, extra_espacio, ancho` (16 numeros con el codigo).
//! - `2` rectangulo: `x, y, ancho, alto, relleno?, r, g, b, a, borde?, r, g,
//!   b, a, grosor, discontinuo(0 solida, 1 rayas, 2 puntos, 3 raya-punto)`.
//! - `3` linea: `x0, y0, x1, y1, r, g, b, a, grosor, discontinuo`.
//! - `4` imagen: `imagen, x, y, ancho, alto, recorte?, izq, arriba, der,
//!   abajo, opacidad`.
//! - `5` trayecto: `figuras, relleno?, r, g, b, a, borde?, r, g, b, a,
//!   grosor, discontinuo` y por figura `cerrada, rellenar, trazar,
//!   sombra(0..5), x, y, tramos`, y por tramo `0, x, y` (recta) o `1, c1x,
//!   c1y, c2x, c2y, x, y` (Bezier).
//! - `6` grupo: `a, b, c, d, e, f, recorte?, x, y, ancho, alto` (la
//!   transformada de Direct2D: `x' = a x + c y + e`). `7` cierra el grupo.
//!
//! Los colores van de 0 a 1. Un degradado va como su color medio; los
//! efectos (sombras, brillo) se omiten y se pinta lo de dentro.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use pixpin_office::pagina::{
    Campo, Discontinuo, Elemento, Figura, Relleno, Sombra, Subrayado, Tramo, Trazo,
};
use pixpin_office::{Documento, Letra, Medidor, Metricas};

use crate::ErrorNucleo;

/// La version del formato de [`PaginaOffice`].
pub const VERSION_PAGINA_OFFICE: u32 = 1;

/// Lo vertical de una letra **a tamano 1000**, en puntos.
#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct MetricasLetra {
    pub ascenso: f32,
    pub descenso: f32,
    pub hueco: f32,
}

/// **Quien mide las letras**: Core Text desde Swift.
#[uniffi::export(callback_interface)]
pub trait MedidorLetras: Send + Sync {
    /// El avance de cada caracter (escalar Unicode) de `caracteres`, en
    /// puntos **a tamano 1000**, sin interletrado ni ajuste entre pares. Uno
    /// por caracter.
    fn avances(
        &self,
        caracteres: String,
        familia: String,
        negrita: bool,
        cursiva: bool,
    ) -> Vec<f32>;
    /// Ascenso, descenso y hueco a tamano 1000.
    fn metricas(&self, familia: String, negrita: bool, cursiva: bool) -> MetricasLetra;
    /// Si la familia esta instalada.
    fn existe(&self, familia: String) -> bool;
}

/// Una letra sin tamano: lo que se guarda en la cache.
type ClaveLetra = (Arc<str>, bool, bool);

#[derive(Default)]
struct Cache {
    avances: HashMap<ClaveLetra, HashMap<char, f32>>,
    metricas: HashMap<ClaveLetra, Metricas>,
    existe: HashMap<String, bool>,
}

/// El [`Medidor`] de `pixpin-office` sobre el de Swift, con cache.
pub(crate) struct MedidorConCache {
    swift: Box<dyn MedidorLetras>,
    cache: Mutex<Cache>,
}

fn clave(l: &Letra) -> ClaveLetra {
    (l.familia.clone(), l.negrita, l.cursiva)
}

impl MedidorConCache {
    pub(crate) fn nuevo(swift: Box<dyn MedidorLetras>) -> MedidorConCache {
        MedidorConCache {
            swift,
            cache: Mutex::new(Cache::default()),
        }
    }
}

impl Medidor for MedidorConCache {
    fn avances(&self, texto: &str, letra: &Letra, salida: &mut Vec<f32>) {
        let k = clave(letra);
        let mut c = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        let mapa = c.avances.entry(k.clone()).or_default();
        let mut faltan: Vec<char> = texto.chars().filter(|ch| !mapa.contains_key(ch)).collect();
        if !faltan.is_empty() {
            faltan.sort_unstable();
            faltan.dedup();
            let pedidos: String = faltan.iter().collect();
            let v = self.swift.avances(
                pedidos,
                letra.familia.to_string(),
                letra.negrita,
                letra.cursiva,
            );
            for (i, ch) in faltan.iter().enumerate() {
                let a = v.get(i).copied().filter(|a| a.is_finite()).unwrap_or(500.0);
                mapa.insert(*ch, a / 1000.0);
            }
        }
        let tam = letra.tam;
        salida.extend(
            texto
                .chars()
                .map(|ch| mapa.get(&ch).copied().unwrap_or(0.5) * tam),
        );
    }

    fn metricas(&self, letra: &Letra) -> Metricas {
        let k = clave(letra);
        let mut c = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        let m = *c.metricas.entry(k).or_insert_with(|| {
            let m = self
                .swift
                .metricas(letra.familia.to_string(), letra.negrita, letra.cursiva);
            let ok = |v: f32, d: f32| if v.is_finite() { v / 1000.0 } else { d };
            Metricas {
                ascenso: ok(m.ascenso, 0.8),
                descenso: ok(m.descenso, 0.2),
                hueco: ok(m.hueco, 0.0),
            }
        });
        Metricas {
            ascenso: m.ascenso * letra.tam,
            descenso: m.descenso * letra.tam,
            hueco: m.hueco * letra.tam,
        }
    }

    fn existe(&self, familia: &str) -> bool {
        let mut c = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(e) = c.existe.get(familia) {
            return *e;
        }
        let e = self.swift.existe(familia.to_string());
        c.existe.insert(familia.to_string(), e);
        e
    }
}

/// Una letra de la tabla de [`PaginaOffice`].
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct LetraOffice {
    pub familia: String,
    pub negrita: bool,
    pub cursiva: bool,
}

/// Un hipervinculo de la pagina: su caja en puntos y adonde va.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct EnlaceOffice {
    pub x: f32,
    pub y: f32,
    pub ancho: f32,
    pub alto: f32,
    pub destino: String,
}

/// **Una pagina maquetada**, lista para pintar (ver la doc del modulo).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct PaginaOffice {
    /// Desde 0.
    pub indice: u32,
    /// En puntos.
    pub ancho: f32,
    pub alto: f32,
    /// El fondo (0..1), si lo tiene.
    pub fondo: Option<Vec<f32>>,
    pub datos: Vec<u8>,
    pub textos: Vec<String>,
    pub letras: Vec<LetraOffice>,
    /// Los ids de las imagenes (sus bytes con [`DocumentoOffice::imagen`]).
    pub imagenes: Vec<String>,
    pub enlaces: Vec<EnlaceOffice>,
    /// El texto de la pagina en orden de lectura (buscar, copiar, leer).
    pub texto_plano: String,
}

/// Lo que se va escribiendo al empaquetar una pagina.
#[derive(Default)]
struct Bufer {
    f: Vec<f32>,
    textos: Vec<String>,
    letras: Vec<LetraOffice>,
    indice_letra: HashMap<(String, bool, bool), usize>,
    imagenes: Vec<String>,
    indice_imagen: HashMap<String, usize>,
}

fn rgba(c: pixpin_office::Color) -> [f32; 4] {
    [
        c.r as f32 / 255.0,
        c.g as f32 / 255.0,
        c.b as f32 / 255.0,
        c.a as f32 / 255.0,
    ]
}

fn discontinuo(d: Discontinuo) -> f32 {
    match d {
        Discontinuo::Solida => 0.0,
        Discontinuo::Rayas => 1.0,
        Discontinuo::Puntos => 2.0,
        Discontinuo::RayaPunto => 3.0,
    }
}

fn subrayado(s: Option<Subrayado>) -> f32 {
    match s {
        None => 0.0,
        Some(Subrayado::Simple) => 1.0,
        Some(Subrayado::Doble) => 2.0,
        Some(Subrayado::Grueso) => 3.0,
        Some(Subrayado::Puntos) => 4.0,
        Some(Subrayado::Rayas) => 5.0,
        Some(Subrayado::Ondulado) => 6.0,
    }
}

fn sombra(s: Sombra) -> f32 {
    match s {
        Sombra::Ninguna => 0.0,
        Sombra::Oscura => 1.0,
        Sombra::OscuraSuave => 2.0,
        Sombra::Clara => 3.0,
        Sombra::ClaraSuave => 4.0,
        Sombra::Aparte => 5.0,
    }
}

impl Bufer {
    fn relleno(&mut self, r: &Option<Relleno>) {
        match r {
            Some(r) => {
                self.f.push(1.0);
                self.f.extend(rgba(r.color_medio()));
            }
            None => self.f.extend([0.0; 5]),
        }
    }

    fn borde(&mut self, b: &Option<Trazo>) {
        match b {
            Some(t) => {
                self.f.push(1.0);
                self.f.extend(rgba(t.color));
                self.f.push(t.grosor);
                self.f.push(discontinuo(t.discontinuo));
            }
            None => self.f.extend([0.0; 7]),
        }
    }

    fn letra(&mut self, l: &Letra) -> usize {
        let k = (l.familia.to_string(), l.negrita, l.cursiva);
        if let Some(i) = self.indice_letra.get(&k) {
            return *i;
        }
        let i = self.letras.len();
        self.letras.push(LetraOffice {
            familia: k.0.clone(),
            negrita: k.1,
            cursiva: k.2,
        });
        self.indice_letra.insert(k, i);
        i
    }

    fn imagen(&mut self, id: &str) -> usize {
        if let Some(i) = self.indice_imagen.get(id) {
            return *i;
        }
        let i = self.imagenes.len();
        self.imagenes.push(id.to_string());
        self.indice_imagen.insert(id.to_string(), i);
        i
    }

    fn elementos(&mut self, els: &[Elemento], total: Option<usize>) {
        for e in els {
            self.elemento(e, total);
        }
    }

    fn elemento(&mut self, e: &Elemento, total: Option<usize>) {
        match e {
            Elemento::Texto(t) => {
                let texto = match t.campo {
                    Some(Campo::TotalPaginas) => {
                        total.map_or_else(|| t.texto.clone(), |n| n.to_string())
                    }
                    _ => t.texto.clone(),
                };
                let i = self.textos.len();
                self.textos.push(texto);
                let l = self.letra(&t.letra);
                self.f
                    .extend([1.0, t.x, t.y, i as f32, l as f32, t.letra.tam]);
                self.f.extend(rgba(t.color));
                self.f.extend([
                    subrayado(t.subrayado),
                    if t.tachado { 1.0 } else { 0.0 },
                    t.espaciado,
                    t.extra_espacio,
                    t.ancho,
                ]);
            }
            Elemento::Rect {
                rect,
                relleno,
                borde,
            } => {
                self.f.extend([2.0, rect.x, rect.y, rect.ancho, rect.alto]);
                self.relleno(relleno);
                self.borde(borde);
            }
            Elemento::Linea {
                desde,
                hasta,
                trazo,
            } => {
                self.f.extend([3.0, desde.0, desde.1, hasta.0, hasta.1]);
                self.f.extend(rgba(trazo.color));
                self.f.push(trazo.grosor);
                self.f.push(discontinuo(trazo.discontinuo));
            }
            Elemento::Imagen {
                id,
                rect,
                recorte,
                opacidad,
            } => {
                let i = self.imagen(id);
                self.f
                    .extend([4.0, i as f32, rect.x, rect.y, rect.ancho, rect.alto]);
                match recorte {
                    Some(r) => {
                        self.f.push(1.0);
                        self.f.extend(r.iter().copied());
                    }
                    None => self.f.extend([0.0; 5]),
                }
                self.f.push(*opacidad);
            }
            Elemento::Trayecto {
                figuras,
                relleno,
                borde,
            } => {
                self.f.extend([5.0, figuras.len() as f32]);
                self.relleno(relleno);
                self.borde(borde);
                for fig in figuras {
                    self.figura(fig);
                }
            }
            Elemento::Grupo {
                transformada,
                recorte,
                elementos,
            } => {
                self.f.push(6.0);
                self.f.extend(transformada.iter().copied());
                match recorte {
                    Some(r) => self.f.extend([1.0, r.x, r.y, r.ancho, r.alto]),
                    None => self.f.extend([0.0; 5]),
                }
                self.elementos(elementos, total);
                self.f.push(7.0);
            }
            Elemento::Efecto { elementos, .. } => self.elementos(elementos, total),
        }
    }

    fn figura(&mut self, fig: &Figura) {
        let b = |v: bool| if v { 1.0 } else { 0.0 };
        self.f.extend([
            b(fig.cerrada),
            b(fig.rellenar),
            b(fig.trazar),
            sombra(fig.sombra),
            fig.inicio.0,
            fig.inicio.1,
            fig.tramos.len() as f32,
        ]);
        for t in &fig.tramos {
            match t {
                Tramo::Linea(x, y) => self.f.extend([0.0, *x, *y]),
                Tramo::Bezier(a, c, d) => self.f.extend([1.0, a.0, a.1, c.0, c.1, d.0, d.1]),
            }
        }
    }
}

fn empaquetar(indice: u32, p: &pixpin_office::Pagina, total: Option<usize>) -> PaginaOffice {
    let mut b = Bufer::default();
    b.f.push(VERSION_PAGINA_OFFICE as f32);
    b.elementos(&p.elementos, total);
    let mut enlaces = Vec::new();
    let mut texto_plano = String::new();
    let mut ultima_y = f32::NAN;
    for (t, r) in p.textos() {
        if let Some(destino) = &t.enlace {
            enlaces.push(EnlaceOffice {
                x: r.x,
                y: r.y,
                ancho: r.ancho,
                alto: r.alto,
                destino: destino.to_string(),
            });
        }
        if !texto_plano.is_empty() {
            texto_plano.push(if (t.y - ultima_y).abs() > 0.5 {
                '\n'
            } else {
                ' '
            });
        }
        ultima_y = t.y;
        texto_plano.push_str(t.texto.trim_end());
    }
    let datos: Vec<u8> = b.f.iter().flat_map(|v| v.to_le_bytes()).collect();
    PaginaOffice {
        indice,
        ancho: p.ancho,
        alto: p.alto,
        fondo: p.fondo.as_ref().map(|r| rgba(r.color_medio()).to_vec()),
        datos,
        textos: b.textos,
        letras: b.letras,
        imagenes: b.imagenes,
        enlaces,
        texto_plano,
    }
}

/// **Un Word o un PowerPoint abierto** para leerlo por paginas. Abrir solo
/// lee el ZIP y los estilos; cada pagina se maqueta cuando se pide (Word, en
/// orden). Bloqueante: Swift lo usa fuera del hilo principal.
#[derive(uniffi::Object)]
pub struct DocumentoOffice {
    documento: Mutex<Documento>,
    medidor: MedidorConCache,
}

#[uniffi::export]
impl DocumentoOffice {
    /// Abre `ruta` (`.docx`, `.pptx` y sus parientes).
    #[uniffi::constructor]
    pub fn abrir(
        ruta: String,
        medidor: Box<dyn MedidorLetras>,
    ) -> Result<Arc<DocumentoOffice>, ErrorNucleo> {
        let d =
            Documento::abrir(std::path::Path::new(&ruta)).map_err(|e| ErrorNucleo::Argumento {
                detalle: format!("no se abre como Office: {e}"),
            })?;
        Ok(Arc::new(DocumentoOffice {
            documento: Mutex::new(d),
            medidor: MedidorConCache {
                swift: medidor,
                cache: Mutex::new(Cache::default()),
            },
        }))
    }

    /// Si es un PowerPoint (cada diapositiva, una hoja).
    pub fn es_presentacion(&self) -> bool {
        self.tomar().tipo() == pixpin_office::Tipo::PowerPoint
    }

    /// Cuantas paginas hay (Word: la estimacion de Word hasta maquetar todo).
    pub fn total(&self) -> u32 {
        u32::try_from(self.tomar().total()).unwrap_or(u32::MAX)
    }

    /// Si ya se sabe el total exacto.
    pub fn terminado(&self) -> bool {
        self.tomar().terminado()
    }

    /// **La pagina `n`** (desde 0), maquetandola si hace falta (en Word, con
    /// las anteriores). `None` si no existe.
    pub fn pagina(&self, n: u32) -> Option<PaginaOffice> {
        let mut d = self.tomar();
        let p = d.pagina(n as usize, &self.medidor)?;
        let total = d.terminado().then(|| d.total());
        Some(empaquetar(n, &p, total))
    }

    /// Los bytes de una imagen de una pagina (PNG, JPEG, EMF... tal cual).
    pub fn imagen(&self, id: String) -> Option<Vec<u8>> {
        self.tomar().recursos().bytes(&id).map(|b| b.to_vec())
    }
}

impl DocumentoOffice {
    pub(crate) fn tomar(&self) -> std::sync::MutexGuard<'_, Documento> {
        self.documento.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Si el nombre es de los que se paginan aqui (`.docx`, `.pptx`...).
#[uniffi::export]
pub fn es_documento_office(nombre: String) -> bool {
    pixpin_office::tipo_de(&nombre).is_some()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::io::Write;

    /// Un medidor de ancho fijo, como `MedidorFijo` de las pruebas de
    /// `pixpin-office`, que cuenta cuantas veces le preguntan.
    struct Fijo(std::sync::atomic::AtomicUsize);

    impl MedidorLetras for Fijo {
        fn avances(&self, caracteres: String, _: String, _: bool, _: bool) -> Vec<f32> {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
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

    /// Un `.docx` minimo: dos parrafos.
    fn docx(ruta: &std::path::Path) {
        let f = std::fs::File::create(ruta).unwrap();
        let mut z = zip::ZipWriter::new(f);
        let o: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default();
        z.start_file("[Content_Types].xml", o).unwrap();
        z.write_all(br#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#).unwrap();
        z.start_file("_rels/.rels", o).unwrap();
        z.write_all(br#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#).unwrap();
        z.start_file("word/document.xml", o).unwrap();
        z.write_all(br#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Hola mundo</w:t></w:r></w:p><w:p><w:r><w:t>Segundo parrafo</w:t></w:r></w:p><w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1417" w:right="1701" w:bottom="1417" w:left="1701"/></w:sectPr></w:body></w:document>"#).unwrap();
        z.finish().unwrap();
    }

    #[test]
    fn un_word_se_pagina_y_cruza_en_un_bufer() {
        let d = std::env::temp_dir().join(format!("pixpin-nucleo-office-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let ruta = d.join("acta.docx");
        docx(&ruta);
        assert!(es_documento_office("acta.docx".into()));
        assert!(!es_documento_office("acta.pdf".into()));
        let medidor = Box::new(Fijo(Default::default()));
        let doc = DocumentoOffice::abrir(ruta.display().to_string(), medidor).unwrap();
        assert!(!doc.es_presentacion());
        let p = doc.pagina(0).expect("la primera pagina");
        // A4 en puntos.
        assert!((p.ancho - 595.3).abs() < 1.0, "{}", p.ancho);
        assert!((p.alto - 841.9).abs() < 1.0, "{}", p.alto);
        assert!(p.texto_plano.contains("Hola mundo"), "{}", p.texto_plano);
        assert!(p.textos.iter().any(|t| t.contains("Hola")));
        // El bufer empieza por su version y es de f32 enteros.
        assert_eq!(p.datos.len() % 4, 0);
        assert_eq!(f32::from_le_bytes(p.datos[0..4].try_into().unwrap()), 1.0);
        assert!(doc.pagina(40).is_none());
        // Caso negativo: un fichero que no es Office no se abre.
        let falso = d.join("falso.docx");
        std::fs::write(&falso, b"no soy un zip").unwrap();
        assert!(
            DocumentoOffice::abrir(
                falso.display().to_string(),
                Box::new(Fijo(Default::default()))
            )
            .is_err()
        );
    }
}
