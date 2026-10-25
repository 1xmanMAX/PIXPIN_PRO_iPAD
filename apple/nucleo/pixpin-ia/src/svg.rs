//! **Ilustraciones por SVG** (`dibujar_svg`): el modelo escribe un SVG y aqui
//! se convierte en figuras normales del lienzo —manchas rellenas con sus
//! agujeros, lineas, rectangulos, elipses y textos—, editables y
//! sincronizables como cualquier trazo.
//!
//! ## Por que SVG
//!
//! Un arbol en isometrico, una persona o un perro no se dibujan bien con
//! rectangulos y elipses sueltos: hacen falta curvas. Los modelos de la nube
//! escriben SVG muy bien (lo han visto millones de veces) y uno local, al
//! menos uno sencillo. Asi el asistente puede hacer ilustraciones de verdad
//! sin un formato propio que aprender.
//!
//! ## Lo que se entiende
//!
//! `path` (M L H V C S Q T A Z, absolutos y relativos: las Bezier y los arcos
//! se convierten en polilineas), `rect` (con esquinas redondeadas), `circle`,
//! `ellipse`, `line`, `polyline`, `polygon`, `text`/`tspan`, `g` y `svg`
//! anidados con `transform` (matrix, translate, scale, rotate, skewX/Y),
//! `use` de un `#id` del mismo SVG, `<style>` con reglas de clase, de tipo y
//! de id, y los degradados como su color medio. `fill`, `stroke`,
//! `stroke-width`, `opacity`, `fill-opacity`, `stroke-dasharray`,
//! `font-size`, `text-anchor`, `display:none`.
//!
//! ## Saneado y limites
//!
//! Lo que llega es texto de un modelo: se trata como hostil. Sin DTD ni
//! entidades (roxmltree las rechaza: nada de bombas de XML), con un tope de
//! nodos, de bytes, de figuras, de puntos y de anidamiento; `script`,
//! `foreignObject`, `image` y lo que apunte fuera del SVG se ignoran (nunca
//! se ejecuta ni se baja nada). Un SVG que se pasa de los topes se rechaza
//! con el motivo, para que el modelo lo simplifique.

use std::collections::HashMap;

use serde_json::{Map, Value, json};

use crate::herramientas::{ALTO_LIENZO, ANCHO_LIENZO, ErrorHerramienta, Herramienta};
use crate::lienzo::Fabrica;

pub const NOMBRE: &str = "dibujar_svg";
/// Lo mas largo que se acepta un SVG.
pub const MAX_BYTES: usize = 200_000;
/// Cuantas figuras como mucho salen de un SVG.
pub const MAX_FIGURAS: usize = 400;
/// Cuantos puntos como mucho entre todas.
pub const MAX_PUNTOS: usize = 60_000;
/// Lo mas hondo que se anidan grupos y `use`.
pub const MAX_PROFUNDIDAD: usize = 32;
/// Cuantos nodos de XML como mucho.
const MAX_NODOS: u32 = 20_000;

fn mal(m: impl Into<String>) -> ErrorHerramienta {
    ErrorHerramienta::Parametro(m.into())
}

pub fn herramienta() -> Herramienta {
    Herramienta {
        nombre: NOMBRE.into(),
        descripcion: concat!(
            "Dibuja en el lienzo una ilustracion escrita en SVG: path (con curvas C, Q y arcos A), rect, circle, ellipse, line, ",
            "polyline, polygon, text, g con transform, fill, stroke, stroke-width, opacity. Sale como figuras del lienzo, editables. ",
            "Usala para dibujos con curvas o detalle: personas, animales, arboles, objetos, vistas isometricas. ",
            "Pon viewBox; sin scripts ni imagenes; como mucho 400 figuras. «trazo»: liso (por omision), a_mano, lapiz o grafito (con «dureza»). ",
            "Ejemplo: {\"svg\":\"<svg viewBox='0 0 100 100'><circle cx='50' cy='40' r='30' fill='#2f9e44'/><rect x='45' y='65' width='10' height='30' fill='#8b5a2b'/></svg>\"}"
        )
        .into(),
        parametros: json!({
            "type": "object",
            "required": ["svg"],
            "properties": {
                "svg": {"type": "string", "maxLength": MAX_BYTES, "description": "el SVG entero, de <svg> a </svg>"},
                "caja": {"type": "array", "items": {"type": "number"}, "description": "[x, y, ancho, alto] del lienzo 800x600 donde encajarlo; omitir para el centro"},
                "trazo": {"type": "string", "enum": ["liso", "a_mano", "lapiz", "grafito"]},
                "dureza": {"type": "string", "enum": ["4H", "2H", "HB", "2B", "4B", "6B", "8B"]}
            }
        }),
    }
}

/// Como se trazan las lineas del dibujo.
#[derive(Debug, Clone, PartialEq)]
pub enum Modo {
    /// Liso, como un dibujo vectorial (rugosidad 0).
    Liso,
    /// Con el temblor de la mano de Excalidraw.
    AMano,
    /// Las lineas a lapiz (`freedraw`) con su material, y los rellenos como
    /// sombreado suave.
    Lapiz {
        material: &'static str,
        dureza: Option<&'static str>,
    },
}

/// Con que se pinta una pieza, ya en el lienzo.
#[derive(Debug, Clone, PartialEq)]
pub struct Pintura {
    pub relleno: Option<String>,
    pub trazo: Option<String>,
    /// En pixeles del lienzo.
    pub grosor: f64,
    /// De 0 a 100.
    pub opacidad: f64,
    pub discontinuo: bool,
}

/// Una pieza del dibujo, ya en coordenadas del lienzo de 800x600.
#[derive(Debug, Clone, PartialEq)]
pub enum Pieza {
    /// Un rectangulo o una elipse derechos.
    Caja {
        tipo: &'static str,
        caja: [f64; 4],
        redondo: bool,
        pinta: Pintura,
    },
    /// Uno o varios anillos (los subcaminos de un `path`), con si cada uno
    /// esta cerrado.
    Forma {
        anillos: Vec<(Vec<(f64, f64)>, bool)>,
        pinta: Pintura,
    },
    Texto {
        x: f64,
        y: f64,
        texto: String,
        tam: f64,
        alinear: &'static str,
        color: String,
        opacidad: f64,
    },
}

/// Un SVG leido, saneado y encajado.
#[derive(Debug, Clone, PartialEq)]
pub struct Svg {
    pub piezas: Vec<Pieza>,
    pub modo: Modo,
}

// ---------------------------------------------------------------------------
// Validar: leer, sanear, encajar.
// ---------------------------------------------------------------------------

pub fn validar(
    args: &Map<String, Value>,
    avisos: &mut Vec<String>,
) -> Result<Svg, ErrorHerramienta> {
    let texto = match args.get("svg") {
        Some(Value::String(s)) => s.as_str(),
        _ => return Err(mal("falta «svg» (el dibujo en SVG, de <svg> a </svg>)")),
    };
    let caja = match args.get("caja").filter(|v| !v.is_null()) {
        None => None,
        Some(Value::Array(l)) => {
            let n: Vec<f64> = l
                .iter()
                .filter_map(Value::as_f64)
                .filter(|v| v.is_finite())
                .collect();
            match n.as_slice() {
                [x, y, w, h]
                    if *w >= 10.0
                        && *h >= 10.0
                        && x.abs() < 2000.0
                        && y.abs() < 2000.0
                        && *w < 4000.0
                        && *h < 4000.0 =>
                {
                    Some([*x, *y, *w, *h])
                }
                _ => {
                    return Err(mal(
                        "«caja» tiene que ser [x, y, ancho, alto] con ancho y alto de 10 o mas",
                    ));
                }
            }
        }
        Some(_) => return Err(mal("«caja» tiene que ser [x, y, ancho, alto]")),
    };
    let dureza = args
        .get("dureza")
        .and_then(Value::as_str)
        .and_then(crate::herramientas::dureza);
    let modo = match args
        .get("trazo")
        .and_then(Value::as_str)
        .map(crate::acciones::normalizar)
        .as_deref()
    {
        None | Some("liso" | "vectorial" | "limpio") => Modo::Liso,
        Some("a_mano" | "a mano" | "mano" | "boceto") => Modo::AMano,
        Some("lapiz" | "lapiz2b") => Modo::Lapiz {
            material: if dureza.is_some() {
                "cuadritos"
            } else {
                "lapiz2b"
            },
            dureza,
        },
        Some("grafito" | "cuadritos") => Modo::Lapiz {
            material: "cuadritos",
            dureza,
        },
        Some(otro) => {
            return Err(mal(format!(
                "«trazo» = «{otro}» no existe; usa liso, a_mano, lapiz o grafito"
            )));
        }
    };
    let piezas = leer(
        texto,
        caja.unwrap_or([60.0, 40.0, ANCHO_LIENZO - 120.0, ALTO_LIENZO - 80.0]),
        avisos,
    )?;
    Ok(Svg { piezas, modo })
}

