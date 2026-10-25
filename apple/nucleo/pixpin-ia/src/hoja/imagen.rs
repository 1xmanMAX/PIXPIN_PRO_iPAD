//! **Lo que se ve en la imagen de la hoja, sin modelo**: tinta, rayas largas
//! y finas, fotos (zonas con medios tonos) y manchas de tinta que no son
//! texto (dibujos, graficas, trozos de formula que el OCR no leyo).
//!
//! Todo sobre la hoja **reducida** a ~1000 px de ancho (promediando, que
//! tambien limpia el grano de un escaneo): una hoja de 1700x2200 se analiza
//! en unas decenas de ms. Lo que sale va en pixeles de la imagen original.

use super::Caja;

/// El ancho al que se reduce la hoja para mirarla.
pub const ANCHO_ANALISIS: usize = 1000;

/// La hoja reducida y en grises, con su tinta.
#[derive(Debug, Clone)]
pub struct Analisis {
    /// Pixeles reducidos = pixeles originales * `f`.
    pub f: f64,
    pub w: usize,
    pub h: usize,
    pub gris: Vec<u8>,
    /// El gris del papel.
    pub fondo: u8,
    /// Mas oscuro que esto es tinta.
    pub umbral: u8,
    pub tinta: Vec<bool>,
}

impl Analisis {
    /// Una caja de la pagina, en pixeles reducidos `(x0, y0, x1, y1)`.
    pub fn a_reducida(&self, c: &Caja) -> (usize, usize, usize, usize) {
        let x0 = ((c.x * self.f).floor().max(0.0) as usize).min(self.w);
        let y0 = ((c.y * self.f).floor().max(0.0) as usize).min(self.h);
        let x1 = ((c.x1() * self.f).ceil().max(0.0) as usize).min(self.w);
        let y1 = ((c.y1() * self.f).ceil().max(0.0) as usize).min(self.h);
        (x0, y0, x1.max(x0), y1.max(y0))
    }

    /// Una caja reducida, en pixeles de la pagina.
    pub fn a_pagina(&self, x0: usize, y0: usize, x1: usize, y1: usize) -> Caja {
        Caja::de_esquinas(x0 as f64 / self.f, y0 as f64 / self.f, x1 as f64 / self.f, y1 as f64 / self.f)
    }

    /// Que parte de la caja es tinta (0-1).
    pub fn densidad(&self, c: &Caja) -> f64 {
        let (x0, y0, x1, y1) = self.a_reducida(c);
        let n = (x1 - x0) * (y1 - y0);
        if n == 0 {
            return 0.0;
        }
        let mut t = 0;
        for y in y0..y1 {
            t += self.tinta[y * self.w + x0..y * self.w + x1].iter().filter(|&&b| b).count();
        }
        t as f64 / n as f64
    }

    /// Lo que hay en la caja: parte de tinta, parte de medios tonos (ni papel
    /// ni tinta: una foto) y la desviacion del gris.
    pub fn estadisticas(&self, c: &Caja) -> Estadisticas {
        let (x0, y0, x1, y1) = self.a_reducida(c);
        let n = ((x1 - x0) * (y1 - y0)).max(1) as f64;
        let (mut t, mut m, mut s, mut s2) = (0usize, 0usize, 0f64, 0f64);
        let medio_hasta = self.fondo.saturating_sub(28);
        for y in y0..y1 {
            for x in x0..x1 {
                let g = self.gris[y * self.w + x];
                if g < self.umbral {
                    t += 1;
                } else if g < medio_hasta {
                    m += 1;
                }
                s += g as f64;
                s2 += (g as f64) * (g as f64);
            }
        }
        let media = s / n;
        Estadisticas { tinta: t as f64 / n, medios: m as f64 / n, desviacion: (s2 / n - media * media).max(0.0).sqrt() }
    }
}

/// Lo que hay dentro de una caja.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Estadisticas {
    pub tinta: f64,
    pub medios: f64,
    pub desviacion: f64,
}

impl Estadisticas {
    /// Si parece una foto: muchos medios tonos, o mucho no-papel variado.
    pub fn parece_foto(&self) -> bool {
        // Un titulo grande en negrita tambien llena celdas de tinta, pero
        // casi sin medios tonos: sin ellos no es foto.
        self.medios > 0.30 || (self.medios > 0.15 && self.medios + self.tinta > 0.55 && self.desviacion > 22.0)
    }
}

