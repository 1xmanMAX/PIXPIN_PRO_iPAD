//! **Lo que entra y sale de una tabla por el portapapeles** (J2): pegar desde
//! Excel, Google Sheets o LibreOffice, y copiar de vuelta a cualquiera de
//! ellos. Es `motor/PortapapelesDeTabla.kt` del movil, regla a regla.
//!
//! # Lo que traen al copiar
//!
//! Las tres dejan texto con tabuladores (los valores como se ven) y una tabla
//! HTML, que es la que interesa porque puede traer mas:
//!
//! - **Google Sheets** pone en cada celda `data-sheets-formula` con la
//!   formula **en R1C1 relativa** (`=SUM(R[-3]C[0]:R[-1]C[0])`): se traduce a
//!   `A1` en el sitio donde se pega ([`r1c1`]) y funciona igual que alli.
//! - **Excel** marca los numeros (`x:num`) y, en algunas versiones, deja la
//!   formula en `x:fmla`, ya en `A1`: esa se pega tal cual.
//! - La **negrita** viaja en el estilo de la celda o como `<b>`.
//!
//! Si no hay tabla (se copio una palabra de otra aplicacion), el texto.
//!
//! # Lo que se deja al copiar
//!
//! Texto con tabuladores con **los valores** (lo que se pega en un chat es lo
//! que se ve, no `=SUMA(B2:B9)`) y una tabla HTML con cada formula en R1C1 en
//! `data-sheets-formula`: pegar en Sheets, en otra tabla de PixPin o en la
//! pagina web exportada conserva las formulas.
//!
//! Aqui no se toca el portapapeles de Windows: eso es
//! `pixpin_codec::portapapeles::tabla`. Todo lo de aqui es texto y se prueba.

use crate::formula;
use crate::importar_hojas::{self, MAX_COLS, MAX_FILAS};
use crate::tabla::{EstiloDeCelda, Ref, Tabla, ref_a};

/// Una celda pegada: lo que se escribe en ella y como hay que entenderlo.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Pegada {
    pub texto: String,
    /// La formula viene en R1C1 (la de Sheets) y hay que pasarla a `A1`.
    pub r1c1: bool,
    pub negrita: bool,
}

impl Pegada {
    fn de(texto: impl Into<String>) -> Pegada {
        Pegada {
            texto: texto.into(),
            ..Default::default()
        }
    }
}

/// **Lo que hay en el portapapeles**, como filas de celdas; `None` si no hay
/// nada. La tabla HTML manda sobre el texto.
pub fn leer(html: Option<&str>, texto: Option<&str>) -> Option<Vec<Vec<Pegada>>> {
    if let Some(h) = html.filter(|h| !h.trim().is_empty())
        && let Some(filas) = de_html(h).filter(|f| !f.is_empty())
    {
        return Some(filas);
    }
    let texto = texto.filter(|t| !t.is_empty())?;
    Some(
        de_tsv(texto)
            .into_iter()
            .map(|f| f.into_iter().map(Pegada::de).collect())
            .collect(),
    )
}

/// Texto con tabuladores, con las comillas de Excel: una celda con un salto
/// o un tabulador dentro va entre comillas, y sus comillas van dobladas.
pub fn de_tsv(texto: &str) -> Vec<Vec<String>> {
    importar_hojas::de_separado(texto, '\t')
}

/// Busca `aguja` (ya en minusculas) en `pajar_bajo` desde `desde`.
fn buscar(pajar_bajo: &str, aguja: &str, desde: usize) -> Option<usize> {
    pajar_bajo.get(desde..)?.find(aguja).map(|i| i + desde)
}

/// Quita los comentarios y los bloques `<style>`, `<script>` y `<head>`:
/// Excel mete alli su hoja de estilo entera, con `<td` que no son celdas.
fn sin_ruido(html: &str) -> String {
    let mut s = String::with_capacity(html.len());
    let mut resto = html;
    while let Some(i) = resto.find("<!--") {
        s.push_str(&resto[..i]);
        resto = match resto[i + 4..].find("-->") {
            Some(j) => &resto[i + 4 + j + 3..],
            None => "",
        };
    }
    s.push_str(resto);
    for bloque in ["style", "script", "head"] {
        loop {
            let bajo = s.to_ascii_lowercase();
            let Some(a) = buscar(&bajo, &format!("<{bloque}"), 0).filter(|&a| {
                bajo[a + 1 + bloque.len()..].starts_with(|c: char| c == '>' || c.is_whitespace())
            }) else {
                break;
            };
            let cierre = format!("</{bloque}>");
            let b = buscar(&bajo, &cierre, a).map_or(s.len(), |b| b + cierre.len());
            s.replace_range(a..b, "");
        }
    }
    s
}

