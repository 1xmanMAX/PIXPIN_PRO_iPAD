//! **Encontrar en la pagina lo que dijo el modelo** (el `ancla`).
//!
//! El modelo copia un trozo del texto; aqui se busca en las palabras de la
//! pagina sin mirar mayusculas, tildes, signos ni espacios (un modelo
//! escribe «f(x) = 2x + 1» donde el PDF tiene «f(x)=2x+1»; el OCR parte o
//! junta palabras). Si no esta tal cual (el OCR leyo «rnodelo» por
//! «modelo», el modelo cambio una palabra), se busca el trozo de la pagina
//! que mas se le parece palabra a palabra. Con varios sitios iguales manda
//! el renglon que dijo el modelo. Lo que no se parece lo bastante **no se
//! anota**: mejor una anotacion menos que una en el sitio equivocado.

use super::{Pagina, union};
use crate::acciones::normalizar;

/// Donde quedo anclada una anotacion: una caja por renglon tocado.
#[derive(Debug, Clone, PartialEq)]
pub struct Anclaje {
    pub cajas: Vec<[f32; 4]>,
    /// El primer y el ultimo renglon tocados (indices de la pagina).
    pub renglon: usize,
    pub ultimo: usize,
    /// Si se encontro tal cual (no por parecido).
    pub exacto: bool,
}

impl Anclaje {
    pub fn union(&self) -> [f32; 4] {
        union(self.cajas.iter().copied()).unwrap_or([0.0; 4])
    }
}

/// Una palabra de la pagina para comparar: donde esta y como se compara.
struct Ficha {
    renglon: usize,
    palabra: usize,
    norma: String,
}

/// Sin mayusculas, tildes ni signos: solo letras y numeros.
fn norma(s: &str) -> String {
    normalizar(s)
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

fn fichas(p: &Pagina) -> Vec<Ficha> {
    let mut v = Vec::new();
    for (r, ren) in p.renglones.iter().enumerate() {
        for (k, w) in ren.palabras.iter().enumerate() {
            let n = norma(&w.texto);
            if !n.is_empty() {
                v.push(Ficha {
                    renglon: r,
                    palabra: k,
                    norma: n,
                });
            }
        }
    }
    v
}

/// Las cajas de las fichas `desde..hasta`, una por renglon.
fn cajas_de(p: &Pagina, f: &[Ficha], desde: usize, hasta: usize, exacto: bool) -> Option<Anclaje> {
    let tramo = f.get(desde..hasta)?;
    let primero = tramo.first()?.renglon;
    let ultimo = tramo.last()?.renglon;
    let mut cajas = Vec::new();
    for r in primero..=ultimo {
        let de_r = tramo
            .iter()
            .filter(|x| x.renglon == r)
            .map(|x| p.renglones[r].palabras[x.palabra].caja);
        if let Some(c) = union(de_r) {
            cajas.push(c);
        }
    }
    (!cajas.is_empty()).then_some(Anclaje {
        cajas,
        renglon: primero,
        ultimo,
        exacto,
    })
}

/// **Ancla `ancla` en la pagina.** `linea`: el renglon que dijo el modelo
/// (desde 1), para elegir entre varios sitios o, si el texto no aparece, el
/// renglon entero... solo si el texto se le parece algo.
pub fn anclar(p: &Pagina, ancla: &str, linea: Option<usize>) -> Option<Anclaje> {
    if let Some(a) = por_celda(p, ancla) {
        return Some(a);
    }
    let f = fichas(p);
    if f.is_empty() {
        return None;
    }
    let pedido = norma(ancla);
    if pedido.is_empty() {
        return None;
    }
    let cerca = |r: usize| linea.map_or(0, |l| (r as i64 - (l as i64 - 1)).unsigned_abs() as usize);
    // 1. Tal cual, sin espacios ni signos, empezando y acabando en palabra.
    let mut exactos = tal_cual(&f, &pedido);
    exactos.sort_by_key(|(d, _)| cerca(f[*d].renglon));
    if let Some((d, h)) = exactos.first() {
        return cajas_de(p, &f, *d, *h, true);
    }
    // 2. Por parecido, palabra a palabra.
    let palabras: Vec<String> = ancla
        .split_whitespace()
        .map(norma)
        .filter(|s| !s.is_empty())
        .collect();
    if let Some((d, h)) = parecido(&f, &palabras, &cerca) {
        return cajas_de(p, &f, d, h, false);
    }
    None
}

/// Los sitios donde esta `pedido` tal cual: `(desde, hasta)` en fichas.
/// Ha de empezar al principio de una palabra y acabar al final de otra
/// (si no, «de» se encontraria dentro de «cuaderno»); con un pedido largo
/// (8 letras o mas) vale tambien a medias por los extremos, que es lo que
/// pasa cuando el modelo corta la primera o la ultima palabra.
fn tal_cual(f: &[Ficha], pedido: &str) -> Vec<(usize, usize)> {
    let largo = pedido.chars().count();
    let mut v = Vec::new();
    for estricto in [true, false] {
        if !estricto && largo < 8 {
            break;
        }
        for d in 0..f.len() {
            let mut acum = String::new();
            for h in d..f.len() {
                acum.push_str(&f[h].norma);
                if acum.len() > pedido.len() + f[h].norma.len() + f[d].norma.len() {
                    break;
                }
                let ok = if estricto {
                    acum == pedido
                } else {
                    // A medias: el pedido dentro, cortado solo en la primera
                    // y la ultima palabra (sin la primera ya no estaria), y
                    // cubriendo casi todo el tramo.
                    acum.contains(pedido)
                        && !acum[f[d].norma.len()..].contains(pedido)
                        && pedido.len() * 10 >= acum.len() * 7
                };
                if ok {
                    v.push((d, h + 1));
                    break;
                }
                if estricto && !pedido.starts_with(&acum) {
                    break;
                }
            }
        }
        if !v.is_empty() {
            break;
        }
    }
    v
}

/// Distancia de edicion acotada (para «rnodelo» / «modelo»).
fn distancia(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut antes: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut ahora = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            let c = usize::from(a[i - 1] != b[j - 1]);
            ahora[j] = (antes[j] + 1).min(ahora[j - 1] + 1).min(antes[j - 1] + c);
        }
        antes = ahora;
    }
    antes[b.len()]
}