/// **Lee un SVG** y lo encaja en `destino` (del lienzo de 800x600),
/// centrado y sin deformarlo.
pub fn leer(
    texto: &str,
    destino: [f64; 4],
    avisos: &mut Vec<String>,
) -> Result<Vec<Pieza>, ErrorHerramienta> {
    if texto.len() > MAX_BYTES {
        return Err(mal(format!(
            "el SVG ocupa {} KB; el maximo es {} KB: simplificalo (menos puntos, menos figuras)",
            texto.len() / 1024,
            MAX_BYTES / 1024
        )));
    }
    // Lo de alrededor (vallas de Markdown, una frase) se quita.
    let desde = texto
        .find("<svg")
        .ok_or_else(|| mal("no hay ningun <svg> en «svg»"))?;
    let hasta = texto.rfind("</svg>").map(|i| i + 6).unwrap_or(texto.len());
    if hasta <= desde {
        return Err(mal("el SVG no se cierra con </svg>"));
    }
    let trozo = &texto[desde..hasta];
    let opciones = roxmltree::ParsingOptions {
        allow_dtd: false,
        nodes_limit: MAX_NODOS,
    };
    let doc = roxmltree::Document::parse_with_options(trozo, opciones).map_err(|e| {
        mal(format!(
            "el SVG no es XML valido ({e}); sin DOCTYPE ni entidades"
        ))
    })?;
    let raiz = doc.root_element();
    if raiz.tag_name().name() != "svg" {
        return Err(mal("el elemento de fuera tiene que ser <svg>"));
    }
    let estilos = Hojas::de(&doc);
    let degradados = degradados(&doc);
    let ids: HashMap<&str, roxmltree::Node> = doc
        .descendants()
        .filter_map(|n| n.attribute("id").map(|i| (i, n)))
        .collect();
    // El rectangulo del dibujo: el viewBox, o el ancho y alto, o lo que
    // ocupe lo dibujado.
    let vista = raiz
        .attribute("viewBox")
        .and_then(|v| {
            let n = numeros_sueltos(v);
            (n.len() == 4 && n[2] > 0.0 && n[3] > 0.0).then(|| [n[0], n[1], n[2], n[3]])
        })
        .or_else(|| {
            let w = raiz.attribute("width").and_then(longitud)?;
            let h = raiz.attribute("height").and_then(longitud)?;
            (w > 0.0 && h > 0.0).then_some([0.0, 0.0, w, h])
        });
    let mut lector = Recorrido {
        estilos: &estilos,
        degradados: &degradados,
        ids: &ids,
        piezas: Vec::new(),
        puntos: 0,
        ignorados: Vec::new(),
        detalle: 1.0 / 4.0,
    };
    // El detalle de las curvas depende de cuanto se agranda el dibujo: con
    // la vista se sabe ya; sin ella, se recorre una vez para medirlo.
    let escala_de = |v: [f64; 4]| (destino[2] / v[2]).min(destino[3] / v[3]);
    let vista = match vista {
        Some(v) => v,
        None => {
            lector.recorrer(raiz, Matriz::IDENTIDAD, &Estilo::raiz(), 0)?;
            let c = caja_de(&lector.piezas).ok_or_else(|| mal("el SVG no dibuja nada visible"))?;
            lector.piezas.clear();
            lector.puntos = 0;
            [c[0], c[1], c[2].max(1.0), c[3].max(1.0)]
        }
    };
    let escala = escala_de(vista);
    if !escala.is_finite() || escala <= 0.0 {
        return Err(mal("el viewBox del SVG no tiene medida"));
    }
    // Un segmento cada ~4 px del lienzo.
    lector.detalle = escala / 4.0;
    lector.recorrer(raiz, Matriz::IDENTIDAD, &Estilo::raiz(), 0)?;
    if !lector.ignorados.is_empty() {
        lector.ignorados.sort();
        lector.ignorados.dedup();
        avisos.push(format!(
            "del SVG se ignoro: {}",
            lector.ignorados.join(", ")
        ));
    }
    if lector.piezas.is_empty() {
        return Err(mal(
            "el SVG no dibuja nada visible (¿todo con fill=\"none\" y sin stroke?)",
        ));
    }
    // Encajar: centrado en el destino.
    let (ox, oy) = (
        destino[0] + (destino[2] - vista[2] * escala) / 2.0 - vista[0] * escala,
        destino[1] + (destino[3] - vista[3] * escala) / 2.0 - vista[1] * escala,
    );
    let a = |(x, y): (f64, f64)| (ox + x * escala, oy + y * escala);
    let tolerancia = 0.35;
    let piezas = lector
        .piezas
        .into_iter()
        .map(|p| match p {
            Pieza::Caja {
                tipo,
                caja: [x, y, w, h],
                redondo,
                mut pinta,
            } => {
                pinta.grosor = (pinta.grosor * escala).clamp(0.5, 40.0);
                let (x, y) = a((x, y));
                Pieza::Caja {
                    tipo,
                    caja: [x, y, w * escala, h * escala],
                    redondo,
                    pinta,
                }
            }
            Pieza::Forma { anillos, mut pinta } => {
                pinta.grosor = (pinta.grosor * escala).clamp(0.5, 40.0);
                let anillos = anillos
                    .into_iter()
                    .map(|(pts, cerrado)| {
                        (
                            simplificar(&pts.into_iter().map(a).collect::<Vec<_>>(), tolerancia),
                            cerrado,
                        )
                    })
                    .filter(|(pts, _)| pts.len() >= 2)
                    .collect();
                Pieza::Forma { anillos, pinta }
            }
            Pieza::Texto {
                x,
                y,
                texto,
                tam,
                alinear,
                color,
                opacidad,
            } => {
                let (x, y) = a((x, y));
                Pieza::Texto {
                    x,
                    y,
                    texto,
                    tam: (tam * escala).clamp(4.0, 400.0),
                    alinear,
                    color,
                    opacidad,
                }
            }
        })
        .filter(|p| !matches!(p, Pieza::Forma { anillos, .. } if anillos.is_empty()))
        .collect();
    Ok(piezas)
}

/// La caja de unas piezas (sin encajar).
fn caja_de(piezas: &[Pieza]) -> Option<[f64; 4]> {
    let mut c: Option<(f64, f64, f64, f64)> = None;
    let mut meter = |x: f64, y: f64| {
        c = Some(match c {
            None => (x, y, x, y),
            Some((a, b, d, e)) => (a.min(x), b.min(y), d.max(x), e.max(y)),
        });
    };
    for p in piezas {
        match p {
            Pieza::Caja {
                caja: [x, y, w, h], ..
            } => {
                meter(*x, *y);
                meter(x + w, y + h);
            }
            Pieza::Forma { anillos, .. } => anillos
                .iter()
                .flat_map(|(a, _)| a)
                .for_each(|&(x, y)| meter(x, y)),
            Pieza::Texto {
                x, y, tam, texto, ..
            } => {
                meter(*x, *y);
                meter(x + tam * 0.6 * texto.chars().count() as f64, y + tam);
            }
        }
    }
    c.map(|(a, b, d, e)| [a, b, d - a, e - b])
}

// ---------------------------------------------------------------------------
// Matrices, numeros, colores
// ---------------------------------------------------------------------------

/// Una transformacion afin `[a b c d e f]` de SVG.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Matriz([f64; 6]);

impl Matriz {
    const IDENTIDAD: Matriz = Matriz([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);

    /// `self` despues de `otra` (la de dentro se aplica primero).
    fn por(self, otra: Matriz) -> Matriz {
        let [a, b, c, d, e, f] = self.0;
        let [g, h, i, j, k, l] = otra.0;
        Matriz([
            a * g + c * h,
            b * g + d * h,
            a * i + c * j,
            b * i + d * j,
            a * k + c * l + e,
            b * k + d * l + f,
        ])
    }

    fn aplicar(&self, (x, y): (f64, f64)) -> (f64, f64) {
        let [a, b, c, d, e, f] = self.0;
        (a * x + c * y + e, b * x + d * y + f)
    }

    /// Cuanto agranda (para el grosor de las lineas).
    fn escala(&self) -> f64 {
        let [a, b, c, d, ..] = self.0;
        (a * d - b * c).abs().sqrt()
    }

    /// Sin giro ni sesgo: un rectangulo sigue siendo un rectangulo.
    fn derecha(&self) -> bool {
        self.0[1].abs() < 1e-9 && self.0[2].abs() < 1e-9 && self.0[0] > 0.0 && self.0[3] > 0.0
    }

    fn trasladar(x: f64, y: f64) -> Matriz {
        Matriz([1.0, 0.0, 0.0, 1.0, x, y])
    }
}

/// `transform="translate(10 20) rotate(30)"`. Lo que no se entiende se
/// ignora (la identidad).
fn transformacion(t: &str) -> Matriz {
    let mut m = Matriz::IDENTIDAD;
    let mut resto = t;
    while let Some(abre) = resto.find('(') {
        let nombre = resto[..abre]
            .trim_matches(|c: char| c.is_whitespace() || c == ',')
            .to_ascii_lowercase();
        let Some(cierra) = resto[abre..].find(')') else {
            break;
        };
        let n = numeros_sueltos(&resto[abre + 1..abre + cierra]);
        let r = |g: f64| g.to_radians();
        let otra = match (nombre.as_str(), n.as_slice()) {
            ("matrix", [a, b, c, d, e, f]) => Matriz([*a, *b, *c, *d, *e, *f]),
            ("translate", [x]) => Matriz::trasladar(*x, 0.0),
            ("translate", [x, y]) => Matriz::trasladar(*x, *y),
            ("scale", [s]) => Matriz([*s, 0.0, 0.0, *s, 0.0, 0.0]),
            ("scale", [sx, sy]) => Matriz([*sx, 0.0, 0.0, *sy, 0.0, 0.0]),
            ("rotate", [g]) => Matriz([
                r(*g).cos(),
                r(*g).sin(),
                -r(*g).sin(),
                r(*g).cos(),
                0.0,
                0.0,
            ]),
            ("rotate", [g, cx, cy]) => Matriz::trasladar(*cx, *cy)
                .por(Matriz([
                    r(*g).cos(),
                    r(*g).sin(),
                    -r(*g).sin(),
                    r(*g).cos(),
                    0.0,
                    0.0,
                ]))
                .por(Matriz::trasladar(-cx, -cy)),
            ("skewx", [g]) => Matriz([1.0, 0.0, r(*g).tan(), 1.0, 0.0, 0.0]),
            ("skewy", [g]) => Matriz([1.0, r(*g).tan(), 0.0, 1.0, 0.0, 0.0]),
            _ => Matriz::IDENTIDAD,
        };
        if otra.0.iter().all(|v| v.is_finite()) {
            m = m.por(otra);
        }
        resto = &resto[abre + cierra + 1..];
    }
    m
}

/// Los numeros de una lista de SVG (`"1,2 3-4.5e1.5"`).
fn numeros_sueltos(s: &str) -> Vec<f64> {
    let mut l = Lector {
        b: s.as_bytes(),
        i: 0,
    };
    let mut v = Vec::new();
    while let Some(n) = l.numero() {
        v.push(n);
    }
    v
}

/// Una longitud: `12`, `12px`, `1.5em` (a 16 px), `50%` no (no se sabe de que).
fn longitud(s: &str) -> Option<f64> {
    let t = s.trim();
    let (num, k) = if let Some(n) = t.strip_suffix("px") {
        (n, 1.0)
    } else if let Some(n) = t.strip_suffix("pt") {
        (n, 4.0 / 3.0)
    } else if let Some(n) = t.strip_suffix("em") {
        (n, 16.0)
    } else if t.ends_with('%') {
        return None;
    } else {
        (t, 1.0)
    };
    num.trim()
        .parse::<f64>()
        .ok()
        .map(|v| v * k)
        .filter(|v| v.is_finite())
}

/// Lee numeros y banderas de los datos de un `path`.
struct Lector<'a> {
    b: &'a [u8],
    i: usize,
}

