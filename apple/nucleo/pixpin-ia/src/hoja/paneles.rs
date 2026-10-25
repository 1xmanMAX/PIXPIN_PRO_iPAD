//! **Los recuadros de una hoja** (17-oct-2026): las tarjetas con su fondo
//! (las grises de «Cuadrilateros II» con Romboide, Rombo, Rectangulo...),
//! las cajas con su borde («¡Tenga en cuenta que...!», «En la figura:»), la
//! franja de la cabecera, la pastilla de «Agregar comentario...».
//!
//! El usuario (17-oct): «la segmentacion no es correcta, algunas cosas salen
//! desubicadas y otras imagenes se comen algo del texto». El seccionado de
//! siempre ([`super::seccion`]) mira la hoja como un solo plano, y lo de una
//! tarjeta se mezclaba con lo de la de al lado (un «dibujo» se tragaba cuatro
//! tarjetas; la marca de agua salia como foto). Aqui:
//!
//! 1. **Se buscan los recuadros** ([`buscar`]): un fondo liso distinto del
//!    papel con sus cuatro lados rectos, o cuatro rayas que cierran una caja
//!    con texto dentro. Con su relleno, su borde y si es redondeado.
//! 2. **Cada recuadro se secciona aparte** ([`seccionar`]): su interior es una
//!    hoja (con su fondo como papel) y lo que hay fuera, otra, con los
//!    recuadros tapados. Asi el titulo, la figura y las formulas de una
//!    tarjeta son suyos y de nadie mas.
//! 3. **Lo que es adorno se quita** ([`quitar_adornos`]): lo gris claro sin
//!    tinta (la marca de agua «CESAR VALLEJO», los iconos de la cabecera) no
//!    se pega como imagen.
//!
//! El recuadro sale como [`Tipo::Recuadro`]: un rectangulo relleno (la
//! herramienta de relleno del lienzo), debajo de lo suyo.

use super::imagen::{self, Analisis};
use super::seccion;
use super::{Alineacion, Caja, PalabraHoja, Seccionado, Tipo, Zona, mediana};

/// Como se ve un recuadro.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Estilo {
    /// Su fondo (`None`: el del papel, solo borde).
    pub relleno: Option<[u8; 3]>,
    /// Su borde (`None`: sin borde, solo fondo).
    pub borde: Option<[u8; 3]>,
    /// El grueso del borde, en pixeles de la pagina.
    pub grosor: f64,
    /// Esquinas redondeadas.
    pub redondo: bool,
}

/// Un recuadro, en pixeles de la pagina (su caja lleva el borde).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Panel {
    pub caja: Caja,
    pub estilo: Estilo,
}

impl Panel {
    /// Lo de dentro (sin el borde).
    pub fn interior(&self) -> Caja {
        let g = if self.estilo.borde.is_some() { self.estilo.grosor + 1.0 } else { 1.0 };
        Caja::nueva(self.caja.x + g, self.caja.y + g, (self.caja.w - 2.0 * g).max(1.0), (self.caja.h - 2.0 * g).max(1.0))
    }
}

/// El lado de la celda (pixeles de la hoja reducida) con que se busca el fondo.
const CELDA: usize = 2;

/// La mediana de cada canal de los pixeles de `caja` que cumplen `vale`
/// (cada `paso` pixeles).
fn color_mediano(rgba: &[u8], w: u32, h: u32, caja: &Caja, paso: usize, vale: impl Fn([u8; 3]) -> bool) -> Option<[u8; 3]> {
    let c = caja.dentro(w as f64, h as f64);
    let mut canales: [Vec<u8>; 3] = Default::default();
    for y in (c.y as u32..c.y1() as u32).step_by(paso.max(1)) {
        for x in (c.x as u32..c.x1() as u32).step_by(paso.max(1)) {
            let i = ((y * w + x) * 4) as usize;
            let p = [rgba[i], rgba[i + 1], rgba[i + 2]];
            if vale(p) {
                for k in 0..3 {
                    canales[k].push(p[k]);
                }
            }
        }
    }
    if canales[0].is_empty() {
        return None;
    }
    let mut m = [0u8; 3];
    for k in 0..3 {
        canales[k].sort_unstable();
        m[k] = canales[k][canales[k].len() / 2];
    }
    Some(m)
}

/// La distancia entre dos colores (euclidea en RGB).
fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn gris(p: [u8; 3]) -> f64 {
    (p[0] as f64 * 299.0 + p[1] as f64 * 587.0 + p[2] as f64 * 114.0) / 1000.0
}

/// **Los recuadros de la hoja.** `hm`: el alto de letra del cuerpo.
pub fn buscar(rgba: &[u8], w: u32, h: u32, a: &Analisis, palabras: &[PalabraHoja], hm: f64) -> Vec<Panel> {
    let mut v = por_fondo(rgba, w, h, a, hm);
    // Los bordes suaves (el gris claro de una tarjeta, que no llega a tinta):
    // las rayas, con un umbral casi de papel. Asi sale la tarjeta que la marca
    // de agua parte en trozos (su fondo no es un solo grupo).
    let mut suave = a.clone();
    suave.umbral = a.fondo.saturating_sub(25);
    suave.tinta = suave.gris.iter().map(|&g| g < suave.umbral).collect();
    let mut bordes = por_borde(rgba, w, h, a, a, palabras, hm);
    bordes.extend(por_borde(rgba, w, h, a, &suave, palabras, hm));
    for b in bordes {
        // El mismo recuadro visto por su fondo y por su borde: uno, con los dos.
        match v.iter_mut().find(|p| p.caja.crecer(hm).contiene(b.caja.x, b.caja.y) && b.caja.crecer(hm).contiene(p.caja.x1(), p.caja.y1()) && p.caja.crecer(hm).contiene(b.caja.x1(), b.caja.y1())) {
            Some(p) => {
                if p.estilo.borde.is_none() {
                    p.estilo.borde = b.estilo.borde;
                    p.estilo.grosor = b.estilo.grosor;
                    p.caja = p.caja.union(&b.caja);
                }
            }
            None => v.push(b),
        }
    }
    // (Uno que casi es la hoja entera es el papel.)
    let hoja = w as f64 * h as f64;
    v.retain(|p| p.caja.area() < 0.85 * hoja);
    v
}