/// Lo que se parecen dos palabras (0 a 1).
fn semejanza(a: &str, b: &str) -> f32 {
    if a == b {
        return 1.0;
    }
    let (la, lb) = (a.chars().count(), b.chars().count());
    if la.min(lb) >= 4 && (la.abs_diff(lb) <= 2 || a.starts_with(b) || b.starts_with(a)) {
        let d = distancia(a, b);
        if d == 1 {
            return 0.85;
        }
        if d == 2 && la.min(lb) >= 7 {
            return 0.7;
        }
        if a.starts_with(b) || b.starts_with(a) {
            return 0.7;
        }
    }
    0.0
}

/// El tramo de la pagina que mas se parece a `palabras`: `(desde, hasta)`.
/// Tiene que parecerse en un 70 % o mas (y en dos palabras como minimo, o
/// en una larga casi igual).
fn parecido(
    f: &[Ficha],
    palabras: &[String],
    cerca: &dyn Fn(usize) -> usize,
) -> Option<(usize, usize)> {
    let n = palabras.len();
    if n == 0 {
        return None;
    }
    if n == 1 && palabras[0].chars().count() < 5 {
        return None;
    }
    // Lo que se parece cada palabra pedida a cada una de la pagina, una sola
    // vez (y solo las que se parecen algo): las ventanas solo suman.
    let parecidas: Vec<Vec<(usize, f32)>> = palabras
        .iter()
        .map(|q| {
            f.iter()
                .enumerate()
                .map(|(i, w)| (i, semejanza(q, &w.norma)))
                .filter(|x| x.1 > 0.0)
                .collect()
        })
        .collect();
    let mut mejor: Option<(f32, usize, usize)> = None;
    for largo in [n, n + 1, n.saturating_sub(1).max(1)] {
        if largo > f.len() {
            continue;
        }
        for d in 0..=(f.len() - largo) {
            let suma: f32 = parecidas
                .iter()
                .map(|l| {
                    l.iter()
                        .filter(|(i, _)| (d..d + largo).contains(i))
                        .map(|x| x.1)
                        .fold(0.0, f32::max)
                })
                .sum();
            let nota = suma / n.max(largo) as f32;
            let mejor_que = match mejor {
                None => true,
                Some((m, md, _)) => {
                    nota > m + 1e-4
                        || ((nota - m).abs() <= 1e-4 && cerca(f[d].renglon) < cerca(f[md].renglon))
                }
            };
            if mejor_que {
                mejor = Some((nota, d, d + largo));
            }
        }
    }
    let (nota, d, h) = mejor?;
    (nota >= 0.7).then_some((d, h))
}

/// El primer renglon con texto (el ancla de un resumen de la pagina).
pub fn primer_renglon(p: &Pagina) -> Option<Anclaje> {
    let (r, ren) = p
        .renglones
        .iter()
        .enumerate()
        .find(|(_, r)| !r.palabras.is_empty())?;
    Some(Anclaje {
        cajas: vec![ren.caja()?],
        renglon: r,
        ultimo: r,
        exacto: true,
    })
}

// ---------------------------------------------------------------------------
// Celdas de una hoja de calculo

/// «B4» → (fila 3, columna 1), desde 0.
pub fn celda(s: &str) -> Option<(u32, u32)> {
    let s = s.trim().trim_start_matches('$').replace('$', "");
    let letras: String = s.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    let numero = &s[letras.len()..];
    if letras.is_empty()
        || letras.len() > 3
        || numero.is_empty()
        || !numero.chars().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    let col = letras
        .to_ascii_uppercase()
        .bytes()
        .fold(0u32, |a, b| a * 26 + (b - b'A' + 1) as u32)
        - 1;
    let fila: u32 = numero.parse().ok()?;
    (fila >= 1).then_some((fila - 1, col))
}

/// «B4:D9» → las dos esquinas ordenadas; «B4», la misma dos veces.
pub fn rango(s: &str) -> Option<((u32, u32), (u32, u32))> {
    let s = s.trim();
    let (a, b) = s.split_once(':').unwrap_or((s, s));
    let (a, b) = (celda(a)?, celda(b)?);
    Some(((a.0.min(b.0), a.1.min(b.1)), (a.0.max(b.0), a.1.max(b.1))))
}

/// Un ancla que es una celda o un rango: las celdas con etiqueta dentro.
fn por_celda(p: &Pagina, ancla: &str) -> Option<Anclaje> {
    let ((f0, c0), (f1, c1)) = rango(ancla)?;
    let mut cajas = Vec::new();
    let (mut primero, mut ultimo) = (usize::MAX, 0);
    for (i, r) in p.renglones.iter().enumerate() {
        let Some((f, c)) = r.etiqueta.as_deref().and_then(celda) else {
            continue;
        };
        if (f0..=f1).contains(&f)
            && (c0..=c1).contains(&c)
            && let Some(caja) = r.caja()
        {
            cajas.push(caja);
            primero = primero.min(i);
            ultimo = ultimo.max(i);
        }
    }
    let u = union(cajas.iter().copied())?;
    Some(Anclaje {
        cajas: vec![u],
        renglon: primero,
        ultimo,
        exacto: true,
    })
}