impl Lector<'_> {
    fn saltar(&mut self) {
        while self.i < self.b.len()
            && (self.b[self.i].is_ascii_whitespace() || self.b[self.i] == b',')
        {
            self.i += 1;
        }
    }

    fn numero(&mut self) -> Option<f64> {
        self.saltar();
        let ini = self.i;
        let b = self.b;
        let mut i = self.i;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        let mut digitos = 0;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
            digitos += 1;
        }
        if i < b.len() && b[i] == b'.' {
            i += 1;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
                digitos += 1;
            }
        }
        if digitos == 0 {
            return None;
        }
        if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
            let mut j = i + 1;
            if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
                j += 1;
            }
            if j < b.len() && b[j].is_ascii_digit() {
                while j < b.len() && b[j].is_ascii_digit() {
                    j += 1;
                }
                i = j;
            }
        }
        let v: f64 = std::str::from_utf8(&b[ini..i]).ok()?.parse().ok()?;
        self.i = i;
        v.is_finite().then_some(v)
    }

    /// Las banderas de un arco: un solo `0` o `1`, pegado o no.
    fn bandera(&mut self) -> Option<bool> {
        self.saltar();
        match self.b.get(self.i) {
            Some(b'0') => {
                self.i += 1;
                Some(false)
            }
            Some(b'1') => {
                self.i += 1;
                Some(true)
            }
            _ => None,
        }
    }

    fn comando(&mut self) -> Option<u8> {
        self.saltar();
        let c = *self.b.get(self.i)?;
        if c.is_ascii_alphabetic() && c != b'e' && c != b'E' {
            self.i += 1;
            Some(c)
        } else {
            None
        }
    }

    fn fin(&mut self) -> bool {
        self.saltar();
        self.i >= self.b.len()
    }
}

/// Un color de SVG a `#rrggbb`, o `None` si es `none`.
fn color_svg(
    t: &str,
    actual: &str,
    degradados: &HashMap<String, String>,
) -> Option<Option<String>> {
    let t = t.trim();
    let l = t.to_ascii_lowercase();
    if l == "none" || l == "transparent" {
        return Some(None);
    }
    if l == "currentcolor" {
        return Some(Some(actual.to_string()));
    }
    if let Some(id) = l.strip_prefix("url(") {
        let id = id
            .trim_end_matches(')')
            .trim()
            .trim_matches(|c| c == '\'' || c == '"')
            .trim_start_matches('#');
        // Un degradado: su color medio. Uno que no existe: lo que diga la
        // reserva que va detras (`url(#a) red`), o nada.
        if let Some(c) = degradados.get(id) {
            return Some(Some(c.clone()));
        }
        let reserva = t.split(')').nth(1).map(str::trim).filter(|s| !s.is_empty());
        return reserva.and_then(|r| color_svg(r, actual, degradados));
    }
    if let Some(dentro) = l.strip_prefix("rgb(").or_else(|| l.strip_prefix("rgba(")) {
        let partes: Vec<&str> = dentro
            .trim_end_matches(')')
            .split([',', ' ', '/'])
            .filter(|s| !s.is_empty())
            .collect();
        if partes.len() < 3 {
            return None;
        }
        let canal = |s: &str| -> Option<u8> {
            if let Some(p) = s.strip_suffix('%') {
                p.trim()
                    .parse::<f64>()
                    .ok()
                    .map(|v| (v.clamp(0.0, 100.0) * 2.55).round() as u8)
            } else {
                s.trim()
                    .parse::<f64>()
                    .ok()
                    .map(|v| v.clamp(0.0, 255.0).round() as u8)
            }
        };
        let (r, g, b) = (canal(partes[0])?, canal(partes[1])?, canal(partes[2])?);
        return Some(Some(format!("#{r:02x}{g:02x}{b:02x}")));
    }
    if l.starts_with('#') {
        return crate::herramientas::color(&l).map(|c| Some(c.0));
    }
    let hex = match l.as_str() {
        "black" => "#000000",
        "white" => "#ffffff",
        "red" => "#ff0000",
        "green" => "#008000",
        "blue" => "#0000ff",
        "yellow" => "#ffff00",
        "orange" => "#ffa500",
        "purple" => "#800080",
        "pink" => "#ffc0cb",
        "gray" | "grey" => "#808080",
        "brown" => "#a52a2a",
        "cyan" | "aqua" => "#00ffff",
        "magenta" | "fuchsia" => "#ff00ff",
        "lime" => "#00ff00",
        "navy" => "#000080",
        "teal" => "#008080",
        "olive" => "#808000",
        "maroon" => "#800000",
        "silver" => "#c0c0c0",
        "gold" => "#ffd700",
        "beige" => "#f5f5dc",
        "tan" => "#d2b48c",
        "khaki" => "#f0e68c",
        "coral" => "#ff7f50",
        "salmon" => "#fa8072",
        "tomato" => "#ff6347",
        "crimson" => "#dc143c",
        "indigo" => "#4b0082",
        "violet" => "#ee82ee",
        "orchid" => "#da70d6",
        "plum" => "#dda0dd",
        "lavender" => "#e6e6fa",
        "turquoise" => "#40e0d0",
        "skyblue" => "#87ceeb",
        "lightblue" => "#add8e6",
        "steelblue" => "#4682b4",
        "royalblue" => "#4169e1",
        "darkblue" => "#00008b",
        "darkgreen" => "#006400",
        "forestgreen" => "#228b22",
        "seagreen" => "#2e8b57",
        "mediumseagreen" => "#3cb371",
        "lightgreen" => "#90ee90",
        "palegreen" => "#98fb98",
        "yellowgreen" => "#9acd32",
        "olivedrab" => "#6b8e23",
        "darkolivegreen" => "#556b2f",
        "springgreen" => "#00ff7f",
        "lightgray" | "lightgrey" => "#d3d3d3",
        "darkgray" | "darkgrey" => "#a9a9a9",
        "dimgray" | "dimgrey" => "#696969",
        "slategray" | "slategrey" => "#708090",
        "gainsboro" => "#dcdcdc",
        "whitesmoke" => "#f5f5f5",
        "snow" => "#fffafa",
        "ivory" => "#fffff0",
        "wheat" => "#f5deb3",
        "saddlebrown" => "#8b4513",
        "sienna" => "#a0522d",
        "chocolate" => "#d2691e",
        "peru" => "#cd853f",
        "goldenrod" => "#daa520",
        "darkorange" => "#ff8c00",
        "darkred" => "#8b0000",
        "firebrick" => "#b22222",
        "hotpink" => "#ff69b4",
        "deeppink" => "#ff1493",
        "lightpink" => "#ffb6c1",
        "lightyellow" => "#ffffe0",
        _ => return crate::herramientas::color(&l).map(|c| Some(c.0)),
    };
    Some(Some(hex.to_string()))
}

/// El color medio de cada degradado (`linearGradient`, `radialGradient`),
/// por su id. Los que heredan sus paradas con `href`, tambien.
fn degradados(doc: &roxmltree::Document) -> HashMap<String, String> {
    let mut paradas: HashMap<String, Vec<(f64, f64, f64)>> = HashMap::new();
    let mut herencia: Vec<(String, String)> = Vec::new();
    for n in doc
        .descendants()
        .filter(|n| matches!(n.tag_name().name(), "linearGradient" | "radialGradient"))
    {
        let Some(id) = n.attribute("id") else {
            continue;
        };
        let mut lista = Vec::new();
        for s in n.children().filter(|c| c.tag_name().name() == "stop") {
            let mut c = s.attribute("stop-color").map(str::to_string);
            if let Some(st) = s.attribute("style") {
                for (k, v) in declaraciones(st) {
                    if k == "stop-color" {
                        c = Some(v);
                    }
                }
            }
            if let Some(Some(hex)) = c
                .as_deref()
                .and_then(|c| color_svg(c, "#000000", &HashMap::new()))
            {
                let h = hex.trim_start_matches('#');
                let v = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).unwrap_or(0) as f64;
                lista.push((v(0), v(2), v(4)));
            }
        }
        if lista.is_empty() {
            if let Some(h) = href(&n) {
                herencia.push((id.to_string(), h.to_string()));
            }
        }
        paradas.insert(id.to_string(), lista);
    }
    for (id, de) in herencia {
        if let Some(l) = paradas.get(&de).cloned() {
            paradas.insert(id, l);
        }
    }
    paradas
        .into_iter()
        .filter(|(_, l)| !l.is_empty())
        .map(|(id, l)| {
            let n = l.len() as f64;
            let (r, g, b) = l
                .iter()
                .fold((0.0, 0.0, 0.0), |a, c| (a.0 + c.0, a.1 + c.1, a.2 + c.2));
            (
                id,
                format!(
                    "#{:02x}{:02x}{:02x}",
                    (r / n) as u8,
                    (g / n) as u8,
                    (b / n) as u8
                ),
            )
        })
        .collect()
}

