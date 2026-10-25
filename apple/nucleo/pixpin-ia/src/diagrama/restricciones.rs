//! **La geometria exacta**: cada figura con su propio marco y el texto como
//! pistas.
//!
//! Queja del usuario (15-oct-2026, lamina «CIRCUNFERENCIA I»): «las
//! graficas geometricas deben graficarse PERFECTAMENTE: circunferencias,
//! semicircunferencias, rectas, punteadas... un semicirculo se dibuja con la
//! herramienta de CIRCULO y luego se recorta. No todo comparte el mismo eje:
//! cada figura tiene los suyos. Y que se guie del TEXTO: el texto da pistas
//! de que es cada cosa».
//!
//! Lo medido (`geometria`) ya son rectas, circunferencias y arcos ajustados
//! por minimos cuadrados, pero cada uno por su cuenta: el diametro de la
//! semicircunferencia no pasaba por su centro, el cuadrante tenia 88° y
//! OA != OB, un punto «sobre» la circunferencia caia a 2 px de ella. Aqui:
//!
//! 1. **Figuras** ([`figuras`]): la lamina se parte en las piezas que se
//!    tocan (componentes de la geometria). Cada figura tiene su escala y sus
//!    tolerancias (relativas a su tamano) y nada se impone de una a otra.
//! 2. **Nudos**: los puntos y los extremos de rectas y arcos que coinciden
//!    son un nudo; las letras sueltas (A, B, O, P) nombran al nudo de al lado.
//! 3. **Hechos** ([`hechos_de_texto`]): del OCR de la hoja y de la nube
//!    («semicircunferencia», «AOB es un cuadrante», «AB es diametro»,
//!    «m∢AOB=90°», «O centro», «θ=90°»).
//! 4. **Restricciones**, unas vueltas hasta que cuadran: horizontales y
//!    verticales casi exactas -> exactas; angulos casi rectos -> rectos;
//!    arco de casi 180° -> semicircunferencia con su diametro (centro en el
//!    punto medio); arco de casi 90° con su centro -> cuadrante (90° y
//!    OA = OB); los hechos del texto, con mas holgura; los nudos que caen
//!    casi sobre una circunferencia o una recta, sobre ella.
//! 5. **Marcas de angulo**: la mancha de color en un vertice pasa a sector
//!    exacto (centro en el vertice, entre sus dos rayas), y los bordes de
//!    las regiones se pegan a las rayas y arcos que las cierran.
//!
//! Todo en Rust, sin modelo, en milisegundos: la nube solo aporta hechos.

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use super::ajuste::dist;
use super::mascara::P;
use super::{Diagrama, Region};

// ---------------------------------------------------------------------------
// Hechos del texto

/// **Lo que dice el texto** de la figura.
#[derive(Debug, Clone, PartialEq)]
pub enum Hecho {
    /// «semicircunferencia»: el arco de casi 180° es una semicircunferencia
    /// (180° exactos, centro en el punto medio de su diametro).
    Semicircunferencia,
    /// «cuadrante»: el arco de casi 90° con su centro es un cuadrante.
    Cuadrante,
    /// «AOB es un cuadrante»: O centro, OA = OB, angulo recto en O.
    CuadranteDe { o: char, a: char, b: char },
    /// «AB es diametro».
    Diametro(char, char),
    /// «O centro» / «de centro O».
    Centro(char),
    /// «m∢AOB = 90°»: el angulo de vertice `v`.
    Angulo { a: char, v: char, b: char, grados: f64 },
    /// «θ = 90°» sin letras: hay un angulo recto marcado.
    Recto,
}

/// Sin tildes y con los signos de angulo unificados.
fn normal(t: &str) -> String {
    t.chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'Á' => 'A',
            'É' => 'E',
            'Í' => 'I',
            'Ó' => 'O',
            'Ú' => 'U',
            '∢' | '∡' | '∠' | '<' | '‹' | '«' => '∢',
            'º' | '˚' => '°',
            c => c,
        })
        .collect::<String>()
        // Lo que escribe la nube en LaTeX.
        .replace("\\measuredangle", "∢")
        .replace("\\sphericalangle", "∢")
        .replace("\\angle", "∢")
        .replace("^{\\circ}", "°")
        .replace("^\\circ", "°")
        .replace("\\circ", "°")
        .replace("\\theta", "θ")
        .replace("\\overline", "")
        .replace(['{', '}', '\u{305}', '\u{304}'], "")
}

/// Las mayusculas sueltas seguidas (`AOB` -> [A, O, B]) que empiezan en `i`.
fn letras_en(c: &[char], i: usize) -> Vec<char> {
    let mut v = Vec::new();
    let mut k = i;
    while k < c.len() && c[k].is_ascii_uppercase() {
        v.push(c[k]);
        k += 1;
    }
    v
}

/// El numero que empieza en `i` (con coma o punto).
fn numero_en(c: &[char], i: usize) -> Option<f64> {
    let mut s = String::new();
    let mut k = i;
    while k < c.len() && (c[k].is_ascii_digit() || ((c[k] == '.' || c[k] == ',') && !s.is_empty())) {
        s.push(if c[k] == ',' { '.' } else { c[k] });
        k += 1;
    }
    s.trim_end_matches('.').parse().ok()
}

