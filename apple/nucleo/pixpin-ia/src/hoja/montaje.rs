//! **Cada zona, a elementos del lienzo** (en Excalidraw, como todo lo que
//! dibuja el asistente: `lienzo::Fabrica`), ya en su caja destino.
//!
//! - Parrafos y encabezados → **texto del lienzo**, con el tamano de letra
//!   que hace que cada renglon mida lo que media en el papel, su alineacion,
//!   negrita en los titulos y los mismos saltos de renglon. Si el
//!   interlineado del papel no es el de la letra, cada renglon va suelto en
//!   su sitio (agrupados), que si no el parrafo se estiraria.
//! - Tablas → **la tabla dibujada** del motor (`tabla_dibujada`), con la
//!   letra elegida para que ocupe el ancho de la del papel.
//! - Fotos → **la imagen recortada**, como una imagen pegada.
//! - Lineas → rectas lisas.
//! - Formulas → el LaTeX del modelo por **`escribir_ecuacion`**
//!   (`ecuacion::validar` + `ecuacion::piezas`), encajada en su caja.
//! - Dibujos (y graficas que no son funciones) → **calcado**
//!   (`calcar::calcar`), recolocado en su caja.
//! - Graficas de funciones → **`graficar_funcion`** (`funciones::piezas`)
//!   en su caja.
//!
//! Cada zona sale en su grupo (se mueve de una vez y se desagrupa para
//! tocar una pieza). Lo que la comprobacion rehace va por otra [`Via`].

use serde_json::{Map, Value, json};

use pixpin_motor2d::ecuacion::{Estilo, elemento_texto};
use pixpin_motor2d::elemento::{ColorRgba, Elemento};
use pixpin_motor2d::texto::{AlineacionTexto, EstiloDeTexto};
use pixpin_motor2d::vector::Punto2;

use super::{Alineacion, Caja, Mapa, Opciones, Renglon, Tipo, Zona, imagen, mediana};
use crate::lienzo::Fabrica;

/// Como se hace una zona.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    /// Lo normal: lo mas editable (texto corrido, tabla dibujada, ecuacion).
    Editable,
    /// Cada renglon (o trozo de tabla) en su sitio exacto, y las rayas tal
    /// cual: lo que se usa si lo editable no se parece.
    Fiel,
    /// El recorte de la imagen: lo ultimo, cuando nada se parece.
    Imagen,
}

/// Lo que el modelo leyo de un recorte.
#[derive(Debug, Clone, PartialEq)]
pub enum Leido {
    /// Una formula, en LaTeX (una linea por renglon).
    Latex(Vec<String>),
    /// Lo que es una zona dudosa.
    Tipo(Tipo),
    /// Una grafica de funciones (los argumentos de `graficar_funcion`).
    Funciones(Map<String, Value>),
    /// Los rotulos de una figura (texto y centro en fracciones del recorte).
    Rotulos(Vec<(String, f64, f64)>, Vec<String>),
    /// Un bloque de texto en texto rico del lienzo (`$...$`, colores, cursiva).
    Rico(String),
    /// **Los renglones de la zona, corregidos** (17-oct): uno por renglon del
    /// OCR, en el mismo orden, en texto rico. Cada uno va en el sitio de su
    /// renglon ([`Via::Fiel`]): nada se mueve, solo se arreglan las letras.
    Renglones(Vec<String>),
    /// **Elementos ya hechos por un especialista** (la tabla del lienzo de
    /// `tabla_de_fichero`), con su esquina en (0, 0) y, aparte, donde cae esa
    /// esquina en el recorte de la zona (pixeles).
    Elementos(Vec<Value>, (f64, f64)),
}

/// Una imagen que nace con la zona: va al almacen del lienzo con este id
/// (su `fileId` es `pc<hex>`).
#[derive(Debug, Clone, PartialEq)]
pub struct ImagenNueva {
    pub id: u64,
    pub rgba: Vec<u8>,
    pub ancho: u32,
    pub alto: u32,
}

/// Una zona hecha.
#[derive(Debug, Clone, PartialEq)]
pub struct Hecho {
    pub zona: usize,
    pub tipo: Tipo,
    pub via: Via,
    pub elementos: Vec<Value>,
    pub imagenes: Vec<ImagenNueva>,
    /// Lo que se hizo, en corto (para el registro y el banco).
    pub nota: String,
}

/// Lo que recibe el trabajador de una zona.
pub struct Entrada<'a> {
    pub zona: &'a Zona,
    pub mapa: Mapa,
    pub opciones: &'a Opciones,
    /// La imagen de la pagina (RGBA, ancho, alto).
    pub pagina: (&'a [u8], u32, u32),
    /// El alto de letra del cuerpo, en pixeles de la pagina.
    pub alto_letra: f64,
    pub semilla: u64,
    /// Quien lee las letras sueltas de un diagrama (el OCR de la app).
    pub lector: Option<&'a crate::diagrama::Lector>,
    /// El texto de la hoja que no es de esta zona (cajas en pixeles de la
    /// pagina): un diagrama no lo vectoriza aunque caiga en su recorte.
    pub ajenas: &'a [Caja],
    /// **El texto de toda la hoja** (renglones): las pistas de la geometria
    /// exacta de un diagrama («AB es diametro» en la columna de al lado).
    pub pistas: &'a [String],
}

impl Entrada<'_> {
    pub fn destino(&self) -> Caja {
        self.mapa.caja(&self.zona.caja)
    }
    /// **La semilla de ESTA zona**: la de la hoja mezclada con el numero de
    /// zona. Con la misma semilla en todas, los ids (`ia-<semilla>-<n>`) se
    /// repetian de una zona a otra y rehacer una (que borra por id) se
    /// llevaba piezas de las demas (banco del 15-oct, cuadratica 32/65).
    pub fn semilla_de_zona(&self) -> u64 {
        id_de_imagen(self.semilla, 1_000_000 + self.zona.id)
    }
    fn fabrica(&self) -> Fabrica {
        let mut f = Fabrica::nueva((0.0, 0.0), self.semilla_de_zona());
        f.grupo = Some(f.nombre_de_grupo(&format!("hoja-{}", self.zona.id)));
        f
    }
    /// El texto ajeno que cae en la zona, en pixeles del recorte.
    pub fn ajenas_en_zona(&self) -> Vec<Caja> {
        let z = &self.zona.caja;
        self.ajenas.iter().filter(|c| c.comun(z) > 0.0).map(|c| Caja::nueva(c.x - z.x, c.y - z.y, c.w, c.h)).collect()
    }
    /// El recorte de la zona (RGBA, ancho, alto).
    pub fn recorte(&self) -> (Vec<u8>, u32, u32) {
        let (rgba, w, h) = self.pagina;
        imagen::recortar(rgba, w, h, &self.zona.caja)
    }
}

const NEGRO: &str = "#1e1e1e";

fn negro() -> ColorRgba {
    ColorRgba::opaco(0.118, 0.118, 0.118)
}