/// `href` o `xlink:href`, solo si apunta dentro del SVG (`#id`).
fn href<'a>(n: &roxmltree::Node<'a, '_>) -> Option<&'a str> {
    n.attribute("href")
        .or_else(|| n.attribute(("http://www.w3.org/1999/xlink", "href")))
        .and_then(|h| h.trim().strip_prefix('#'))
}

/// `a: b; c: d` en pares.
fn declaraciones(s: &str) -> Vec<(String, String)> {
    s.split(';')
        .filter_map(|d| {
            let (k, v) = d.split_once(':')?;
            Some((
                k.trim().to_ascii_lowercase(),
                v.trim().trim_end_matches("!important").trim().to_string(),
            ))
        })
        .filter(|(k, v)| !k.is_empty() && !v.is_empty())
        .collect()
}

// ---------------------------------------------------------------------------
// <style>: reglas de tipo, de clase y de id
// ---------------------------------------------------------------------------

/// Las reglas de los `<style>`, en orden y con su peso (tipo 1, clase 10,
/// id 100), para aplicarlas de menos a mas.
struct Hojas {
    reglas: Vec<Regla>,
}

/// Una regla de estilo: su especificidad, el selector y sus propiedades.
type Regla = (u32, Selector, Vec<(String, String)>);

struct Selector {
    tipo: Option<String>,
    clases: Vec<String>,
    id: Option<String>,
}

impl Hojas {
    fn de(doc: &roxmltree::Document) -> Hojas {
        let mut reglas = Vec::new();
        for st in doc.descendants().filter(|n| n.tag_name().name() == "style") {
            let css: String = st.descendants().filter_map(|t| t.text()).collect();
            // Sin comentarios.
            let mut limpio = String::new();
            let mut resto = css.as_str();
            while let Some(i) = resto.find("/*") {
                limpio.push_str(&resto[..i]);
                resto = resto[i..].find("*/").map_or("", |j| &resto[i + j + 2..]);
            }
            limpio.push_str(resto);
            for bloque in limpio.split('}') {
                let Some((sel, decl)) = bloque.split_once('{') else {
                    continue;
                };
                let decl = declaraciones(decl);
                for s in sel.split(',') {
                    let s = s.trim();
                    // Solo selectores simples (`g.hoja`, `.tronco`, `#cielo`).
                    if s.is_empty()
                        || s.contains([' ', '>', '+', '~', ':', '['])
                        || s.starts_with('@')
                    {
                        continue;
                    }
                    let mut sel = Selector {
                        tipo: None,
                        clases: Vec::new(),
                        id: None,
                    };
                    let mut peso = 0;
                    // Partir en `tipo`, `.clase` y `#id`.
                    let mut actual = String::new();
                    let mut clase_de = ' ';
                    for c in s.chars().chain(std::iter::once('\0')) {
                        if c == '.' || c == '#' || c == '\0' {
                            if !actual.is_empty() {
                                match clase_de {
                                    '.' => {
                                        sel.clases.push(actual.clone());
                                        peso += 10;
                                    }
                                    '#' => {
                                        sel.id = Some(actual.clone());
                                        peso += 100;
                                    }
                                    _ if actual != "*" => {
                                        sel.tipo = Some(actual.clone());
                                        peso += 1;
                                    }
                                    _ => {}
                                }
                            }
                            actual.clear();
                            clase_de = c;
                        } else {
                            actual.push(c);
                        }
                    }
                    reglas.push((peso, sel, decl.clone()));
                }
            }
        }
        reglas.sort_by_key(|r| r.0);
        Hojas { reglas }
    }

    fn para<'h>(&'h self, n: &roxmltree::Node) -> impl Iterator<Item = &'h (String, String)> + 'h {
        let tipo = n.tag_name().name().to_string();
        let clases: Vec<String> = n
            .attribute("class")
            .unwrap_or("")
            .split_whitespace()
            .map(str::to_string)
            .collect();
        let id = n.attribute("id").map(str::to_string);
        self.reglas
            .iter()
            .filter(move |(_, s, _)| {
                s.tipo.as_ref().is_none_or(|t| *t == tipo)
                    && s.clases.iter().all(|c| clases.contains(c))
                    && s.id.as_ref().is_none_or(|i| Some(i) == id.as_ref())
            })
            .flat_map(|(_, _, d)| d.iter())
    }
}

// ---------------------------------------------------------------------------
// El recorrido
// ---------------------------------------------------------------------------

/// Lo que se hereda de los grupos.
#[derive(Debug, Clone)]
struct Estilo {
    relleno: Option<String>,
    trazo: Option<String>,
    grosor: f64,
    opacidad: f64,
    opacidad_relleno: f64,
    opacidad_trazo: f64,
    color: String,
    tam_letra: f64,
    ancla: &'static str,
    discontinuo: bool,
    oculto: bool,
}

impl Estilo {
    /// Lo de fabrica de SVG: relleno negro, sin trazo.
    fn raiz() -> Estilo {
        Estilo {
            relleno: Some("#000000".into()),
            trazo: None,
            grosor: 1.0,
            opacidad: 1.0,
            opacidad_relleno: 1.0,
            opacidad_trazo: 1.0,
            color: "#000000".into(),
            tam_letra: 16.0,
            ancla: "left",
            discontinuo: false,
            oculto: false,
        }
    }

    fn poner(&mut self, k: &str, v: &str, degradados: &HashMap<String, String>) {
        match k {
            "fill" => {
                if let Some(c) = color_svg(v, &self.color, degradados) {
                    self.relleno = c;
                }
            }
            "stroke" => {
                if let Some(c) = color_svg(v, &self.color, degradados) {
                    self.trazo = c;
                }
            }
            "color" => {
                if let Some(Some(c)) = color_svg(v, &self.color, degradados) {
                    self.color = c;
                }
            }
            "stroke-width" => {
                if let Some(g) = longitud(v).filter(|g| *g >= 0.0) {
                    self.grosor = g;
                }
            }
            "opacity" => {
                if let Ok(o) = v.trim().parse::<f64>() {
                    // La de un grupo multiplica a la de sus hijos.
                    self.opacidad *= o.clamp(0.0, 1.0);
                }
            }
            "fill-opacity" => {
                if let Ok(o) = v.trim().parse::<f64>() {
                    self.opacidad_relleno = o.clamp(0.0, 1.0);
                }
            }
            "stroke-opacity" => {
                if let Ok(o) = v.trim().parse::<f64>() {
                    self.opacidad_trazo = o.clamp(0.0, 1.0);
                }
            }
            "font-size" => {
                if let Some(t) = longitud(v).filter(|t| *t > 0.0) {
                    self.tam_letra = t;
                }
            }
            "text-anchor" => {
                self.ancla = match v.trim() {
                    "middle" => "center",
                    "end" => "right",
                    _ => "left",
                };
            }
            "stroke-dasharray" => self.discontinuo = !matches!(v.trim(), "none" | "0" | ""),
            "display" => self.oculto |= v.trim() == "none",
            "visibility" => self.oculto |= matches!(v.trim(), "hidden" | "collapse"),
            _ => {}
        }
    }

    fn pintura(&self, escala: f64) -> Pintura {
        // La opacidad de Excalidraw es una por elemento: la del relleno si
        // lo hay, si no la del trazo.
        let o = self.opacidad
            * if self.relleno.is_some() {
                self.opacidad_relleno
            } else {
                self.opacidad_trazo
            };
        Pintura {
            relleno: self.relleno.clone(),
            trazo: self.trazo.clone(),
            grosor: (self.grosor * escala).max(0.1),
            opacidad: (o * 100.0).round().clamp(5.0, 100.0),
            discontinuo: self.discontinuo,
        }
    }
}

const PROPIEDADES: [&str; 13] = [
    "fill",
    "stroke",
    "color",
    "stroke-width",
    "opacity",
    "fill-opacity",
    "stroke-opacity",
    "font-size",
    "text-anchor",
    "stroke-dasharray",
    "display",
    "visibility",
    "font-family",
];

struct Recorrido<'d, 'a, 'i> {
    estilos: &'d Hojas,
    degradados: &'d HashMap<String, String>,
    ids: &'d HashMap<&'a str, roxmltree::Node<'a, 'i>>,
    piezas: Vec<Pieza>,
    puntos: usize,
    ignorados: Vec<String>,
    /// Segmentos de curva por unidad del SVG.
    detalle: f64,
}