/// **Reduce y pasa a grises.** RGBA de `ancho * alto * 4` bytes; lo
/// transparente es papel.
pub fn analizar(rgba: &[u8], ancho: u32, alto: u32) -> Analisis {
    let (aw, ah) = (ancho as usize, alto as usize);
    if aw == 0 || ah == 0 || rgba.len() < aw * ah * 4 {
        return Analisis { f: 1.0, w: 0, h: 0, gris: Vec::new(), fondo: 255, umbral: 128, tinta: Vec::new() };
    }
    let k = aw.div_ceil(ANCHO_ANALISIS).max(1);
    let (w, h) = (aw.div_ceil(k), ah.div_ceil(k));
    let mut gris = vec![255u8; w * h];
    for ry in 0..h {
        for rx in 0..w {
            let (mut s, mut n) = (0u32, 0u32);
            for y in ry * k..((ry + 1) * k).min(ah) {
                for x in rx * k..((rx + 1) * k).min(aw) {
                    let i = (y * aw + x) * 4;
                    let a = rgba[i + 3] as u32;
                    let g = (rgba[i] as u32 * 299 + rgba[i + 1] as u32 * 587 + rgba[i + 2] as u32 * 114) / 1000;
                    // Lo transparente cuenta como papel blanco.
                    s += (g * a + 255 * (255 - a)) / 255;
                    n += 1;
                }
            }
            gris[ry * w + rx] = (s / n.max(1)) as u8;
        }
    }
    // El papel: el gris mas comun de la mitad clara.
    let mut hist = [0usize; 256];
    for &g in &gris {
        hist[g as usize] += 1;
    }
    let fondo = (100..256).max_by_key(|&g| (hist[g], g)).unwrap_or(255) as u8;
    let umbral = ((fondo as f64) * 0.62).round().clamp(60.0, 200.0) as u8;
    let tinta = gris.iter().map(|&g| g < umbral).collect();
    Analisis { f: 1.0 / k as f64, w, h, gris, fondo, umbral, tinta }
}

/// Una raya larga y fina, en pixeles de la pagina.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Raya {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
    pub grosor: f64,
    pub vertical: bool,
}

impl Raya {
    pub fn largo(&self) -> f64 {
        if self.vertical { self.y1 - self.y0 } else { self.x1 - self.x0 }
    }
    pub fn caja(&self) -> Caja {
        let g = self.grosor / 2.0;
        if self.vertical {
            Caja::de_esquinas(self.x0 - g, self.y0, self.x0 + g, self.y1)
        } else {
            Caja::de_esquinas(self.x0, self.y0 - g, self.x1, self.y0 + g)
        }
    }
}

/// Un tramo de una fila (o columna) de tinta seguida.
#[derive(Debug, Clone, Copy)]
struct Tramo {
    linea: usize,
    a: usize,
    b: usize,
}

/// **Las rayas** horizontales y verticales de al menos `largo_min` (en
/// fraccion del lado de la hoja) y como mucho [`GROSOR_MAX`] de grueso. Se
/// toleran cortes de 2 pixeles (un escaneo).
pub fn rayas(a: &Analisis, largo_min: f64) -> Vec<Raya> {
    let mut v = rayas_en(a, false, largo_min);
    v.extend(rayas_en(a, true, largo_min));
    v
}

/// Lo mas grueso que es una raya, en pixeles reducidos.
pub const GROSOR_MAX: usize = 5;