/// **Hace una zona.** `leido`: lo que dijo el modelo (formulas, dudosas,
/// graficas); sin el, lo que se pueda sin modelo.
pub fn hacer(e: &Entrada, leido: Option<&Leido>, via: Via) -> Hecho {
    let z = e.zona;
    let tipo = match (z.tipo, leido) {
        // Lo que dijo el modelo de una dudosa, o de un dibujo o foto por los
        // que pregunto la app (antes la respuesta de estos se perdia y un
        // sistema de ecuaciones escaneado salia calcado).
        (Tipo::Dudosa | Tipo::Dibujo | Tipo::Foto, Some(Leido::Tipo(t))) => *t,
        (Tipo::Dudosa | Tipo::Dibujo | Tipo::Foto, Some(Leido::Latex(_))) => Tipo::Formula,
        (t, _) => t,
    };
    let mut imagenes = Vec::new();
    // Un recuadro (`paneles`): su fondo y su borde, con la herramienta de
    // relleno; lo de dentro son otras zonas.
    if tipo == Tipo::Recuadro {
        return Hecho { zona: z.id, tipo, via: Via::Editable, elementos: recuadro(e), imagenes: Vec::new(), nota: "recuadro con su fondo y su borde".into() };
    }
    // Una «tabla» que es un recuadro de color con texto (un aviso, un dato).
    // (No si el especialista de tablas ya la hizo: eso va primero.)
    if tipo == Tipo::Tabla && via != Via::Imagen
        && !matches!(leido, Some(Leido::Elementos(v, _)) if !v.is_empty())
        && let Some(elementos) = caja_de_color(e)
    {
        return Hecho { zona: z.id, tipo, via, elementos, imagenes: Vec::new(), nota: "recuadro de color con su texto en su sitio".into() };
    }
    // Lo que hizo un especialista (la tabla): en el sitio de la zona.
    if let Some(Leido::Elementos(v, (ox, oy))) = leido
        && !v.is_empty()
    {
        let d = e.destino();
        let (dx, dy) = (d.x + ox * e.mapa.escala, d.y + oy * e.mapa.escala);
        let semilla = e.semilla_de_zona();
        let elementos = v
            .iter()
            .enumerate()
            .map(|(k, x)| {
                let mut x = x.clone();
                x["x"] = json!(x["x"].as_f64().unwrap_or(0.0) + dx);
                x["y"] = json!(x["y"].as_f64().unwrap_or(0.0) + dy);
                x["id"] = json!(format!("ia-{semilla:x}-t{k}"));
                x
            })
            .collect();
        return Hecho { zona: z.id, tipo, via: Via::Editable, elementos, imagenes: Vec::new(), nota: "tabla del lienzo, como en la foto".into() };
    }
    // Sus renglones corregidos por la nube: cada uno en su sitio.
    if let Some(Leido::Renglones(ls)) = leido
        // (Otro numero que el OCR: `fiel_con` los reparte por la zona, 18-oct.)
        && !ls.is_empty()
        && matches!(tipo, Tipo::Parrafo | Tipo::Encabezado)
    {
        return Hecho { zona: z.id, tipo, via: Via::Fiel, elementos: fiel_con(e, Some(ls)), imagenes: Vec::new(), nota: "renglones en su sitio, corregidos".into() };
    }
    // Un bloque que la nube paso a texto rico: un solo texto, en su sitio.
    if let Some(Leido::Rico(fuente)) = leido {
        let elementos = bloque_rico(e, fuente);
        if !elementos.is_empty() {
            return Hecho { zona: z.id, tipo: Tipo::Parrafo, via, elementos, imagenes: Vec::new(), nota: format!("texto rico: {}", fuente.chars().take(60).collect::<String>()) };
        }
    }
    let (elementos, nota) = match (via, tipo) {
        (Via::Imagen, _) | (_, Tipo::Foto) => {
            let (els, img) = foto(e);
            imagenes.extend(img);
            (els, "imagen recortada".to_string())
        }
        (_, Tipo::Linea) => (linea(e), "recta".into()),
        // Una tabla con iconos en sus celdas: cada cosa en su sitio y los
        // iconos recortados encima (la tabla dibujada no lleva imagenes).
        (_, Tipo::Tabla) if !z.dentro.is_empty() => {
            let mut els = fiel(e);
            for (k, c) in z.dentro.iter().enumerate() {
                let grupo = els.first().map(|v| v["groupIds"].clone());
                if let Some((v, img)) = foto_de_caja(e, c, k + 1, grupo) {
                    els.push(v);
                    imagenes.push(img);
                }
            }
            (els, format!("tabla en su sitio con {} imagen(es)", z.dentro.len()))
        }
        (Via::Fiel, Tipo::Parrafo | Tipo::Encabezado | Tipo::Tabla) => (fiel(e), "renglones en su sitio".into()),
        (_, Tipo::Parrafo | Tipo::Encabezado) => (texto(e, tipo == Tipo::Encabezado), "texto".into()),
        (_, Tipo::Tabla) => match tabla(e) {
            Some(els) => (els, "tabla dibujada".into()),
            None => (fiel(e), "tabla sin celdas: renglones en su sitio".into()),
        },
        (_, Tipo::Formula) => match leido {
            Some(Leido::Latex(l)) if !l.is_empty() => match formula(e, l) {
                Some(els) => (els, format!("ecuacion: {}", l.join(" \\\\ "))),
                None => {
                    let (els, img) = foto(e);
                    imagenes.extend(img);
                    (els, "formula ilegible: imagen".into())
                }
            },
            // Sin LaTeX (sin modelo o fallo): la formula como imagen, que
            // se ve igual que en el papel.
            _ => {
                let (els, img) = foto(e);
                imagenes.extend(img);
                (els, "formula sin leer: imagen".into())
            }
        },
        (_, Tipo::Grafica) => match leido {
            Some(Leido::Funciones(args)) => match grafica(e, args) {
                Some(els) => (els, "grafica de funciones".into()),
                None => {
                    let (els, img) = foto(e);
                    imagenes.extend(img);
                    (els, "grafica que no se dibuja: recorte de la imagen".into())
                }
            },
            _ => diagrama(e, leido).unwrap_or_else(|| {
                let (els, img) = foto(e);
                imagenes.extend(img);
                (els, "grafica que no se dibuja: recorte de la imagen".into())
            }),
        },
        // Lo que no es diagrama (una foto, un dibujo realista, un logo) no se
        // calca a lapiz: el recorte de la imagen, en su sitio (15-oct).
        (_, Tipo::Dibujo | Tipo::Diagrama) => diagrama(e, leido).unwrap_or_else(|| {
            let (els, img) = foto(e);
            imagenes.extend(img);
            (els, "dibujo que no se dibuja: recorte de la imagen".into())
        }),
        (_, Tipo::Dudosa) => {
            // Sin modelo que lo diga: como imagen, que es lo que menos
            // inventa.
            let (els, img) = foto(e);
            imagenes.extend(img);
            (els, "dudosa: imagen".into())
        }
        // (Ya salio arriba: aqui solo con `Via::Imagen` no llega.)
        (_, Tipo::Recuadro) => (recuadro(e), "recuadro con su fondo y su borde".into()),
    };
    // Nada salio (un «texto» sin palabras que dijo el modelo, una tabla
    // vacia, un calco en blanco): el recorte, que algo hay.
    if elementos.is_empty() && via != Via::Imagen {
        let mut h = hacer(e, None, Via::Imagen);
        h.tipo = tipo;
        h.nota = format!("{nota}: no salio nada, imagen");
        return h;
    }
    // Lo que salio como figuras exactas es un diagrama (para el registro,
    // el banco y la app, que le pregunta los rotulos a la nube).
    let tipo = if nota.starts_with("diagrama") { Tipo::Diagrama } else { tipo };
    Hecho { zona: z.id, tipo, via, elementos, imagenes, nota }
}

// ---------------------------------------------------------------------------
// Texto

/// Lo que mide un texto con su letra (y su negrita, si hay medidor).
fn medir(t: &str, tam: f32, familia: &str, negrita: bool) -> (f32, f32) {
    if pixpin_motor2d::texto::hay_medidor() {
        pixpin_motor2d::texto::medida(t, tam, familia, EstiloDeTexto { negrita, ..Default::default() })
    } else {
        crate::lienzo::medir_con(t, tam, familia)
    }
}

/// **El tamano de letra** que hace que los renglones midan lo que median:
/// la mediana de lo que pide cada renglon largo, sujeta a lo que dice su
/// alto (un renglon con pocas letras anchas no manda).
pub fn tam_de_letra(rs: &[Renglon], escala: f64, familia: &str, negrita: bool) -> f64 {
    let por_alto = mediana(rs.iter().map(|r| r.alto() * escala * 1.05).collect()).unwrap_or(16.0).max(4.0);
    let mut por_ancho: Vec<f64> = rs
        .iter()
        .filter(|r| r.texto.chars().count() >= 10)
        .filter_map(|r| {
            let (w, _) = medir(&r.texto, 100.0, familia, negrita);
            (w > 1.0).then(|| 100.0 * r.caja.w * escala / w as f64)
        })
        .collect();
    if por_ancho.is_empty() {
        por_ancho = rs
            .iter()
            .filter_map(|r| {
                let (w, _) = medir(&r.texto, 100.0, familia, negrita);
                (w > 1.0).then(|| 100.0 * r.caja.w * escala / w as f64)
            })
            .collect();
    }
    let t = mediana(por_ancho).unwrap_or(por_alto).clamp(por_alto * 0.7, por_alto * 1.45);
    (t * 2.0).round() / 2.0
}

fn alineacion_motor(a: Alineacion) -> AlineacionTexto {
    match a {
        Alineacion::Centro => AlineacionTexto::Centro,
        Alineacion::Derecha => AlineacionTexto::Derecha,
        Alineacion::Izquierda | Alineacion::Justificada => AlineacionTexto::Izquierda,
    }
}

/// El enlace de un texto (una direccion que empieza por `http`, `www.` o
/// tiene un dominio con barra), sin la puntuacion de alrededor.
pub fn enlace_de(t: &str) -> Option<String> {
    let w = t.split_whitespace().find(|w| {
        let w = w.to_lowercase();
        w.starts_with("http") || w.starts_with("www.") || [".com/", ".nz/", ".org/", ".net/", ".pe/", ".es/"].iter().any(|d| w.contains(d))
    })?;
    let w = w.trim_matches(|c: char| matches!(c, '(' | ')' | '«' | '»' | '"' | ',' | ';'));
    Some(if w.to_lowercase().starts_with("http") { w.to_string() } else { format!("https://{w}") })
}