/// El valor de un atributo en el texto de atributos de una etiqueta, ya sin
/// entidades. Presente sin valor, cadena vacia; ausente, `None`.
fn atributo(atributos: &str, nombre: &str) -> Option<String> {
    let bajo = atributos.to_ascii_lowercase();
    let nombre = nombre.to_ascii_lowercase();
    let mut desde = 0;
    while let Some(i) = buscar(&bajo, &nombre, desde) {
        desde = i + 1;
        let antes_bien = i == 0 || bajo[..i].ends_with(|c: char| c.is_whitespace());
        let tras = &atributos[i + nombre.len()..];
        let tras_bien =
            tras.is_empty() || tras.starts_with(|c: char| c.is_whitespace() || c == '=');
        if !antes_bien || !tras_bien {
            continue;
        }
        let tras = tras.trim_start();
        let Some(valor) = tras.strip_prefix('=') else {
            return Some(String::new());
        };
        let valor = valor.trim_start();
        let crudo = match valor.chars().next() {
            Some(q @ ('"' | '\'')) => valor[1..].split(q).next().unwrap_or(""),
            _ => valor
                .split(|c: char| c.is_whitespace() || c == '>')
                .next()
                .unwrap_or(""),
        };
        return Some(entidades(crudo));
    }
    None
}

/// Las entidades de un HTML: las cinco de siempre, `&nbsp;` y las numericas.
pub fn entidades(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut salida = String::with_capacity(s.len());
    let mut resto = s;
    while let Some(i) = resto.find('&') {
        salida.push_str(&resto[..i]);
        let tras = &resto[i..];
        let fin = tras[..tras.len().min(12)].find(';');
        let c = fin.and_then(|fin| {
            let e = &tras[1..fin];
            if let Some(h) = e.strip_prefix("#x").or_else(|| e.strip_prefix("#X")) {
                u32::from_str_radix(h, 16).ok().and_then(char::from_u32)
            } else if let Some(d) = e.strip_prefix('#') {
                d.parse().ok().and_then(char::from_u32)
            } else {
                match e.to_ascii_lowercase().as_str() {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" => Some('\''),
                    "nbsp" => Some(' '),
                    _ => None,
                }
            }
        });
        match (c, fin) {
            (Some(c), Some(fin)) => {
                salida.push(c);
                resto = &tras[fin + 1..];
            }
            _ => {
                salida.push('&');
                resto = &tras[1..];
            }
        }
    }
    salida.push_str(resto);
    salida
}