/// Los recuadros por su **fondo liso**: celdas lisas algo mas oscuras que el
/// papel (sin ser tinta), juntas si tienen el mismo gris, y un grupo es un
/// recuadro si sus cuatro lados son rectos y llenos (la marca de agua y un
/// sombreado no lo son).
fn por_fondo(rgba: &[u8], w: u32, h: u32, a: &Analisis, hm: f64) -> Vec<Panel> {
    let (cw, ch) = (a.w / CELDA, a.h / CELDA);
    if cw < 4 || ch < 4 {
        return Vec::new();
    }
    // **En color** (18-oct): el boton azul «ver Mapa», la barra oscura de un
    // navegador, una tarjeta crema. Una celda vale si es lisa en sus tres
    // canales y no es papel; da igual lo oscura que sea (el texto no forma
    // rectangulos grandes con los cuatro lados llenos, que es lo que se pide).
    let kp = CELDA as f64 / a.f;
    let papel = [a.fondo as f64; 3];
    let mut color = vec![[0f64; 3]; cw * ch];
    let mut vale = vec![false; cw * ch];
    for cy in 0..ch {
        for cx in 0..cw {
            let (x0, y0) = ((cx as f64 * kp) as u32, (cy as f64 * kp) as u32);
            let (x1, y1) = ((((cx + 1) as f64 * kp) as u32).min(w), (((cy + 1) as f64 * kp) as u32).min(h));
            let (mut s, mut n) = ([0f64; 3], 0f64);
            let (mut lo, mut hi) = ([255u8; 3], [0u8; 3]);
            for y in y0..y1.max(y0 + 1).min(h) {
                for x in x0..x1.max(x0 + 1).min(w) {
                    let i = ((y * w + x) * 4) as usize;
                    for c in 0..3 {
                        let v = rgba[i + c];
                        s[c] += v as f64;
                        lo[c] = lo[c].min(v);
                        hi[c] = hi[c].max(v);
                    }
                    n += 1.0;
                }
            }
            let m = [s[0] / n.max(1.0), s[1] / n.max(1.0), s[2] / n.max(1.0)];
            color[cy * cw + cx] = m;
            let liso = (0..3).all(|c| hi[c] - lo[c] <= 12);
            vale[cy * cw + cx] = liso && dist(m, papel) >= 7.0;
        }
    }
    let media: Vec<u8> = color.iter().map(|m| gris([m[0] as u8, m[1] as u8, m[2] as u8]) as u8).collect();
    // Grupos de celdas vecinas del mismo gris.
    let mut padre: Vec<usize> = (0..cw * ch).collect();
    fn raiz(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    for cy in 0..ch {
        for cx in 0..cw {
            let i = cy * cw + cx;
            if !vale[i] {
                continue;
            }
            for j in [if cx + 1 < cw { Some(i + 1) } else { None }, if cy + 1 < ch { Some(i + cw) } else { None }].into_iter().flatten() {
                if vale[j] && dist(color[i], color[j]) <= 8.0 {
                    let (ri, rj) = (raiz(&mut padre, i), raiz(&mut padre, j));
                    if ri != rj {
                        padre[rj] = ri;
                    }
                }
            }
        }
    }
    let mut grupos: std::collections::HashMap<usize, (usize, usize, usize, usize, usize)> = Default::default();
    let mut de = vec![usize::MAX; cw * ch];
    for i in 0..cw * ch {
        if !vale[i] {
            continue;
        }
        let r = raiz(&mut padre, i);
        de[i] = r;
        let (x, y) = (i % cw, i / cw);
        let g = grupos.entry(r).or_insert((x, y, x, y, 0));
        g.0 = g.0.min(x);
        g.1 = g.1.min(y);
        g.2 = g.2.max(x);
        g.3 = g.3.max(y);
        g.4 += 1;
    }
    let k = CELDA as f64 / a.f;
    let mut salida = Vec::new();
    for (r, (mut x0, mut y0, mut x1, mut y1, n)) in grupos {
        let (bw, bh) = (x1 - x0 + 1, y1 - y0 + 1);
        if (bw as f64) * k < 5.0 * hm || (bh as f64) * k < 2.2 * hm || bw < 8 || bh < 6 {
            continue;
        }
        let depurar = std::env::var_os("PIXPIN_PANELES_DEPURAR").is_some();
        if depurar {
            eprintln!("por_fondo: grupo {}x{} celdas en ({x0},{y0}) n={n}", bw, bh);
        }
        if (n as f64) < 0.3 * (bw * bh) as f64 {
            continue;
        }
        // Los cuatro lados, llenos (sin las esquinas, que pueden ser redondas).
        let m = (bw.min(bh) / 6).clamp(2, 8);
        let lleno = |xs: &mut dyn Iterator<Item = (usize, usize)>| {
            let (mut t, mut s) = (0usize, 0usize);
            for (x, y) in xs {
                t += 1;
                s += (de[y * cw + x] == r) as usize;
            }
            if t == 0 { 0.0 } else { s as f64 / t as f64 }
        };
        // (Un lado deshilachado, la sombra o el redondeo de un boton que
        // asoma una celda de mas: se recorta ese lado, hasta 3 celdas. El
        // boton «Descargar Croquis» de la ONPE se perdia por eso.)
        let mut recortado = [0usize; 4];
        let lados = loop {
            let lados = [
                lleno(&mut (x0 + m..=x1 - m).flat_map(|x| [(x, y0), (x, y0 + 1)])),
                lleno(&mut (x0 + m..=x1 - m).flat_map(|x| [(x, y1), (x, y1 - 1)])),
                lleno(&mut (y0 + m..=y1 - m).flat_map(|y| [(x0, y), (x0 + 1, y)])),
                lleno(&mut (y0 + m..=y1 - m).flat_map(|y| [(x1, y), (x1 - 1, y)])),
            ];
            let peor = (0..4).min_by(|&i, &j| lados[i].total_cmp(&lados[j])).unwrap_or(0);
            if lados[peor] >= 0.6 || recortado[peor] >= 3 || x1 - x0 < 8 + 2 * m || y1 - y0 < 6 + 2 * m {
                break lados;
            }
            recortado[peor] += 1;
            match peor {
                0 => y0 += 1,
                1 => y1 -= 1,
                2 => x0 += 1,
                _ => x1 -= 1,
            }
        };
        if lados.iter().any(|&l| l < 0.6) {
            if depurar {
                eprintln!("  lados {lados:?}: no");
            }
            continue;
        }
        let redondo = de[y0 * cw + x0] != r && de[y0 * cw + x0 + m] == r;
        let caja = Caja::de_esquinas(x0 as f64 * k, y0 as f64 * k, (x1 + 1) as f64 * k, (y1 + 1) as f64 * k);
        // El fondo: lo liso de dentro, en color.
        let g_medio = media.iter().zip(&de).filter(|(_, d)| **d == r).map(|(m, _)| *m as f64).sum::<f64>() / n as f64;
        let mut suma = [0f64; 3];
        for (c, _) in color.iter().zip(&de).filter(|(_, d)| **d == r) {
            for k in 0..3 {
                suma[k] += c[k];
            }
        }
        let relleno = Some([(suma[0] / n as f64) as u8, (suma[1] / n as f64) as u8, (suma[2] / n as f64) as u8]);
        // El borde: una franja mas oscura justo por fuera, en tres lados o mas.
        let (borde, grosor, caja) = borde_de(rgba, w, h, &caja, g_medio, hm);
        salida.push(Panel { caja, estilo: Estilo { relleno, borde, grosor, redondo } });
    }
    salida
}

/// El borde de un recuadro de fondo: mira por fuera de cada lado hasta
/// `hm / 2`; si en tres lados o mas hay una franja mas oscura que el fondo,
/// ese es el borde (su color y su grueso) y la caja crece hasta el.
fn borde_de(rgba: &[u8], w: u32, h: u32, caja: &Caja, g_fondo: f64, hm: f64) -> (Option<[u8; 3]>, f64, Caja) {
    let hasta = (hm * 0.5).clamp(2.0, 8.0) as i64;
    let mut gruesos = Vec::new();
    let mut colores: Vec<[u8; 3]> = Vec::new();
    let px = |x: i64, y: i64| -> Option<[u8; 3]> {
        if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 {
            return None;
        }
        let i = ((y as u32 * w + x as u32) * 4) as usize;
        Some([rgba[i], rgba[i + 1], rgba[i + 2]])
    };
    // (lado: desde donde y hacia donde se mira; se toma la mitad del lado)
    let (x0, y0, x1, y1) = (caja.x as i64, caja.y as i64, caja.x1() as i64, caja.y1() as i64);
    let lados: [(Box<dyn Fn(i64, i64) -> (i64, i64)>, i64, i64); 4] = [
        (Box::new(move |t, d| (t, y0 - 1 - d)), x0 + (x1 - x0) / 4, x1 - (x1 - x0) / 4),
        (Box::new(move |t, d| (t, y1 + d)), x0 + (x1 - x0) / 4, x1 - (x1 - x0) / 4),
        (Box::new(move |t, d| (x0 - 1 - d, t)), y0 + (y1 - y0) / 4, y1 - (y1 - y0) / 4),
        (Box::new(move |t, d| (x1 + d, t)), y0 + (y1 - y0) / 4, y1 - (y1 - y0) / 4),
    ];
    let mut con = 0;
    for (donde, t0, t1) in &lados {
        // El grueso de la franja oscura en este lado (la mediana a lo largo).
        let mut g = Vec::new();
        for t in (*t0..*t1).step_by(3) {
            let mut n = 0;
            for d in 0..=hasta {
                let (x, y) = donde(t, d);
                match px(x, y) {
                    Some(p) if gris(p) < g_fondo - 22.0 => {
                        n += 1;
                        if d < 3 {
                            colores.push(p);
                        }
                    }
                    _ if n > 0 => break,
                    _ => {
                        if d >= 2 {
                            break;
                        }
                    }
                }
            }
            g.push(n as f64);
        }
        let gm = mediana(g).unwrap_or(0.0);
        if gm >= 1.0 {
            con += 1;
            gruesos.push(gm);
        }
    }
    if con < 3 {
        return (None, 0.0, *caja);
    }
    let grosor = mediana(gruesos).unwrap_or(1.0).min(hasta as f64);
    let mut c: Vec<[u8; 3]> = colores;
    c.sort_by(|a, b| gris(*a).total_cmp(&gris(*b)));
    let color = c.get(c.len() / 3).copied();
    (color, grosor, caja.crecer(grosor))
}

/// Los recuadros por su **borde**: dos rayas horizontales del mismo largo y
/// dos verticales que las unen, con texto dentro y sin rayas que lo partan
/// (eso es una tabla). El de una figura (el rectangulo de «Rectangulo») no
/// lleva texto dentro: no es un recuadro.
fn por_borde(rgba: &[u8], w: u32, h: u32, a: &Analisis, de_rayas: &Analisis, palabras: &[PalabraHoja], hm: f64) -> Vec<Panel> {
    let rayas = imagen::rayas(de_rayas, 0.04);
    let hs: Vec<&imagen::Raya> = rayas.iter().filter(|r| !r.vertical && r.largo() >= 6.0 * hm).collect();
    let vs: Vec<&imagen::Raya> = rayas.iter().filter(|r| r.vertical).collect();
    let mut salida: Vec<Panel> = Vec::new();
    for t in &hs {
        // La de abajo mas cercana que cierre la caja.
        let mut abajo: Vec<&&imagen::Raya> = hs.iter().filter(|b| b.y0 - t.y0 >= 2.0 * hm).collect();
        abajo.sort_by(|a, b| a.y0.total_cmp(&b.y0));
        for b in abajo {
            let (x0, x1) = (t.x0.max(b.x0), t.x1.min(b.x1));
            if x1 - x0 < 0.85 * t.largo().min(b.largo()) {
                continue;
            }
            // (Una caja cortada por el borde de la imagen, 18-oct: los
            // recuadros de la columna de la derecha de Polinomios llegan al
            // borde y no tienen lado derecho. El borde la cierra.)
            let al_borde = |x: f64| x <= hm * 0.8 || x >= w as f64 - hm * 0.8;
            let cierra = |x: f64| al_borde(x) || vs.iter().any(|v| (v.x0 - x).abs() <= hm * 0.8 && v.y0 <= t.y0 + hm && v.y1 >= b.y0 - hm && (v.y1.min(b.y0) - v.y0.max(t.y0)) >= 0.8 * (b.y0 - t.y0));
            if !(cierra(x0) && cierra(x1)) {
                continue;
            }
            let caja = Caja::de_esquinas(x0, t.y0, x1, b.y0);
            let dentro = caja.crecer(-hm * 0.3);
            let letras: usize = palabras.iter().filter(|p| dentro.contiene(p.caja.cx(), p.caja.cy())).map(|p| p.texto.chars().filter(|c| c.is_alphanumeric()).count()).sum();
            // Una raya que la cruza entera por dentro: una tabla.
            let partida = rayas.iter().any(|r| {
                if r.vertical {
                    r.x0 > x0 + hm && r.x0 < x1 - hm && (r.y1.min(b.y0) - r.y0.max(t.y0)) >= 0.8 * (b.y0 - t.y0)
                } else {
                    r.y0 > t.y0 + hm && r.y0 < b.y0 - hm && (r.x1.min(x1) - r.x0.max(x0)) >= 0.8 * (x1 - x0)
                }
            });
            if std::env::var_os("PIXPIN_PANELES_DEPURAR").is_some() {
                eprintln!("caja por borde {caja:?}: letras {letras}, partida {partida}");
            }
            let papel = a.fondo as f64;
            let relleno = color_mediano(rgba, w, h, &caja.crecer(-hm * 0.2), 4, |p| gris(p) > a.umbral as f64 + 30.0).filter(|c| (gris(*c) - papel).abs() > 8.0);
            // (Con fondo propio es una tarjeta aunque el OCR no leyera nada dentro.)
            if (letras >= 4 || relleno.is_some()) && !partida {
                let grosor = ((t.grosor + b.grosor) / 2.0).max(1.0);
                let borde = color_mediano(rgba, w, h, &t.caja(), 1, |p| gris(p) < 200.0);
                salida.push(Panel { caja: caja.crecer(grosor / 2.0), estilo: Estilo { relleno, borde, grosor, redondo: false } });
            }
            break;
        }
    }
    // **Cajas en U** (17-oct, «¡Tenga en cuenta que...!»): arriba no hay raya
    // sino la pestana oscura del titulo; la raya de abajo y dos laterales que
    // empiezan a la misma altura la cierran.
    for b in &hs {
        if salida.iter().any(|p| (p.caja.y1() - b.y0).abs() <= hm && (p.caja.x - b.x0).abs() <= hm) {
            continue;
        }
        let lado = |x: f64| vs.iter().filter(|v| (v.x0 - x).abs() <= hm * 0.8 && (v.y1 - b.y0).abs() <= hm && v.largo() >= 3.0 * hm).max_by(|p, q| p.largo().total_cmp(&q.largo())).copied();
        // (Un lado en el borde de la imagen: vale el otro.)
        let en_borde = |x: f64| x <= hm * 0.8 || x >= w as f64 - hm * 0.8;
        let arriba = match (lado(b.x0), lado(b.x1)) {
            (Some(i), Some(d)) if (i.y0 - d.y0).abs() <= 2.0 * hm => i.y0.max(d.y0),
            (Some(i), None) if en_borde(b.x1) => i.y0,
            (None, Some(d)) if en_borde(b.x0) => d.y0,
            _ => continue,
        };
        let caja = Caja::de_esquinas(b.x0, arriba, b.x1, b.y0);
        let letras: usize = palabras.iter().filter(|p| caja.contiene(p.caja.cx(), p.caja.cy())).map(|p| p.texto.chars().filter(|c| c.is_alphanumeric()).count()).sum();
        if letras < 4 {
            continue;
        }
        let grosor = b.grosor.max(1.0);
        let borde = color_mediano(rgba, w, h, &b.caja(), 1, |p| gris(p) < 200.0);
        salida.push(Panel { caja: caja.crecer(grosor / 2.0), estilo: Estilo { relleno: None, borde, grosor, redondo: false } });
    }
    salida
}

/// **Quita los adornos**: fotos, dibujos y dudosas que son solo gris claro,
/// sin nada de tinta (la marca de agua, los iconos palidos de una cabecera,
/// un sombreado). Pegarlos como imagen ensuciaba el lienzo y tapaba texto.
pub fn quitar_adornos(s: &mut Seccionado, rgba: &[u8]) {
    let (w, h) = (s.ancho, s.alto);
    if rgba.len() < (w * h * 4) as usize {
        return;
    }
    let a = imagen::analizar(rgba, w, h);
    let claro = a.umbral as f64 + 20.0;
    s.zonas.retain(|z| {
        if !matches!(z.tipo, Tipo::Foto | Tipo::Dibujo | Tipo::Dudosa | Tipo::Grafica) || !z.renglones.is_empty() {
            return true;
        }
        let c = z.caja.dentro(w as f64, h as f64);
        let mut g: Vec<f64> = Vec::new();
        let mut sat: Vec<f64> = Vec::new();
        let paso = ((c.w * c.h / 4000.0).sqrt() as u32).max(1);
        for y in (c.y as u32..c.y1() as u32).step_by(paso as usize) {
            for x in (c.x as u32..c.x1() as u32).step_by(paso as usize) {
                let i = ((y * w + x) * 4) as usize;
                let p = [rgba[i], rgba[i + 1], rgba[i + 2]];
                g.push(gris(p));
                sat.push((p.iter().max().unwrap() - p.iter().min().unwrap()) as f64);
            }
        }
        if g.len() < 16 {
            return true;
        }
        g.sort_by(f64::total_cmp);
        let oscuro = g[g.len() / 50];
        let s_media = mediana(sat).unwrap_or(0.0);
        let adorno = oscuro >= claro && s_media < 25.0;
        !adorno
    });
    for (i, z) in s.zonas.iter_mut().enumerate() {
        z.id = i;
        z.orden = i;
    }
}

/// **Lo escrito en blanco sobre oscuro** (la pestana «¡Tenga en cuenta
/// que...!»): como imagen. Como texto salia minusculo y negro sobre blanco,
/// sin su cartel.
fn invertidos(s: &mut Seccionado, rgba: &[u8], fondo: [u8; 3]) {
    // (Sobre un fondo oscuro la letra blanca es lo normal: es texto.)
    if gris(fondo) < 128.0 {
        return;
    }
    let (w, h) = (s.ancho, s.alto);
    for z in s.zonas.iter_mut().filter(|z| matches!(z.tipo, Tipo::Parrafo | Tipo::Encabezado | Tipo::Formula | Tipo::Dudosa)) {
        let Some(c) = color_mediano(rgba, w, h, &z.caja, 2, |_| true) else { continue };
        if gris(c) < 110.0 {
            z.tipo = Tipo::Foto;
            z.renglones.clear();
        }
    }
}

/// **Borra lo palido de dentro de cada recuadro con fondo** (la marca de agua
/// «CESAR VALLEJO» encima de la tarjeta Rombo): lo gris casi del color del
/// fondo pasa a ser fondo. Con ella, la figura de la tarjeta salia con manchas
/// y la tarjeta como un recorte. Lo oscuro (la tinta) no se toca. Devuelve la
/// hoja limpia (`None` si no habia nada que limpiar).
fn limpiar(rgba: &[u8], w: u32, h: u32, paneles: &[Panel]) -> Option<Vec<u8>> {
    let mut v: Option<Vec<u8>> = None;
    for p in paneles {
        let Some(f) = p.estilo.relleno else { continue };
        let gf = gris(f);
        // (Solo en los claros: en un boton o una barra oscura no hay marca de agua.)
        if gf < 150.0 {
            continue;
        }
        let c = p.interior().dentro(w as f64, h as f64);
        for y in c.y as u32..c.y1() as u32 {
            for x in c.x as u32..c.x1() as u32 {
                let i = ((y * w + x) * 4) as usize;
                let px = [rgba[i], rgba[i + 1], rgba[i + 2]];
                let g = gris(px);
                let sat = px.iter().max().unwrap() - px.iter().min().unwrap();
                if g >= gf - 34.0 && g <= gf + 20.0 && sat < 25 && (g - gf).abs() >= 3.0 {
                    let d = v.get_or_insert_with(|| rgba.to_vec());
                    d[i..i + 3].copy_from_slice(&f);
                }
            }
        }
    }
    v
}

/// **Una formula corta en la misma linea que un renglon de texto es de ese
/// renglon** («... Si AD=13 y» con «13 y» leido como formula aparte): se le
/// junta. Aparte, la nube que corrige el renglon veia «13 y» en la imagen y
/// lo ponia tambien, y salia dos veces.
/// **Renglones de texto con formulas** (18-oct, Polinomios: «P(x)=x⁴+6 es un
/// polinomio…» salia en tres zonas que se pisaban, y el texto dos veces):
/// las zonas de texto, formula o tinta suelta que se pisan o siguen en el
/// mismo renglon son UNA zona de texto, con sus renglones rehechos por
/// altura. La nube lee cada renglon entero y pone sus formulas en linea
/// (`$...$`). Un dibujo de verdad (grande y sin letras) no entra.
fn juntar_texto_y_formulas(s: &mut Seccionado) {
    let a = s.alto_letra.max(4.0);
    let entra = |z: &Zona| match z.tipo {
        Tipo::Parrafo | Tipo::Encabezado | Tipo::Formula | Tipo::Dudosa => z.caja.h <= 14.0 * a,
        // Un trozo de dibujo pequeno con letras (una formula que el
        // seccionado vio como dibujo).
        Tipo::Dibujo => !z.renglones.is_empty() && z.caja.h <= 12.0 * a && z.caja.w <= 30.0 * a,
        _ => false,
    };
    let juntas = |p: &Zona, q: &Zona| {
        let c = p.caja.comun(&q.caja);
        if c > 0.0 && c >= 0.15 * p.caja.area().min(q.caja.area()) {
            return true;
        }
        // En el mismo renglon y casi pegadas.
        let mismo = p.caja.solape_y(&q.caja) >= 0.5 * p.caja.h.min(q.caja.h).min(2.0 * a);
        let hueco = if q.caja.x >= p.caja.x1() { q.caja.x - p.caja.x1() } else if p.caja.x >= q.caja.x1() { p.caja.x - q.caja.x1() } else { 0.0 };
        mismo && hueco <= 1.5 * a && (p.caja.h <= 3.0 * a || q.caja.h <= 3.0 * a)
    };
    // Grupos por union de vecinos (hasta que no crezcan).
    let n = s.zonas.len();
    let mut grupo: Vec<usize> = (0..n).collect();
    let raiz = |g: &Vec<usize>, mut i: usize| {
        while g[i] != i {
            i = g[i];
        }
        i
    };
    for i in 0..n {
        if !entra(&s.zonas[i]) {
            continue;
        }
        for j in i + 1..n {
            if entra(&s.zonas[j]) && juntas(&s.zonas[i], &s.zonas[j]) {
                let (ri, rj) = (raiz(&grupo, i), raiz(&grupo, j));
                if ri != rj {
                    grupo[rj] = ri;
                }
            }
        }
    }
    let mut por_raiz: std::collections::BTreeMap<usize, Vec<usize>> = std::collections::BTreeMap::new();
    for i in 0..n {
        por_raiz.entry(raiz(&grupo, i)).or_default().push(i);
    }
    let mut quitar: Vec<usize> = Vec::new();
    for (r0, miembros) in por_raiz {
        // Sola, se rehace si algun renglon es tan alto que son dos.
        let alto_de_mas = miembros.len() == 1 && entra(&s.zonas[r0]) && s.zonas[r0].renglones.iter().any(|rg| rg.caja.h > 1.9 * a);
        if miembros.len() < 2 && !alto_de_mas {
            continue;
        }
        // Los renglones de todos, y la tinta sin letras de los que no tienen.
        let renglones: Vec<super::Renglon> = miembros.iter().flat_map(|&k| s.zonas[k].renglones.iter().cloned()).collect();
        let sueltas: Vec<Caja> = miembros.iter().filter(|&&k| s.zonas[k].renglones.is_empty()).map(|&k| s.zonas[k].caja).collect();
        let mut caja = s.zonas[r0].caja;
        for &k in &miembros {
            caja = caja.union(&s.zonas[k].caja);
        }
        // Por altura, palabra a palabra: un renglon por banda (el centro de
        // cada palabra). Los renglones que traian mezclaban a veces dos
        // (una formula alta con exponentes).
        let mut palabras: Vec<PalabraHoja> = renglones.iter().flat_map(|r| r.palabras.iter().cloned()).collect();
        palabras.sort_by(|p, q| p.caja.cy().total_cmp(&q.caja.cy()));
        let mut bandas: Vec<(f64, super::Renglon)> = Vec::new();
        for p in palabras {
            let c = p.caja.cy();
            let color = renglones.iter().find(|r| r.palabras.iter().any(|q| q.caja.x == p.caja.x && q.caja.y == p.caja.y)).and_then(|r| r.color);
            match bandas.iter_mut().find(|(cb, _)| (*cb - c).abs() < 0.55 * a) {
                Some((cb, b)) => {
                    let n = b.palabras.len() as f64;
                    *cb = (*cb * n + c) / (n + 1.0);
                    b.negrita = b.negrita && p.negrita;
                    b.caja = b.caja.union(&p.caja);
                    if b.color.is_none() {
                        b.color = color;
                    }
                    b.palabras.push(p);
                }
                None => {
                    let negrita = p.negrita;
                    bandas.push((c, super::Renglon { texto: String::new(), caja: p.caja, palabras: vec![p], negrita, color }));
                }
            }
        }
        let mut bandas: Vec<super::Renglon> = bandas.into_iter().map(|(_, b)| b).collect();
        for b in &mut bandas {
            b.palabras.sort_by(|p, q| p.caja.x.total_cmp(&q.caja.x));
            b.texto = b.palabras.iter().map(|p| p.texto.as_str()).collect::<Vec<_>>().join(" ");
        }
        // La tinta suelta (una formula sin leer) alarga el renglon que pisa.
        for c in &sueltas {
            if let Some(b) = bandas.iter_mut().max_by(|p, q| p.caja.solape_y(c).total_cmp(&q.caja.solape_y(c))).filter(|b| b.caja.solape_y(c) > 0.0) {
                b.caja = b.caja.union(&Caja::de_esquinas(c.x, b.caja.y, c.x1(), b.caja.y1()));
            }
        }
        bandas.sort_by(|p, q| p.caja.y.total_cmp(&q.caja.y));
        let z = &mut s.zonas[r0];
        z.caja = caja;
        z.tipo = Tipo::Parrafo;
        z.renglones = bandas;
        z.confianza = z.confianza.max(0.8);
        quitar.extend(miembros.iter().copied().filter(|&k| k != r0));
    }
    if quitar.is_empty() {
        return;
    }
    let mut i = 0;
    s.zonas.retain(|_| {
        let q = quitar.contains(&i);
        i += 1;
        !q
    });
    for (i, z) in s.zonas.iter_mut().enumerate() {
        z.id = i;
        z.orden = i;
    }
}

fn juntar_formulas_al_renglon(s: &mut Seccionado) {
    let a = s.alto_letra.max(4.0);
    let mut quitar: Vec<usize> = Vec::new();
    for fi in 0..s.zonas.len() {
        let f = &s.zonas[fi];
        if f.tipo != Tipo::Formula || f.renglones.len() != 1 || f.texto().chars().count() > 30 {
            continue;
        }
        // (La caja de la ZONA: lleva la tinta que el OCR no leyo, el «AD=» de «AD=13 y».)
        let mut fr = f.renglones[0].clone();
        fr.caja = fr.caja.union(&f.caja);
        let sitio = s.zonas.iter().enumerate().filter(|(i, z)| *i != fi && matches!(z.tipo, Tipo::Parrafo | Tipo::Encabezado)).find_map(|(i, z)| {
            z.renglones.iter().position(|r| {
                let misma_linea = r.caja.solape_y(&fr.caja) >= 0.6 * r.caja.h.min(fr.caja.h);
                let hueco = if fr.caja.x >= r.caja.x1() { fr.caja.x - r.caja.x1() } else if r.caja.x >= fr.caja.x1() { r.caja.x - fr.caja.x1() } else { 0.0 };
                misma_linea && hueco <= 3.0 * a
            }).map(|k| (i, k))
        });
        let Some((ti, k)) = sitio else { continue };
        let r = &mut s.zonas[ti].renglones[k];
        r.palabras.extend(fr.palabras.iter().cloned());
        r.palabras.sort_by(|x, y| x.caja.x.total_cmp(&y.caja.x));
        r.texto = r.palabras.iter().map(|p| p.texto.as_str()).collect::<Vec<_>>().join(" ");
        r.caja = r.caja.union(&fr.caja);
        let c = s.zonas[ti].caja.union(&fr.caja);
        s.zonas[ti].caja = c;
        quitar.push(fi);
    }
    if quitar.is_empty() {
        return;
    }
    let mut i = 0;
    s.zonas.retain(|_| {
        let q = quitar.contains(&i);
        i += 1;
        !q
    });
    for (i, z) in s.zonas.iter_mut().enumerate() {
        z.id = i;
        z.orden = i;
    }
}

/// **El color de cada renglon, que se vea sobre su fondo** (18-oct): sobre la
/// barra oscura de un navegador o un boton azul, el OCR daba la letra
/// «negra» (o ningun color) y salia invisible. Si el color leido casi no se
/// distingue del fondo, se toma el de la tinta del renglon que mas se aparta
/// de el (la mediana de esos pixeles).
fn contrastar(s: &mut Seccionado, rgba: &[u8], fondo: [u8; 3]) {
    let (w, h) = (s.ancho, s.alto);
    for z in &mut s.zonas {
        // El fondo de AQUI (un boton que no se vio como recuadro): lo que mas
        // hay alrededor de la zona; si casi no hay, el de la region.
        let marco = z.caja.crecer((z.caja.h * 0.25).clamp(3.0, 12.0));
        let local = color_mediano(rgba, w, h, &marco, 2, |_| true).unwrap_or(fondo);
        let f = [local[0] as f64, local[1] as f64, local[2] as f64];
        let cerca = |c: Option<[u8; 3]>| {
            let c = c.unwrap_or([20, 20, 20]);
            dist([c[0] as f64, c[1] as f64, c[2] as f64], f) < 90.0
        };
        for r in &mut z.renglones {
            // (El renglon o alguna palabra: de las palabras sale el color de
            // la zona.)
            if !cerca(r.color) && !r.palabras.iter().any(|p| cerca(p.color)) {
                continue;
            }
            if let Some(c) = color_mediano(rgba, w, h, &r.caja, 1, |p| dist([p[0] as f64, p[1] as f64, p[2] as f64], f) >= 90.0) {
                if cerca(r.color) {
                    r.color = Some(c);
                }
                for p in r.palabras.iter_mut().filter(|p| cerca(p.color)) {
                    p.color = Some(c);
                }
            }
        }
    }
}

/// **Una formula enmarcada que el OCR no leyo** (18-oct, «Cα=90°−α» de
/// Reforzamiento): el marco sale como rayas sueltas (dos horizontales de la
/// misma anchura, y a veces sus laterales) y lo de dentro, sin letras, se
/// perdia. Las rayas y lo de dentro pasan a ser UNA zona de formula.
fn cajitas(s: &mut Seccionado) {
    let hm = s.alto_letra.max(4.0);
    let horizontal = |z: &Zona| z.tipo == Tipo::Linea && z.caja.h <= 4.0 && z.caja.w >= hm * 2.0;
    let mut i = 0;
    while i < s.zonas.len() {
        if !horizontal(&s.zonas[i]) {
            i += 1;
            continue;
        }
        let a = s.zonas[i].caja;
        let pareja = (0..s.zonas.len()).find(|&j| {
            let b = s.zonas[j].caja;
            j != i && horizontal(&s.zonas[j]) && b.y > a.y && (b.y - a.y) >= hm * 1.2 && (b.y - a.y) <= hm * 4.5
                && (b.x - a.x).abs() <= hm * 0.6 && (b.x1() - a.x1()).abs() <= hm * 0.6
        });
        let Some(j) = pareja else {
            i += 1;
            continue;
        };
        // (Con aire: los laterales del marco quedan un pelo por fuera de las rayas.)
        let marco = a.union(&s.zonas[j].caja).crecer((hm * 0.5).max(3.0));
        // (Si dentro hay letras leidas, ya es texto: no se toca.)
        let con_letras = s.zonas.iter().any(|z| z.renglones.iter().any(|r| marco.contiene(r.caja.cx(), r.caja.cy())));
        if con_letras {
            i += 1;
            continue;
        }
        let dentro: Vec<usize> = (0..s.zonas.len()).filter(|&k| k != i && s.zonas[k].renglones.is_empty() && s.zonas[k].caja.dentro_de(&marco) >= 0.8).collect();
        // (Solo lo de dentro, sin las rayas: el marco lo dibuja la formula.)
        let b = s.zonas[j].caja;
        let caja = Caja::de_esquinas(a.x + 3.0, a.y1() + 2.0, a.x1() - 3.0, b.y - 2.0);
        let z = &mut s.zonas[i];
        z.caja = caja;
        z.tipo = Tipo::Formula;
        z.rayas.clear();
        for k in dentro.into_iter().rev() {
            s.zonas.remove(k);
            if k < i {
                i -= 1;
            }
        }
        i += 1;
    }
}

/// **Los iconos de un boton** (18-oct: el mapa blanco sobre azul salia como
/// cuatro rayas negras): sobre un fondo de color, lo que no lleva letras y
/// esta junto (rayas, dudosas, trozos de dibujo) es UN simbolo: una imagen
/// recortada con sus colores.
fn iconos(s: &mut Seccionado) {
    let hm0 = s.alto_letra.max(4.0);
    let sin_letras = |z: &Zona| z.renglones.is_empty() && matches!(z.tipo, Tipo::Linea | Tipo::Dudosa | Tipo::Dibujo | Tipo::Foto) && z.caja.w <= hm0 * 8.0 && z.caja.h <= hm0 * 8.0;
    let hm = s.alto_letra.max(4.0);
    let mut i = 0;
    while i < s.zonas.len() {
        if !sin_letras(&s.zonas[i]) {
            i += 1;
            continue;
        }
        let mut caja = s.zonas[i].caja;
        let mut j = i + 1;
        while j < s.zonas.len() {
            if sin_letras(&s.zonas[j]) && s.zonas[j].caja.crecer(hm * 0.6).comun(&caja.crecer(hm * 0.6)) > 0.0 {
                caja = caja.union(&s.zonas[j].caja);
                s.zonas.remove(j);
                j = i + 1;
            } else {
                j += 1;
            }
        }
        let z = &mut s.zonas[i];
        z.caja = caja;
        z.tipo = Tipo::Foto;
        z.rayas.clear();
        z.dentro.clear();
        i += 1;
    }
}

/// **La pestana de un titulo** (18-oct, «¡Recuerde que...!», «Desafío» de
/// Polinomios: letras blancas sobre una pestana gris): un renglon cuyo fondo
/// de al lado es otro color que el de la region lleva debajo un recuadro de
/// ese color (la capa de abajo); su letra ya contrasta (`contrastar`).
fn etiquetas_de_color(s: &mut Seccionado, rgba: &[u8], fondo: [u8; 3]) {
    let (w, h) = (s.ancho, s.alto);
    let f = [fondo[0] as f64, fondo[1] as f64, fondo[2] as f64];
    let a = s.alto_letra.max(4.0);
    let mut nuevas: Vec<(usize, Zona)> = Vec::new();
    let mut tinta: Vec<(usize, [u8; 3])> = Vec::new();
    for (i, z) in s.zonas.iter().enumerate() {
        if z.renglones.is_empty() || z.tipo == Tipo::Recuadro || z.caja.h > 4.0 * a {
            continue;
        }
        // Lo que hay alrededor de las letras (sin ellas): su fondo.
        let marco = z.caja.crecer(2.0);
        let Some(local) = color_mediano(rgba, w, h, &marco, 2, |_| true) else { continue };
        let l = [local[0] as f64, local[1] as f64, local[2] as f64];
        if dist(l, f) < 70.0 {
            continue;
        }
        // Que sea una mancha llena de verdad: casi todo el marco de ese color.
        let mut iguales = 0usize;
        let mut total = 0usize;
        let c = marco.dentro(w as f64, h as f64);
        for y in (c.y as u32..c.y1() as u32).step_by(2) {
            for x in (c.x as u32..c.x1() as u32).step_by(2) {
                let k = ((y * w + x) * 4) as usize;
                total += 1;
                if dist([rgba[k] as f64, rgba[k + 1] as f64, rgba[k + 2] as f64], l) < 40.0 {
                    iguales += 1;
                }
            }
        }
        if total == 0 || (iguales as f64) < total as f64 * 0.55 {
            continue;
        }
        // La letra: lo que se aparta de la pestana (blanco sobre gris).
        if let Some(c) = color_mediano(rgba, w, h, &z.caja, 1, |p| dist([p[0] as f64, p[1] as f64, p[2] as f64], l) >= 90.0) {
            tinta.push((i, c));
        }
        let mut r = z.clone();
        r.tipo = Tipo::Recuadro;
        r.renglones.clear();
        r.rayas.clear();
        r.dentro.clear();
        r.caja = marco;
        r.panel = Some(Estilo { relleno: Some(local), borde: None, grosor: 0.0, redondo: true });
        nuevas.push((i, r));
    }
    for (i, c) in tinta {
        for rg in &mut s.zonas[i].renglones {
            rg.color = Some(c);
            for p in &mut rg.palabras {
                p.color = Some(c);
            }
        }
    }
    // Cada recuadro antes de su texto (la capa de abajo).
    for (i, r) in nuevas.into_iter().rev() {
        s.zonas.insert(i, r);
    }
    for (i, z) in s.zonas.iter_mut().enumerate() {
        z.id = i;
        z.orden = i;
    }
}

/// Una copia del recorte en la que el `fondo` es blanco y la tinta, oscura
/// en proporcion a lo que se aparta de el (mas clara o mas oscura).
pub fn entintada(rgba: &[u8], fondo: [u8; 3]) -> Vec<u8> {
    let f = [fondo[0] as f64, fondo[1] as f64, fondo[2] as f64];
    let mut v = rgba.to_vec();
    for px in v.chunks_exact_mut(4) {
        let d = dist([px[0] as f64, px[1] as f64, px[2] as f64], f);
        let g = (255.0 - d * 1.6).clamp(0.0, 255.0) as u8;
        px[0] = g;
        px[1] = g;
        px[2] = g;
    }
    v
}

/// Los colores de cada renglon y palabra, medidos en la imagen de verdad:
/// la tinta que se aparta del fondo (despues de seccionar en la entintada).
fn recolorear(s: &mut Seccionado, rgba: &[u8], fondo: [u8; 3]) {
    let (w, h) = (s.ancho, s.alto);
    let f = [fondo[0] as f64, fondo[1] as f64, fondo[2] as f64];
    let lejos = |p: [u8; 3]| dist([p[0] as f64, p[1] as f64, p[2] as f64], f) >= 90.0;
    for z in &mut s.zonas {
        for r in &mut z.renglones {
            if let Some(c) = color_mediano(rgba, w, h, &r.caja, 1, lejos) {
                r.color = Some(c);
            }
            for p in &mut r.palabras {
                if let Some(c) = color_mediano(rgba, w, h, &p.caja, 1, lejos) {
                    p.color = Some(c);
                }
            }
        }
    }
}

/// **La negrita, por el grosor del trazo** (18-oct, «las palabras en negrita
/// ponlas en negrita»): en cada palabra, lo que mide de ancho cada trozo de
/// tinta en cada fila (la tinta es lo que se aparta de su fondo, sea claro u
/// oscuro), partido por su alto. La negrita tiene el trazo un tercio mas
/// grueso que la mediana de la pagina; asi no engañan las mayusculas (mas
/// tinta por area) ni un boton con una sola palabra.
fn negritas(rgba: &[u8], w: u32, h: u32, palabras: &mut [PalabraHoja]) {
    let grosor = |c: &Caja| -> Option<f64> {
        let c = c.dentro(w as f64, h as f64);
        let (x0, y0, x1, y1) = (c.x as u32, c.y as u32, c.x1() as u32, c.y1() as u32);
        if x1 <= x0 + 3 || y1 <= y0 + 4 {
            return None;
        }
        let px = |x: u32, y: u32| {
            let i = ((y * w + x) * 4) as usize;
            [rgba[i] as f64, rgba[i + 1] as f64, rgba[i + 2] as f64]
        };
        // El fondo: el color mas comun del borde de la caja.
        let mut borde: [Vec<u8>; 3] = Default::default();
        for x in x0..x1 {
            for y in [y0, y1 - 1] {
                let p = px(x, y);
                for k in 0..3 {
                    borde[k].push(p[k] as u8);
                }
            }
        }
        let mut fondo = [0.0; 3];
        for k in 0..3 {
            borde[k].sort_unstable();
            fondo[k] = borde[k][borde[k].len() / 2] as f64;
        }
        let lejos: Vec<f64> = (y0..y1).flat_map(|y| (x0..x1).map(move |x| (x, y))).map(|(x, y)| dist(px(x, y), fondo)).collect();
        let tope = lejos.iter().cloned().fold(0.0, f64::max);
        if tope < 60.0 {
            return None;
        }
        // Cada trozo de tinta de cada fila (cortado a media tinta) y su
        // anchura con fraccion: cada pixel cuenta lo que tiene de tinta.
        let corte = tope * 0.5;
        let ancho = (x1 - x0) as usize;
        let (mut tinta, mut trozos) = (0.0f64, 0usize);
        for fila in lejos.chunks(ancho) {
            let mut dentro = false;
            for &d in fila {
                tinta += (d / (tope * 0.85)).clamp(0.0, 1.0) * f64::from(u8::from(d >= tope * 0.2));
                if d >= corte && !dentro {
                    trozos += 1;
                }
                dentro = d >= corte;
            }
        }
        if trozos < 6 {
            return None;
        }
        Some(tinta / trozos as f64 / c.h.max(1.0))
    };
    let g: Vec<Option<f64>> = palabras.iter().map(|p| if p.texto.chars().filter(|c| c.is_alphanumeric()).count() >= 2 { grosor(&p.caja) } else { None }).collect();
    let validos: Vec<f64> = g.iter().flatten().copied().collect();
    if validos.len() < 8 {
        return;
    }
    let Some(media) = mediana(validos) else { return };
    if std::env::var_os("PIXPIN_NEGRITA_DEPURAR").is_some() {
        for (p, g) in palabras.iter().zip(&g) {
            eprintln!("negrita? {:<24} {:?} media {media:.3}", p.texto, g.map(|g| (g / media * 100.0).round()));
        }
    }
    // Por renglones (misma altura y seguidas): la media de sus palabras,
    // pesada por letras. Una palabra sola mide con mucho ruido.
    let mut orden: Vec<usize> = (0..palabras.len()).collect();
    orden.sort_by(|&i, &j| palabras[i].caja.cy().total_cmp(&palabras[j].caja.cy()).then(palabras[i].caja.x.total_cmp(&palabras[j].caja.x)));
    let mut renglones: Vec<Vec<usize>> = Vec::new();
    for i in orden {
        let p = &palabras[i].caja;
        let sitio = renglones.iter_mut().find(|r| {
            let u = &palabras[*r.last().unwrap()].caja;
            (u.cy() - p.cy()).abs() < u.h.max(p.h) * 0.45 && p.x >= u.x && p.x - u.x1() < 1.6 * u.h.max(p.h)
        });
        match sitio {
            Some(r) => r.push(i),
            None => renglones.push(vec![i]),
        }
    }
    for r in renglones {
        let (mut suma, mut letras) = (0.0, 0.0);
        for &i in &r {
            if let Some(gi) = g[i] {
                let n = palabras[i].texto.chars().count() as f64;
                suma += gi * n;
                letras += n;
            }
        }
        if letras >= 3.0 && suma / letras > media * 1.15 {
            for &i in &r {
                palabras[i].negrita = true;
            }
        }
    }
}

/// Mueve lo seccionado en un recorte a pixeles de la pagina.
fn mover(z: &mut Zona, dx: f64, dy: f64) {
    let m = |c: &mut Caja| {
        c.x += dx;
        c.y += dy;
    };
    m(&mut z.caja);
    for r in &mut z.renglones {
        m(&mut r.caja);
        for p in &mut r.palabras {
            m(&mut p.caja);
        }
    }
    for r in &mut z.rayas {
        r[0] += dx;
        r[1] += dy;
        r[2] += dx;
        r[3] += dy;
    }
    for c in &mut z.dentro {
        m(c);
    }
}

/// **Secciona la hoja por recuadros.** Sin recuadros, es el seccionado de
/// siempre (con los adornos quitados). `unir`: cada dibujo, una zona (una
/// lamina, `seccion::unir_dibujos`).
pub fn seccionar(rgba: &[u8], ancho: u32, alto: u32, palabras: Vec<PalabraHoja>, unir: bool) -> Seccionado {
    seccionar_limpia(rgba, ancho, alto, palabras, unir).0
}

/// Lo mismo, y la hoja con lo palido de los recuadros borrado ([`limpiar`]):
/// quien monte las zonas tiene que usar ESA (las figuras se vectorizan de
/// ella). `None`: la de entrada vale.
pub fn seccionar_limpia(rgba: &[u8], ancho: u32, alto: u32, mut palabras: Vec<PalabraHoja>, unir: bool) -> (Seccionado, Option<Vec<u8>>) {
    negritas(rgba, ancho, alto, &mut palabras);
    let a = imagen::analizar(rgba, ancho, alto);
    let hm = mediana(palabras.iter().map(|p| p.caja.h).collect()).unwrap_or(alto as f64 * 0.012).max(4.0);
    let mut paneles = buscar(rgba, ancho, alto, &a, &palabras, hm);
    // Uno sin texto dentro es una figura (el rectangulo de «Rectangulo»): en punta.
    for p in &mut paneles {
        if !palabras.iter().any(|w| p.caja.contiene(w.caja.cx(), w.caja.cy())) {
            p.estilo.redondo = false;
        }
    }
    if paneles.is_empty() {
        let mut s = seccion::seccionar(rgba, ancho, alto, palabras);
        if unir {
            seccion::unir_dibujos(&mut s, rgba);
        }
        quitar_adornos(&mut s, rgba);
        cajitas(&mut s);
        invertidos(&mut s, rgba, [a.fondo; 3]);
        if unir {
            juntar_formulas_al_renglon(&mut s);
            juntar_texto_y_formulas(&mut s);
        }
        return (s, None);
    }
    let limpia = limpiar(rgba, ancho, alto, &paneles);
    let rgba: &[u8] = limpia.as_deref().unwrap_or(rgba);
    // Quien contiene a quien (el mas pequeno que lo contiene).
    let padre: Vec<Option<usize>> = (0..paneles.len())
        .map(|i| {
            (0..paneles.len())
                .filter(|&j| j != i && paneles[j].caja.area() > paneles[i].caja.area() && paneles[i].caja.dentro_de(&paneles[j].caja) >= 0.95)
                .min_by(|&x, &y| paneles[x].caja.area().total_cmp(&paneles[y].caja.area()))
        })
        .collect();
    let papel = [a.fondo; 3];
    let cx = Region { rgba, ancho, alto, palabras: &palabras, paneles: &paneles, padre: &padre, papel, unir };
    let mut zonas = cx.zonas(None);
    for (i, z) in zonas.iter_mut().enumerate() {
        z.id = i;
        z.orden = i;
    }
    let columnas = 1;
    (Seccionado { ancho, alto, zonas, columnas, alto_letra: hm, paneles: paneles.iter().map(|p| p.caja).collect() }, limpia)
}

struct Region<'a> {
    rgba: &'a [u8],
    ancho: u32,
    alto: u32,
    palabras: &'a [PalabraHoja],
    paneles: &'a [Panel],
    padre: &'a [Option<usize>],
    papel: [u8; 3],
    unir: bool,
}