fn color_motor(c: Option<[u8; 3]>) -> ColorRgba {
    match c {
        Some([r, g, b]) => ColorRgba::opaco(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0),
        None => negro(),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn un_texto(t: &str, x: f64, y: f64, tam: f64, familia: &str, negrita: bool, al: Alineacion, color: Option<[u8; 3]>) -> Elemento {
    let estilo = Estilo { color: color_motor(color), tam: tam as f32, opacidad: 1.0, familia: familia.to_string() };
    let medida = medir(t, tam as f32, familia, negrita);
    let mut el = elemento_texto(t, x as f32, y as f32, medida, tam as f32, false, &estilo);
    el.extras.negrita = negrita;
    el.extras.alineacion = Some(alineacion_motor(al));
    // **Texto rico** (`$...$`, `\color{rojo}{X}`, `**negrita**`): las
    // lineas de una resolucion con sus formulas en linea y sus colores
    // (`pixpin_motor2d::texto_rico`). Un texto sin marcas queda igual.
    if pixpin_motor2d::texto_rico::tiene_marcas(t) {
        pixpin_motor2d::texto_rico::poner_fuente(&mut el, t);
    }
    // Un enlace sigue siendo un enlace: el lienzo lo abre con Ctrl+clic.
    el.enlace = enlace_de(t);
    el
}

/// **Un parrafo o un encabezado** (con su marco, si estaba en un recuadro).
fn texto(e: &Entrada, titulo: bool) -> Vec<Value> {
    let z = e.zona;
    if z.renglones.is_empty() {
        return Vec::new();
    }
    let familia = if titulo { &e.opciones.familia_titulos } else { &e.opciones.familia };
    let negrita = titulo && (z.renglones.iter().any(|r| r.negrita) || z.renglones.iter().all(|r| r.alto() <= e.alto_letra * 1.3));
    let negrita = negrita || z.renglones.iter().all(|r| r.negrita);
    let esc = e.mapa.escala;
    let tam = tam_de_letra(&z.renglones, esc, familia, negrita);
    let k = pixpin_motor2d::texto::interlineado_de(familia).unwrap_or(pixpin_motor2d::texto::INTERLINEADO) as f64;
    // Que el parrafo no pase del alto que tenia en el papel: con la letra
    // elegida por el ancho, una cursiva estrecha salia mas alta y se montaba
    // en el parrafo de abajo y en la raya del recuadro (banco del 15-oct,
    // amor-a-sofia).
    let alto_papel = e.destino().h;
    let n = z.renglones.len() as f64;
    let tam = if n >= 2.0 && alto_papel > 0.0 { tam.min(alto_papel * 1.04 / (n * k)).max(tam * 0.7) } else { tam };
    let paso = {
        let v: Vec<f64> = z.renglones.windows(2).map(|w| (w[1].caja.cy() - w[0].caja.cy()) * esc).filter(|d| *d > 0.0).collect();
        mediana(v)
    };
    let color = super::seccion::color_mayoritario(z.renglones.iter().map(|r| (r.color, r.texto.chars().count())));
    // Renglones de varios enlaces: cada uno suelto, para que cada uno abra
    // el suyo.
    let enlaces = z.renglones.iter().filter(|r| enlace_de(&r.texto).is_some()).count();
    let suelto = enlaces > 1 || paso.is_some_and(|p| (p / (tam * k) - 1.0).abs() > 0.15);
    let mut f = e.fabrica();
    let mut piezas = Vec::new();
    if suelto {
        for r in &z.renglones {
            let d = e.mapa.caja(&r.caja);
            piezas.push(un_texto(&r.texto, d.x, d.cy() - tam * k / 2.0, tam, familia, negrita || r.negrita, Alineacion::Izquierda, r.color.or(color)));
        }
    } else {
        let todo = z.renglones.iter().map(|r| r.texto.as_str()).collect::<Vec<_>>().join("\n");
        let primero = e.mapa.caja(&z.renglones[0].caja);
        let y = primero.cy() - tam * k / 2.0;
        let d = e.destino();
        let (w, _) = medir(&todo, tam as f32, familia, negrita);
        let x = match z.alineacion {
            Alineacion::Centro => d.cx() - w as f64 / 2.0,
            Alineacion::Derecha => d.x1() - w as f64,
            _ => z.renglones.iter().map(|r| e.mapa.caja(&r.caja).x).fold(f64::MAX, f64::min),
        };
        piezas.push(un_texto(&todo, x, y, tam, familia, negrita, z.alineacion, color));
    }
    f.de_motor(&piezas);
    // El marco de un recuadro.
    for r in &z.rayas {
        let (a, b) = (e.mapa.punto(r[0], r[1]), e.mapa.punto(r[2], r[3]));
        recta(&mut f, a, b, r[4] * esc);
    }
    f.salida
}

/// **Cada renglon en su sitio**, con las rayas de la zona: lo fiel.
fn fiel(e: &Entrada) -> Vec<Value> {
    fiel_con(e, None)
}

/// Lo mismo con otro texto para cada renglon (los corregidos por la nube).
fn fiel_con(e: &Entrada, textos: Option<&[String]>) -> Vec<Value> {
    let z = e.zona;
    // **Otro numero de renglones que el OCR** (`vision::renglones_libres`,
    // o los del supervisor): repartidos por el alto de la zona, con la letra
    // de sus renglones.
    if let Some(v) = textos.filter(|v| !v.is_empty() && v.len() != z.renglones.len()) {
        return libres(e, v);
    }
    let familia = &e.opciones.familia;
    let esc = e.mapa.escala;
    let k = pixpin_motor2d::texto::interlineado_de(familia).unwrap_or(pixpin_motor2d::texto::INTERLINEADO) as f64;
    let mut piezas = Vec::new();
    for (i, r) in z.renglones.iter().enumerate() {
        let tam = tam_de_letra(std::slice::from_ref(r), esc, familia, r.negrita);
        let t = textos.and_then(|v| v.get(i)).map_or(r.texto.as_str(), String::as_str);
        // (Vacio de la nube: lo suyo va en otro renglon, el de la fraccion.)
        if textos.is_some() && t.trim().is_empty() {
            continue;
        }
        // (Si se quedo con lo de otro trozo de su misma linea, ocupa los dos:
        // «2 día(s) Responder» salia diminuto en el sitio de «Responder».)
        let mut caja = r.caja;
        if let Some(v) = textos {
            for (j, o) in z.renglones.iter().enumerate() {
                if j != i && v.get(j).is_some_and(|x| x.trim().is_empty()) && (o.caja.cy() - r.caja.cy()).abs() < r.caja.h * 0.5 {
                    caja = caja.union(&o.caja);
                }
            }
        }
        let d = e.mapa.caja(&caja);
        // Un renglon entero en negrita (`**...**`): negrita de verdad, no las
        // marcas a la vista (18-oct, salia «*·IE ADVENTISTA PUNO**}»).
        // (Unas marcas descuadradas no se ensenan nunca.)
        let sin_sueltas;
        let t = if t.matches("**").count() % 2 == 1 {
            sin_sueltas = t.replace("**", "");
            sin_sueltas.as_str()
        } else {
            t
        };
        let (t, negrita) = match t.trim().strip_prefix("**").and_then(|x| x.strip_suffix("**")).filter(|x| !x.contains("**")) {
            Some(x) => (x.trim(), true),
            None => (t, r.negrita),
        };
        let mut el = un_texto(t, d.x, d.cy() - tam * k / 2.0, tam, familia, negrita, Alineacion::Izquierda, r.color);
        // Corregido (con sus formulas compuestas) no puede pasar del ancho de
        // su renglon: se pisaba con lo de al lado.
        // (Hasta el borde de su ZONA, no del renglon del OCR: «Calcule P» era
        // lo leido y la formula entera, mas ancha, salia diminuta.)
        let hasta = (e.destino().x1() - d.x).max(d.w);
        if textos.is_some() && el.ancho as f64 > hasta * 1.04 {
            let t2 = tam * (hasta * 1.02 / el.ancho as f64).max(0.6);
            el = un_texto(t, d.x, d.cy() - t2 * k / 2.0, t2, familia, negrita, Alineacion::Izquierda, r.color);
        }
        piezas.push(el);
    }
    let mut f = e.fabrica();
    f.de_motor(&piezas);
    for r in &z.rayas {
        let (a, b) = (e.mapa.punto(r[0], r[1]), e.mapa.punto(r[2], r[3]));
        recta(&mut f, a, b, r[4] * esc);
    }
    f.salida
}

/// Renglones sin sitio del OCR: uno tras otro, repartidos por la zona.
fn libres(e: &Entrada, lineas: &[String]) -> Vec<Value> {
    let z = e.zona;
    let familia = &e.opciones.familia;
    let d = e.destino();
    let esc = e.mapa.escala;
    let k = pixpin_motor2d::texto::interlineado_de(familia).unwrap_or(pixpin_motor2d::texto::INTERLINEADO) as f64;
    let alto_letra = if z.renglones.is_empty() { e.alto_letra } else { mediana(z.renglones.iter().map(|r| r.alto()).collect()).unwrap_or(e.alto_letra) };
    let paso_zona = d.h / lineas.len() as f64;
    // (Que quepan en su zona: con la del titulo salian enormes.)
    let tam = (alto_letra * esc * 1.05).min(paso_zona / k * 0.95).clamp(8.0, 72.0);
    let paso = paso_zona.max(tam * k);
    let color = super::seccion::color_mayoritario(z.renglones.iter().map(|r| (r.color, r.texto.chars().count())));
    let mut piezas = Vec::new();
    for (i, l) in lineas.iter().enumerate() {
        let sin_sueltas = if l.matches("**").count() % 2 == 1 { l.replace("**", "") } else { l.clone() };
        let (t, negrita) = match sin_sueltas.trim().strip_prefix("**").and_then(|x| x.strip_suffix("**")).filter(|x| !x.contains("**")) {
            Some(x) => (x.to_string(), true),
            None => (sin_sueltas.clone(), false),
        };
        let y = d.y + paso * i as f64 + (paso - tam * k) / 2.0;
        let mut el = un_texto(&t, d.x, y, tam, familia, negrita, Alineacion::Izquierda, color);
        if el.ancho as f64 > d.w * 1.04 {
            let t2 = tam * (d.w * 1.02 / el.ancho as f64).max(0.6);
            el = un_texto(&t, d.x, y, t2, familia, negrita, Alineacion::Izquierda, color);
        }
        piezas.push(el);
    }
    let mut f = e.fabrica();
    f.de_motor(&piezas);
    f.salida
}

fn recta(f: &mut Fabrica, a: (f64, f64), b: (f64, f64), grosor: f64) {
    f.trazo("line", &[a, b], NEGRO, None);
    if let Some(v) = f.salida.last_mut() {
        v["roughness"] = json!(0);
        v["strokeWidth"] = json!((grosor.max(1.0) * 2.0).round() / 2.0);
    }
}

/// **Un recuadro**: un rectangulo con su fondo (relleno solido), su borde y
/// sus esquinas, del tamano del de la hoja.
fn recuadro(e: &Entrada) -> Vec<Value> {
    let Some(st) = e.zona.panel else { return Vec::new() };
    let d = e.destino();
    let mut f = e.fabrica();
    let trazo = st.borde.map(imagen::hex).unwrap_or_else(|| "transparent".into());
    let relleno = st.relleno.map(imagen::hex);
    let mut v = f.base("rectangle", [d.x, d.y, d.w, d.h], &trazo, relleno.as_deref());
    v["strokeWidth"] = json!(if st.borde.is_some() { (st.grosor * e.mapa.escala).max(1.0) } else { 0.0 });
    v["roughness"] = json!(0);
    // (El radio del lienzo es fijo: en uno pequeno se ve enorme.)
    if st.redondo && d.w.min(d.h) >= 160.0 {
        v["roundness"] = json!({"type": 3});
    }
    f.salida.push(v);
    f.salida
}

fn linea(e: &Entrada) -> Vec<Value> {
    let mut f = e.fabrica();
    for r in &e.zona.rayas {
        let (a, b) = (e.mapa.punto(r[0], r[1]), e.mapa.punto(r[2], r[3]));
        recta(&mut f, a, b, r[4] * e.mapa.escala);
    }
    f.salida
}

// ---------------------------------------------------------------------------
// Tabla

/// **La tabla dibujada** del motor, con la letra que la hace tan ancha como
/// la del papel. `None` si no hay celdas.
fn tabla(e: &Entrada) -> Option<Vec<Value>> {
    let t = e.zona.tabla.as_ref()?;
    if t.filas.is_empty() || t.columnas() == 0 {
        return None;
    }
    let familia = e.opciones.familia.clone();
    let d = e.destino();
    let medir_t = |s: &str, tam: f32| crate::lienzo::medir_con(s, tam, &familia);
    let cabecera = t.parece_cabecera();
    let color = super::seccion::color_mayoritario(e.zona.renglones.iter().map(|r| (r.color, r.texto.chars().count())));
    let hacer = |tam: f64| {
        let estilo = Estilo { color: color_motor(color), tam: tam as f32, opacidad: 1.0, familia: familia.clone() };
        pixpin_motor2d::tabla_dibujada::elementos_de_tabla(&t.filas, &estilo, Punto2::nuevo(d.x as f32, d.y as f32), &medir_t, cabecera)
    };
    let caja_de = |els: &[Elemento]| {
        els.iter()
            .map(Elemento::caja)
            .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
            .map(|(x0, y0, x1, y1)| Caja::de_esquinas(x0 as f64, y0 as f64, x1 as f64, y1 as f64))
    };
    let mut tam = (e.alto_letra * e.mapa.escala).max(6.0);
    let mut els = hacer(tam);
    // Dos vueltas bastan: el aire de las celdas no escala con la letra.
    for _ in 0..2 {
        let c = caja_de(&els)?;
        if c.w <= 1.0 {
            break;
        }
        let k = (d.w / c.w).min(if c.h > 1.0 { d.h * 1.25 / c.h } else { 10.0 });
        let nuevo = (tam * k).clamp(6.0, 72.0);
        if (nuevo - tam).abs() < 0.5 {
            break;
        }
        tam = nuevo;
        els = hacer(tam);
    }
    if els.is_empty() {
        return None;
    }
    // Los enlaces de las celdas, abribles y de su color.
    for el in &mut els {
        if let pixpin_motor2d::Figura::Texto { texto, .. } = &el.figura
            && let Some(url) = enlace_de(texto)
        {
            if let Some(c) = e.zona.renglones.iter().find(|r| r.texto.contains(texto.as_str()) || texto.contains(r.texto.as_str())).and_then(|r| r.color) {
                el.trazo = color_motor(Some(c));
            }
            el.enlace = Some(url);
        }
    }
    let mut f = e.fabrica();
    f.de_motor(&els);
    Some(f.salida)
}

// ---------------------------------------------------------------------------
// Foto

/// Lo mas grande que se guarda una foto: dos veces su caja en el lienzo (se
/// ve nitida con zoom) y nunca mas de 2000 px de lado.
const LADO_FOTO: f64 = 2000.0;

/// Un id para el almacen de imagenes: distinto por hoja y zona, nunca cero,
/// por debajo de 2^48 (el `pc<hex>` cabe en doce cifras).
pub fn id_de_imagen(semilla: u64, zona: usize) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ semilla;
    for b in (zona as u64).to_le_bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    (h & 0x0000_ffff_ffff_ffff).max(2)
}

/// **Lo que no se dibuja** (una foto, un dibujo realista, un logo
/// detallado): el trozo de la hoja original en su sitio y a su escala, con
/// el recorte del propio lienzo (`crop` de la imagen, `RecorteImagen`): la
/// hoja entera va una sola vez al almacen (el mismo id en todas las zonas)
/// y cada zona la ensena recortada, con 2 px de aire.
fn foto(e: &Entrada) -> (Vec<Value>, Vec<ImagenNueva>) {
    let caja = sin_texto_alrededor(e, e.zona.caja).crecer(2.0);
    // **Sin el texto de otras zonas** (17-oct, «imagenes que se comen algo del
    // texto»): lo que cae dentro del recorte y es de otra zona ya va como
    // texto; en la imagen se tapa con su fondo, y el texto sale una sola vez.
    let suyas: Vec<Caja> = e.zona.renglones.iter().flat_map(|r| r.palabras.iter().map(|p| p.caja)).collect();
    let ajenas: Vec<Caja> = e
        .ajenas
        .iter()
        .filter(|w| w.comun(&caja) > 0.25 * w.area().max(1.0))
        .filter(|w| !suyas.iter().any(|s| s.comun(w) > 0.5 * w.area().max(1.0)))
        // (Lo que cae de lleno dentro de la zona es suyo aunque el OCR lo
        // apuntara en otra: tapado, la caja de «Cα=90°−α» salia vacia.)
        .filter(|w| !(matches!(e.zona.tipo, Tipo::Formula | Tipo::Dudosa) && w.dentro_de(&e.zona.caja) >= 0.9))
        .copied()
        .collect();
    // (Un dibujo, una formula o una dudosa recortados: sin la marca de agua.)
    let palido = !matches!(e.zona.tipo, Tipo::Foto);
    if (!ajenas.is_empty() || palido)
        && let Some((v, img)) = foto_tapada(e, &caja, &ajenas, palido)
    {
        return (vec![v], vec![img]);
    }
    match foto_recortada(e, &caja) {
        Some((v, img)) => (vec![v], vec![img]),
        None => (Vec::new(), Vec::new()),
    }
}

/// El lado mayor de la hoja que va al almacen para los recortes.
const LADO_HOJA: u32 = 2400;

/// El elemento imagen de `caja` (pixeles de la pagina) recortado de la hoja.
pub fn foto_recortada(e: &Entrada, caja: &Caja) -> Option<(Value, ImagenNueva)> {
    let (rgba, w, h) = e.pagina;
    let caja = caja.dentro(w as f64, h as f64);
    if caja.w < 1.0 || caja.h < 1.0 || w == 0 || h == 0 {
        return None;
    }
    let (hoja, hw, hh) = reducir(rgba, w, h, LADO_HOJA);
    let k = hw as f64 / w as f64;
    let id = id_de_imagen(e.semilla, 999_999_999);
    let d = e.mapa.caja(&caja);
    let mut f = e.fabrica();
    let mut v = f.base("image", [d.x, d.y, d.w, d.h], "transparent", None);
    v["id"] = json!(format!("ia-{:x}-foto-0", e.semilla_de_zona()));
    v["fileId"] = json!(pixpin_motor2d::enlace::id_de_texto(id));
    v["status"] = json!("saved");
    v["scale"] = json!([1.0, 1.0]);
    v["roughness"] = json!(0);
    v["strokeWidth"] = json!(0);
    v["crop"] = json!({
        "x": caja.x * k,
        "y": caja.y * k,
        "width": caja.w * k,
        "height": caja.h * k,
        "naturalWidth": hw,
        "naturalHeight": hh,
    });
    Some((v, ImagenNueva { id, rgba: hoja, ancho: hw, alto: hh }))
}


/// El recorte de `caja` con las palabras `tapar` cubiertas del color del
/// papel de alrededor (la mediana del borde del recorte), como imagen propia.
/// `palido`: ademas, lo gris claro (la marca de agua de una lamina) pasa a
/// ser papel; para lo que no es una foto de verdad.
fn foto_tapada(e: &Entrada, caja: &Caja, tapar: &[Caja], palido: bool) -> Option<(Value, ImagenNueva)> {
    let (rgba, w, h) = e.pagina;
    let c = caja.dentro(w as f64, h as f64);
    let (mut sub, sw, sh) = imagen::recortar(rgba, w, h, &c);
    if sw < 2 || sh < 2 {
        return None;
    }
    let mut borde: [Vec<u8>; 3] = Default::default();
    for x in 0..sw {
        for y in [0, sh - 1] {
            let i = ((y * sw + x) * 4) as usize;
            for k in 0..3 {
                borde[k].push(sub[i + k]);
            }
        }
    }
    let mut fondo = [255u8; 3];
    for k in 0..3 {
        borde[k].sort_unstable();
        fondo[k] = borde[k][borde[k].len() / 2];
    }
    if palido {
        let gris = |p: &[u8]| (p[0] as f64 * 299.0 + p[1] as f64 * 587.0 + p[2] as f64 * 114.0) / 1000.0;
        let gf = gris(&fondo);
        for px in sub.chunks_exact_mut(4) {
            let sat = px[..3].iter().max().unwrap() - px[..3].iter().min().unwrap();
            if sat < 30 && gris(px) >= gf - 50.0 {
                px[..3].copy_from_slice(&fondo);
            }
        }
    }
    for t in tapar {
        let q = t.crecer(1.5);
        let (x0, y0) = ((q.x - c.x).max(0.0) as u32, (q.y - c.y).max(0.0) as u32);
        let (x1, y1) = (((q.x1() - c.x).max(0.0) as u32).min(sw), ((q.y1() - c.y).max(0.0) as u32).min(sh));
        for y in y0..y1 {
            for x in x0..x1 {
                let i = ((y * sw + x) * 4) as usize;
                sub[i..i + 3].copy_from_slice(&fondo);
                // (Transparente, no solo del color del fondo: si la imagen
                // queda encima del texto de la otra zona, se ve a traves.)
                sub[i + 3] = 0;
            }
        }
    }
    let d = e.mapa.caja(&c);
    let lado = (d.w.max(d.h) * 2.0).clamp(64.0, LADO_FOTO);
    let (sub, sw, sh) = reducir(&sub, sw, sh, lado as u32);
    let id = id_de_imagen(e.semilla, e.zona.id * 1000 + 999);
    let mut f = e.fabrica();
    let mut v = f.base("image", [d.x, d.y, d.w, d.h], "transparent", None);
    v["id"] = json!(format!("ia-{:x}-foto-0", e.semilla_de_zona()));
    v["fileId"] = json!(pixpin_motor2d::enlace::id_de_texto(id));
    v["status"] = json!("saved");
    v["scale"] = json!([1.0, 1.0]);
    v["roughness"] = json!(0);
    v["strokeWidth"] = json!(0);
    Some((v, ImagenNueva { id, rgba: sub, ancho: sw, alto: sh }))
}

/// **Un trozo de la hoja como imagen del lienzo** (la foto de la zona, o el
/// icono `k` de una celda), en su sitio. `grupo`: el de la zona, si ya hay.
fn foto_de_caja(e: &Entrada, caja: &Caja, k: usize, grupo: Option<Value>) -> Option<(Value, ImagenNueva)> {
    let (rgba, w, h) = e.pagina;
    let (rgba, w, h) = imagen::recortar(rgba, w, h, caja);
    if w == 0 || h == 0 {
        return None;
    }
    let d = e.mapa.caja(caja);
    let lado = (d.w.max(d.h) * 2.0).clamp(64.0, LADO_FOTO);
    let (rgba, w, h) = reducir(&rgba, w, h, lado as u32);
    let id = id_de_imagen(e.semilla, e.zona.id * 1000 + k);
    // Su propio id, que el de una fabrica nueva (`ia-<zona>-2`) es el mismo
    // que el de la primera pieza de la tabla que lleva este icono.
    let mut f = e.fabrica();
    let mut v = f.base("image", [d.x, d.y, d.w, d.h], "transparent", None);
    v["id"] = json!(format!("ia-{:x}-foto-{k}", e.semilla_de_zona()));
    v["fileId"] = json!(pixpin_motor2d::enlace::id_de_texto(id));
    v["status"] = json!("saved");
    v["scale"] = json!([1.0, 1.0]);
    v["roughness"] = json!(0);
    v["strokeWidth"] = json!(0);
    if let Some(g) = grupo {
        v["groupIds"] = g;
    }
    Some((v, ImagenNueva { id, rgba, ancho: w, alto: h }))
}

/// Reduce (promediando) si el lado mayor pasa de `lado`.
pub fn reducir(rgba: &[u8], w: u32, h: u32, lado: u32) -> (Vec<u8>, u32, u32) {
    let mayor = w.max(h);
    if mayor <= lado || lado == 0 {
        return (rgba.to_vec(), w, h);
    }
    let k = mayor as f64 / lado as f64;
    let (nw, nh) = (((w as f64 / k).round() as u32).max(1), ((h as f64 / k).round() as u32).max(1));
    let mut out = vec![0u8; (nw * nh * 4) as usize];
    for y in 0..nh {
        let (sy0, sy1) = ((y as f64 * k) as u32, (((y + 1) as f64 * k) as u32).min(h).max((y as f64 * k) as u32 + 1));
        for x in 0..nw {
            let (sx0, sx1) = ((x as f64 * k) as u32, (((x + 1) as f64 * k) as u32).min(w).max((x as f64 * k) as u32 + 1));
            let mut s = [0u32; 4];
            let mut n = 0;
            for yy in sy0..sy1.min(h) {
                for xx in sx0..sx1.min(w) {
                    let i = ((yy * w + xx) * 4) as usize;
                    for c in 0..4 {
                        s[c] += rgba[i + c] as u32;
                    }
                    n += 1;
                }
            }
            let o = ((y * nw + x) * 4) as usize;
            for c in 0..4 {
                out[o + c] = (s[c] / n.max(1)) as u8;
            }
        }
    }
    (out, nw, nh)
}

// ---------------------------------------------------------------------------
// Formula

/// **Una formula leida, compuesta en su sitio.** Punto de enganche de las
/// lineas de formulas de una lamina (la columna «RESOLUCION»): aqui se
/// decide su color (el de la tinta del OCR) y su marco (el recuadro verde
/// de «θ=90°»); el texto rico con formulas en linea, cuando exista, entra
/// por aqui.
fn formula(e: &Entrada, latex: &[String]) -> Option<Vec<Value>> {
    let mut args = Map::new();
    // Los renglones de verdad (una respuesta trae a veces varios con `\` en uno).
    let lineas: Vec<String> = latex.iter().flat_map(|l| crate::ecuacion::partir_lineas(&latex_para_el_motor(l))).collect();
    args.insert("ecuacion".into(), json!(lineas.join("\n")));
    let mut avisos = Vec::new();
    let mut ec = crate::ecuacion::validar(&args, &mut avisos).ok()?;
    let d = e.destino();
    // **Varias lineas (la columna de una resolucion)**: texto rico del
    // lienzo, cada linea con su formula en linea y el color de su tinta, en
    // el sitio de su renglon si el OCR leyo tantos renglones como lineas.
    if lineas.len() >= 2 {
        let mut f = e.fabrica();
        let rs = &e.zona.renglones;
        let por_renglon = rs.len() == lineas.len();
        let color_zona = color_de_la_zona(e.zona);
        let hex = |c: [u8; 3]| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]);
        let de_color = |c: Option<[u8; 3]>| c.filter(|c| (*c.iter().max().unwrap() as i32 - *c.iter().min().unwrap() as i32) >= 40);
        let fuente_de = |l: &str, c: Option<[u8; 3]>| {
            let l = pixpin_motor2d::texto_rico::sin_barras_dobles(l);
            match de_color(c) {
                Some(c) => format!("$\\color{{{}}}{{{l}}}$", hex(c)),
                None => format!("${l}$"),
            }
        };
        let tam_base = (e.alto_letra * e.mapa.escala * 1.1).clamp(8.0, 96.0);
        let mut piezas = Vec::new();
        if por_renglon {
            for (l, r) in lineas.iter().zip(rs) {
                let c = e.mapa.caja(&r.caja);
                let tam = (c.h * 0.95).clamp(8.0, 96.0);
                piezas.push(un_texto(&fuente_de(l, r.color.or(color_zona)), c.x, c.y, tam, &e.opciones.familia, r.negrita, Alineacion::Izquierda, None));
            }
        } else {
            let fuente: Vec<String> = lineas.iter().map(|l| fuente_de(l, color_zona)).collect();
            piezas.push(un_texto(&fuente.join("\n"), d.x, d.y, tam_base, &e.opciones.familia, false, Alineacion::Izquierda, None));
        }
        f.de_motor(&piezas);
        marco(e, &mut f);
        return Some(f.salida);
    }
    ec.color = color_de_la_zona(e.zona).map_or_else(|| NEGRO.to_string(), |c| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]));
    ec.tam = (e.alto_letra * e.mapa.escala * 1.1).clamp(8.0, 96.0);
    ec.donde = Some((d.x, d.y));
    let (_, (w, h)) = crate::ecuacion::piezas(&ec);
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let k = (d.h / h as f64).min(d.w * 1.15 / w as f64).clamp(0.5, 1.3);
    ec.tam = (ec.tam * k).clamp(8.0, 120.0);
    // Un renglon: del alto de su letra en el papel (salia diminuta).
    // (Solo si es plano: con fracciones el alto de la zona no es el de la letra.)
    if lineas.len() == 1 && (h as f64) < 1.6 * ec.tam / k.max(1e-6) && e.zona.renglones.len() <= 1 {
        ec.tam = ec.tam.max(0.85 * d.h).min(120.0);
    }
    // (Sin renglones del OCR, la caja puede ser la de su marco: no mas de la
    // letra de la hoja y media.)
    if e.zona.renglones.is_empty() {
        ec.tam = ec.tam.min(e.alto_letra * e.mapa.escala * 1.5).max(8.0);
    }
    let (_, (w, h)) = crate::ecuacion::piezas(&ec);
    // Centrada en la caja del papel.
    ec.donde = Some((d.cx() - w as f64 / 2.0, d.cy() - h as f64 / 2.0));
    let (piezas, _) = crate::ecuacion::piezas(&ec);
    let mut f = e.fabrica();
    f.de_motor(&piezas);
    marco(e, &mut f);
    Some(f.salida)
}