/// El texto de dentro de una celda: `<br>` es un salto, las etiquetas
/// fuera, los blancos juntos.
fn texto_de(html: &str) -> String {
    let mut s = String::with_capacity(html.len());
    let bajo = html.to_ascii_lowercase();
    let mut i = 0;
    while i < html.len() {
        if html[i..].starts_with('<') {
            let fin = html[i..].find('>').map_or(html.len(), |k| i + k + 1);
            if bajo[i..].starts_with("<br")
                && bajo[i + 3..].starts_with(|c: char| c == '>' || c == '/' || c.is_whitespace())
            {
                s.push('\0');
            }
            i = fin;
            continue;
        }
        let fin = html[i..].find('<').map_or(html.len(), |k| i + k);
        s.push_str(&html[i..fin]);
        i = fin;
    }
    let mut junto = String::with_capacity(s.len());
    let mut blanco = false;
    for c in s.chars() {
        if matches!(c, ' ' | '\t' | '\r' | '\n') {
            if !blanco {
                junto.push(' ');
            }
            blanco = true;
        } else {
            junto.push(c);
            blanco = false;
        }
    }
    entidades(&junto)
        .split('\0')
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn es_negrita_css(css: &str) -> bool {
    let bajo = css.to_ascii_lowercase();
    let mut desde = 0;
    while let Some(i) = buscar(&bajo, "font-weight:", desde) {
        let v = bajo[i + 12..].trim_start();
        if v.starts_with("bold")
            || (v.len() >= 3 && matches!(&v[..1], "6" | "7" | "8" | "9") && &v[1..3] == "00")
        {
            return true;
        }
        desde = i + 1;
    }
    false
}

/// **El numero de verdad, no el que se ve.** Sheets pone el valor exacto en
/// `data-sheets-value` (`{"1":3,"3":1234.5678}`, tipo 3 = numero) y Excel en
/// `x:num`, y lo que se ve puede venir redondeado. Solo cuando lo que se ve
/// ES un numero: una fecha o un porcentaje se quedan como se ven.
fn exacto(atributos: &str, texto: &str) -> Option<String> {
    if texto.contains('%') || formula::numero_escrito(texto).is_none() {
        return None;
    }
    if let Some(n) = atributo(atributos, "x:num").filter(|n| formula::numero_escrito(n).is_some()) {
        return Some(n);
    }
    let valor = atributo(atributos, "data-sheets-value")?;
    let sin_blancos: String = valor.chars().filter(|c| !c.is_whitespace()).collect();
    let tipo = sin_blancos.find("\"1\":")?;
    let tras_tipo = &sin_blancos[tipo + 4..];
    if !tras_tipo.starts_with('3') || tras_tipo[1..].starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let i = sin_blancos.find("\"3\":")? + 4;
    let n: String = sin_blancos[i..]
        .chars()
        .take_while(|c| c.is_ascii_digit() || matches!(c, '-' | '.' | 'e' | 'E' | '+'))
        .collect();
    n.parse::<f64>().ok().map(|_| n)
}

/// **Las filas de la primera tabla del HTML**, o `None` si no hay tabla.
pub fn de_html(html: &str) -> Option<Vec<Vec<Pegada>>> {
    let limpio = sin_ruido(html);
    let bajo = limpio.to_ascii_lowercase();
    let a = buscar(&bajo, "<table", 0)?;
    let b = buscar(&bajo, "</table>", a)?;
    let tabla = &limpio[a..b];
    let tabla_bajo = &bajo[a..b];
    let mut filas = Vec::new();
    let mut desde = 0;
    while let Some(tr) = buscar(tabla_bajo, "<tr", desde) {
        let Some(abre_fin) = buscar(tabla_bajo, ">", tr) else {
            break;
        };
        let cierra = buscar(tabla_bajo, "</tr>", abre_fin).unwrap_or(tabla.len());
        let dentro = &tabla[abre_fin + 1..cierra];
        let dentro_bajo = &tabla_bajo[abre_fin + 1..cierra];
        let mut fila = Vec::new();
        let mut k = 0;
        loop {
            let td = buscar(dentro_bajo, "<td", k);
            let th = buscar(dentro_bajo, "<th", k);
            let Some(c0) = [td, th].into_iter().flatten().min() else {
                break;
            };
            let Some(c1) = buscar(dentro_bajo, ">", c0) else {
                break;
            };
            let atributos = &dentro[c0 + 3..c1];
            let fin = [
                buscar(dentro_bajo, "</td>", c1),
                buscar(dentro_bajo, "</th>", c1),
            ]
            .into_iter()
            .flatten()
            .min()
            .unwrap_or(dentro.len());
            let interior = &dentro[c1 + 1..fin];
            k = fin + 1;
            let negrita = atributo(atributos, "style").is_some_and(|s| es_negrita_css(&s))
                || {
                    let ib = interior.to_ascii_lowercase();
                    ib.contains("<b>") || ib.contains("<b ") || ib.contains("<strong")
                }
                || es_negrita_css(interior);
            let texto = texto_de(interior);
            let de_sheets =
                atributo(atributos, "data-sheets-formula").filter(|f| f.starts_with('='));
            let de_excel = atributo(atributos, "x:fmla").filter(|f| f.starts_with('='));
            fila.push(if let Some(f) = de_sheets {
                Pegada {
                    texto: f,
                    r1c1: true,
                    negrita,
                }
            } else if let Some(f) = de_excel {
                Pegada {
                    texto: f,
                    r1c1: false,
                    negrita,
                }
            } else if texto.starts_with('=') {
                // Un texto que empieza por `=` en una hoja es texto: que no
                // se vuelva formula al pegarlo.
                Pegada {
                    texto: format!("'{texto}"),
                    r1c1: false,
                    negrita,
                }
            } else {
                Pegada {
                    texto: exacto(atributos, &texto).unwrap_or(texto),
                    r1c1: false,
                    negrita,
                }
            });
            let juntas: usize = atributo(atributos, "colspan")
                .and_then(|v| v.parse().ok())
                .unwrap_or(1);
            for _ in 1..juntas.clamp(1, 51) {
                fila.push(Pegada::default());
            }
        }
        filas.push(fila);
        desde = cierra + 1;
    }
    Some(filas)
}

/// Una celda de la tabla **como se ve**, para dibujarla: su texto (el valor,
/// no la formula), su negrita y cuantas filas y columnas ocupa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vista {
    pub texto: String,
    pub negrita: bool,
    pub filas: usize,
    pub columnas: usize,
    /// La tapa otra combinada: no se dibuja ni lleva rayas por dentro.
    pub tapada: bool,
}

impl Default for Vista {
    fn default() -> Vista {
        Vista {
            texto: String::new(),
            negrita: false,
            filas: 1,
            columnas: 1,
            tapada: false,
        }
    }
}