impl<'a, 'i> Recorrido<'_, 'a, 'i> {
    fn recorrer(
        &mut self,
        n: roxmltree::Node<'a, 'i>,
        m: Matriz,
        padre: &Estilo,
        hondo: usize,
    ) -> Result<(), ErrorHerramienta> {
        if hondo > MAX_PROFUNDIDAD {
            return Err(mal(format!(
                "el SVG anida mas de {MAX_PROFUNDIDAD} niveles de grupos"
            )));
        }
        let tipo = n.tag_name().name();
        match tipo {
            "script" | "foreignObject" | "image" | "iframe" | "video" | "audio" | "animate"
            | "animateTransform" | "set" | "filter" | "mask" | "pattern" => {
                self.ignorados.push(tipo.to_string());
                return Ok(());
            }
            // Lo que no se pinta por si solo.
            "defs" | "style" | "title" | "desc" | "metadata" | "linearGradient"
            | "radialGradient" | "clipPath" | "symbol" | "marker" => return Ok(()),
            _ => {}
        }
        // El estilo: lo heredado, los atributos, las reglas de <style> y el
        // `style` del elemento, por ese orden.
        // La opacidad no se hereda tal cual: la del grupo multiplica a la
        // del hijo, una sola vez.
        let mut propio = Estilo {
            opacidad: 1.0,
            ..padre.clone()
        };
        for k in PROPIEDADES {
            if let Some(v) = n.attribute(k) {
                propio.poner(k, v, self.degradados);
            }
        }
        for (k, v) in self.estilos.para(&n) {
            propio.poner(k, v, self.degradados);
        }
        if let Some(st) = n.attribute("style") {
            for (k, v) in declaraciones(st) {
                propio.poner(&k, &v, self.degradados);
            }
        }
        propio.opacidad *= padre.opacidad;
        let e = propio;
        if e.oculto {
            return Ok(());
        }
        let m = match n.attribute("transform") {
            Some(t) => m.por(transformacion(t)),
            None => m,
        };
        let num = |k: &str| n.attribute(k).and_then(longitud).unwrap_or(0.0);
        let escala = m.escala();
        match tipo {
            "svg" if hondo > 0 => {
                // Un svg anidado: su x, y como traslado.
                let m = m.por(Matriz::trasladar(num("x"), num("y")));
                for h in n.children().filter(|h| h.is_element()) {
                    self.recorrer(h, m, &e, hondo + 1)?;
                }
            }
            "svg" | "g" | "a" | "switch" => {
                for h in n.children().filter(|h| h.is_element()) {
                    self.recorrer(h, m, &e, hondo + 1)?;
                }
            }
            "use" => match href(&n).and_then(|id| self.ids.get(id)).copied() {
                Some(objetivo) if objetivo != n => {
                    let m = m.por(Matriz::trasladar(num("x"), num("y")));
                    if objetivo.tag_name().name() == "symbol" {
                        for h in objetivo.children().filter(|h| h.is_element()) {
                            self.recorrer(h, m, &e, hondo + 2)?;
                        }
                    } else {
                        self.recorrer(objetivo, m, &e, hondo + 2)?;
                    }
                }
                _ => self.ignorados.push("use externo".into()),
            },
            "rect" => {
                let (x, y, w, h) = (num("x"), num("y"), num("width"), num("height"));
                if w <= 0.0 || h <= 0.0 {
                    return Ok(());
                }
                let rx = n
                    .attribute("rx")
                    .or(n.attribute("ry"))
                    .and_then(longitud)
                    .unwrap_or(0.0);
                if m.derecha() {
                    let (x0, y0) = m.aplicar((x, y));
                    let (x1, y1) = m.aplicar((x + w, y + h));
                    self.caja(
                        "rectangle",
                        [x0, y0, x1 - x0, y1 - y0],
                        rx > 0.0,
                        e.pintura(escala),
                    )?;
                } else {
                    let anillo = vec![(x, y), (x + w, y), (x + w, y + h), (x, y + h)];
                    self.forma(
                        vec![(anillo.into_iter().map(|p| m.aplicar(p)).collect(), true)],
                        e.pintura(escala),
                    )?;
                }
            }
            "circle" | "ellipse" => {
                let (cx, cy) = (num("cx"), num("cy"));
                let (rx, ry) = if tipo == "circle" {
                    (num("r"), num("r"))
                } else {
                    (num("rx"), num("ry"))
                };
                if rx <= 0.0 || ry <= 0.0 {
                    return Ok(());
                }
                if m.derecha() {
                    let (x0, y0) = m.aplicar((cx - rx, cy - ry));
                    let (x1, y1) = m.aplicar((cx + rx, cy + ry));
                    self.caja(
                        "ellipse",
                        [x0, y0, x1 - x0, y1 - y0],
                        false,
                        e.pintura(escala),
                    )?;
                } else {
                    let pasos =
                        ((rx.max(ry) * escala * self.detalle * std::f64::consts::TAU).ceil()
                            as usize)
                            .clamp(12, 72);
                    let anillo: Vec<(f64, f64)> = (0..pasos)
                        .map(|k| {
                            let a = k as f64 / pasos as f64 * std::f64::consts::TAU;
                            m.aplicar((cx + rx * a.cos(), cy + ry * a.sin()))
                        })
                        .collect();
                    self.forma(vec![(anillo, true)], e.pintura(escala))?;
                }
            }
            "line" => {
                let a = m.aplicar((num("x1"), num("y1")));
                let b = m.aplicar((num("x2"), num("y2")));
                let mut p = e.pintura(escala);
                p.relleno = None;
                self.forma(vec![(vec![a, b], false)], p)?;
            }
            "polyline" | "polygon" => {
                let v = numeros_sueltos(n.attribute("points").unwrap_or(""));
                let pts: Vec<(f64, f64)> =
                    v.chunks_exact(2).map(|c| m.aplicar((c[0], c[1]))).collect();
                if pts.len() >= 2 {
                    let cerrado = tipo == "polygon";
                    self.forma(vec![(pts, cerrado)], e.pintura(escala))?;
                }
            }
            "path" => {
                let anillos = camino(n.attribute("d").unwrap_or(""), m, self.detalle);
                if !anillos.is_empty() {
                    self.forma(anillos, e.pintura(escala))?;
                }
            }
            "text" => {
                let texto: String = n
                    .descendants()
                    .filter(|t| t.is_text())
                    .filter_map(|t| t.text())
                    .collect::<String>()
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                if texto.is_empty() {
                    return Ok(());
                }
                // El tamano y el ancla del primer tspan si los lleva.
                let mut e = e.clone();
                if let Some(t) = n.children().find(|h| h.tag_name().name() == "tspan") {
                    for k in ["font-size", "text-anchor", "fill"] {
                        if let Some(v) = t.attribute(k) {
                            e.poner(k, v, self.degradados);
                        }
                    }
                }
                let primero = |k: &str| {
                    n.attribute(k)
                        .and_then(|v| numeros_sueltos(v).first().copied())
                        .unwrap_or(0.0)
                };
                let (x, y) = m.aplicar((primero("x"), primero("y")));
                let tam = e.tam_letra * escala;
                let color = e
                    .relleno
                    .clone()
                    .or(e.trazo.clone())
                    .unwrap_or_else(|| "#000000".into());
                self.contar(1)?;
                self.piezas.push(Pieza::Texto {
                    x,
                    // La `y` de SVG es la linea base; la del lienzo, arriba.
                    y: y - tam * 0.8,
                    texto: texto.chars().take(crate::herramientas::MAX_TEXTO).collect(),
                    tam,
                    alinear: e.ancla,
                    color,
                    opacidad: (e.opacidad * e.opacidad_relleno * 100.0)
                        .round()
                        .clamp(5.0, 100.0),
                });
            }
            otro => self.ignorados.push(otro.to_string()),
        }
        Ok(())
    }

    fn contar(&mut self, puntos: usize) -> Result<(), ErrorHerramienta> {
        self.puntos += puntos;
        if self.piezas.len() >= MAX_FIGURAS {
            return Err(mal(format!(
                "el SVG pasa de {MAX_FIGURAS} figuras: junta formas en un solo path o quita detalle"
            )));
        }
        if self.puntos > MAX_PUNTOS {
            return Err(mal(format!(
                "el SVG pasa de {MAX_PUNTOS} puntos: simplifica las curvas"
            )));
        }
        Ok(())
    }

    fn caja(
        &mut self,
        tipo: &'static str,
        caja: [f64; 4],
        redondo: bool,
        pinta: Pintura,
    ) -> Result<(), ErrorHerramienta> {
        if pinta.relleno.is_none() && pinta.trazo.is_none() || caja.iter().any(|v| !v.is_finite()) {
            return Ok(());
        }
        self.contar(4)?;
        self.piezas.push(Pieza::Caja {
            tipo,
            caja,
            redondo,
            pinta,
        });
        Ok(())
    }

    fn forma(
        &mut self,
        anillos: Vec<(Vec<(f64, f64)>, bool)>,
        pinta: Pintura,
    ) -> Result<(), ErrorHerramienta> {
        if pinta.relleno.is_none() && pinta.trazo.is_none() {
            return Ok(());
        }
        let anillos: Vec<(Vec<(f64, f64)>, bool)> = anillos
            .into_iter()
            .map(|(a, c)| {
                (
                    a.into_iter()
                        .filter(|p| p.0.is_finite() && p.1.is_finite())
                        .collect::<Vec<_>>(),
                    c,
                )
            })
            .filter(|(a, _)| a.len() >= 2)
            .collect();
        if anillos.is_empty() {
            return Ok(());
        }
        self.contar(anillos.iter().map(|a| a.0.len()).sum())?;
        self.piezas.push(Pieza::Forma { anillos, pinta });
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Los datos de un path
// ---------------------------------------------------------------------------

/// **Los subcaminos de un `d`**, ya transformados y con las curvas hechas
/// polilineas (`detalle`: segmentos por unidad, ya en el espacio de fuera).
/// Lo que no se entiende corta el camino ahi, como hacen los navegadores.
fn camino(d: &str, m: Matriz, detalle: f64) -> Vec<(Vec<(f64, f64)>, bool)> {
    let mut l = Lector {
        b: d.as_bytes(),
        i: 0,
    };
    let mut anillos: Vec<(Vec<(f64, f64)>, bool)> = Vec::new();
    let mut actual: Vec<(f64, f64)> = Vec::new();
    // Todo en coordenadas del SVG; se transforma al guardar el punto.
    let (mut x, mut y) = (0.0, 0.0);
    let (mut sx, mut sy) = (0.0, 0.0);
    let mut control: Option<(f64, f64)> = None;
    let mut ultimo = b' ';
    let pasos = |largo: f64| ((largo * detalle).ceil() as usize).clamp(2, 48);
    let largo_en = |pts: &[(f64, f64)]| -> f64 {
        pts.windows(2)
            .map(|w| {
                let (a, b) = (m.aplicar(w[0]), m.aplicar(w[1]));
                (a.0 - b.0).hypot(a.1 - b.1)
            })
            .sum()
    };
    let cerrar = |actual: &mut Vec<(f64, f64)>,
                  anillos: &mut Vec<(Vec<(f64, f64)>, bool)>,
                  cerrado: bool| {
        if actual.len() >= 2 {
            anillos.push((std::mem::take(actual), cerrado));
        } else {
            actual.clear();
        }
    };
    loop {
        if l.fin() {
            break;
        }
        let cmd = match l.comando() {
            Some(c) => c,
            // Numeros sin letra: se repite la ultima (tras M, es L).
            None => match ultimo {
                b'M' => b'L',
                b'm' => b'l',
                b'Z' | b'z' | b' ' => break,
                c => c,
            },
        };
        let rel = cmd.is_ascii_lowercase();
        let (bx, by) = if rel { (x, y) } else { (0.0, 0.0) };
        let mut ok = true;
        match cmd.to_ascii_uppercase() {
            b'M' => match (l.numero(), l.numero()) {
                (Some(a), Some(b)) => {
                    cerrar(&mut actual, &mut anillos, false);
                    x = bx + a;
                    y = by + b;
                    sx = x;
                    sy = y;
                    actual.push(m.aplicar((x, y)));
                    control = None;
                }
                _ => ok = false,
            },
            b'L' => match (l.numero(), l.numero()) {
                (Some(a), Some(b)) => {
                    x = bx + a;
                    y = by + b;
                    actual.push(m.aplicar((x, y)));
                    control = None;
                }
                _ => ok = false,
            },
            b'H' => match l.numero() {
                Some(a) => {
                    x = if rel { x + a } else { a };
                    actual.push(m.aplicar((x, y)));
                    control = None;
                }
                None => ok = false,
            },
            b'V' => match l.numero() {
                Some(b) => {
                    y = if rel { y + b } else { b };
                    actual.push(m.aplicar((x, y)));
                    control = None;
                }
                None => ok = false,
            },
            b'C' | b'S' => {
                let c1 = if cmd.eq_ignore_ascii_case(&b'C') {
                    match (l.numero(), l.numero()) {
                        (Some(a), Some(b)) => Some((bx + a, by + b)),
                        _ => None,
                    }
                } else {
                    // El reflejo del control anterior (o el punto actual).
                    Some(match control {
                        Some((cx, cy)) if matches!(ultimo.to_ascii_uppercase(), b'C' | b'S') => {
                            (2.0 * x - cx, 2.0 * y - cy)
                        }
                        _ => (x, y),
                    })
                };
                match (c1, l.numero(), l.numero(), l.numero(), l.numero()) {
                    (Some(c1), Some(a), Some(b), Some(c), Some(d)) => {
                        let c2 = (bx + a, by + b);
                        let fin = (bx + c, by + d);
                        let p0 = (x, y);
                        let n = pasos(largo_en(&[p0, c1, c2, fin]));
                        for k in 1..=n {
                            let t = k as f64 / n as f64;
                            let u = 1.0 - t;
                            let px = u * u * u * p0.0
                                + 3.0 * u * u * t * c1.0
                                + 3.0 * u * t * t * c2.0
                                + t * t * t * fin.0;
                            let py = u * u * u * p0.1
                                + 3.0 * u * u * t * c1.1
                                + 3.0 * u * t * t * c2.1
                                + t * t * t * fin.1;
                            actual.push(m.aplicar((px, py)));
                        }
                        control = Some(c2);
                        (x, y) = fin;
                    }
                    _ => ok = false,
                }
            }
            b'Q' | b'T' => {
                let c1 = if cmd.eq_ignore_ascii_case(&b'Q') {
                    match (l.numero(), l.numero()) {
                        (Some(a), Some(b)) => Some((bx + a, by + b)),
                        _ => None,
                    }
                } else {
                    Some(match control {
                        Some((cx, cy)) if matches!(ultimo.to_ascii_uppercase(), b'Q' | b'T') => {
                            (2.0 * x - cx, 2.0 * y - cy)
                        }
                        _ => (x, y),
                    })
                };
                match (c1, l.numero(), l.numero()) {
                    (Some(c1), Some(a), Some(b)) => {
                        let fin = (bx + a, by + b);
                        let p0 = (x, y);
                        let n = pasos(largo_en(&[p0, c1, fin]));
                        for k in 1..=n {
                            let t = k as f64 / n as f64;
                            let u = 1.0 - t;
                            let px = u * u * p0.0 + 2.0 * u * t * c1.0 + t * t * fin.0;
                            let py = u * u * p0.1 + 2.0 * u * t * c1.1 + t * t * fin.1;
                            actual.push(m.aplicar((px, py)));
                        }
                        control = Some(c1);
                        (x, y) = fin;
                    }
                    _ => ok = false,
                }
            }
            b'A' => {
                match (
                    l.numero(),
                    l.numero(),
                    l.numero(),
                    l.bandera(),
                    l.bandera(),
                    l.numero(),
                    l.numero(),
                ) {
                    (
                        Some(rx),
                        Some(ry),
                        Some(giro),
                        Some(grande),
                        Some(barre),
                        Some(a),
                        Some(b),
                    ) => {
                        let fin = (bx + a, by + b);
                        for p in arco((x, y), rx, ry, giro, grande, barre, fin, &|largo| {
                            pasos(largo)
                        }) {
                            actual.push(m.aplicar(p));
                        }
                        (x, y) = fin;
                        control = None;
                    }
                    _ => ok = false,
                }
            }
            b'Z' => {
                cerrar(&mut actual, &mut anillos, true);
                x = sx;
                y = sy;
                // Un trazo que sigue tras Z sin M empieza donde acabo.
                actual.push(m.aplicar((x, y)));
                control = None;
            }
            _ => ok = false,
        }
        if !ok {
            break;
        }
        ultimo = cmd;
    }
    cerrar(&mut actual, &mut anillos, false);
    anillos
}

/// Los puntos de un arco de SVG (de `desde` a `hasta`, sin el primero),
/// por la conversion de su apendice F.6.5.
#[allow(clippy::too_many_arguments)]
fn arco(
    desde: (f64, f64),
    rx: f64,
    ry: f64,
    giro: f64,
    grande: bool,
    barre: bool,
    hasta: (f64, f64),
    pasos: &dyn Fn(f64) -> usize,
) -> Vec<(f64, f64)> {
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx < 1e-9 || ry < 1e-9 || (desde.0 - hasta.0).hypot(desde.1 - hasta.1) < 1e-12 {
        return vec![hasta];
    }
    let fi = giro.to_radians();
    let (c, s) = (fi.cos(), fi.sin());
    let dx = (desde.0 - hasta.0) / 2.0;
    let dy = (desde.1 - hasta.1) / 2.0;
    let x1 = c * dx + s * dy;
    let y1 = -s * dx + c * dy;
    let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if lambda > 1.0 {
        rx *= lambda.sqrt();
        ry *= lambda.sqrt();
    }
    let num = (rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1).max(0.0);
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let mut k = if den > 0.0 { (num / den).sqrt() } else { 0.0 };
    if grande == barre {
        k = -k;
    }
    let cx1 = k * rx * y1 / ry;
    let cy1 = -k * ry * x1 / rx;
    let cx = c * cx1 - s * cy1 + (desde.0 + hasta.0) / 2.0;
    let cy = s * cx1 + c * cy1 + (desde.1 + hasta.1) / 2.0;
    let angulo = |ux: f64, uy: f64, vx: f64, vy: f64| (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
    let t1 = angulo(1.0, 0.0, (x1 - cx1) / rx, (y1 - cy1) / ry);
    let mut dt = angulo(
        (x1 - cx1) / rx,
        (y1 - cy1) / ry,
        (-x1 - cx1) / rx,
        (-y1 - cy1) / ry,
    );
    if !barre && dt > 0.0 {
        dt -= std::f64::consts::TAU;
    } else if barre && dt < 0.0 {
        dt += std::f64::consts::TAU;
    }
    let n = pasos(dt.abs() * rx.max(ry));
    (1..=n)
        .map(|i| {
            if i == n {
                return hasta;
            }
            let t = t1 + dt * i as f64 / n as f64;
            let (px, py) = (rx * t.cos(), ry * t.sin());
            (c * px - s * py + cx, s * px + c * py + cy)
        })
        .collect()
}

/// Ramer-Douglas-Peucker: quita los puntos que no cambian la forma mas de
/// `tol` pixeles.
fn simplificar(pts: &[(f64, f64)], tol: f64) -> Vec<(f64, f64)> {
    let mut sin_repetir: Vec<(f64, f64)> = Vec::with_capacity(pts.len());
    for &p in pts {
        if sin_repetir
            .last()
            .is_none_or(|q: &(f64, f64)| (q.0 - p.0).hypot(q.1 - p.1) > 1e-6)
        {
            sin_repetir.push(p);
        }
    }
    if sin_repetir.len() <= 3 {
        return sin_repetir;
    }
    let mut guardar = vec![false; sin_repetir.len()];
    guardar[0] = true;
    *guardar.last_mut().unwrap() = true;
    let mut pila = vec![(0, sin_repetir.len() - 1)];
    while let Some((a, b)) = pila.pop() {
        if b <= a + 1 {
            continue;
        }
        let (pa, pb) = (sin_repetir[a], sin_repetir[b]);
        let (dx, dy) = (pb.0 - pa.0, pb.1 - pa.1);
        let largo = dx.hypot(dy);
        let mut mayor = (0.0, a);
        for (i, p) in sin_repetir.iter().enumerate().take(b).skip(a + 1) {
            let d = if largo < 1e-9 {
                (p.0 - pa.0).hypot(p.1 - pa.1)
            } else {
                ((p.0 - pa.0) * dy - (p.1 - pa.1) * dx).abs() / largo
            };
            if d > mayor.0 {
                mayor = (d, i);
            }
        }
        if mayor.0 > tol {
            guardar[mayor.1] = true;
            pila.push((a, mayor.1));
            pila.push((mayor.1, b));
        }
    }
    sin_repetir
        .into_iter()
        .zip(guardar)
        .filter(|(_, g)| *g)
        .map(|(p, _)| p)
        .collect()
}

/// Si `p` cae dentro del poligono (par/impar).
fn dentro(p: (f64, f64), poligono: &[(f64, f64)]) -> bool {
    let mut si = false;
    let n = poligono.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poligono[i], poligono[j]);
        if (a.1 > p.1) != (b.1 > p.1)
            && p.0 < (b.0 - a.0) * (p.1 - a.1) / (b.1 - a.1 + 1e-300) + a.0
        {
            si = !si;
        }
        j = i;
    }
    si
}