/// El color de la tinta de una zona (el de sus palabras, el mas comun por
/// letras), si no es negro.
fn color_de_la_zona(z: &Zona) -> Option<[u8; 3]> {
    let mut cuenta: Vec<([u8; 3], usize)> = Vec::new();
    for p in z.renglones.iter().flat_map(|r| r.palabras.iter()) {
        let Some(c) = p.color else { continue };
        let n = p.texto.chars().count();
        match cuenta.iter_mut().find(|(k, _)| k.iter().zip(c.iter()).all(|(a, b)| (*a as i32 - *b as i32).abs() < 40)) {
            Some(x) => x.1 += n,
            None => cuenta.push((c, n)),
        }
    }
    let (c, _) = cuenta.into_iter().max_by_key(|x| x.1)?;
    let (mx, mn) = (*c.iter().max()? as i32, *c.iter().min()? as i32);
    (mx - mn >= 40).then_some(c)
}

/// **El recuadro alrededor de una formula** («θ=90°» en su caja verde): se
/// busca en el papel de alrededor y se dibuja con el rectangulo del lienzo.
fn marco(e: &Entrada, f: &mut Fabrica) {
    let (rgba, w, h) = e.pagina;
    let z = &e.zona.caja;
    let aire = e.alto_letra * 2.0;
    let c = z.crecer(aire).dentro(w as f64, h as f64);
    let (sub, sw, sh) = imagen::recortar(rgba, w, h, &c);
    if sw < 8 || sh < 8 {
        return;
    }
    let tapar: Vec<Caja> = e.zona.renglones.iter().flat_map(|r| r.palabras.iter()).map(|p| Caja::nueva(p.caja.x - c.x, p.caja.y - c.y, p.caja.w, p.caja.h)).collect();
    let o = crate::diagrama::Opciones { alto_letra: e.alto_letra, tapar, exacta: false, ..Default::default() };
    let mut d = crate::diagrama::vectorizar(&sub, sw, sh, &[], &o, None);
    // Solo un recuadro que rodea la formula (y no mucho mas grande).
    let (zx, zy) = (z.cx() - c.x, z.cy() - c.y);
    d.recuadros.retain(|r| r.x0 < zx && r.x1 > zx && r.y0 < zy && r.y1 > zy && (r.x1 - r.x0) <= z.w + 3.0 * aire && (r.y1 - r.y0) <= z.h + 3.0 * aire);
    if d.recuadros.is_empty() {
        return;
    }
    d.recuadros.truncate(1);
    let solo = crate::diagrama::Diagrama { ancho: d.ancho, alto: d.alto, recuadros: d.recuadros, ..Default::default() };
    let dest = e.mapa.caja(&c);
    // (Debajo de la formula: su relleno no la tapa.)
    let n0 = f.salida.len();
    crate::diagrama::elementos(&solo, (dest.x, dest.y), e.mapa.escala, f, &e.opciones.familia);
    let nuevos = f.salida.split_off(n0);
    f.salida.splice(0..0, nuevos);
}