/// **La tabla del HTML tal como se ve**, con sus celdas combinadas en su
/// sitio (`rowspan` y `colspan`), para dibujarla en el lienzo.
///
/// El texto con tabuladores que dejan Sheets y Excel no sabe de celdas
/// combinadas: pone la combinada en su primera celda y deja las demas
/// vacias, y dibujada asi sale una raya partiendo «MENSUALIDAD RUBY» en tres.
/// Aqui cada celda tapada queda marcada y la rejilla es regular: todas las
/// filas con el mismo numero de columnas. `None` si no hay tabla.
pub fn vista_de_html(html: &str) -> Option<Vec<Vec<Vista>>> {
    let limpio = sin_ruido(html);
    let bajo = limpio.to_ascii_lowercase();
    let a = buscar(&bajo, "<table", 0)?;
    let b = buscar(&bajo, "</table>", a)?;
    let tabla = &limpio[a..b];
    let tabla_bajo = &bajo[a..b];
    // Lo que las combinadas de filas anteriores ya ocupan: (fila, col).
    let mut ocupadas = std::collections::HashSet::<(usize, usize)>::new();
    let mut filas: Vec<Vec<Vista>> = Vec::new();
    let mut desde = 0;
    while let Some(tr) = buscar(tabla_bajo, "<tr", desde) {
        if filas.len() >= MAX_FILAS as usize {
            break;
        }
        let Some(abre_fin) = buscar(tabla_bajo, ">", tr) else {
            break;
        };
        let cierra = buscar(tabla_bajo, "</tr>", abre_fin).unwrap_or(tabla.len());
        let dentro = &tabla[abre_fin + 1..cierra];
        let dentro_bajo = &tabla_bajo[abre_fin + 1..cierra];
        let f = filas.len();
        let mut fila: Vec<Vista> = Vec::new();
        let mut k = 0;
        loop {
            let td = buscar(dentro_bajo, "<td", k);
            let th = buscar(dentro_bajo, "<th", k);
            let Some(c0) = [td, th].into_iter().flatten().min() else {
                break;
            };
            let Some(c1) = buscar(dentro_bajo, ">", c0) else {
                break;
            };
            let atributos = &dentro[c0 + 3..c1];
            let fin = [
                buscar(dentro_bajo, "</td>", c1),
                buscar(dentro_bajo, "</th>", c1),
            ]
            .into_iter()
            .flatten()
            .min()
            .unwrap_or(dentro.len());
            let interior = &dentro[c1 + 1..fin];
            k = fin + 1;
            // Los huecos que tapa una combinada de arriba van antes.
            while ocupadas.contains(&(f, fila.len())) {
                fila.push(Vista {
                    tapada: true,
                    ..Vista::default()
                });
            }
            let negrita = atributo(atributos, "style").is_some_and(|s| es_negrita_css(&s))
                || {
                    let ib = interior.to_ascii_lowercase();
                    ib.contains("<b>") || ib.contains("<b ") || ib.contains("<strong")
                }
                || es_negrita_css(interior)
                // Una `<th>` es un titulo: el navegador la pone en negrita.
                || dentro_bajo[c0..].starts_with("<th");
            let tramo = |nombre: &str| {
                atributo(atributos, nombre)
                    .and_then(|v| v.trim().parse::<usize>().ok())
                    .unwrap_or(1)
                    .clamp(1, 50)
            };
            let (alto, ancho) = (tramo("rowspan"), tramo("colspan"));
            let col = fila.len();
            for df in 0..alto {
                for dc in 0..ancho {
                    if df > 0 {
                        ocupadas.insert((f + df, col + dc));
                    }
                }
            }
            fila.push(Vista {
                texto: texto_de(interior),
                negrita,
                filas: alto,
                columnas: ancho,
                tapada: false,
            });
            for _ in 1..ancho {
                fila.push(Vista {
                    tapada: true,
                    ..Vista::default()
                });
            }
        }
        // Una combinada de arriba puede tapar el final de esta fila.
        while ocupadas.contains(&(f, fila.len())) {
            fila.push(Vista {
                tapada: true,
                ..Vista::default()
            });
        }
        fila.truncate(MAX_COLS as usize);
        filas.push(fila);
        desde = cierra + 1;
    }
    // Las filas de rellenar del final (Sheets deja alguna vacia) fuera, y
    // todas al mismo ancho.
    while filas
        .last()
        .is_some_and(|f| f.iter().all(|c| c.texto.trim().is_empty() && !c.tapada))
    {
        filas.pop();
    }
    let ancho = filas.iter().map(Vec::len).max().unwrap_or(0);
    if filas.is_empty() || ancho == 0 {
        return None;
    }
    for f in &mut filas {
        f.resize(ancho, Vista::default());
    }
    // Una combinada no puede salirse de la tabla.
    let alto_total = filas.len();
    for (i, f) in filas.iter_mut().enumerate() {
        for (j, c) in f.iter_mut().enumerate() {
            c.filas = c.filas.min(alto_total - i);
            c.columnas = c.columnas.min(ancho - j);
        }
    }
    Some(filas)
}