/// **Los hechos de unos renglones de texto** (del OCR de la hoja, de la
/// nube). Lo que no se entiende no es un hecho: nunca se inventa.
pub fn hechos_de_texto<S: AsRef<str>>(renglones: &[S]) -> Vec<Hecho> {
    let mut v: Vec<Hecho> = Vec::new();
    let anadir = |h: Hecho, v: &mut Vec<Hecho>| {
        if !v.contains(&h) {
            v.push(h);
        }
    };
    for r in renglones {
        let n = normal(r.as_ref());
        let bajo = n.to_lowercase();
        let compacto: String = bajo.chars().filter(|c| !c.is_whitespace()).collect();
        if compacto.contains("semicircunferencia") || compacto.contains("semicirculo") {
            anadir(Hecho::Semicircunferencia, &mut v);
        }
        if bajo.contains("cuadrante") {
            anadir(Hecho::Cuadrante, &mut v);
        }
        let c: Vec<char> = n.chars().collect();
        // Las palabras (con sus letras de puntos).
        let palabras: Vec<&str> = n.split(|ch: char| ch.is_whitespace() || ch == ',' || ch == ':' || ch == ';').filter(|p| !p.is_empty()).collect();
        let bajas: Vec<String> = palabras.iter().map(|p| p.to_lowercase()).collect();
        let es_nombre = |p: &str| p.chars().count() >= 2 && p.chars().count() <= 3 && p.chars().all(|ch| ch.is_ascii_uppercase());
        for (k, p) in palabras.iter().enumerate() {
            if !es_nombre(p) {
                continue;
            }
            let l: Vec<char> = p.chars().collect();
            let siguientes: Vec<&str> = bajas.iter().skip(k + 1).take(4).map(String::as_str).collect();
            let antes: Vec<&str> = bajas.iter().take(k).rev().take(2).map(String::as_str).collect();
            let dice = |w: &str| siguientes.iter().any(|s| s.starts_with(w)) || antes.iter().any(|s| s.starts_with(w));
            if l.len() == 2 && dice("diametro") {
                anadir(Hecho::Diametro(l[0], l[1]), &mut v);
            }
            if l.len() == 3 && dice("cuadrante") {
                anadir(Hecho::CuadranteDe { o: l[1], a: l[0], b: l[2] }, &mut v);
            }
        }
        // «centro O», «O es el centro», «de centro O».
        for (k, p) in bajas.iter().enumerate() {
            if p.starts_with("centro") {
                let vecino = palabras.get(k + 1).filter(|q| q.chars().count() == 1 && q.chars().all(|ch| ch.is_ascii_uppercase()));
                let previo = (k >= 2 && matches!(bajas[k - 1].as_str(), "el" | "es"))
                    .then(|| palabras[..k].iter().rev().find(|q| q.chars().count() == 1 && q.chars().all(|ch| ch.is_ascii_uppercase())))
                    .flatten();
                if let Some(o) = vecino.or(previo).and_then(|q| q.chars().next()) {
                    anadir(Hecho::Centro(o), &mut v);
                }
            }
        }
        // Angulos: «∢AOB = 90°» (con o sin m delante).
        for i in 0..c.len() {
            if c[i] != '∢' {
                continue;
            }
            let mut k = i + 1;
            while k < c.len() && c[k] == ' ' {
                k += 1;
            }
            let l = letras_en(&c, k);
            if l.len() != 3 {
                continue;
            }
            k += 3;
            while k < c.len() && c[k] == ' ' {
                k += 1;
            }
            if c.get(k) != Some(&'=') {
                continue;
            }
            k += 1;
            while k < c.len() && c[k] == ' ' {
                k += 1;
            }
            if let Some(g) = numero_en(&c, k).filter(|g| *g > 0.0 && *g < 360.0) {
                anadir(Hecho::Angulo { a: l[0], v: l[1], b: l[2], grados: g }, &mut v);
            }
        }
        // «θ = 90°», «α=90°»: un recto marcado.
        for i in 0..c.len() {
            if !matches!(c[i], 'θ' | 'α' | 'β' | 'φ' | 'x' | 'X') {
                continue;
            }
            let mut k = i + 1;
            while k < c.len() && c[k] == ' ' {
                k += 1;
            }
            if c.get(k) != Some(&'=') {
                continue;
            }
            k += 1;
            while k < c.len() && c[k] == ' ' {
                k += 1;
            }
            if numero_en(&c, k).is_some_and(|g| (g - 90.0).abs() < 1e-6) && c.get(k + 2).is_none_or(|ch| !ch.is_ascii_digit()) {
                anadir(Hecho::Recto, &mut v);
            }
        }
    }
    v
}

// ---------------------------------------------------------------------------
// Figuras

/// Una pieza del diagrama.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Pieza {
    Segmento(usize),
    Arco(usize),
    Punto(usize),
    Recuadro(usize),
    Region(usize),
}

/// **Una figura**: las piezas que se tocan, con su caja y su escala.
#[derive(Debug, Clone, PartialEq)]
pub struct Figura {
    pub piezas: Vec<Pieza>,
    pub caja: (f64, f64, f64, f64),
}

impl Figura {
    /// Su tamano (la diagonal de la caja): de aqui salen las tolerancias.
    pub fn escala(&self) -> f64 {
        dist((self.caja.0, self.caja.1), (self.caja.2, self.caja.3))
    }
}