// ---------------------------------------------------------------------------
// Al lienzo
// ---------------------------------------------------------------------------

/// **Las piezas como elementos del lienzo**, todas en un grupo.
pub(crate) fn elementos(f: &mut Fabrica, s: &Svg) {
    let antes = f.grupo.clone();
    f.grupo = Some(f.nombre_de_grupo("svg"));
    let rugosidad = match s.modo {
        Modo::AMano => 1,
        _ => 0,
    };
    let familia = match s.modo {
        Modo::Liso => 2,
        _ => 1,
    };
    let lapiz = match &s.modo {
        Modo::Lapiz { material, dureza } => Some((*material, *dureza)),
        _ => None,
    };
    for p in &s.piezas {
        let desde = f.salida.len();
        match p {
            Pieza::Caja {
                tipo,
                caja,
                redondo,
                pinta,
            } => {
                let trazo = pinta
                    .trazo
                    .as_deref()
                    .or(pinta.relleno.as_deref())
                    .unwrap_or("#1e1e1e");
                f.caja(tipo, *caja, trazo, pinta.relleno.as_deref());
                if let Some(e) = f.salida.last_mut() {
                    if *redondo {
                        e["roundness"] = json!({"type": 3});
                    }
                    pintar(e, pinta, rugosidad);
                    // A lapiz, el borde tambien es de grafito.
                    if let Some((material, dureza)) = lapiz {
                        e["material"] = json!(material);
                        if let Some(d) = dureza.filter(|d| *d != "HB") {
                            e["dureza"] = json!(d);
                        }
                        e["strokeWidth"] = json!(pinta.grosor.max(1.5));
                        if pinta.trazo.is_none() {
                            e["strokeColor"] =
                                json!(oscurecer(pinta.relleno.as_deref().unwrap_or("#1e1e1e")));
                        }
                    }
                }
            }
            Pieza::Forma { anillos, pinta } => forma(f, anillos, pinta, rugosidad, lapiz),
            Pieza::Texto {
                x,
                y,
                texto,
                tam,
                alinear,
                color,
                opacidad,
            } => {
                // `texto()` mide con 0.6 por letra para alinear al centro.
                f.texto(*x, *y, texto, *tam, color, alinear);
                if let Some(e) = f.salida.last_mut() {
                    e["fontFamily"] = json!(familia);
                    e["opacity"] = json!(opacidad);
                }
            }
        }
        // En modo lapiz, los rellenos se aclaran: son el sombreado.
        if lapiz.is_some() {
            for e in f.salida.iter_mut().skip(desde) {
                if e["type"] == "pixpin-region"
                    || e["backgroundColor"] != "transparent" && e["type"] != "freedraw"
                {
                    let o = e["opacity"].as_f64().unwrap_or(100.0);
                    e["opacity"] = json!((o * 0.45).round().max(5.0));
                }
            }
        }
    }
    f.grupo = antes;
}