// ---------------------------------------------------------------------------
// Calco y grafica

/// **El enganche con el calcado**: sus trazos vienen en el lienzo de 800x600
/// (`lienzo = original * escala + desplazamiento`); aqui vuelven a pixeles
/// del recorte y de ahi a la caja destino.
pub fn calco_en_caja(c: &crate::calcar::Calcado, destino: Caja, escala_pagina: f64, semilla: u64) -> Vec<Value> {
    use crate::calcar::{Calcado, Trazo};
    if c.escala <= 0.0 {
        return Vec::new();
    }
    let a = |(x, y): (f64, f64)| {
        (destino.x + (x - c.desplazamiento.0) / c.escala * escala_pagina, destino.y + (y - c.desplazamiento.1) / c.escala * escala_pagina)
    };
    let todos = |v: &[(f64, f64)]| v.iter().map(|&p| a(p)).collect::<Vec<_>>();
    let trazos = c
        .trazos
        .iter()
        .map(|t| match t {
            Trazo::Mancha { contorno, huecos, color } => Trazo::Mancha {
                contorno: todos(contorno),
                huecos: huecos.iter().map(|h| todos(h)).collect(),
                color: color.clone(),
            },
            Trazo::Linea { puntos, cerrada, color } => Trazo::Linea { puntos: todos(puntos), cerrada: *cerrada, color: color.clone() },
            Trazo::Raya { puntos, color, grosor } => Trazo::Raya { puntos: todos(puntos), color: color.clone(), grosor: grosor / c.escala * escala_pagina },
            Trazo::Fondo { caja, color } => {
                let (x0, y0) = a((caja[0], caja[1]));
                let (x1, y1) = a((caja[0] + caja[2], caja[1] + caja[3]));
                Trazo::Fondo { caja: [x0, y0, x1 - x0, y1 - y0], color: color.clone() }
            }
        })
        .collect();
    let recolocado = Calcado { trazos, escala: 1.0, desplazamiento: (0.0, 0.0) };
    crate::lienzo::elementos_de_calcado(&recolocado, (0.0, 0.0), semilla)
}