fn muestras_de(d: &Diagrama, p: Pieza) -> Vec<P> {
    let tramo = |a: P, b: P, v: &mut Vec<P>| {
        let n = ((dist(a, b) / 3.0).ceil() as usize).max(1);
        for k in 0..=n {
            let t = k as f64 / n as f64;
            v.push((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
        }
    };
    let mut v = Vec::new();
    match p {
        Pieza::Segmento(i) => tramo(d.segmentos[i].a, d.segmentos[i].b, &mut v),
        Pieza::Arco(i) => {
            let a = &d.arcos[i];
            let n = ((a.r * a.barrido / 3.0).ceil() as usize).max(2);
            for k in 0..=n {
                let t = a.inicio + a.barrido * k as f64 / n as f64;
                v.push((a.c.0 + a.r * t.cos(), a.c.1 + a.r * t.sin()));
            }
        }
        Pieza::Punto(i) => v.push(d.puntos[i].c),
        Pieza::Recuadro(i) => {
            let r = &d.recuadros[i];
            let c = [(r.x0, r.y0), (r.x1, r.y0), (r.x1, r.y1), (r.x0, r.y1), (r.x0, r.y0)];
            for k in 0..4 {
                tramo(c[k], c[k + 1], &mut v);
            }
        }
        Pieza::Region(i) => {
            let c = &d.regiones[i].contorno;
            for k in 0..c.len() {
                tramo(c[k], c[(k + 1) % c.len()], &mut v);
            }
        }
    }
    v
}

fn raiz(padre: &mut [usize], mut i: usize) -> usize {
    while padre[i] != i {
        padre[i] = padre[padre[i]];
        i = padre[i];
    }
    i
}

/// **Las figuras** de un diagrama: piezas que se tocan (a menos de `tol`
/// px), cada grupo una figura. Dos dibujos de la misma lamina, cada uno con
/// su escala, salen como dos figuras.
pub fn figuras(d: &Diagrama, tol: f64) -> Vec<Figura> {
    let mut piezas: Vec<Pieza> = Vec::new();
    piezas.extend((0..d.segmentos.len()).map(Pieza::Segmento));
    piezas.extend((0..d.arcos.len()).map(Pieza::Arco));
    piezas.extend((0..d.puntos.len()).map(Pieza::Punto));
    piezas.extend((0..d.recuadros.len()).map(Pieza::Recuadro));
    piezas.extend((0..d.regiones.len()).map(Pieza::Region));
    let n = piezas.len();
    let mut padre: Vec<usize> = (0..n).collect();
    let celda = tol.max(1.0);
    let mut rejilla: std::collections::HashMap<(i64, i64), Vec<(usize, P)>> = std::collections::HashMap::new();
    let mut cajas = vec![(f64::MAX, f64::MAX, f64::MIN, f64::MIN); n];
    for (k, &p) in piezas.iter().enumerate() {
        for q in muestras_de(d, p) {
            let c = &mut cajas[k];
            c.0 = c.0.min(q.0);
            c.1 = c.1.min(q.1);
            c.2 = c.2.max(q.0);
            c.3 = c.3.max(q.1);
            let (cx, cy) = ((q.0 / celda).floor() as i64, (q.1 / celda).floor() as i64);
            // Los vecinos de su celda y de las de al lado.
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if let Some(v) = rejilla.get(&(cx + dx, cy + dy)) {
                        for &(j, o) in v {
                            if j != k && dist(o, q) <= tol {
                                let (a, b) = (raiz(&mut padre, j), raiz(&mut padre, k));
                                if a != b {
                                    padre[a] = b;
                                }
                            }
                        }
                    }
                }
            }
            rejilla.entry((cx, cy)).or_default().push((k, q));
        }
    }
    // Una region contiene a lo que hay dentro (una marca, una raya que la
    // cruza): ya se tocan por el borde casi siempre; lo de dentro suelto
    // tambien es de ella.
    let mut grupos: std::collections::BTreeMap<usize, Vec<usize>> = Default::default();
    for k in 0..n {
        let r = raiz(&mut padre, k);
        grupos.entry(r).or_default().push(k);
    }
    grupos
        .into_values()
        .map(|ks| {
            let mut caja = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
            for &k in &ks {
                let c = cajas[k];
                caja = (caja.0.min(c.0), caja.1.min(c.1), caja.2.max(c.2), caja.3.max(c.3));
            }
            Figura { piezas: ks.into_iter().map(|k| piezas[k]).collect(), caja }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Nudos y restricciones

/// Un nudo: donde acaban rectas y arcos, o un punto.
#[derive(Debug, Clone)]
struct Nudo {
    p: P,
    nombre: Option<char>,
    /// Lo movio una restriccion fuerte (diametro, cuadrante, hecho): las
    /// debiles (horizontal, recto) no lo tocan.
    fijo: bool,
}

/// Una circunferencia de la figura (uno o varios arcos suyos).
#[derive(Debug, Clone)]
struct Circulo {
    c: P,
    r: f64,
    arcos: Vec<usize>,
    /// El nudo de su centro, si lo hay (un punto, el vertice del cuadrante).
    centro: Option<usize>,
}

struct Figurado {
    nudos: Vec<Nudo>,
    /// (nudo a, nudo b) de cada segmento de la figura.
    segs: Vec<(usize, usize, usize)>,
    /// (arco, nudo inicio, nudo fin) de los arcos con extremos en nudos.
    arcos: Vec<(usize, Option<usize>, Option<usize>)>,
    circulos: Vec<Circulo>,
    /// (punto del diagrama, su nudo).
    puntos: Vec<(usize, usize)>,
}

fn nudo_de(nudos: &mut Vec<Nudo>, p: P, tol: f64) -> usize {
    if let Some(k) = (0..nudos.len()).filter(|&k| dist(nudos[k].p, p) <= tol).min_by(|&a, &b| dist(nudos[a].p, p).total_cmp(&dist(nudos[b].p, p))) {
        return k;
    }
    nudos.push(Nudo { p, nombre: None, fijo: false });
    nudos.len() - 1
}

fn angulo(o: P, p: P) -> f64 {
    (p.1 - o.1).atan2(p.0 - o.0)
}

fn en(c: P, r: f64, a: f64) -> P {
    (c.0 + r * a.cos(), c.1 + r * a.sin())
}

/// Lo que va de `a` a `b` en angulo, en (-pi, pi].
fn dif(a: f64, b: f64) -> f64 {
    let mut d = (b - a).rem_euclid(TAU);
    if d > PI {
        d -= TAU;
    }
    d
}

fn rotar(o: P, p: P, da: f64) -> P {
    let r = dist(o, p);
    en(o, r, angulo(o, p) + da)
}

/// Lo que se puede apartar una medida para que se tome por exacta: los
/// grados de holgura, mas en un trazo corto (que mide peor).
fn holgura(grados: f64, largo: f64) -> f64 {
    grados.to_radians().max((1.5 / largo.max(1.0)).atan())
}

/// La letra suelta de un rotulo (A, B, O, P...), si lo es.
fn letra_de(t: &str) -> Option<char> {
    let t = t.trim().trim_end_matches(['\'', '′']);
    let mut c = t.chars();
    let a = c.next()?;
    (c.next().is_none() && a.is_ascii_uppercase()).then_some(a)
}

/// **Aplica la geometria exacta** a un diagrama ya medido: figura a figura,
/// con los hechos del texto. `hm`: el alto de letra (pixeles del recorte).
pub fn aplicar(d: &mut Diagrama, hechos: &[Hecho], hm: f64) {
    let tol_figura = 4f64.max(0.35 * hm);
    for fig in figuras(d, tol_figura) {
        aplicar_en_figura(d, &fig, hechos, hm);
    }
    marcas_de_angulo(d, hm);
    for fig in figuras(d, tol_figura) {
        bordes_de_regiones(d, &fig);
    }
}

fn aplicar_en_figura(d: &mut Diagrama, fig: &Figura, hechos: &[Hecho], hm: f64) {
    let escala = fig.escala().max(1.0);
    let tol_nudo = 3f64.max(0.012 * escala).min(0.5 * hm.max(8.0));
    let mut f = Figurado { nudos: Vec::new(), segs: Vec::new(), arcos: Vec::new(), circulos: Vec::new(), puntos: Vec::new() };
    // 1. Los nudos: los puntos primero.
    for pz in &fig.piezas {
        if let Pieza::Punto(i) = *pz {
            f.nudos.push(Nudo { p: d.puntos[i].c, nombre: None, fijo: false });
            f.puntos.push((i, f.nudos.len() - 1));
        }
    }
    for pz in &fig.piezas {
        if let Pieza::Segmento(i) = *pz {
            let s = &d.segmentos[i];
            let t = tol_nudo.max(s.grosor / 2.0 + 2.0);
            let a = nudo_de(&mut f.nudos, s.a, t);
            let b = nudo_de(&mut f.nudos, s.b, t);
            if a != b {
                f.segs.push((i, a, b));
            }
        }
    }
    for pz in &fig.piezas {
        if let Pieza::Arco(i) = *pz {
            let a = d.arcos[i].clone();
            // A que circunferencia de la figura pertenece.
            let tol_c = 2.5f64.max(0.03 * a.r);
            let k = match f.circulos.iter().position(|c| dist(c.c, a.c) <= tol_c && (c.r - a.r).abs() <= tol_c) {
                Some(k) => {
                    f.circulos[k].arcos.push(i);
                    k
                }
                None => {
                    f.circulos.push(Circulo { c: a.c, r: a.r, arcos: vec![i], centro: None });
                    f.circulos.len() - 1
                }
            };
            if f.circulos[k].centro.is_none() {
                let tol_centro = tol_nudo.max(0.04 * a.r);
                f.circulos[k].centro = (0..f.nudos.len()).filter(|&n| dist(f.nudos[n].p, a.c) <= tol_centro).min_by(|&x, &y| dist(f.nudos[x].p, a.c).total_cmp(&dist(f.nudos[y].p, a.c)));
            }
            if a.entera() {
                f.arcos.push((i, None, None));
                continue;
            }
            let (p, q) = a.extremos();
            let t = tol_nudo.max(a.grosor / 2.0 + 3.0);
            let cerca = |nudos: &Vec<Nudo>, x: P| (0..nudos.len()).filter(|&n| dist(nudos[n].p, x) <= t).min_by(|&u, &v| dist(nudos[u].p, x).total_cmp(&dist(nudos[v].p, x)));
            let ni = cerca(&f.nudos, p).unwrap_or_else(|| nudo_de(&mut f.nudos, p, 0.0));
            let nf = cerca(&f.nudos, q).unwrap_or_else(|| nudo_de(&mut f.nudos, q, 0.0));
            f.arcos.push((i, Some(ni), Some(nf)));
        }
    }
    if f.nudos.is_empty() {
        return;
    }
    // Una recta que se corta antes de llegar a un punto que esta en su
    // prolongacion (las marcas de angulo del vertice la tapaban): llega a el.
    let es_punto = |f: &Figurado, n: usize| f.puntos.iter().any(|&(_, k)| k == n);
    for k in 0..f.segs.len() {
        for lado in [false, true] {
            let (i, a, b) = f.segs[k];
            let (e, otro) = if lado { (b, a) } else { (a, b) };
            if es_punto(&f, e) {
                continue;
            }
            let (pe, po) = (f.nudos[e].p, f.nudos[otro].p);
            let l = dist(pe, po).max(1e-9);
            let u = ((pe.0 - po.0) / l, (pe.1 - po.1) / l);
            let alcance = 0.25 * escala;
            let mejor = f
                .puntos
                .iter()
                .map(|&(_, n)| n)
                .filter(|&n| n != otro)
                .filter(|&n| {
                    let q = f.nudos[n].p;
                    let (vx, vy) = (q.0 - pe.0, q.1 - pe.1);
                    let delante = vx * u.0 + vy * u.1;
                    let lado = (vx * -u.1 + vy * u.0).abs();
                    delante > 0.0 && delante <= alcance.min(0.6 * l) && lado <= 3.0f64.max(0.07 * delante)
                })
                .min_by(|&x, &y| dist(f.nudos[x].p, pe).total_cmp(&dist(f.nudos[y].p, pe)));
            if let Some(n) = mejor {
                f.segs[k] = if lado { (i, a, n) } else { (i, n, b) };
            }
        }
    }
    // 2. Los nombres: cada letra suelta, al nudo mas cercano (un punto
    //    antes que una esquina cualquiera).
    let mut letras: Vec<(char, P, f64)> = d
        .rotulos
        .iter()
        .filter_map(|r| letra_de(&r.texto).map(|l| (l, (r.caja.cx(), r.caja.cy()), r.caja.h.max(r.caja.w))))
        .filter(|(_, c, _)| c.0 >= fig.caja.0 - 3.0 * hm && c.0 <= fig.caja.2 + 3.0 * hm && c.1 >= fig.caja.1 - 3.0 * hm && c.1 <= fig.caja.3 + 3.0 * hm)
        .collect();
    letras.sort_by(|a, b| a.0.cmp(&b.0));
    let mut parejas: Vec<(f64, usize, usize)> = Vec::new();
    for (li, (_, c, tam)) in letras.iter().enumerate() {
        for (n, nudo) in f.nudos.iter().enumerate() {
            let de = dist(*c, nudo.p);
            let alcance = 1.6 * tam.max(hm) + 6.0;
            if de <= alcance {
                let es_punto = f.puntos.iter().any(|&(_, k)| k == n);
                parejas.push((de * if es_punto { 0.6 } else { 1.0 }, li, n));
            }
        }
    }
    parejas.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut letra_usada = vec![false; letras.len()];
    for (_, li, n) in parejas {
        if letra_usada[li] || f.nudos[n].nombre.is_some() {
            continue;
        }
        letra_usada[li] = true;
        f.nudos[n].nombre = Some(letras[li].0);
    }
    // 3. Las vueltas de restricciones, con un seguro: si mueven algo mucho
    // (mas del 4 % de la figura), los hechos no eran de esta figura o una
    // letra cayo donde no era (DeepSeek, 15-oct: «A» puesta en otro sitio
    // arrastro la circunferencia): otra vez sin los hechos, y si aun asi,
    // la figura se queda como se midio.
    let antes = (f.nudos.clone(), f.circulos.clone());
    let limite = (0.04 * escala).max(1.5 * hm);
    let movido = |f: &Figurado| {
        let n = f.nudos.iter().zip(&antes.0).map(|(a, b)| dist(a.p, b.p)).fold(0.0, f64::max);
        let c = f.circulos.iter().zip(&antes.1).map(|(a, b)| dist(a.c, b.c).max((a.r - b.r).abs())).fold(0.0, f64::max);
        n.max(c)
    };
    vueltas(&mut f, d, hechos, hm);
    if movido(&f) > limite {
        f.nudos = antes.0.clone();
        f.circulos = antes.1.clone();
        vueltas(&mut f, d, &[], hm);
        if movido(&f) > limite {
            return;
        }
    }

    // 4. De vuelta al diagrama.
    for &(i, a, b) in &f.segs {
        let s = &mut d.segmentos[i];
        // Una punta de flecha se queda donde esta (su nudo es la punta).
        s.a = f.nudos[a].p;
        s.b = f.nudos[b].p;
    }
    for &(i, n) in &f.puntos {
        d.puntos[i].c = f.nudos[n].p;
    }
    for c in &f.circulos {
        for &ai in &c.arcos {
            let a = &mut d.arcos[ai];
            let viejo = a.clone();
            a.c = c.c;
            a.r = c.r;
            if a.entera() {
                continue;
            }
            let Some(&(_, Some(ni), Some(nf))) = f.arcos.iter().find(|x| x.0 == ai) else { continue };
            let (pi, pf) = (f.nudos[ni].p, f.nudos[nf].p);
            let ini = angulo(c.c, pi);
            let fin = angulo(c.c, pf);
            // El mismo sentido que tenia, y el barrido mas parecido.
            let mut barrido = (fin - ini).rem_euclid(TAU);
            if barrido < 1e-6 {
                barrido = viejo.barrido;
            }
            if (barrido - viejo.barrido).abs() > PI * 0.75 {
                continue;
            }
            a.inicio = ini.rem_euclid(TAU);
            a.barrido = barrido;
        }
    }
    // (Un vertice nombrado SIN punto en el papel —la P donde se cruzan el
    // arco y la recta, la X de un angulo— se queda sin punto: anadirlo
    // inventaba puntos que el original no tiene.)
}

/// **Las vueltas de restricciones** de una figura (ver la cabecera).
fn vueltas(f: &mut Figurado, d: &Diagrama, hechos: &[Hecho], hm: f64) {
    let nombrado = |f: &Figurado, l: char| f.nudos.iter().position(|n| n.nombre == Some(l));

    // 3. Las vueltas de restricciones.
    let hay = |h: &Hecho| hechos.contains(h);
    let semi_tol = if hay(&Hecho::Semicircunferencia) { 16f64 } else { 5.0 };
    let cuad_tol = if hay(&Hecho::Cuadrante) { 14f64 } else { 4.0 };
    let recto_tol = if hay(&Hecho::Recto) || hechos.iter().any(|h| matches!(h, Hecho::Angulo { grados, .. } if (*grados - 90.0).abs() < 1e-6)) { 4.0 } else { 2.5 };
    for vuelta in 0..5 {
        // a. Horizontales y verticales.
        for &(_, a, b) in &f.segs {
            let (pa, pb) = (f.nudos[a].p, f.nudos[b].p);
            let l = dist(pa, pb);
            if l < 1.5 * hm.min(20.0) {
                continue;
            }
            let ang = angulo(pa, pb).rem_euclid(PI);
            let tol = holgura(1.2, l);
            if ang.min(PI - ang) <= tol {
                let y = if f.nudos[a].fijo && !f.nudos[b].fijo {
                    pa.1
                } else if f.nudos[b].fijo && !f.nudos[a].fijo {
                    pb.1
                } else {
                    (pa.1 + pb.1) / 2.0
                };
                f.nudos[a].p.1 = y;
                f.nudos[b].p.1 = y;
            } else if (ang - FRAC_PI_2).abs() <= tol {
                let x = if f.nudos[a].fijo && !f.nudos[b].fijo {
                    pa.0
                } else if f.nudos[b].fijo && !f.nudos[a].fijo {
                    pb.0
                } else {
                    (pa.0 + pb.0) / 2.0
                };
                f.nudos[a].p.0 = x;
                f.nudos[b].p.0 = x;
            }
        }
        // b. Angulos casi rectos en un vertice comun.
        for i in 0..f.segs.len() {
            for j in i + 1..f.segs.len() {
                let (_, a1, b1) = f.segs[i];
                let (_, a2, b2) = f.segs[j];
                let v = if a1 == a2 || a1 == b2 {
                    a1
                } else if b1 == a2 || b1 == b2 {
                    b1
                } else {
                    continue;
                };
                let p1 = if a1 == v { b1 } else { a1 };
                let p2 = if a2 == v { b2 } else { a2 };
                let o = f.nudos[v].p;
                let (l1, l2) = (dist(o, f.nudos[p1].p), dist(o, f.nudos[p2].p));
                let da = dif(angulo(o, f.nudos[p1].p), angulo(o, f.nudos[p2].p));
                let falta = if da >= 0.0 { FRAC_PI_2 - da } else { -FRAC_PI_2 - da };
                if falta.abs() > holgura(recto_tol, l1.min(l2)) || falta.abs() < 1e-9 {
                    continue;
                }
                // Gira el que menos manda: el no fijo, o el mas corto; si
                // ninguno manda, la mitad cada uno.
                let horizontal_o_vertical = |p: P| {
                    let a = angulo(o, p).rem_euclid(FRAC_PI_2);
                    a.min(FRAC_PI_2 - a) < 1e-6
                };
                // Un extremo sobre una circunferencia manda (Tales: el recto sale
                // solo al ponerlo en ella; girarlo la descuadraba).
                let en_circulo = |p: P| f.circulos.iter().any(|c| (dist(p, c.c) - c.r).abs() <= 3f64.max(0.03 * c.r));
                let manda = |n: usize| f.nudos[n].fijo || horizontal_o_vertical(f.nudos[n].p) || en_circulo(f.nudos[n].p);
                let (m1, m2) = (manda(p1), manda(p2));
                match (m1, m2) {
                    (true, true) => {}
                    (true, false) => f.nudos[p2].p = rotar(o, f.nudos[p2].p, -falta),
                    (false, true) => f.nudos[p1].p = rotar(o, f.nudos[p1].p, falta),
                    (false, false) => {
                        f.nudos[p1].p = rotar(o, f.nudos[p1].p, falta / 2.0);
                        f.nudos[p2].p = rotar(o, f.nudos[p2].p, -falta / 2.0);
                    }
                }
            }
        }
        // c. Hechos de cuadrante con letras: O centro, OA = OB, recto.
        for h in hechos {
            if let Hecho::CuadranteDe { o, a, b } = *h
                && let (Some(no), Some(na), Some(nb)) = (nombrado(f, o), nombrado(f, a), nombrado(f, b))
            {
                cuadrante(f, no, na, nb);
            }
        }
        // d. Arcos: semicircunferencias y cuadrantes.
        for &(ai, ni, nf) in &f.arcos.clone() {
            let (Some(ni), Some(nf)) = (ni, nf) else { continue };
            let barrido = d.arcos[ai].barrido;
            let k = f.circulos.iter().position(|c| c.arcos.contains(&ai)).unwrap_or(0);
            if (barrido - PI).abs() <= semi_tol.to_radians() {
                // El diametro: sus dos extremos; el centro, en medio.
                let (pa, pb) = (f.nudos[ni].p, f.nudos[nf].p);
                let c = ((pa.0 + pb.0) / 2.0, (pa.1 + pb.1) / 2.0);
                f.circulos[k].c = c;
                f.circulos[k].r = dist(pa, pb) / 2.0;
                if let Some(nc) = f.circulos[k].centro {
                    f.nudos[nc].p = c;
                }
                f.nudos[ni].fijo = true;
                f.nudos[nf].fijo = true;
            } else if (barrido - FRAC_PI_2).abs() <= cuad_tol.to_radians()
                && let Some(nc) = f.circulos[k].centro
            {
                cuadrante(f, nc, ni, nf);
                let o = f.nudos[nc].p;
                f.circulos[k].c = o;
                f.circulos[k].r = dist(o, f.nudos[ni].p);
            }
        }
        // e. Diametros y centros que dice el texto.
        for h in hechos {
            match *h {
                Hecho::Diametro(a, b) => {
                    let (Some(na), Some(nb)) = (nombrado(f, a), nombrado(f, b)) else { continue };
                    let (pa, pb) = (f.nudos[na].p, f.nudos[nb].p);
                    // La circunferencia que pasa (mas o menos) por los dos.
                    let k = (0..f.circulos.len())
                        .filter(|&k| {
                            let c = &f.circulos[k];
                            let tol = 0.08 * c.r + 3.0;
                            (dist(pa, c.c) - c.r).abs() <= tol && (dist(pb, c.c) - c.r).abs() <= tol && (dist(pa, pb) - 2.0 * c.r).abs() <= 2.0 * tol
                        })
                        .min_by(|&x, &y| (dist(pa, pb) - 2.0 * f.circulos[x].r).abs().total_cmp(&(dist(pa, pb) - 2.0 * f.circulos[y].r).abs()));
                    if let Some(k) = k {
                        let c = ((pa.0 + pb.0) / 2.0, (pa.1 + pb.1) / 2.0);
                        f.circulos[k].c = c;
                        f.circulos[k].r = dist(pa, pb) / 2.0;
                        if let Some(nc) = f.circulos[k].centro {
                            f.nudos[nc].p = c;
                        }
                        f.nudos[na].fijo = true;
                        f.nudos[nb].fijo = true;
                    }
                }
                Hecho::Centro(o) => {
                    let Some(no) = nombrado(f, o) else { continue };
                    let p = f.nudos[no].p;
                    if let Some(k) = (0..f.circulos.len()).filter(|&k| dist(f.circulos[k].c, p) <= 0.12 * f.circulos[k].r + 3.0).min_by(|&x, &y| dist(f.circulos[x].c, p).total_cmp(&dist(f.circulos[y].c, p))) {
                        f.circulos[k].centro = Some(no);
                        f.circulos[k].c = p;
                    }
                }
                _ => {}
            }
        }
        // f. Un segmento con los dos extremos en la circunferencia y el
        //    centro en medio (casi) es un diametro.
        for &(_, a, b) in &f.segs {
            let (pa, pb) = (f.nudos[a].p, f.nudos[b].p);
            for c in f.circulos.iter_mut() {
                let tol = 2.5f64.max(0.02 * c.r);
                let m = ((pa.0 + pb.0) / 2.0, (pa.1 + pb.1) / 2.0);
                if (dist(pa, c.c) - c.r).abs() <= tol && (dist(pb, c.c) - c.r).abs() <= tol && dist(m, c.c) <= tol {
                    c.c = m;
                    c.r = dist(pa, pb) / 2.0;
                }
            }
        }
        // g. Los centros que son un punto: el punto y el centro, uno.
        for c in f.circulos.iter_mut() {
            if let Some(nc) = c.centro {
                let p = f.nudos[nc].p;
                let m = if f.nudos[nc].fijo { p } else { ((p.0 + c.c.0) / 2.0, (p.1 + c.c.1) / 2.0) };
                c.c = m;
                f.nudos[nc].p = m;
            }
        }
        // h. Los nudos que caen (casi) sobre una circunferencia, sobre ella;
        //    los nombrados con mas holgura. Y los que caen sobre una recta
        //    (una T, un punto en medio de un lado), sobre ella.
        let ultima = vuelta >= 3;
        for n in 0..f.nudos.len() {
            let p = f.nudos[n].p;
            for c in &f.circulos {
                if c.centro == Some(n) {
                    continue;
                }
                let base = if f.nudos[n].nombre.is_some() { 0.05 } else { 0.025 };
                let tol = 3f64.max(base * c.r);
                let de = dist(p, c.c);
                if (de - c.r).abs() <= tol && de > 1e-6 && (ultima || !f.nudos[n].fijo || (de - c.r).abs() > 1e-6) {
                    f.nudos[n].p = en(c.c, c.r, angulo(c.c, p));
                }
            }
            let p = f.nudos[n].p;
            for &(_, a, b) in &f.segs {
                if a == n || b == n {
                    continue;
                }
                let (pa, pb) = (f.nudos[a].p, f.nudos[b].p);
                let l = dist(pa, pb);
                if l < 1e-6 {
                    continue;
                }
                let u = ((pb.0 - pa.0) / l, (pb.1 - pa.1) / l);
                let t = (p.0 - pa.0) * u.0 + (p.1 - pa.1) * u.1;
                let lado = (p.0 - pa.0) * -u.1 + (p.1 - pa.1) * u.0;
                if t > 2.0 && t < l - 2.0 && lado.abs() <= 2.5f64.max(0.01 * l) && lado.abs() > 1e-9 {
                    f.nudos[n].p = (pa.0 + u.0 * t, pa.1 + u.1 * t);
                }
            }
        }
    }
}

/// **Un cuadrante**: OA = OB (la media) y el angulo AOB recto, girando lo
/// mismo los dos alrededor de su bisectriz.
fn cuadrante(f: &mut Figurado, no: usize, na: usize, nb: usize) {
    let o = f.nudos[no].p;
    let (pa, pb) = (f.nudos[na].p, f.nudos[nb].p);
    let r = (dist(o, pa) + dist(o, pb)) / 2.0;
    let (aa, ab) = (angulo(o, pa), angulo(o, pb));
    let d = dif(aa, ab);
    if d.abs() < 0.2 || d.abs() > PI - 0.2 {
        return;
    }
    let medio = aa + d / 2.0;
    let medio_lado = FRAC_PI_2 / 2.0 * d.signum();
    f.nudos[na].p = en(o, r, medio - medio_lado);
    f.nudos[nb].p = en(o, r, medio + medio_lado);
    // Si uno de los lados estaba casi horizontal o vertical, el par entero
    // se gira para que lo este de verdad.
    for k in [na, nb] {
        let a = angulo(o, f.nudos[k].p);
        let q = (a / FRAC_PI_2).round() * FRAC_PI_2;
        let falta = q - a;
        if falta.abs() <= 2.5f64.to_radians() {
            f.nudos[na].p = rotar(o, f.nudos[na].p, falta);
            f.nudos[nb].p = rotar(o, f.nudos[nb].p, falta);
            break;
        }
    }
    f.nudos[na].fijo = true;
    f.nudos[nb].fijo = true;
    f.nudos[no].fijo = true;
}

// ---------------------------------------------------------------------------
// Marcas de angulo y bordes

/// **Las marcas de angulo**: una region pequena junto a un vertice donde se
/// juntan dos rayas pasa a ser el sector exacto entre ellas (con su color).
fn marcas_de_angulo(d: &mut Diagrama, hm: f64) {
    let max_r = 6.0 * hm.max(8.0);
    // Los vertices: extremos de segmentos donde llegan dos o mas, con las
    // direcciones de sus rayas.
    let mut vertices: Vec<(P, Vec<f64>)> = Vec::new();
    // Solo las rayas de verdad (largas): las rayitas de una letra o del
    // borde de la mancha no hacen vertice.
    let largas: Vec<&super::Segmento> = d.segmentos.iter().filter(|s| s.largo() >= 0.5 * max_r).collect();
    for s in largas.iter().copied() {
        for (e, o) in [(s.a, s.b), (s.b, s.a)] {
            let dir = angulo(e, o);
            match vertices.iter_mut().find(|v| dist(v.0, e) <= 3.0) {
                Some(v) => {
                    if !v.1.iter().any(|&x| dif(x, dir).abs() < 0.02) {
                        v.1.push(dir);
                    }
                }
                None => vertices.push((e, vec![dir])),
            }
        }
    }
    // Los puntos tambien son vertices (aunque las rayas no lleguen).
    for p in &d.puntos {
        if !vertices.iter().any(|v| dist(v.0, p.c) <= 3.0) {
            vertices.push((p.c, Vec::new()));
        }
    }
    // Y las rayas que pasan por un vertice (una recta que sigue de largo).
    for v in vertices.iter_mut() {
        for s in largas.iter().copied() {
            if dist(s.a, v.0) > 3.0 && dist(s.b, v.0) > 3.0 && super::ajuste::dist_a_segmento(v.0, s.a, s.b) <= 1.5 {
                for dir in [angulo(v.0, s.a), angulo(v.0, s.b)] {
                    if !v.1.iter().any(|&x| dif(x, dir).abs() < 0.02) {
                        v.1.push(dir);
                    }
                }
            }
            // O que apuntan a el sin llegar (la recta cortada por la marca).
            let l = s.largo().max(1e-9);
            let lado = ((v.0.0 - s.a.0) * -(s.b.1 - s.a.1) / l + (v.0.1 - s.a.1) * (s.b.0 - s.a.0) / l).abs();
            let (cerca, lejos) = if dist(s.a, v.0) < dist(s.b, v.0) { (s.a, s.b) } else { (s.b, s.a) };
            if dist(cerca, v.0) > 3.0 && dist(cerca, v.0) <= 2.0 * max_r && lado <= 2.5 && dist(lejos, v.0) > dist(cerca, v.0) {
                let dir = angulo(v.0, lejos);
                if !v.1.iter().any(|&x| dif(x, dir).abs() < 0.02) {
                    v.1.push(dir);
                }
            }
        }
    }
    vertices.retain(|v| v.1.len() >= 2);
    // Las marcas de antes (otra vuelta de la geometria exacta) cuentan para
    // limpiar lo de alrededor.
    let mut sectores: Vec<(P, f64)> = d.marcas.iter().map(|m| (m.v, m.r)).collect();
    let mut quedan = Vec::with_capacity(d.regiones.len());
    for reg in std::mem::take(&mut d.regiones) {
        if reg.contorno.len() >= 3 && reg.area <= PI * max_r * max_r * 0.5
            && let Some((v, r, inicio, barrido)) = sector_de(&reg, &vertices, max_r)
        {
            sectores.push((v, r));
            d.marcas.push(super::Marca { v, r, inicio, barrido, color: reg.color });
            continue;
        }
        quedan.push(reg);
    }
    d.regiones = quedan;
    if sectores.is_empty() {
        return;
    }
    // Lo que quedaba del borde de la mancha y de sus letras (arquitos con
    // otro centro, rayitas): sobra, el sector ya es exacto.
    let r_max = sectores.iter().map(|s| s.1).fold(0.0, f64::max);
    let dentro = |p: P| sectores.iter().any(|&(v, r)| dist(p, v) <= 1.3 * r + 2.0);
    d.arcos.retain(|a| {
        if a.entera() || a.r > 1.6 * r_max {
            return true;
        }
        !(0..=8).all(|k| dentro(en(a.c, a.r, a.inicio + a.barrido * k as f64 / 8.0)))
    });
    d.segmentos.retain(|s| !(s.largo() < 0.9 * r_max && dentro(s.a) && dentro(s.b)));
}

fn sector_de(reg: &Region, vertices: &[(P, Vec<f64>)], max_r: f64) -> Option<(P, f64, f64, f64)> {
    // El vertice: el que esta en el borde de la mancha (o dentro), con la
    // mancha entera a menos de `max_r`.
    let mut mejor: Option<(usize, f64)> = None;
    for (k, v) in vertices.iter().enumerate() {
        let lejos = reg.contorno.iter().map(|&p| dist(p, v.0)).fold(0.0, f64::max);
        let cerca = reg.contorno.iter().map(|&p| dist(p, v.0)).fold(f64::MAX, f64::min);
        // (Junto al vertice la mancha se pierde: alli las rayas la cortan en
        // cunas que no aguantan la erosion.)
        if lejos <= max_r && cerca <= 0.6 * lejos + 3.0 && !v.1.is_empty() && mejor.is_none_or(|m| cerca < m.1) {
            mejor = Some((k, cerca));
        }
    }
    let (k, _) = mejor?;
    let (o, dirs) = (&vertices[k].0, &vertices[k].1);
    // Los angulos de la mancha vista desde el vertice (de lo que esta lejos
    // de el: junto al vertice el angulo baila).
    let mut ds: Vec<f64> = reg.contorno.iter().map(|&p| dist(p, *o)).collect();
    ds.sort_by(f64::total_cmp);
    let r = ds[(ds.len() * 9 / 10).min(ds.len() - 1)];
    if r < 4.0 {
        return None;
    }
    let angs: Vec<f64> = reg.contorno.iter().filter(|&&p| dist(p, *o) >= 0.35 * r).map(|&p| angulo(*o, p)).collect();
    if angs.len() < 3 {
        return None;
    }
    // El centro del abanico y lo que abarca.
    let (sx, sy) = angs.iter().fold((0.0, 0.0), |a, &t| (a.0 + t.cos(), a.1 + t.sin()));
    let medio = sy.atan2(sx);
    let (mut lo, mut hi) = (0f64, 0f64);
    for &t in &angs {
        let x = dif(medio, t);
        lo = lo.min(x);
        hi = hi.max(x);
    }
    if std::env::var_os("PIXPIN_DIAGRAMA_DEPURAR").is_some() {
        eprintln!("sector: area {:.0} vertice {:?} r {r:.1} medio {:.0} lo {:.0} hi {:.0} rayas {:?}", reg.area, o, medio.to_degrees(), lo.to_degrees(), hi.to_degrees(), dirs.iter().map(|x| x.to_degrees().round()).collect::<Vec<_>>());
    }
    if hi - lo > PI * 0.95 || hi - lo < 0.15 {
        return None;
    }
    // Las rayas que lo cierran: la mas cercana a cada lado (con holgura).
    let raya = |objetivo: f64| dirs.iter().copied().filter(|&x| dif(medio + objetivo, x).abs() <= 0.35).min_by(|a, b| dif(medio + objetivo, *a).abs().total_cmp(&dif(medio + objetivo, *b).abs()));
    let (a0, a1) = (raya(lo)?, raya(hi)?);
    let barrido = dif(a0, a1);
    if barrido <= 0.05 {
        return None;
    }
    Some((*o, r, a0, barrido))
}

/// **Los bordes de las regiones**, pegados a lo que las cierra: cada punto
/// del contorno que cae junto a una raya o un arco de la figura, sobre el;
/// lo que va por un arco, con el arco (no a trozos).
fn bordes_de_regiones(d: &mut Diagrama, fig: &Figura) {
    let segs: Vec<usize> = fig.piezas.iter().filter_map(|p| if let Pieza::Segmento(i) = *p { Some(i) } else { None }).collect();
    let arcs: Vec<usize> = fig.piezas.iter().filter_map(|p| if let Pieza::Arco(i) = *p { Some(i) } else { None }).collect();
    for pz in &fig.piezas {
        let Pieza::Region(ri) = *pz else { continue };
        // Las marcas de angulo (pequenas) ya son sectores exactos.
        let caja_fig = (fig.caja.2 - fig.caja.0) * (fig.caja.3 - fig.caja.1);
        if d.regiones[ri].area < 0.03 * caja_fig {
            continue;
        }
        let contorno = d.regiones[ri].contorno.clone();
        if contorno.len() < 3 {
            continue;
        }
        // Lo que toca cada punto: (Some(arco), proyeccion) o (None, proyeccion).
        let mut nuevo: Vec<(Option<usize>, P)> = Vec::with_capacity(contorno.len());
        for &p in &contorno {
            let mut mejor: Option<(f64, Option<usize>, P)> = None;
            for &i in &segs {
                let s = &d.segmentos[i];
                let tol = s.grosor / 2.0 + 3.0;
                let l = dist(s.a, s.b);
                if l < 1e-6 {
                    continue;
                }
                let u = ((s.b.0 - s.a.0) / l, (s.b.1 - s.a.1) / l);
                let t = (p.0 - s.a.0) * u.0 + (p.1 - s.a.1) * u.1;
                if t < 0.0 || t > l {
                    continue;
                }
                let q = (s.a.0 + u.0 * t, s.a.1 + u.1 * t);
                let de = dist(p, q);
                if de <= tol && mejor.is_none_or(|m| de < m.0) {
                    mejor = Some((de, None, q));
                }
            }
            for &i in &arcs {
                let a = &d.arcos[i];
                let tol = a.grosor / 2.0 + 3.0;
                let de = (dist(p, a.c) - a.r).abs();
                let t = angulo(a.c, p);
                let dentro = a.entera() || dif(a.inicio, t).rem_euclid(TAU) <= a.barrido + 0.02;
                if dentro && de <= tol && mejor.is_none_or(|m| de < m.0) {
                    mejor = Some((de, Some(i), en(a.c, a.r, t)));
                }
            }
            nuevo.push(match mejor {
                Some((_, arco, q)) => (arco, q),
                None => (None, p),
            });
        }
        // Lo que va por un arco, muestreado del arco (cada 2 grados).
        let mut salida: Vec<P> = Vec::with_capacity(nuevo.len());
        for k in 0..nuevo.len() {
            let (ak, pk) = nuevo[k];
            salida.push(pk);
            let (an, pn) = nuevo[(k + 1) % nuevo.len()];
            if let (Some(i), Some(j)) = (ak, an)
                && i == j
            {
                let a = &d.arcos[i];
                let (t0, t1) = (angulo(a.c, pk), angulo(a.c, pn));
                let da = dif(t0, t1);
                let pasos = (da.abs() / 2f64.to_radians()).floor() as usize;
                for s in 1..pasos {
                    salida.push(en(a.c, a.r, t0 + da * s as f64 / pasos as f64));
                }
            }
        }
        d.regiones[ri].contorno = salida;
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_hechos_salen_del_texto_de_la_lamina() {
        let h = hechos_de_texto(&[
            "• SEMI CIRCUNFERENCIA",
            "Se observa que AB es diámetro:",
            "m∢AOB=90°",
            "Además AOB es un cuadrante:",
            "m∢BPD=45°",
            "θ=90°",
            "Circunferencia de centro O",
            "Del gráfico. Calcule la m<COD.",
        ]);
        assert!(h.contains(&Hecho::Semicircunferencia), "{h:?}");
        assert!(h.contains(&Hecho::Diametro('A', 'B')), "{h:?}");
        assert!(h.contains(&Hecho::Angulo { a: 'A', v: 'O', b: 'B', grados: 90.0 }), "{h:?}");
        assert!(h.contains(&Hecho::Angulo { a: 'B', v: 'P', b: 'D', grados: 45.0 }), "{h:?}");
        assert!(h.contains(&Hecho::CuadranteDe { o: 'O', a: 'A', b: 'B' }), "{h:?}");
        assert!(h.contains(&Hecho::Cuadrante), "{h:?}");
        assert!(h.contains(&Hecho::Recto), "{h:?}");
        assert!(h.contains(&Hecho::Centro('O')), "{h:?}");
        // La pregunta no es un hecho (no tiene medida).
        assert!(!h.iter().any(|x| matches!(x, Hecho::Angulo { a: 'C', .. })), "{h:?}");
        // En LaTeX, como lo escribe la nube.
        let h = hechos_de_texto(&[r"m\measuredangle AOB = 90^{\circ}", r"\overline{AB} es diametro"]);
        assert!(h.contains(&Hecho::Angulo { a: 'A', v: 'O', b: 'B', grados: 90.0 }), "{h:?}");
        assert!(h.contains(&Hecho::Diametro('A', 'B')), "{h:?}");
        // Nada que ver: nada.
        assert!(hechos_de_texto(&["Calcule el area del triangulo", "x = 45"]).is_empty());
    }
}