/// `[-3]` es relativo; `5`, absoluto (la fila 5); nada, la misma. Con si
/// queda fijo.
fn parte(t: &str, desde: u32) -> Option<(i64, bool)> {
    if t.is_empty() {
        return Some((desde as i64, false));
    }
    if let Some(dentro) = t.strip_prefix('[').and_then(|x| x.strip_suffix(']')) {
        return dentro
            .parse::<i64>()
            .ok()
            .map(|d| (desde as i64 + d, false));
    }
    t.parse::<i64>().ok().map(|n| (n - 1, true))
}

/// **Una formula en R1C1** (`=SUM(R[-3]C[0]:R[-1]C)`) escrita en `A1` para la
/// celda (`fila`, `col`). Lo que queda fuera de la hoja, `#¡REF!`.
pub fn r1c1(formula: &str, fila: u32, col: u32) -> String {
    let cs: Vec<char> = formula.chars().collect();
    let n = cs.len();
    let mut s = String::with_capacity(formula.len() + 8);
    let mut i = 0;
    let palabra = |c: char| c.is_alphanumeric() || c == '.' || c == '_';
    while i < n {
        let c = cs[i];
        if c == '"' {
            // Hasta la comilla que cierra de verdad (las dobladas no cierran).
            let mut j = i + 1;
            while j < n {
                if cs[j] == '"' {
                    if cs.get(j + 1) == Some(&'"') {
                        j += 2;
                        continue;
                    }
                    break;
                }
                j += 1;
            }
            let fin = j.min(n - 1);
            s.extend(&cs[i..=fin]);
            i = fin + 1;
            continue;
        }
        let antes = if i > 0 { cs[i - 1] } else { ' ' };
        if (c == 'R' || c == 'r') && !palabra(antes) {
            // R(\[-?\d+\]|\d+)?C(\[-?\d+\]|\d+)?
            let tomar = |mut j: usize| -> (String, usize) {
                let ini = j;
                if cs.get(j) == Some(&'[') {
                    j += 1;
                    if cs.get(j) == Some(&'-') {
                        j += 1;
                    }
                    let d0 = j;
                    while j < n && cs[j].is_ascii_digit() {
                        j += 1;
                    }
                    if j > d0 && cs.get(j) == Some(&']') {
                        return (cs[ini..=j].iter().collect(), j + 1);
                    }
                    return (String::new(), ini);
                }
                while j < n && cs[j].is_ascii_digit() {
                    j += 1;
                }
                (cs[ini..j].iter().collect(), j)
            };
            let (pf, j) = tomar(i + 1);
            if matches!(cs.get(j), Some('C' | 'c')) {
                let (pc, k) = tomar(j + 1);
                let despues = cs.get(k).copied().unwrap_or(' ');
                if !palabra(despues) && despues != '(' {
                    match (parte(&pf, fila), parte(&pc, col)) {
                        (Some((f, ff)), Some((k2, cf)))
                            if (0..MAX_FILAS as i64).contains(&f)
                                && (0..MAX_COLS as i64).contains(&k2) =>
                        {
                            let dir = ref_a(Ref {
                                columna: k2 as u32,
                                fila: f as u32,
                            });
                            let corte = dir.find(|x: char| x.is_ascii_digit()).unwrap_or(dir.len());
                            if cf {
                                s.push('$');
                            }
                            s.push_str(&dir[..corte]);
                            if ff {
                                s.push('$');
                            }
                            s.push_str(&dir[corte..]);
                        }
                        _ => s.push_str("#¡REF!"),
                    }
                    i = k;
                    continue;
                }
            }
        }
        s.push(c);
        i += 1;
    }
    s
}

/// La formula de la celda (`fila`, `col`) en R1C1, para dejarla en
/// `data-sheets-formula`.
pub fn a_r1c1(formula: &str, fila: u32, col: u32) -> String {
    let Some(cuerpo) = formula.strip_prefix('=') else {
        return formula.to_string();
    };
    let mut s = String::from("=");
    let mut ultimo = 0;
    for p in importar_hojas::refs_en(cuerpo) {
        s.push_str(&cuerpo[ultimo..p.desde]);
        if p.fila_fija {
            s.push_str(&format!("R{}", p.fila + 1));
        } else {
            s.push_str(&format!("R[{}]", p.fila as i64 - fila as i64));
        }
        if p.columna_fija {
            s.push_str(&format!("C{}", p.columna + 1));
        } else {
            s.push_str(&format!("C[{}]", p.columna as i64 - col as i64));
        }
        ultimo = p.hasta;
    }
    s.push_str(&cuerpo[ultimo..]);
    s
}