/// **Una grafica de funciones** de verdad (`graficar_funcion`) en su caja.
fn grafica(e: &Entrada, args: &Map<String, Value>) -> Option<Vec<Value>> {
    let mut avisos = Vec::new();
    let p = crate::funciones::validar(args, &mut avisos).ok()?;
    let d = e.destino();
    let piezas = crate::funciones::piezas(&p, (d.x, d.y, d.w.max(120.0), d.h.max(90.0)));
    if piezas.is_empty() {
        return None;
    }
    let mut f = e.fabrica();
    f.de_motor(&piezas);
    Some(f.salida)
}

// ---------------------------------------------------------------------------
// Diagrama

/// **El diagrama con figuras exactas** (`diagrama::vectorizar`): segmentos,
/// circunferencias y arcos, puntos, flechas, regiones, recuadros y los
/// rotulos (el texto del OCR de dentro y las letras sueltas). `None` si no
/// es un diagrama (un dibujo a mano, una foto): va al calco.
fn diagrama(e: &Entrada, leido: Option<&Leido>) -> Option<(Vec<Value>, String)> {
    // Con aire (12 px): un recuadro pegado al borde de la zona («θ=45°»)
    // entra entero.
    let (pagina, pw, ph) = e.pagina;
    let caja = e.zona.caja.crecer(12.0).dentro(pw as f64, ph as f64);
    // Lo que ve la nube (`png_del_recorte`): la zona con 6 px de aire.
    let de_la_nube = e.zona.caja.crecer(6.0).dentro(pw as f64, ph as f64);
    let (mut rgba, w, h) = imagen::recortar(pagina, pw, ph, &caja);
    if w < 8 || h < 8 {
        return None;
    }
    sin_marca_de_agua(&mut rgba);
    let z = e.zona;
    let palabras: Vec<super::PalabraHoja> = z
        .renglones
        .iter()
        .flat_map(|r| r.palabras.iter())
        .map(|p| {
            let mut q = p.clone();
            q.caja = Caja::nueva(p.caja.x - caja.x, p.caja.y - caja.y, p.caja.w, p.caja.h);
            q
        })
        .collect();
    // El texto de las OTRAS zonas (el de esta va como rotulo: «Inicio» en su ovalo).
    let propias: Vec<Caja> = z.renglones.iter().flat_map(|r| r.palabras.iter().map(|p| p.caja)).collect();
    let es_propia = |c: &Caja| propias.iter().any(|q| (q.x - c.x).abs() < 0.5 && (q.y - c.y).abs() < 0.5 && (q.w - c.w).abs() < 0.5);
    let tapar: Vec<Caja> = e.ajenas.iter().filter(|c| c.comun(&caja) > 0.0 && !es_propia(c)).map(|c| Caja::nueva(c.x - caja.x, c.y - caja.y, c.w, c.h)).collect();
    let mut pistas: Vec<String> = e.pistas.to_vec();
    pistas.extend(z.renglones.iter().map(|r| r.texto.clone()));
    let o = crate::diagrama::Opciones { alto_letra: e.alto_letra, tapar, pistas, ..Default::default() };
    let mut d = crate::diagrama::vectorizar(&rgba, w, h, &palabras, &o, e.lector);
    if !d.es_diagrama() {
        return None;
    }
    if let Some(Leido::Rotulos(v, hechos)) = leido {
        // Sus fracciones, pasadas a este recorte.
        let v: Vec<(String, f64, f64)> = v.iter().map(|(t, fx, fy)| (t.clone(), (de_la_nube.x + fx * de_la_nube.w - caja.x) / w as f64, (de_la_nube.y + fy * de_la_nube.h - caja.y) / h as f64)).collect();
        crate::diagrama::corregir_rotulos(&mut d, &v, e.alto_letra, Some((&rgba, w, h)));
        // Lo que la nube leyo encima del texto de OTRA zona (el «θ=90°» de la
        // formula de debajo, que asoma en el recorte) no es un rotulo de aqui.
        let ajenas: Vec<Caja> = o.tapar.iter().map(|c| c.crecer(e.alto_letra)).collect();
        d.rotulos.retain(|r| !ajenas.iter().any(|c| c.contiene(r.caja.cx(), r.caja.cy())));
        // Con las letras y los hechos de la nube, otra vuelta de geometria
        // exacta (lo ya exacto no se mueve).
        let mut pistas = o.pistas.clone();
        pistas.extend(hechos.iter().cloned());
        crate::diagrama::exacta(&mut d, &pistas, e.alto_letra);
    }
    let dest = e.mapa.caja(&caja);
    let mut f = e.fabrica();
    crate::diagrama::elementos(&d, (dest.x, dest.y), e.mapa.escala, &mut f, &e.opciones.familia);
    Some((f.salida, format!("diagrama: {}", d.resumen())))
}