fn rayas_en(a: &Analisis, vertical: bool, largo_min: f64) -> Vec<Raya> {
    let (n_lineas, largo) = if vertical { (a.w, a.h) } else { (a.h, a.w) };
    if n_lineas == 0 {
        return Vec::new();
    }
    let minimo = ((largo as f64 * largo_min) as usize).max(24);
    let en = |l: usize, i: usize| if vertical { a.tinta[i * a.w + l] } else { a.tinta[l * a.w + i] };
    let mut tramos: Vec<Tramo> = Vec::new();
    for l in 0..n_lineas {
        let mut i = 0;
        while i < largo {
            if !en(l, i) {
                i += 1;
                continue;
            }
            let ini = i;
            let mut fin = i;
            let mut hueco = 0;
            while i < largo {
                if en(l, i) {
                    fin = i;
                    hueco = 0;
                } else {
                    hueco += 1;
                    if hueco > 2 {
                        break;
                    }
                }
                i += 1;
            }
            if fin + 1 - ini >= minimo {
                tramos.push(Tramo { linea: l, a: ini, b: fin + 1 });
            }
        }
    }
    // Juntar tramos de lineas vecinas que se solapan: una raya de 3 px son
    // tres tramos.
    let mut grupos: Vec<(usize, usize, usize, usize)> = Vec::new(); // (l0, l1, a, b)
    let mut abiertos: Vec<usize> = Vec::new();
    for t in tramos {
        let mut puesto = false;
        for &g in &abiertos {
            let (l0, l1, ga, gb) = grupos[g];
            let _ = l0;
            if t.linea <= l1 + 1 && t.a < gb && t.b > ga {
                let solape = t.b.min(gb) - t.a.max(ga);
                if solape * 2 >= (t.b - t.a).min(gb - ga) {
                    grupos[g] = (grupos[g].0, t.linea, ga.min(t.a), gb.max(t.b));
                    puesto = true;
                    break;
                }
            }
        }
        if !puesto {
            grupos.push((t.linea, t.linea, t.a, t.b));
            abiertos.push(grupos.len() - 1);
        }
        abiertos.retain(|&g| grupos[g].1 + 1 >= t.linea);
    }
    grupos
        .into_iter()
        .filter(|&(l0, l1, _, _)| l1 + 1 - l0 <= GROSOR_MAX)
        .map(|(l0, l1, ia, ib)| {
            let c = (l0 + l1 + 1) as f64 / 2.0 / a.f;
            let g = ((l1 + 1 - l0) as f64 / a.f).max(1.0);
            let (p0, p1) = (ia as f64 / a.f, ib as f64 / a.f);
            if vertical {
                Raya { x0: c, y0: p0, x1: c, y1: p1, grosor: g, vertical: true }
            } else {
                Raya { x0: p0, y0: c, x1: p1, y1: c, grosor: g, vertical: false }
            }
        })
        .collect()
}

/// **Las fotos**: celdas con medios tonos juntas en una zona grande. Cajas
/// en pixeles de la pagina. `minimo`: el lado minimo de una foto (pagina).
pub fn fotos(a: &Analisis, minimo: f64) -> Vec<Caja> {
    let cs = 10usize;
    let (cw, ch) = (a.w.div_ceil(cs), a.h.div_ceil(cs));
    let mut es = vec![false; cw * ch];
    for cy in 0..ch {
        for cx in 0..cw {
            let c = a.a_pagina(cx * cs, cy * cs, ((cx + 1) * cs).min(a.w), ((cy + 1) * cs).min(a.h));
            es[cy * cw + cx] = a.estadisticas(&c).parece_foto();
        }
    }
    // Cerrar huecos de una celda (una zona clara dentro de la foto).
    let cerrado = dilatar(&es, cw, ch, 1);
    let mut v = Vec::new();
    for (x0, y0, x1, y1, n) in componentes(&cerrado, cw, ch) {
        let caja = a.a_pagina(x0 * cs, y0 * cs, ((x1 + 1) * cs).min(a.w), ((y1 + 1) * cs).min(a.h));
        // Que de verdad sea foto por dentro (no un borde de texto).
        let de_foto = (y0..=y1).flat_map(|y| (x0..=x1).map(move |x| (x, y))).filter(|&(x, y)| es[y * cw + x]).count();
        let area = (x1 + 1 - x0) * (y1 + 1 - y0);
        if caja.w >= minimo && caja.h >= minimo && n >= 6 && de_foto * 10 >= area * 4 {
            v.push(caja);
        }
    }
    v
}

/// Una mancha de tinta que no es texto ni raya.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mancha {
    pub caja: Caja,
    pub est: Estadisticas,
}