/// Un `#rrggbb` mas oscuro (el contorno a lapiz de una mancha).
fn oscurecer(hex: &str) -> String {
    let h = hex.trim_start_matches('#');
    let c = |i: usize| {
        (u8::from_str_radix(h.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0) as f64 * 0.55) as u8
    };
    format!("#{:02x}{:02x}{:02x}", c(0), c(2), c(4))
}

fn pintar(e: &mut Value, pinta: &Pintura, rugosidad: u8) {
    e["strokeWidth"] = json!((pinta.grosor * 100.0).round() / 100.0);
    e["opacity"] = json!(pinta.opacidad);
    e["roughness"] = json!(rugosidad);
    if pinta.discontinuo {
        e["strokeStyle"] = json!("dashed");
    }
    if pinta.trazo.is_none() {
        // Sin trazo: un borde fino del color del relleno tapa la rendija
        // entre dos manchas vecinas (como el calcado).
        e["strokeWidth"] = json!(1);
    }
}

fn forma(
    f: &mut Fabrica,
    anillos: &[(Vec<(f64, f64)>, bool)],
    pinta: &Pintura,
    rugosidad: u8,
    lapiz: Option<(&'static str, Option<&'static str>)>,
) {
    if let Some(relleno) = &pinta.relleno {
        // Los anillos que caen dentro de un numero impar de otros son
        // agujeros del que los contiene (la regla par/impar).
        let cerrados: Vec<&Vec<(f64, f64)>> = anillos
            .iter()
            .map(|(a, _)| a)
            .filter(|a| a.len() >= 3)
            .collect();
        let nivel: Vec<usize> = cerrados
            .iter()
            .enumerate()
            .map(|(i, a)| {
                cerrados
                    .iter()
                    .enumerate()
                    .filter(|(j, b)| *j != i && dentro(a[0], b))
                    .count()
            })
            .collect();
        for (i, a) in cerrados
            .iter()
            .enumerate()
            .filter(|(i, _)| nivel[*i] % 2 == 0)
        {
            let huecos: Vec<Vec<(f64, f64)>> = cerrados
                .iter()
                .enumerate()
                .filter(|(j, b)| nivel[*j] == nivel[i] + 1 && dentro(b[0], a))
                .map(|(_, b)| (*b).clone())
                .collect();
            let trazo = pinta
                .trazo
                .as_deref()
                .filter(|_| lapiz.is_none())
                .unwrap_or(relleno);
            f.region(a, &huecos, trazo, relleno);
            if let Some(e) = f.salida.last_mut() {
                pintar(e, pinta, rugosidad);
                if lapiz.is_some() || pinta.trazo.is_none() {
                    e["strokeWidth"] = json!(1);
                }
            }
        }
    }
    // A lapiz, una mancha sin trazo lleva su contorno de grafito, en un
    // tono mas oscuro que su relleno: el dibujo se lee por sus lineas.
    let contorno_de_lapiz = match (&pinta.trazo, &pinta.relleno, lapiz) {
        (None, Some(r), Some(_)) => Some(oscurecer(r)),
        _ => None,
    };
    let Some(trazo) = pinta.trazo.clone().or(contorno_de_lapiz) else {
        return;
    };
    let trazo = &trazo;
    // El contorno: con relleno, solo si no es a lapiz ya lo pinto la mancha
    // (los anillos cerrados); los abiertos y el modo lapiz van como lineas.
    for (a, cerrado) in anillos {
        if pinta.relleno.is_some() && lapiz.is_none() && (a.len() >= 3) {
            continue;
        }
        let mut pts = a.clone();
        if *cerrado && pts.len() >= 3 {
            pts.push(pts[0]);
        }
        match lapiz {
            Some((material, dureza)) => {
                f.lapiz(&pts, trazo, Some(material), dureza);
                if let Some(e) = f.salida.last_mut() {
                    e["strokeWidth"] = json!((pinta.grosor * 100.0).round() / 100.0);
                    e["opacity"] = json!(pinta.opacidad);
                }
            }
            None => {
                f.trazo("line", &pts, trazo, None);
                if let Some(e) = f.salida.last_mut() {
                    pintar(e, pinta, rugosidad);
                    e["strokeWidth"] = json!((pinta.grosor * 100.0).round() / 100.0);
                }
            }
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn leer_(svg: &str) -> Result<Vec<Pieza>, ErrorHerramienta> {
        leer(svg, [0.0, 0.0, 800.0, 600.0], &mut Vec::new())
    }

    fn validar_(args: Value) -> Result<Svg, ErrorHerramienta> {
        let Value::Object(m) = args else { panic!() };
        validar(&m, &mut Vec::new())
    }

    #[test]
    fn un_arbol_sencillo_sale_en_figuras_que_lee_el_lienzo() {
        let s = validar_(json!({"svg": r##"<svg viewBox="0 0 100 100">
            <rect x="45" y="60" width="10" height="35" fill="#8b5a2b"/>
            <circle cx="50" cy="40" r="30" fill="#2f9e44" stroke="#1e1e1e" stroke-width="2"/>
            <path d="M10 95 C 30 85, 70 85, 90 95" fill="none" stroke="green"/>
            <text x="50" y="12" text-anchor="middle" font-size="8">Arbol</text></svg>"##}))
        .unwrap();
        assert_eq!(s.piezas.len(), 4);
        let mut f = Fabrica::nueva((0.0, 0.0), 3);
        elementos(&mut f, &s);
        let doc = crate::lienzo::documento(f.salida.clone());
        let lienzo = pixpin_motor2d::excalidraw::leer(&doc).unwrap();
        assert_eq!(lienzo.cuantos_ajenos(), 0, "todo editable");
        let els = lienzo.elementos();
        assert_eq!(els.len(), 4);
        // Todo en un grupo, para moverlo de una vez.
        assert!(
            els.iter()
                .all(|e| e.grupos == els[0].grupos && !e.grupos.is_empty())
        );
        // Encajado en el hueco de fabrica (680x520 con margen): 100
        // unidades -> 520 px.
        let tronco = &f.salida[0];
        assert_eq!(tronco["type"], "rectangle");
        assert!(
            (tronco["width"].as_f64().unwrap() - 52.0).abs() < 0.01,
            "{tronco}"
        );
        assert_eq!(f.salida[1]["type"], "ellipse");
        assert_eq!(f.salida[1]["backgroundColor"], "#2f9e44");
        // La curva de Bezier, hecha polilinea.
        assert_eq!(f.salida[2]["type"], "line");
        assert!(f.salida[2]["points"].as_array().unwrap().len() > 4);
        assert_eq!(f.salida[3]["type"], "text");
        assert_eq!(f.salida[3]["text"], "Arbol");
    }

    #[test]
    fn caminos_con_relativos_arcos_y_agujeros() {
        // Un anillo: un cuadrado con un agujero dentro, en un solo path.
        let p = leer_(r#"<svg viewBox="0 0 100 100"><path fill="red" d="M0 0 H100 V100 H0 Z M25 25 v50 h50 v-50 z"/></svg>"#).unwrap();
        let Pieza::Forma { anillos, .. } = &p[0] else {
            panic!()
        };
        assert_eq!(anillos.len(), 2);
        let mut f = Fabrica::nueva((0.0, 0.0), 1);
        elementos(
            &mut f,
            &Svg {
                piezas: p,
                modo: Modo::Liso,
            },
        );
        assert_eq!(f.salida.len(), 1);
        assert_eq!(f.salida[0]["type"], "pixpin-region");
        assert_eq!(f.salida[0]["huecos"].as_array().unwrap().len(), 1);
        // Un arco: media circunferencia de radio 50.
        let p = leer_(r#"<svg viewBox="0 0 200 200"><path d="M0 100 A 50 50 0 0 1 100 100" fill="none" stroke="black"/></svg>"#).unwrap();
        let Pieza::Forma { anillos, .. } = &p[0] else {
            panic!()
        };
        let medio = anillos[0].0[anillos[0].0.len() / 2];
        // El punto de arriba del arco: (50, 50) en el SVG -> (150, 50)*3 + desplazamiento.
        assert!(anillos[0].0.len() > 5, "{:?}", anillos[0].0);
        assert!(medio.1 < 300.0 - 100.0, "el arco sube: {medio:?}");
    }

    #[test]
    fn grupos_con_transformaciones_y_estilos() {
        let p = leer_(
            r#"<svg viewBox="0 0 100 100"><style>.hoja{fill:#2f9e44} #t{stroke:blue}</style>
               <g transform="translate(50 50) scale(2)" fill="red" opacity="0.5">
                 <rect class="hoja" x="-5" y="-5" width="10" height="10"/>
                 <rect x="0" y="0" width="5" height="5" style="fill:yellow"/>
                 <rect id="t" x="0" y="0" width="5" height="5" transform="rotate(45)"/>
               </g></svg>"#,
        )
        .unwrap();
        assert_eq!(p.len(), 3);
        let Pieza::Caja { caja, pinta, .. } = &p[0] else {
            panic!("{:?}", p[0])
        };
        // 100 unidades -> 600 px, centrado en 800: x = 100 + (50-10)*6.
        assert!(
            (caja[0] - 340.0).abs() < 0.01 && (caja[2] - 120.0).abs() < 0.01,
            "{caja:?}"
        );
        assert_eq!(pinta.relleno.as_deref(), Some("#2f9e44"));
        assert_eq!(pinta.opacidad, 50.0);
        let Pieza::Caja { pinta, .. } = &p[1] else {
            panic!()
        };
        assert_eq!(pinta.relleno.as_deref(), Some("#ffff00"));
        // Girado: ya no es un rectangulo derecho.
        let Pieza::Forma { pinta, anillos } = &p[2] else {
            panic!("{:?}", p[2])
        };
        assert_eq!(anillos[0].0.len(), 4);
        assert_eq!(pinta.trazo.as_deref(), Some("#0000ff"));
    }

    #[test]
    fn un_svg_malicioso_o_enorme_se_rechaza_o_se_sanea() {
        // DTD con entidades (bomba de XML): fuera.
        let bomba = r#"<!DOCTYPE svg [<!ENTITY a "aaaaaaaaaa"><!ENTITY b "&a;&a;&a;&a;&a;&a;">]><svg viewBox="0 0 10 10"><text>&b;</text></svg>"#;
        assert!(validar_(json!({"svg": bomba})).is_err());
        // Script, imagen externa y foreignObject: se ignoran (y se avisa).
        let mut avisos = Vec::new();
        let p = leer(
            r#"<svg viewBox="0 0 10 10"><script>alert(1)</script><image href="http://mal.example/x.png" width="5" height="5"/>
               <foreignObject><div>hola</div></foreignObject><use href="http://mal.example/a.svg#x"/><rect width="5" height="5"/></svg>"#,
            [0.0, 0.0, 800.0, 600.0],
            &mut avisos,
        )
        .unwrap();
        assert_eq!(p.len(), 1);
        assert!(
            avisos[0].contains("script") && avisos[0].contains("image"),
            "{avisos:?}"
        );
        // Demasiado grande, demasiadas figuras, demasiado hondo, nada.
        let enorme = format!("<svg>{}</svg>", "<!-- relleno -->".repeat(MAX_BYTES / 10));
        assert!(
            validar_(json!({"svg": enorme}))
                .unwrap_err()
                .to_string()
                .contains("KB")
        );
        let muchas = format!(
            r#"<svg viewBox="0 0 10 10">{}</svg>"#,
            r#"<rect width="1" height="1"/>"#.repeat(MAX_FIGURAS + 5)
        );
        assert!(
            validar_(json!({"svg": muchas}))
                .unwrap_err()
                .to_string()
                .contains("figuras")
        );
        let hondo = format!(
            r#"<svg viewBox="0 0 10 10">{}<rect width="1" height="1"/>{}</svg>"#,
            "<g>".repeat(40),
            "</g>".repeat(40)
        );
        assert!(
            validar_(json!({"svg": hondo}))
                .unwrap_err()
                .to_string()
                .contains("niveles")
        );
        // Un `use` que se apunta a si mismo no se cuelga.
        let bucle = r##"<svg viewBox="0 0 10 10"><g id="a"><use href="#a"/><rect width="1" height="1"/></g></svg>"##;
        assert!(validar_(json!({"svg": bucle})).is_err());
        assert!(validar_(json!({"svg": "<svg viewBox='0 0 10 10'><rect width='5' height='5' fill='none'/></svg>"})).is_err());
        assert!(validar_(json!({"svg": "un perro"})).is_err());
        assert!(validar_(json!({"svg": "<svg><rect width='5'"})).is_err());
        assert!(validar_(json!({})).is_err());
        assert!(validar_(json!({"svg": "<svg viewBox='0 0 10 10'><rect width='5' height='5'/></svg>", "trazo": "acuarela"})).is_err());
        // Numeros rotos en el camino: se corta ahi, sin panico.
        let p = leer_(
            r#"<svg viewBox="0 0 10 10"><path d="M1 1 L 5 5 L nan 3 Z" stroke="red"/></svg>"#,
        )
        .unwrap();
        assert_eq!(p.len(), 1);
    }

    #[test]
    fn a_lapiz_las_lineas_son_grafito() {
        let s = validar_(json!({"svg": r#"<svg viewBox="0 0 10 10"><path d="M1 1 Q 5 9 9 1" fill="none" stroke="black"/><circle cx="5" cy="5" r="2" fill="gray"/></svg>"#, "trazo": "grafito", "dureza": "6B"}))
        .unwrap();
        let mut f = Fabrica::nueva((0.0, 0.0), 1);
        elementos(&mut f, &s);
        let trazo = f
            .salida
            .iter()
            .find(|e| e["type"] == "freedraw")
            .expect("un trazo de lapiz");
        assert_eq!(trazo["material"], "cuadritos");
        assert_eq!(trazo["dureza"], "6B");
        let lienzo =
            pixpin_motor2d::excalidraw::leer(&crate::lienzo::documento(f.salida.clone())).unwrap();
        assert_eq!(lienzo.cuantos_ajenos(), 0);
    }

    #[test]
    fn sin_viewbox_se_encaja_por_lo_dibujado() {
        let p = leer_(r#"<svg><rect x="1000" y="1000" width="10" height="20"/></svg>"#).unwrap();
        let Pieza::Caja { caja, .. } = &p[0] else {
            panic!()
        };
        // 20 de alto -> los 600 del lienzo.
        assert!((caja[3] - 600.0).abs() < 0.01, "{caja:?}");
        assert!(caja[0] >= 0.0 && caja[0] + caja[2] <= 800.0);
    }

    #[test]
    fn numeros_pegados_como_los_escriben_los_programas() {
        assert_eq!(numeros_sueltos("1.5.5-2e1,3"), vec![1.5, 0.5, -20.0, 3.0]);
        let m = transformacion("translate(10,20) scale(2)");
        assert_eq!(m.aplicar((1.0, 1.0)), (12.0, 22.0));
        assert_eq!(
            color_svg("rgb(255, 0, 0)", "", &HashMap::new()),
            Some(Some("#ff0000".into()))
        );
        assert_eq!(color_svg("none", "", &HashMap::new()), Some(None));
        assert_eq!(
            color_svg("verde", "", &HashMap::new()),
            Some(Some("#2f9e44".into()))
        );
    }
}