/// **Sin la marca de agua** (18-oct, Practica: el «CÉSAR VALLEJO» gris
/// detras de las figuras salia como rectas y letras de mas): sobre papel
/// claro, lo gris y palido (poco color, poco mas oscuro que el papel) pasa
/// a papel. Las rectas negras y los rellenos de la figura, mucho mas
/// oscuros, se quedan.
fn sin_marca_de_agua(rgba: &mut [u8]) {
    let gris = |p: &[u8]| (p[0] as f64 * 299.0 + p[1] as f64 * 587.0 + p[2] as f64 * 114.0) / 1000.0;
    let mut g: Vec<u8> = rgba.chunks_exact(4).step_by(3).map(|p| gris(p) as u8).collect();
    if g.is_empty() {
        return;
    }
    g.sort_unstable();
    let papel = g[g.len() / 2] as f64;
    if papel < 215.0 {
        return;
    }
    for p in rgba.chunks_exact_mut(4) {
        let sat = p[..3].iter().max().unwrap() - p[..3].iter().min().unwrap();
        if sat < 30 && gris(p) >= papel - 70.0 {
            let v = papel as u8;
            p[0] = v;
            p[1] = v;
            p[2] = v;
        }
    }
}

/// Si la zona es un diagrama (sin leer sus letras: rapido), para saber de
/// antemano que no hay que esperar a la nube para ponerla.
pub fn es_diagrama(e: &Entrada) -> bool {
    let (mut rgba, w, h) = e.recorte();
    sin_marca_de_agua(&mut rgba);
    if w < 8 || h < 8 {
        return false;
    }
    let o = crate::diagrama::Opciones { alto_letra: e.alto_letra, tapar: e.ajenas_en_zona(), exacta: false, ..Default::default() };
    crate::diagrama::vectorizar(&rgba, w, h, &[], &o, None).es_diagrama()
}

/// **El LaTeX de la nube, limpio para el motor de formulas comun**: lo que
/// la nube escribe por error (`\gets` por el angulo, medido con DeepSeek el
/// 15-oct: salia «m\getsCOD») pasa a lo que quiso decir. Lo demas
/// (`\measuredangle`, `\overset{\frown}{AC}`) ya lo entiende el motor.
pub fn latex_para_el_motor(l: &str) -> String {
    l.replace(r"\gets ", r"\sphericalangle ").replace(r"\gets", r"\sphericalangle ")
}

#[cfg(test)]
mod pruebas_latex {
    #[test]
    fn el_angulo_mal_escrito_por_la_nube_se_arregla() {
        assert_eq!(super::latex_para_el_motor(r"m\getsCOD"), r"m\sphericalangle COD");
        assert_eq!(super::latex_para_el_motor(r"m\gets COD"), r"m\sphericalangle COD");
        assert_eq!(super::latex_para_el_motor(r"m\overset{\frown}{AC}=2a"), r"m\overset{\frown}{AC}=2a");
    }
}