/// **Las manchas**: la tinta que queda al quitar las cajas de `quitar`
/// (palabras, fotos, tablas) y las rayas, juntada en celdas de `celda`
/// pixeles de la pagina (lo que separa menos que una celda es la misma
/// mancha).
pub fn manchas(a: &Analisis, quitar: &[Caja], rayas: &[Raya], celda: f64) -> Vec<Mancha> {
    if a.w == 0 {
        return Vec::new();
    }
    let mut libre = a.tinta.clone();
    let borrar = |libre: &mut Vec<bool>, c: &Caja| {
        let (x0, y0, x1, y1) = a.a_reducida(c);
        for y in y0..y1 {
            for v in &mut libre[y * a.w + x0..y * a.w + x1] {
                *v = false;
            }
        }
    };
    for c in quitar {
        borrar(&mut libre, c);
    }
    for r in rayas {
        borrar(&mut libre, &r.caja().crecer(2.0 / a.f));
    }
    let cs = ((celda * a.f).round() as usize).clamp(3, 40);
    let (cw, ch) = (a.w.div_ceil(cs), a.h.div_ceil(cs));
    let mut ocupada = vec![false; cw * ch];
    for y in 0..a.h {
        for x in 0..a.w {
            if libre[y * a.w + x] {
                ocupada[(y / cs) * cw + x / cs] = true;
            }
        }
    }
    // Una celda con un solo pixel suelto es grano.
    let mut cuenta = vec![0u16; cw * ch];
    for y in 0..a.h {
        for x in 0..a.w {
            if libre[y * a.w + x] {
                cuenta[(y / cs) * cw + x / cs] = cuenta[(y / cs) * cw + x / cs].saturating_add(1);
            }
        }
    }
    for (o, &n) in ocupada.iter_mut().zip(&cuenta) {
        *o = *o && n >= 2;
    }
    let unidas = dilatar(&ocupada, cw, ch, 1);
    componentes(&unidas, cw, ch)
        .into_iter()
        .filter_map(|(x0, y0, x1, y1, _)| {
            // Ajustar a la tinta de verdad dentro de las celdas.
            let (px0, py0, px1, py1) = (x0 * cs, y0 * cs, ((x1 + 1) * cs).min(a.w), ((y1 + 1) * cs).min(a.h));
            let (mut mx0, mut my0, mut mx1, mut my1) = (usize::MAX, usize::MAX, 0, 0);
            for y in py0..py1 {
                for x in px0..px1 {
                    if libre[y * a.w + x] {
                        mx0 = mx0.min(x);
                        my0 = my0.min(y);
                        mx1 = mx1.max(x + 1);
                        my1 = my1.max(y + 1);
                    }
                }
            }
            if mx0 == usize::MAX {
                return None;
            }
            let caja = a.a_pagina(mx0, my0, mx1, my1);
            Some(Mancha { caja, est: a.estadisticas(&caja) })
        })
        .collect()
}

/// Agranda lo marcado `r` celdas (8 vecinos).
fn dilatar(m: &[bool], w: usize, h: usize, r: usize) -> Vec<bool> {
    let mut o = vec![false; w * h];
    for y in 0..h {
        for x in 0..w {
            if !m[y * w + x] {
                continue;
            }
            for yy in y.saturating_sub(r)..(y + r + 1).min(h) {
                for xx in x.saturating_sub(r)..(x + r + 1).min(w) {
                    o[yy * w + xx] = true;
                }
            }
        }
    }
    o
}

/// Las componentes conexas (8 vecinos): `(x0, y0, x1, y1, celdas)`.
fn componentes(m: &[bool], w: usize, h: usize) -> Vec<(usize, usize, usize, usize, usize)> {
    let mut visto = vec![false; w * h];
    let mut v = Vec::new();
    let mut pila = Vec::new();
    for i in 0..w * h {
        if !m[i] || visto[i] {
            continue;
        }
        visto[i] = true;
        pila.push(i);
        let (mut x0, mut y0, mut x1, mut y1, mut n) = (usize::MAX, usize::MAX, 0, 0, 0);
        while let Some(j) = pila.pop() {
            let (x, y) = (j % w, j / w);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
            n += 1;
            for yy in y.saturating_sub(1)..(y + 2).min(h) {
                for xx in x.saturating_sub(1)..(x + 2).min(w) {
                    let k = yy * w + xx;
                    if m[k] && !visto[k] {
                        visto[k] = true;
                        pila.push(k);
                    }
                }
            }
        }
        v.push((x0, y0, x1, y1, n));
    }
    v
}