impl Region<'_> {
    /// Las zonas de la hoja (`None`) o de un recuadro, con las de sus
    /// recuadros de dentro, en orden de arriba abajo.
    fn zonas(&self, de: Option<usize>) -> Vec<Zona> {
        let area = match de {
            Some(i) => self.paneles[i].interior(),
            None => Caja::nueva(0.0, 0.0, self.ancho as f64, self.alto as f64),
        };
        let fondo = de.and_then(|i| self.paneles[i].estilo.relleno).unwrap_or(self.papel);
        let hijos: Vec<usize> = (0..self.paneles.len()).filter(|&j| self.padre[j] == de).collect();
        let (mut sub, sw, sh) = imagen::recortar(self.rgba, self.ancho, self.alto, &area);
        // Los recuadros de dentro, tapados con el fondo: se seccionan aparte.
        for &j in &hijos {
            let c = self.paneles[j].caja.crecer(2.0);
            let (x0, y0) = ((c.x - area.x).max(0.0) as u32, (c.y - area.y).max(0.0) as u32);
            let (x1, y1) = (((c.x1() - area.x).max(0.0) as u32).min(sw), ((c.y1() - area.y).max(0.0) as u32).min(sh));
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = ((y * sw + x) * 4) as usize;
                    sub[i..i + 3].copy_from_slice(&fondo);
                    sub[i + 3] = 255;
                }
            }
        }
        let en_hijo = |p: &PalabraHoja| hijos.iter().any(|&j| self.paneles[j].caja.contiene(p.caja.cx(), p.caja.cy()));
        let suyas: Vec<PalabraHoja> = self
            .palabras
            .iter()
            .filter(|p| area.contiene(p.caja.cx(), p.caja.cy()) && !en_hijo(p))
            .map(|p| {
                let mut q = p.clone();
                q.caja.x -= area.x;
                q.caja.y -= area.y;
                q
            })
            .collect();
        // **Sobre un fondo que no es papel blanco** (18-oct: un boton azul con
        // su icono blanco, una tabla de tema oscuro), lo que es tinta es lo
        // que se aparta del fondo, sea mas claro o mas oscuro: se secciona
        // una copia «entintada» (fondo blanco, tinta oscura) y los colores
        // se miden luego en la de verdad.
        let entintar = dist([fondo[0] as f64, fondo[1] as f64, fondo[2] as f64], [255.0; 3]) > 90.0;
        let tinta = entintar.then(|| entintada(&sub, fondo));
        let ver: &[u8] = tinta.as_deref().unwrap_or(&sub);
        let mut s = seccion::seccionar(ver, sw, sh, suyas);
        if self.unir {
            seccion::unir_dibujos(&mut s, ver);
        }
        quitar_adornos(&mut s, ver);
        cajitas(&mut s);
        if entintar {
            recolorear(&mut s, &sub, fondo);
            iconos(&mut s);
        }
        invertidos(&mut s, &sub, fondo);
        if self.unir {
            juntar_formulas_al_renglon(&mut s);
            juntar_texto_y_formulas(&mut s);
        }
        contrastar(&mut s, &sub, fondo);
        etiquetas_de_color(&mut s, &sub, fondo);
        // Grupos: cada zona suelta (en el orden de lectura del seccionado), y
        // cada recuadro de dentro con lo suyo, antes de lo primero de su
        // columna que queda debajo de el.
        let mut grupos: Vec<Vec<Zona>> = Vec::new();
        for mut z in s.zonas {
            mover(&mut z, area.x, area.y);
            grupos.push(vec![z]);
        }
        let mut hijos = hijos;
        hijos.sort_by(|&x, &y| self.paneles[x].caja.y.total_cmp(&self.paneles[y].caja.y));
        for &j in &hijos {
            let p = &self.paneles[j];
            let mut v = vec![Zona {
                id: 0,
                tipo: Tipo::Recuadro,
                caja: p.caja,
                orden: 0,
                columna: 0,
                confianza: 0.9,
                renglones: Vec::new(),
                alineacion: Alineacion::Izquierda,
                tabla: None,
                rayas: Vec::new(),
                dentro: Vec::new(),
                panel: Some(p.estilo),
            }];
            v.extend(self.zonas(Some(j)));
            let debajo = |g: &Vec<Zona>| g[0].tipo != Tipo::Recuadro && g[0].caja.y >= p.caja.y;
            let donde = grupos
                .iter()
                .position(|g| debajo(g) && g[0].caja.solape_x(&p.caja) > 0.0)
                .or_else(|| grupos.iter().position(debajo))
                .unwrap_or(grupos.len());
            grupos.insert(donde, v);
        }
        grupos.into_iter().flatten().collect()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Depurar con una hoja de verdad: `PIXPIN_PANELES_HOJA=<png>`.
    #[test]
    #[ignore = "con una hoja del disco"]
    fn depurar_paneles() {
        let Some(ruta) = std::env::var_os("PIXPIN_PANELES_HOJA") else { return };
        let bytes = std::fs::read(ruta).unwrap();
        let img = image::load_from_memory(&bytes).unwrap().to_rgba8();
        let (w, h) = (img.width(), img.height());
        let rgba = img.into_raw();
        let a = imagen::analizar(&rgba, w, h);
        println!("fondo {} umbral {} f {}", a.fondo, a.umbral, a.f);
        let mut suave = a.clone();
        suave.umbral = a.fondo.saturating_sub(25);
        suave.tinta = suave.gris.iter().map(|&g| g < suave.umbral).collect();
        for r in imagen::rayas(&suave, 0.04).into_iter().filter(|r| r.largo() > 60.0) {
            println!("raya {} ({:.0},{:.0})-({:.0},{:.0}) g {:.1}", if r.vertical { "V" } else { "H" }, r.x0, r.y0, r.x1, r.y1, r.grosor);
        }
        for p in buscar(&rgba, w, h, &a, &[], 12.0) {
            println!("panel {:?}", p);
        }
    }
}