#[cfg(test)]
mod pruebas_rico {
    #[test]
    fn una_linea_de_resolucion_sale_como_texto_rico() {
        let e = super::un_texto(r"$\color{#c04040}{m\measuredangle AOB=90^{\circ}}$", 0.0, 0.0, 20.0, "Work Sans", false, super::Alineacion::Izquierda, None);
        let pixpin_motor2d::elemento::Figura::Texto { texto, .. } = &e.figura else { panic!() };
        eprintln!("texto {texto:?} rico {:?}", e.extras.rico);
        assert!(!texto.contains(char::from(92)), "{texto}");
        assert!(e.extras.rico.is_some());
    }
}

/// **Un bloque de texto rico** (lo que transcribio la nube) como UN texto del
/// lienzo: arriba a la izquierda de la zona, con la letra medida en el papel
/// (el alto de sus renglones) y, si al componerlo se sale mucho de su caja,
/// encogido hasta caber.
fn bloque_rico(e: &Entrada, fuente: &str) -> Vec<Value> {
    let d = e.destino();
    let familia = &e.opciones.familia;
    // La letra, medida en los renglones de texto (los de una formula leidos
    // por el OCR miden mal: «2a+2b» encima de su raya).
    let de_texto: Vec<Renglon> = e
        .zona
        .renglones
        .iter()
        .filter(|r| {
            let n = r.texto.chars().count().max(1);
            r.texto.chars().filter(|c| c.is_alphabetic() || *c == ' ').count() * 10 >= n * 7 && n >= 6
        })
        .cloned()
        .collect();
    let rs = if de_texto.is_empty() { e.zona.renglones.clone() } else { de_texto };
    let mut tam = if rs.is_empty() { (e.alto_letra * e.mapa.escala * 1.05).max(6.0) } else { tam_de_letra(&rs, e.mapa.escala, familia, false) };
    // Una zona sola lleva su alineacion (un titulo centrado); un bloque, la
    // de cada renglon (`\centro`).
    let al = if fuente.contains('\n') { Alineacion::Izquierda } else { e.zona.alineacion };
    let mut el = un_texto(fuente, d.x, d.y, tam, familia, false, al, None);
    for _ in 0..3 {
        let k = (d.w * 1.3 / el.ancho.max(1.0) as f64).min(d.h * 1.5 / el.alto.max(1.0) as f64);
        if k >= 1.0 {
            break;
        }
        tam = (tam * k.max(0.6)).max(6.0);
        el = un_texto(fuente, d.x, d.y, tam, familia, false, al, None);
    }
    let mut f = e.fabrica();
    f.de_motor(&[el]);
    // Su recuadro de formula (el verde de «θ=90°»), si lo tiene.
    if !fuente.contains(char::from(10)) {
        marco(e, &mut f);
    }
    // El marco de un recuadro (el titulo en su caja).
    let esc = e.mapa.escala;
    for r in &e.zona.rayas {
        let (a, b) = (e.mapa.punto(r[0], r[1]), e.mapa.punto(r[2], r[3]));
        recta(&mut f, a, b, r[4] * esc);
    }
    f.salida
}

/// **Un recuadro de color con texto** (16-oct, el «Dato del mes» del
/// boletin: borde naranja y fondo crema): el seccionado lo toma por una
/// tabla de una celda, y salia con el borde negro y un trozo de su fondo
/// como «icono». Si la zona es un solo recuadro (el que mide el diagrama en
/// su recorte), va como el rectangulo del lienzo con su color y su relleno,
/// y encima sus renglones en su sitio.
fn caja_de_color(e: &Entrada) -> Option<Vec<Value>> {
    let (rgba, w, h) = e.pagina;
    let z = &e.zona;
    let c = z.caja.crecer(4.0).dentro(w as f64, h as f64);
    let (sub, sw, sh) = imagen::recortar(rgba, w, h, &c);
    if sw < 16 || sh < 16 {
        return None;
    }
    let tapar: Vec<Caja> = z.renglones.iter().flat_map(|r| r.palabras.iter()).map(|p| Caja::nueva(p.caja.x - c.x, p.caja.y - c.y, p.caja.w, p.caja.h)).collect();
    let o = crate::diagrama::Opciones { alto_letra: e.alto_letra, tapar, exacta: false, ..Default::default() };
    let d = crate::diagrama::vectorizar(&sub, sw, sh, &[], &o, None);
    // Un recuadro que es casi toda la zona, y nada mas que lo parta (una tabla
    // de verdad tiene rayas por dentro).
    let q = d.recuadros.iter().find(|q| (q.x1 - q.x0) >= 0.85 * z.caja.w && (q.y1 - q.y0) >= 0.85 * z.caja.h)?;
    let dentro = d.segmentos.iter().filter(|s| s.largo() >= 0.5 * (q.x1 - q.x0).min(q.y1 - q.y0) && s.a.0 > q.x0 + 4.0 && s.a.0 < q.x1 - 4.0 && s.a.1 > q.y0 + 4.0 && s.a.1 < q.y1 - 4.0).count();
    if dentro > 0 {
        return None;
    }
    let mut q = q.clone();
    // Su relleno: lo que hay dentro sin letras (el crema), si no es papel.
    if q.relleno.is_none() {
        let papel = crate::diagrama::mascara::fondo(&crate::diagrama::mascara::Img::de_rgba(&sub, sw as usize, sh as usize));
        let mut v: Vec<[f32; 3]> = Vec::new();
        for y in (q.y0 + q.grosor + 3.0) as u32..(q.y1 - q.grosor - 3.0).max(0.0) as u32 {
            for x in ((q.x0 + q.grosor + 3.0) as u32..(q.x1 - q.grosor - 3.0).max(0.0) as u32).step_by(3) {
                let i = ((y * sw + x) * 4) as usize;
                v.push([sub[i] as f32, sub[i + 1] as f32, sub[i + 2] as f32]);
            }
        }
        if !v.is_empty() {
            let canal = |k: usize| {
                let mut x: Vec<f32> = v.iter().map(|p| p[k]).collect();
                x.sort_by(f32::total_cmp);
                x[x.len() / 2]
            };
            let m = [canal(0), canal(1), canal(2)];
            if crate::diagrama::mascara::dist_color(m, papel) >= 8.0 {
                q.relleno = Some([m[0] as u8, m[1] as u8, m[2] as u8]);
            }
        }
    }
    let solo = crate::diagrama::Diagrama { ancho: d.ancho, alto: d.alto, recuadros: vec![q], ..Default::default() };
    let dest = e.mapa.caja(&c);
    let mut f = e.fabrica();
    crate::diagrama::elementos(&solo, (dest.x, dest.y), e.mapa.escala, &mut f, &e.opciones.familia);
    // Sus renglones en su sitio, sin las rayas de la «tabla» (ya estan).
    let mut sin_rayas = (*e.zona).clone();
    sin_rayas.rayas.clear();
    let e2 = Entrada { zona: &sin_rayas, mapa: e.mapa, opciones: e.opciones, pagina: e.pagina, alto_letra: e.alto_letra, semilla: e.semilla, lector: e.lector, ajenas: e.ajenas, pistas: e.pistas };
    // (Con su propio id: la fabrica de los renglones empieza por los mismos.)
    let mut els: Vec<Value> = f
        .salida
        .into_iter()
        .map(|mut v| {
            let id = v["id"].as_str().unwrap_or_default().to_string();
            v["id"] = json!(format!("{id}-caja"));
            v
        })
        .collect();
    els.extend(fiel(&e2));
    Some(els)
}

/// La caja de una foto sin el texto que el seccionado le metio arriba o
/// abajo (el parrafo de encima de la foto del boletin salia dos veces: como
/// texto y dentro del recorte).
fn sin_texto_alrededor(e: &Entrada, caja: Caja) -> Caja {
    // Solo una zona sin texto propio (una foto): el recorte de una formula o un
    // titulo leido como imagen es su texto.
    if !e.zona.renglones.is_empty() {
        return caja;
    }
    let mut c = caja;
    let palabras: Vec<&Caja> = e.ajenas.iter().filter(|w| caja.contiene(w.cx(), w.cy())).collect();
    let arriba = palabras.iter().filter(|w| w.cy() < caja.y + 0.4 * caja.h).map(|w| w.y1()).fold(f64::MIN, f64::max);
    let abajo = palabras.iter().filter(|w| w.cy() > caja.y1() - 0.4 * caja.h).map(|w| w.y).fold(f64::MAX, f64::min);
    let (mut y0, mut y1) = (c.y, c.y1());
    if arriba > f64::MIN {
        y0 = arriba + 2.0;
    }
    if abajo < f64::MAX {
        y1 = abajo - 2.0;
    }
    if y1 - y0 >= 0.3 * caja.h {
        c = Caja::de_esquinas(c.x, y0, c.x1(), y1);
    }
    c
}