/// Los valores como texto con tabuladores, con comillas donde hagan falta.
pub fn a_tsv(filas: &[Vec<String>]) -> String {
    filas
        .iter()
        .map(|f| {
            f.iter()
                .map(|v| {
                    if v.contains(['\t', '\n', '\r']) || v.starts_with('"') {
                        format!("\"{}\"", v.replace('"', "\"\""))
                    } else {
                        v.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join("\t")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn escapar(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// **La tabla HTML de lo copiado**: `valores` es lo que se ve y `crudos` lo
/// escrito; (`fila`, `col`) es la esquina, para pasar las formulas a R1C1.
pub fn a_html(
    valores: &[Vec<String>],
    crudos: &[Vec<String>],
    fila: u32,
    col: u32,
    negrita: &dyn Fn(u32, u32) -> bool,
) -> String {
    let mut s = String::from("<meta charset=\"utf-8\"><table>");
    for (i, fv) in valores.iter().enumerate() {
        s.push_str("<tr>");
        for (j, v) in fv.iter().enumerate() {
            s.push_str("<td");
            let crudo = crudos
                .get(i)
                .and_then(|f| f.get(j))
                .map_or("", String::as_str);
            let (f, c) = (fila + i as u32, col + j as u32);
            if Tabla::es_formula(crudo) {
                s.push_str(&format!(
                    " data-sheets-formula=\"{}\"",
                    escapar(&a_r1c1(crudo, f, c))
                ));
            }
            if negrita(f, c) {
                s.push_str(" style=\"font-weight:bold\"");
            }
            s.push('>');
            s.push_str(&escapar(v).replace('\n', "<br>"));
            s.push_str("</td>");
        }
        s.push_str("</tr>");
    }
    s.push_str("</table>");
    s
}

/// **Pega unas celdas** con la esquina en `en`: cada una en su sitio, la
/// formula de Sheets ya en `A1` para donde cae, y su negrita. Lo que se sale
/// de la hoja no entra. Devuelve cuantas celdas cambiaron.
pub fn pegar(t: &mut Tabla, en: Ref, filas: &[Vec<Pegada>]) -> usize {
    let mut cambiadas = 0;
    for (i, fila) in filas.iter().enumerate() {
        let f = en.fila as u64 + i as u64;
        if f >= MAX_FILAS as u64 {
            break;
        }
        for (j, p) in fila.iter().enumerate() {
            let c = en.columna as u64 + j as u64;
            if c >= MAX_COLS as u64 {
                break;
            }
            let r = Ref {
                columna: c as u32,
                fila: f as u32,
            };
            let texto = if p.r1c1 {
                r1c1(&p.texto, r.fila, r.columna)
            } else {
                p.texto.clone()
            };
            let mut cambio = false;
            if t.celda(r) != texto {
                t.poner(r, &texto);
                cambio = true;
            }
            // La negrita viaja con la celda, como en Excel: pegar una sin
            // negrita encima de una con negrita se la quita.
            let dir = ref_a(r);
            let estilo = t.estilos.entry(dir.clone()).or_default();
            if estilo.n != p.negrita {
                estilo.n = p.negrita;
                cambio = true;
            }
            if t.estilos.get(&dir).is_some_and(EstiloDeCelda::vacio) {
                t.estilos.remove(&dir);
            }
            cambiadas += usize::from(cambio);
        }
    }
    cambiadas
}

/// **Copia una celda** (o un rango, de `a` a `b`): el texto con tabuladores
/// con los valores y la tabla HTML con las formulas en R1C1.
pub fn copiar(t: &Tabla, a: Ref, b: Ref, decimal: char) -> (String, String) {
    let (c0, c1) = (a.columna.min(b.columna), a.columna.max(b.columna));
    let (f0, f1) = (a.fila.min(b.fila), a.fila.max(b.fila));
    let mut valores = Vec::new();
    let mut crudos = Vec::new();
    for f in f0..=f1 {
        let mut vf = Vec::new();
        let mut cf = Vec::new();
        for c in c0..=c1 {
            let r = Ref {
                columna: c,
                fila: f,
            };
            vf.push(formula::evaluar(t, r).mostrar(decimal));
            cf.push(t.celda(r).to_string());
        }
        valores.push(vf);
        crudos.push(cf);
    }
    let negrita = |f: u32, c: u32| {
        t.estilos
            .get(&ref_a(Ref {
                columna: c,
                fila: f,
            }))
            .is_some_and(|e| e.n)
    };
    (a_tsv(&valores), a_html(&valores, &crudos, f0, c0, &negrita))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::tabla::ref_de;

    /// Como la deja Google Sheets: la combinada lleva `rowspan` y las filas
    /// que tapa traen una celda menos.
    const GASTOS_DE_SHEETS: &str = r#"<meta charset="utf-8"><google-sheets-html-origin><table xmlns="http://www.w3.org/1999/xhtml" cellspacing="0" cellpadding="0" dir="ltr" border="1"><colgroup><col width="201"/><col width="128"/></colgroup><tbody>
<tr style="height:21px;"><td style="font-weight:bold;border:1px solid #000000;">DESCRIPCIÓN</td><td style="font-weight:bold;">MONTO</td></tr>
<tr style="height:21px;"><td rowspan="3" style="vertical-align:middle;">MENSUALIDAD RUBY</td><td data-sheets-value="{&quot;1&quot;:3,&quot;3&quot;:690}">690</td></tr>
<tr style="height:21px;"><td data-sheets-value="{&quot;1&quot;:3,&quot;3&quot;:687.5}">687.5</td></tr>
<tr style="height:21px;"><td>685.4</td></tr>
<tr style="height:21px;"><td rowspan="2">CELULAR</td><td>39.95</td></tr>
<tr style="height:21px;"><td>19.9</td></tr>
<tr style="height:21px;"><td>PAPÁ</td><td>200</td></tr>
<tr style="height:21px;"><td></td><td style="font-weight:bold;" data-sheets-formula="=SUM(R[-7]C:R[-1]C)">2517.34</td></tr>
<tr style="height:21px;"><td></td><td></td></tr>
</tbody></table>"#;

    #[test]
    fn la_tabla_de_sheets_con_celdas_combinadas_se_ve_con_ellas_en_su_sitio() {
        let v = vista_de_html(GASTOS_DE_SHEETS).unwrap();
        // La fila vacia del final no cuenta; todas las filas, dos columnas.
        assert_eq!(v.len(), 8);
        assert!(v.iter().all(|f| f.len() == 2));
        assert!(v[0][0].negrita && v[0][1].negrita);
        assert_eq!(v[0][0].texto, "DESCRIPCIÓN");
        // La combinada ocupa tres filas y las dos de debajo la tienen tapada:
        // «687.5» sigue en la columna del monto, no se corre a la izquierda.
        assert_eq!(
            (v[1][0].texto.as_str(), v[1][0].filas, v[1][0].columnas),
            ("MENSUALIDAD RUBY", 3, 1)
        );
        assert!(v[2][0].tapada && v[3][0].tapada);
        assert_eq!(v[2][1].texto, "687.5");
        assert_eq!(v[3][1].texto, "685.4");
        assert_eq!(v[4][0].filas, 2);
        assert!(v[5][0].tapada);
        assert_eq!(v[6][0].texto, "PAPÁ");
        assert!(
            !v[6][0].tapada,
            "tras la combinada, la fila vuelve a ser normal"
        );
        // Se ve el valor, no la formula, y con su negrita.
        assert_eq!(v[7][1].texto, "2517.34");
        assert!(v[7][1].negrita);
        assert!(!v[1][1].negrita);
    }

    #[test]
    fn una_combinada_de_columnas_tapa_las_de_su_derecha_y_no_se_sale() {
        let html = "<table><tr><th colspan=2>Total del mes</th><td>x</td></tr><tr><td>a</td><td rowspan=9>b</td><td>c</td></tr></table>";
        let v = vista_de_html(html).unwrap();
        assert_eq!(v[0][0].columnas, 2);
        assert!(v[0][0].negrita, "una <th> es un titulo");
        assert!(v[0][1].tapada);
        assert_eq!(v[0][2].texto, "x");
        assert_eq!(v[1][1].filas, 1, "no ocupa filas que la tabla no tiene");
    }

    #[test]
    fn sin_tabla_en_el_html_no_hay_vista() {
        assert!(vista_de_html("<p>hola</p>").is_none());
        assert!(vista_de_html("<table></table>").is_none());
        assert!(vista_de_html("").is_none());
    }

    #[test]
    fn lo_que_copia_excel_como_texto_se_pega_en_filas_y_columnas() {
        let filas = leer(None, Some("Pan\t1,50\r\nLeche\t\"3\n litros\"\r\n")).unwrap();
        assert_eq!(filas.len(), 2);
        assert_eq!(filas[0][1].texto, "1,50");
        assert_eq!(
            filas[1][1].texto, "3\n litros",
            "una celda con salto va entre comillas"
        );
        // Casos negativos: nada, o vacio.
        assert!(leer(None, None).is_none());
        assert!(leer(Some("  "), Some("")).is_none());
    }

    #[test]
    fn la_tabla_de_excel_trae_sus_numeros_exactos_su_negrita_y_su_formula() {
        // Lo que deja Excel 365 en `HTML Format`: su hoja de estilo en
        // `<style>` (con `td` que no son celdas), `x:num` y a veces `x:fmla`.
        let html = r#"<html><head><style>td {mso-number-format:General;} .xl65{font-weight:700}</style></head><body>
            <!--StartFragment--><table><tr><td class=xl65 style='font-weight:700'>Total</td><td x:num="1234.5678">1.234,57</td>
            <td x:num x:fmla="=SUM(B1:B3)">9</td><td>=no</td><td colspan=2>junta</td><td>a&amp;b&nbsp;c<br>d</td></tr></table><!--EndFragment--></body></html>"#;
        let filas = leer(Some(html), Some("Total\t1.234,57")).unwrap();
        let f = &filas[0];
        assert_eq!(
            f[0],
            Pegada {
                texto: "Total".into(),
                r1c1: false,
                negrita: true
            }
        );
        assert_eq!(
            f[1].texto, "1234.5678",
            "el numero de verdad, no el redondeado"
        );
        assert_eq!(f[2].texto, "=SUM(B1:B3)");
        assert!(!f[2].r1c1);
        assert_eq!(
            f[3].texto, "'=no",
            "un texto con = delante sigue siendo texto"
        );
        assert_eq!(f[4].texto, "junta");
        assert_eq!(f[5].texto, "", "la celda combinada ocupa su sitio");
        assert_eq!(f[6].texto, "a&b c\nd");
    }

    #[test]
    fn la_formula_de_sheets_en_r1c1_se_escribe_para_donde_se_pega() {
        let html = r#"<google-sheets-html-origin><table><tr><td data-sheets-value="{&quot;1&quot;:3,&quot;3&quot;:0.125}">0.13</td><td data-sheets-formula="=SUM(R[-3]C[0]:R[-1]C[0])+R1C1">6</td></tr></table>"#;
        let filas = de_html(html).unwrap();
        assert_eq!(filas[0][0].texto, "0.125");
        assert!(filas[0][1].r1c1);
        let mut t = Tabla::default();
        pegar(&mut t, ref_de("B5").unwrap(), &filas);
        assert_eq!(t.celda(ref_de("B5").unwrap()), "0.125");
        assert_eq!(t.celda(ref_de("C5").unwrap()), "=SUM(C2:C4)+$A$1");
    }

    #[test]
    fn lo_que_se_sale_de_la_hoja_en_r1c1_es_ref() {
        assert_eq!(r1c1("=R[-1]C", 0, 0), "=#¡REF!");
        assert_eq!(r1c1("=RC[1]*2", 4, 1), "=C5*2");
        // Lo que va entre comillas y las funciones que empiezan por R no se tocan.
        assert_eq!(
            r1c1("=ROUND(RC[-1],2)&\"R1C1\"", 0, 1),
            "=ROUND(A1,2)&\"R1C1\""
        );
    }

    #[test]
    fn copiar_deja_valores_para_el_chat_y_formulas_para_la_hoja() {
        let mut t = Tabla::default();
        t.poner(ref_de("A1").unwrap(), "2");
        t.poner(ref_de("A2").unwrap(), "=A1*3");
        t.estilos.insert(
            "A2".into(),
            EstiloDeCelda {
                n: true,
                ..Default::default()
            },
        );
        let (tsv, html) = copiar(&t, ref_de("A1").unwrap(), ref_de("A2").unwrap(), '.');
        assert_eq!(tsv, "2\n6");
        assert!(html.contains(
            "<td data-sheets-formula=\"=R[-1]C[0]*3\" style=\"font-weight:bold\">6</td>"
        ));
        // Y lo copiado vuelve a pegarse igual en otro sitio.
        let filas = de_html(&html).unwrap();
        let mut otra = Tabla::default();
        pegar(&mut otra, ref_de("C3").unwrap(), &filas);
        assert_eq!(otra.celda(ref_de("C4").unwrap()), "=C3*3");
        assert!(otra.estilos["C4"].n);
        assert!(
            !otra.estilos.contains_key("C3"),
            "sin negrita no se guarda un estilo vacio"
        );
    }

    #[test]
    fn pegar_fuera_de_la_hoja_no_entra_y_pegar_lo_mismo_no_cuenta() {
        let mut t = Tabla::default();
        let filas = vec![vec![Pegada::de("1"), Pegada::de("2")]];
        let ultima = Ref {
            columna: MAX_COLS - 1,
            fila: 0,
        };
        assert_eq!(pegar(&mut t, ultima, &filas), 1, "la segunda se saldria");
        assert_eq!(
            pegar(&mut t, ultima, &filas),
            0,
            "lo mismo otra vez no cambia nada"
        );
    }
}