/// **Un trozo de la imagen original** (RGBA), para pegarlo o mandarlo al
/// modelo. La caja se recorta a la imagen.
pub fn recortar(rgba: &[u8], ancho: u32, alto: u32, c: &Caja) -> (Vec<u8>, u32, u32) {
    let c = c.dentro(ancho as f64, alto as f64);
    let (x0, y0) = (c.x.floor() as usize, c.y.floor() as usize);
    let (x1, y1) = ((c.x1().ceil() as usize).min(ancho as usize), (c.y1().ceil() as usize).min(alto as usize));
    let (w, h) = (x1.saturating_sub(x0), y1.saturating_sub(y0));
    let mut out = Vec::with_capacity(w * h * 4);
    for y in y0..y1 {
        let i = (y * ancho as usize + x0) * 4;
        out.extend_from_slice(&rgba[i..i + w * 4]);
    }
    (out, w as u32, h as u32)
}

/// **El color de la tinta** dentro de una caja (una palabra): la media de
/// sus pixeles oscuros, sin el papel ni el borde suavizado. `None` si es
/// negro o gris oscuro (lo normal) o si no hay tinta.
pub fn color_de_tinta(rgba: &[u8], ancho: u32, alto: u32, c: &Caja, umbral: u8) -> Option<[u8; 3]> {
    let c = c.dentro(ancho as f64, alto as f64);
    let (x0, y0, x1, y1) = (c.x as usize, c.y as usize, (c.x1() as usize).min(ancho as usize), (c.y1() as usize).min(alto as usize));
    let (mut s, mut n) = ([0u64; 3], 0u64);
    // El nucleo del trazo: bastante mas oscuro que el umbral (el borde es
    // mezcla con el papel).
    let limite = (umbral as u32 * 85 / 100).max(40);
    for y in y0..y1 {
        for x in x0..x1 {
            let i = (y * ancho as usize + x) * 4;
            let (r, g, b) = (rgba[i] as u32, rgba[i + 1] as u32, rgba[i + 2] as u32);
            let lum = (r * 299 + g * 587 + b * 114) / 1000;
            if lum < limite {
                s[0] += r as u64;
                s[1] += g as u64;
                s[2] += b as u64;
                n += 1;
            }
        }
    }
    if n < 4 {
        return None;
    }
    let m = [(s[0] / n) as u8, (s[1] / n) as u8, (s[2] / n) as u8];
    let (max, min) = (*m.iter().max().unwrap_or(&0), *m.iter().min().unwrap_or(&0));
    // Sin croma: negro o gris (el de siempre).
    if max.saturating_sub(min) < 45 {
        return None;
    }
    Some(m)
}

/// El color como lo escribe Excalidraw.
pub fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

/// **Los fondos de color**: zonas grandes algo mas oscuras que el papel (un
/// logo en su recuadro gris, un recuadro sombreado). Quien llama decide: con
/// casi nada de texto dentro es una imagen; con texto, un recuadro.
pub fn fondos(a: &Analisis, minimo: f64) -> Vec<Caja> {
    let cs = 10usize;
    let (cw, ch) = (a.w.div_ceil(cs), a.h.div_ceil(cs));
    let mut es = vec![false; cw * ch];
    let tope = a.fondo.saturating_sub(5) as u32;
    for cy in 0..ch {
        for cx in 0..cw {
            let (x0, y0, x1, y1) = (cx * cs, cy * cs, ((cx + 1) * cs).min(a.w), ((cy + 1) * cs).min(a.h));
            let (mut s, mut n, mut t, mut lo, mut hi) = (0u32, 0u32, 0u32, 255u8, 0u8);
            for y in y0..y1 {
                for x in x0..x1 {
                    let g = a.gris[y * a.w + x];
                    s += g as u32;
                    n += 1;
                    t += a.tinta[y * a.w + x] as u32;
                    lo = lo.min(g);
                    hi = hi.max(g);
                }
            }
            // Una raya gris fina en la celda no es un fondo: el fondo es liso.
            let liso = hi.saturating_sub(lo) < 30;
            // Liso: algo mas oscuro que el papel y sin tinta (el texto, de
            // media, tambien es mas oscuro que el papel).
            es[cy * cw + cx] = n > 0 && liso && s / n < tope && t * 50 < n;
        }
    }
    let cerrado = dilatar(&es, cw, ch, 1);
    componentes(&cerrado, cw, ch)
        .into_iter()
        .filter(|&(_, _, _, _, n)| n >= 6)
        .map(|(x0, y0, x1, y1, _)| a.a_pagina(x0 * cs, y0 * cs, ((x1 + 1) * cs).min(a.w), ((y1 + 1) * cs).min(a.h)))
        .filter(|c| c.w >= minimo && c.h >= minimo)
        .collect()
}
