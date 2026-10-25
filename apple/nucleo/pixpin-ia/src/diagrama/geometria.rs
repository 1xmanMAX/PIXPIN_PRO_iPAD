//! **De las mascaras a las figuras**: tramos juntados en segmentos y arcos,
//! puntos, flechas, regiones, recuadros, rotulos y la topologia.

use super::ajuste::{self, Circ, Recta, Tramo, dist};
use super::mascara::{self, Img, P};
use super::{Arco, Diagrama, Estilo, Glifo, Lector, Opciones, Punto, Recuadro, Region, Rotulo, Segmento};
use crate::hoja::{Caja, PalabraHoja};

use std::f64::consts::{PI, TAU};

/// Lo que tiene que aguantar una erosion para ser relleno (pixeles): un
/// trazo de hasta ~5 px de ancho no la aguanta.
const RADIO_RELLENO: f32 = 3.5;

/// Lo que hay que saber de la imagen mientras se mide.
struct Mapas<'a> {
    img: &'a Img,
    w: usize,
    h: usize,
    papel: [f32; 3],
    /// Lo lejos del papel que esta cada pixel.
    dpap: Vec<f32>,
    tinta: Vec<bool>,
    texto: Vec<bool>,
    /// Los trazos finos (lo que se esqueletiza).
    linea: Vec<bool>,
    /// Lo relleno (regiones, puntos, puntas).
    relleno: Vec<bool>,
    centros: Vec<[f32; 3]>,
    etiqueta: Vec<u8>,
}

impl Mapas<'_> {
    fn i(&self, p: P) -> Option<usize> {
        let (x, y) = (p.0.floor() as i64, p.1.floor() as i64);
        (x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h).then(|| y as usize * self.w + x as usize)
    }
    /// Si hay algo en `p` que pueda tapar un trazo (texto, relleno, tinta).
    fn tapado(&self, p: P) -> bool {
        // Un relleno no tapa: las rayas van encima de el (contarlo juntaba
        // dos trozos sueltos de lados opuestos de una region en una raya).
        // Ni lo palido (un degradado claro no es una raya que pase por encima).
        self.i(p).is_some_and(|i| self.texto[i] || (self.tinta[i] && !self.relleno[i] && self.dpap[i] > 90.0))
    }
    /// **El ancho de verdad de un trazo**: en cada muestra (punto y normal)
    /// se suma, a lo ancho, cuanto de tinta tiene cada medio pixel respecto
    /// al centro del trazo (el antialias cuenta en su parte). Asi una raya
    /// de 1 px mide 1 y no los 3 que pinta el antialias. Cada lado contra SU
    /// fondo: una raya roja en el borde de un relleno amarillo median el
    /// amarillo como tinta (salia de 4 px y no de 2).
    fn ancho(&self, muestras: &[(P, P)]) -> f64 {
        let mut v: Vec<f64> = Vec::new();
        for &(p, n) in muestras.iter().step_by((muestras.len() / 80).max(1)) {
            let color_en = |s: f64| self.i((p.0 + n.0 * s, p.1 + n.1 * s)).map(|i| self.img.px[i]);
            let es_texto = |s: f64| self.i((p.0 + n.0 * s, p.1 + n.1 * s)).is_some_and(|i| self.texto[i]);
            // El centro: lo mas apartado del papel cerca del esqueleto.
            let Some((k0, c0)) = (-4..=4)
                .filter_map(|k| color_en(k as f64 * 0.5).filter(|_| !es_texto(k as f64 * 0.5)).map(|c| (k, c)))
                .max_by(|a, b| mascara::dist_color(a.1, self.papel).total_cmp(&mascara::dist_color(b.1, self.papel)))
            else {
                continue;
            };
            if mascara::dist_color(c0, self.papel) < 34.0 {
                continue;
            }
            let mut suma = 0.5;
            for lado in [1.0, -1.0] {
                let base = k0 as f64 * 0.5;
                let fondo = color_en(base + lado * 6.0).unwrap_or(self.papel);
                let nucleo = mascara::dist_color(c0, fondo) as f64;
                if nucleo < 25.0 {
                    continue;
                }
                let mut k = 1;
                while k <= 16 {
                    let s = base + lado * k as f64 * 0.5;
                    let c = color_en(s).filter(|_| !es_texto(s)).map_or(0.0, |x| (mascara::dist_color(x, fondo) as f64 / nucleo).min(1.0));
                    if c < 0.15 {
                        break;
                    }
                    suma += c * 0.5;
                    k += 1;
                }
            }
            v.push(suma);
        }
        if v.is_empty() {
            return 1.0;
        }
        v.sort_by(f64::total_cmp);
        v[v.len() / 2].max(1.0)
    }
    /// **Lo que hay alrededor de un trazo** (el papel, o el relleno por el
    /// que pasa): en cada punto, lo menos entintado a 5 px; la mediana.
    fn fondo_de(&self, puntos: &[P]) -> [u8; 3] {
        let mut cs: Vec<[f32; 3]> = Vec::new();
        for &p in puntos.iter().step_by((puntos.len() / 40).max(1)) {
            let mut mejor: Option<(f32, [f32; 3])> = None;
            for dy in (-5..=5).step_by(2) {
                for dx in (-5..=5).step_by(2) {
                    if let Some(i) = self.i((p.0 + dx as f64, p.1 + dy as f64))
                        && !self.texto[i]
                        && mejor.is_none_or(|m| self.dpap[i] < m.0)
                    {
                        mejor = Some((self.dpap[i], self.img.px[i]));
                    }
                }
            }
            if let Some((_, c)) = mejor {
                cs.push(c);
            }
        }
        if cs.is_empty() { a_u8(self.papel) } else { mediana_color(&cs) }
    }
    /// El color de un trazo: en cada punto, el pixel mas entintado de
    /// alrededor; la mediana por canal.
    fn color(&self, puntos: &[P], r: f64) -> [u8; 3] {
        let mut cs: Vec<[f32; 3]> = Vec::new();
        let ri = r.ceil().max(1.0) as i64;
        for &p in puntos.iter().step_by((puntos.len() / 60).max(1)) {
            // Lo de alrededor (el papel, o el relleno por el que pasa): de
            // ahi se aparta la raya. Sobre un amarillo, lo mas «entintado»
            // respecto al papel seria la mezcla, no la raya.
            let mut fondo = self.papel;
            let mut df = f32::MAX;
            for dy in (-5..=5).step_by(2) {
                for dx in (-5..=5).step_by(2) {
                    if let Some(i) = self.i((p.0 + dx as f64, p.1 + dy as f64))
                        && self.dpap[i] < df
                    {
                        df = self.dpap[i];
                        fondo = self.img.px[i];
                    }
                }
            }
            let mut mejor: Option<(f32, [f32; 3])> = None;
            for dy in -ri..=ri {
                for dx in -ri..=ri {
                    if let Some(i) = self.i((p.0 + dx as f64, p.1 + dy as f64))
                        && !self.texto[i]
                        && self.tinta[i]
                    {
                        let d = mascara::dist_color(self.img.px[i], fondo);
                        if mejor.is_none_or(|m| d > m.0) {
                            mejor = Some((d, self.img.px[i]));
                        }
                    }
                }
            }
            if let Some((_, c)) = mejor {
                cs.push(c);
            }
        }
        mediana_color(&cs)
    }
}

fn mediana_color(cs: &[[f32; 3]]) -> [u8; 3] {
    if cs.is_empty() {
        return [30, 30, 30];
    }
    let mut out = [0u8; 3];
    for (q, o) in out.iter_mut().enumerate() {
        let mut v: Vec<f32> = cs.iter().map(|c| c[q]).collect();
        v.sort_by(f32::total_cmp);
        *o = v[v.len() / 2].round().clamp(0.0, 255.0) as u8;
    }
    out
}

fn a_u8(c: [f32; 3]) -> [u8; 3] {
    [c[0].round().clamp(0.0, 255.0) as u8, c[1].round().clamp(0.0, 255.0) as u8, c[2].round().clamp(0.0, 255.0) as u8]
}

/// Aplica `f` a la mascara solo en la caja de lo marcado (con margen): las
/// distancias de una clase pequena no recorren el recorte entero.
fn en_su_caja(m: &[bool], w: usize, h: usize, margen: usize, f: impl Fn(&[bool], usize, usize) -> Vec<bool>) -> Vec<bool> {
    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
    for (i, &b) in m.iter().enumerate() {
        if b {
            let (x, y) = (i % w, i / w);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    let mut out = vec![false; w * h];
    if x0 == usize::MAX {
        return out;
    }
    let (x0, y0) = (x0.saturating_sub(margen), y0.saturating_sub(margen));
    let (x1, y1) = ((x1 + margen).min(w - 1), (y1 + margen).min(h - 1));
    let (bw, bh) = (x1 - x0 + 1, y1 - y0 + 1);
    let mut sub = vec![false; bw * bh];
    for y in 0..bh {
        for x in 0..bw {
            sub[y * bw + x] = m[(y + y0) * w + x + x0];
        }
    }
    let r = f(&sub, bw, bh);
    for y in 0..bh {
        for x in 0..bw {
            out[(y + y0) * w + x + x0] = r[y * bw + x];
        }
    }
    out
}

/// **Mide todo.**
pub(super) fn medir(img: &Img, palabras: &[PalabraHoja], o: &Opciones) -> Diagrama {
    let (w, h) = (img.w, img.h);
    let hm = o.alto_letra.max(6.0);
    let papel = mascara::fondo(img);
    let dpap: Vec<f32> = img.px.iter().map(|&c| mascara::dist_color(c, papel)).collect();
    let tinta: Vec<bool> = dpap.iter().map(|&d| d > o.umbral_tinta).collect();
    // Lo palido (un degradado claro) cuenta para los rellenos, no para los trazos.
    let suave: Vec<bool> = dpap.iter().map(|&d| d > 14.0).collect();
    let mut texto = vec![false; w * h];
    let mut rotulos = Vec::new();
    for p in palabras {
        let c = p.caja.crecer(1.5).dentro(w as f64, h as f64);
        for y in c.y.floor() as usize..(c.y1().ceil() as usize).min(h) {
            for x in c.x.floor() as usize..(c.x1().ceil() as usize).min(w) {
                texto[y * w + x] = true;
            }
        }
        rotulos.push(Rotulo { texto: p.texto.clone(), caja: p.caja, color: p.color, negrita: p.negrita, letra_suelta: false });
    }
    for c in &o.tapar {
        let c = c.crecer(1.5).dentro(w as f64, h as f64);
        for y in c.y.floor() as usize..(c.y1().ceil() as usize).min(h) {
            for x in c.x.floor() as usize..(c.x1().ceil() as usize).min(w) {
                texto[y * w + x] = true;
            }
        }
    }
    let sin_texto: Vec<bool> = (0..w * h).map(|i| tinta[i] && !texto[i]).collect();
    let centros = mascara::colores(img, &sin_texto, papel, 9);
    let rotulos = juntar_renglones(rotulos);
    let mut d = Diagrama { ancho: w, alto: h, rotulos, ..Default::default() };
    d.tinta = sin_texto.iter().filter(|&&b| b).count();
    let (mut n_papel, mut s_papel) = (0usize, 0f64);
    for &v in &dpap {
        if v <= o.umbral_tinta {
            n_papel += 1;
            s_papel += v as f64;
        }
    }
    d.papel = n_papel as f64 / (w * h) as f64;
    d.grano = s_papel / n_papel.max(1) as f64;
    if centros.is_empty() || d.tinta < 8 {
        return d;
    }
    let etiqueta: Vec<u8> = (0..w * h).map(|i| if suave[i] { mascara::clase(&centros, papel, img.px[i]) as u8 } else { u8::MAX }).collect();

    // 1. Rellenos, clase a clase: lo que aguanta una erosion de `radio`.
    let rellenar = |radio: f32| {
        let mut relleno = vec![false; w * h];
        for k in 0..centros.len() {
            let m: Vec<bool> = (0..w * h).map(|i| etiqueta[i] == k as u8 && !texto[i]).collect();
            if m.iter().filter(|&&b| b).count() < 30 {
                continue;
            }
            let f = en_su_caja(&m, w, h, 4, |sub, bw, bh| {
                let nucleo = mascara::erosionar(sub, bw, bh, radio);
                if !nucleo.iter().any(|&b| b) {
                    return vec![false; bw * bh];
                }
                let crece = mascara::dilatar(&nucleo, bw, bh, radio + 0.4);
                let f: Vec<bool> = crece.iter().zip(sub).map(|(a, b)| *a && *b).collect();
                // El antialias del borde, de su mismo color: con el relleno.
                let mas = mascara::dilatar(&f, bw, bh, 1.6);
                mas.iter().zip(sub).map(|(a, b)| *a && *b).collect()
            });
            for i in 0..w * h {
                relleno[i] |= f[i];
            }
        }
        relleno
    };
    let dmax_punto = (0.9 * hm).clamp(7.0, 22.0);
    // **Un punto**: un disco (con los trocitos de las rayas que llegan a el,
    // que no cambian su circulo inscrito). Su centro es el del circulo
    // inscrito, no el centro de masas (que las rayas moverian).
    let es_punto = |comp: &[usize], dt: &[f32]| -> Option<Punto> {
        let (x0, y0, x1, y1) = mascara::caja_de(comp, w);
        let (bw, bh) = ((x1 - x0 + 1) as f64, (y1 - y0 + 1) as f64);
        let area = comp.len() as f64;
        let maxdt = comp.iter().map(|&i| dt[i]).fold(0f32, f32::max) as f64;
        let mayor = bw.max(bh);
        let disco = PI * maxdt * maxdt;
        let redondo = mayor <= dmax_punto && bw.min(bh) >= 3.0 && area >= 0.55 * bw * bh && mayor / bw.min(bh) <= 1.6;
        let con_rayas = maxdt >= 2.5 && maxdt <= 0.3 * hm.max(10.0) && area <= 2.2 * disco;
        if !(redondo || con_rayas) {
            return None;
        }
        let tope = maxdt as f32 - 0.5;
        let centro: Vec<usize> = comp.iter().copied().filter(|&i| dt[i] >= tope).collect();
        let n = centro.len().max(1) as f64;
        let c = (centro.iter().map(|&i| (i % w) as f64 + 0.5).sum::<f64>() / n, centro.iter().map(|&i| (i / w) as f64 + 0.5).sum::<f64>() / n);
        let r = if redondo { (area / PI).sqrt() } else { maxdt + 0.5 };
        let color = a_u8(mascara::media(comp.iter().filter(|&&i| dist(((i % w) as f64 + 0.5, (i / w) as f64 + 0.5), c) <= r).map(|&i| img.px[i])));
        Some(Punto { c, r, color })
    };
    // Los puntos, con una erosion chica (un punto de 7 px no aguanta la de
    // los rellenos, que es grande para que una raya de 6 px siga siendo raya).
    let chico = rellenar(2.2);
    let dt_chico = mascara::distancia(&chico, w, h);
    for comp in mascara::componentes(&chico, w, h) {
        if let Some(p) = es_punto(&comp, &dt_chico) {
            d.puntos.push(p);
        }
    }
    let mut relleno = rellenar(RADIO_RELLENO);
    for p in &d.puntos {
        let r = p.r + 1.5;
        for y in (p.c.1 - r).floor().max(0.0) as usize..((p.c.1 + r).ceil() as usize).min(h) {
            for x in (p.c.0 - r).floor().max(0.0) as usize..((p.c.0 + r).ceil() as usize).min(w) {
                if dist((x as f64 + 0.5, y as f64 + 0.5), p.c) <= r {
                    relleno[y * w + x] = false;
                }
            }
        }
    }

    // 2. Cada mancha de relleno: punto, trazo grueso, pequena (punta o
    //    marca) o region.
    let dt_rel = mascara::distancia(&relleno, w, h);
    let mut gruesos = vec![false; w * h];
    let mut pequenas: Vec<Vec<usize>> = Vec::new();
    for comp in mascara::componentes(&relleno, w, h) {
        let (x0, y0, x1, y1) = mascara::caja_de(&comp, w);
        let (bw, bh) = ((x1 - x0 + 1) as f64, (y1 - y0 + 1) as f64);
        let maxdt = comp.iter().map(|&i| dt_rel[i]).fold(0f32, f32::max) as f64;
        let mayor = bw.max(bh);
        if let Some(p) = es_punto(&comp, &dt_rel) {
            if !d.puntos.iter().any(|q| dist(q.c, p.c) < q.r + p.r) {
                d.puntos.push(p);
            }
            continue;
        }
        if maxdt <= 4.6 && mayor >= 8.0 * maxdt.max(1.0) {
            for &i in &comp {
                gruesos[i] = true;
            }
            continue;
        }
        if mayor <= 1.6 * hm {
            pequenas.push(comp);
        }
    }
    let quitar_de_relleno: Vec<bool> = gruesos.clone();
    let mut linea: Vec<bool> = (0..w * h).map(|i| (tinta[i] && !relleno[i] && !texto[i]) || gruesos[i]).collect();
    for i in 0..w * h {
        if quitar_de_relleno[i] {
            relleno[i] = false;
        }
    }
    // Los puntos fuera del relleno (ya son puntos) y, con un poco de aire,
    // fuera de los trazos: alrededor de un punto se juntan la recta, la
    // circunferencia y el antialias del punto, y su esqueleto salia como un
    // arquito de nada; cortado, cada trazo acaba en el borde y la
    // topologia lo pega al centro.
    for p in &d.puntos {
        let r = p.r + 2.5;
        for y in (p.c.1 - r).floor().max(0.0) as usize..((p.c.1 + r).ceil() as usize).min(h) {
            for x in (p.c.0 - r).floor().max(0.0) as usize..((p.c.0 + r).ceil() as usize).min(w) {
                if dist((x as f64 + 0.5, y as f64 + 0.5), p.c) <= r {
                    relleno[y * w + x] = false;
                    linea[y * w + x] = false;
                }
            }
        }
    }

    // 3. Letras sueltas (manchas de trazo pequenas y aisladas) y guiones
    //    (los trozos de una discontinua o una punteada, que se encadenan).
    let mut glifos_px: Vec<Vec<usize>> = Vec::new();
    let mut guiones: Vec<Guion> = Vec::new();
    for comp in mascara::componentes(&linea, w, h) {
        let (x0, y0, x1, y1) = mascara::caja_de(&comp, w);
        let (bw, bh) = ((x1 - x0 + 1) as f64, (y1 - y0 + 1) as f64);
        let mayor = bw.max(bh);
        if mayor > 1.8 * hm {
            continue;
        }
        let pts: Vec<P> = comp.iter().map(|&i| ((i % w) as f64 + 0.5, (i / w) as f64 + 0.5)).collect();
        let n = pts.len() as f64;
        let c = (pts.iter().map(|p| p.0).sum::<f64>() / n, pts.iter().map(|p| p.1).sum::<f64>() / n);
        // Una mota (el punto de una punteada) o un trazo recto y corto (el
        // guion de una discontinua, una marca): geometria, no una letra.
        // Recto y fino: lo que mide a lo largo contra lo que pesa (un guion
        // en diagonal tiene caja cuadrada).
        if mayor < 0.35 * hm || comp.len() < 4 {
            for &i in &comp {
                linea[i] = false;
            }
            guiones.push(Guion { c, d: None, l: mayor, px: comp });
            continue;
        }
        if let Some(r) = ajuste::recta(&pts) {
            let (t0, t1) = intervalo(&r, &pts);
            let ancho = comp.len() as f64 / (t1 - t0 + 1.0);
            if r.error <= 1.8 && ancho <= 4.5 {
                if t1 - t0 <= 1.5 * hm {
                    for &i in &comp {
                        linea[i] = false;
                    }
                    guiones.push(Guion { c, d: Some(r.d), l: t1 - t0 + 1.0, px: comp });
                }
                continue;
            }
        }
        for &i in &comp {
            linea[i] = false;
        }
        glifos_px.push(comp);
    }
    let (discontinuas, sueltos) = encadenar(&guiones, hm);
    for k in sueltos {
        for &i in &guiones[k].px {
            linea[i] = true;
        }
    }

    let mut m = Mapas { img, w, h, papel, dpap, tinta, texto, linea, relleno, centros, etiqueta };
    for cadena in &discontinuas {
        if let Some(s) = segmento_discontinuo(&m, &guiones, cadena) {
            d.segmentos.push(s);
        }
    }

    for comp in &glifos_px {
        let (x0, y0, x1, y1) = mascara::caja_de(comp, w);
        // En su caja (con un margen de 2): cada letra no recorre el recorte entero.
        let (ox, oy) = (x0.saturating_sub(2), y0.saturating_sub(2));
        let (gw, gh) = ((x1 + 3).min(w) - ox, (y1 + 3).min(h) - oy);
        let mut mascara_g = vec![false; gw * gh];
        for &i in comp {
            mascara_g[(i / w - oy) * gw + (i % w - ox)] = true;
        }
        let esq = mascara::esqueleto(&mascara_g, gw, gh);
        let trazos: Vec<Vec<P>> = mascara::cadenas(&esq, gw, gh)
            .into_iter()
            .filter(|c| c.len() >= 2)
            .map(|c| c.into_iter().map(|(x, y)| (x + ox as f64, y + oy as f64)).collect())
            .collect();
        let color = a_u8(media_mas_tinta(&m, comp));
        // Lo que pesa la letra entre lo que mide su esqueleto, sin el halo
        // del antialias (que cuenta medio).
        let grosor = {
            let largo: f64 = trazos.iter().map(|t| ajuste::largo(t)).sum::<f64>().max(1.0);
            let peso: f64 = comp.iter().map(|&i| (m.dpap[i] as f64 / 200.0).min(1.0)).sum();
            (peso / largo).clamp(1.0, 6.0)
        };
        let pixeles = comp.iter().map(|&i| ((i % w) as u32, (i / w) as u32)).collect();
        d.glifos.push(Glifo { caja: Caja::de_esquinas(x0 as f64, y0 as f64, x1 as f64 + 1.0, y1 as f64 + 1.0), color, trazos, grosor, pixeles });
    }

    // 3b. **Los recuadros, de una vez** (el marco verde de «θ=90°»): una
    //     mancha de trazo cuyos pixeles van casi todos por el borde de su
    //     caja, con los cuatro lados cubiertos, es un rectangulo. Por el
    //     esqueleto sus esquinas salian redondeadas y los lados cortos como
    //     arcos.
    for comp in mascara::componentes(&m.linea, w, h) {
        let (x0, y0, x1, y1) = mascara::caja_de(&comp, w);
        let (bw, bh) = ((x1 - x0 + 1) as f64, (y1 - y0 + 1) as f64);
        if bw < 12.0 || bh < 8.0 || comp.len() < 30 {
            continue;
        }
        let banda = 4usize;
        let en_borde = |i: usize| {
            let (x, y) = (i % w, i / w);
            x <= x0 + banda || x + banda >= x1 || y <= y0 + banda || y + banda >= y1
        };
        let borde = comp.iter().filter(|&&i| en_borde(i)).count();
        if std::env::var_os("PIXPIN_DIAGRAMA_DEPURAR").is_some() {
            eprintln!("caja? {x0},{y0}-{x1},{y1} borde {borde}/{}", comp.len());
        }
        if (borde as f64) < 0.9 * comp.len() as f64 {
            continue;
        }
        // Cada lado, cubierto casi entero.
        let mut arriba = vec![false; x1 - x0 + 1];
        let mut abajo = vec![false; x1 - x0 + 1];
        let mut izq = vec![false; y1 - y0 + 1];
        let mut der = vec![false; y1 - y0 + 1];
        for &i in &comp {
            let (x, y) = (i % w, i / w);
            if y <= y0 + banda {
                arriba[x - x0] = true;
            }
            if y + banda >= y1 {
                abajo[x - x0] = true;
            }
            if x <= x0 + banda {
                izq[y - y0] = true;
            }
            if x + banda >= x1 {
                der[y - y0] = true;
            }
        }
        // Un lado que se separo de las esquinas (otra mancha, un poco mas
        // alla): la columna o la fila de tinta casi entera mas cercana.
        let (mut x0, mut y0, mut x1, mut y1) = (x0, y0, x1, y1);
        let col = |x: usize, ya: usize, yb: usize| (ya..=yb).filter(|&y| m.tinta[y * w + x] && !m.texto[y * w + x]).count() as f64 / (yb - ya + 1) as f64;
        let fila = |y: usize, xa: usize, xb: usize| (xa..=xb).filter(|&x| m.tinta[y * w + x] && !m.texto[y * w + x]).count() as f64 / (xb - xa + 1) as f64;
        let alcance = ((x1 - x0).max(y1 - y0) as f64 * 0.3) as usize;
        let mut lado_movido = false;
        if !(der.iter().filter(|&&b| b).count() as f64 >= 0.85 * der.len() as f64) {
            if let Some(x) = (x1 + 1..=(x1 + alcance).min(w - 1)).find(|&x| col(x, y0 + 2, y1.saturating_sub(2)) >= 0.7) {
                x1 = x;
                der.iter_mut().for_each(|b| *b = true);
                lado_movido = true;
            }
        }
        if !(izq.iter().filter(|&&b| b).count() as f64 >= 0.85 * izq.len() as f64) {
            if let Some(x) = (x0.saturating_sub(alcance)..x0).rev().find(|&x| col(x, y0 + 2, y1.saturating_sub(2)) >= 0.7) {
                x0 = x;
                izq.iter_mut().for_each(|b| *b = true);
                lado_movido = true;
            }
        }
        if !(abajo.iter().filter(|&&b| b).count() as f64 >= 0.85 * abajo.len() as f64) {
            if let Some(y) = (y1 + 1..=(y1 + alcance).min(h - 1)).find(|&y| fila(y, x0 + 2, x1.saturating_sub(2)) >= 0.7) {
                y1 = y;
                abajo.iter_mut().for_each(|b| *b = true);
                lado_movido = true;
            }
        }
        if !(arriba.iter().filter(|&&b| b).count() as f64 >= 0.85 * arriba.len() as f64) {
            if let Some(y) = (y0.saturating_sub(alcance)..y0).rev().find(|&y| fila(y, x0 + 2, x1.saturating_sub(2)) >= 0.7) {
                y0 = y;
                arriba.iter_mut().for_each(|b| *b = true);
                lado_movido = true;
            }
        }
        let lleno = |v: &[bool]| v.iter().filter(|&&b| b).count() as f64 >= 0.85 * v.len() as f64;
        if std::env::var_os("PIXPIN_DIAGRAMA_DEPURAR").is_some() {
            eprintln!("recuadro? caja {x0},{y0}-{x1},{y1} borde {borde}/{} lados {} {} {} {}", comp.len(), lleno(&arriba), lleno(&abajo), lleno(&izq), lleno(&der));
        }
        // Tres lados enteros y el cuarto al menos a medias (una esquina o un
        // lado que el antialias corta).
        let medio_lleno = |v: &[bool]| v.iter().filter(|&&b| b).count() as f64 >= 0.5 * v.len() as f64;
        let lados = [&arriba, &abajo, &izq, &der];
        if lados.iter().filter(|v| lleno(v)).count() < 3 || !lados.iter().all(|v| medio_lleno(v)) {
            continue;
        }
        // Grosor: lo que pesa entre el perimetro; el lado, por el centro del trazo.
        let perimetro = 2.0 * (bw + bh);
        let peso: f64 = comp.iter().map(|&i| (m.dpap[i] as f64 / 160.0).min(1.0)).sum();
        let grosor = (peso / perimetro).clamp(1.0, 6.0);
        let medio = grosor / 2.0;
        let color = a_u8(media_mas_tinta(&m, &comp));
        d.recuadros.push(Recuadro { x0: x0 as f64 + medio, y0: y0 as f64 + medio, x1: x1 as f64 + 1.0 - medio, y1: y1 as f64 + 1.0 - medio, grosor, color, relleno: None });
        for &i in &comp {
            m.linea[i] = false;
        }
        // Y el lado suelto que se le junto.
        if lado_movido {
            for y in y0.saturating_sub(1)..=(y1 + 3).min(h - 1) {
                for x in x0.saturating_sub(1)..=(x1 + 3).min(w - 1) {
                    let borde = x <= x0 + banda || x + banda >= x1 + 3 || y <= y0 + banda || y + banda >= y1 + 3;
                    if borde {
                        m.linea[y * w + x] = false;
                    }
                }
            }
        }
    }

    // 4. Esqueleto, cadenas y tramos.
    let esq = mascara::esqueleto(&m.linea, w, h);
    let radio_max = 3.0 * w.max(h) as f64;
    let mut tramos = Vec::new();
    let cads = mascara::cadenas(&esq, w, h);
    let depurar = std::env::var_os("PIXPIN_DIAGRAMA_DEPURAR").is_some();
    for c in &cads {
        let antes = tramos.len();
        ajuste::trocear(c, radio_max, &mut tramos);
        if depurar {
            eprintln!("cadena {} pts {:?}..{:?} -> {:?}", c.len(), c.first(), c.last(), tramos[antes..].iter().map(|t| match t { Tramo::Recto { puntos, recta } => format!("R{} e{:.2}", puntos.len(), recta.error), Tramo::Curvo { puntos, circ } => format!("C{} r{:.1} c({:.1},{:.1}) e{:.2}", puntos.len(), circ.r, circ.c.0, circ.c.1, circ.error) }).collect::<Vec<_>>());
        }
    }

    // 5. Juntar.
    let mut grupos_c = juntar_arcos(&tramos);
    let mut grupos_l = juntar_rectas(&m, &tramos, hm);
    arcos_de_trozos(&mut grupos_l, &mut grupos_c, hm, w.max(h) as f64);
    absorber_cuerdas(&mut grupos_c, &mut grupos_l);
    fundir_circunferencias(&mut grupos_c);
    for g in &grupos_c {
        for a in arcos_de(&m, g, hm, &d.puntos) {
            // Una circunferencia de nada es un punto (uno pequeno, que no
            // aguanto la erosion y salio como anillo); un arquito asi, ruido.
            if a.r <= 6f64.max(0.4 * hm) {
                if a.barrido >= 0.6 * TAU && !d.puntos.iter().any(|p| dist(p.c, a.c) < p.r + a.r) {
                    d.puntos.push(Punto { c: a.c, r: a.r + a.grosor / 2.0, color: a.color });
                }
                continue;
            }
            // Un arquito gordo o palido junto a un vertice es el borde de una
            // marca de angulo rellena (ya va como region), no un trazo.
            let palido = mascara::dist_color([a.color[0] as f32, a.color[1] as f32, a.color[2] as f32], papel) < 110.0;
            if a.r < 1.5 * hm && (a.grosor >= 0.45 * a.r || palido) {
                continue;
            }
            d.arcos.push(a);
        }
    }
    for g in &grupos_l {
        if let Some(s) = segmento_de(&m, g) {
            d.segmentos.push(s);
        }
    }
    // Lo que queda de una letra pegada a una raya: arquitos y rayitas sueltas.
    limpiar_sueltos(&mut d, hm);
    consolidar_arcos(&m, &mut d, hm);
    partir_por_grosor(&m, &mut d);

    // 6. Flechas, topologia, recuadros.
    let colores_peq: Vec<[u8; 3]> = pequenas.iter().map(|c| a_u8(media_mas_tinta(&m, c))).collect();
    let usadas = flechas(&mut d, &pequenas, &colores_peq, w, hm);
    arquitos_a_alas(&mut d, hm);
    flechas_abiertas(&mut d, hm);
    sin_puntos_en_las_puntas(&mut d);
    puntas_de_discontinuas(&mut d, hm);
    juntar_colineales(&mut d);
    topologia(&mut d, hm);
    juntar_colineales(&mut d);
    quitar_espuelas(&mut d, hm);
    normalizar_grosores(&mut d, hm);
    d.segmentos.retain(|s| s.largo() >= 2.0);
    recuadros(&mut d);
    // Los trazos finos, con su color pleno (no el del centro emborronado).
    // Contra lo que tienen alrededor (el amarillo de un relleno, el papel),
    // (`vivo` deja lo pleno como esta).
    for s in d.segmentos.iter_mut().filter(|s| s.grosor <= 5.0) {
        let pts: Vec<P> = (0..=8).map(|k| (s.a.0 + (s.b.0 - s.a.0) * k as f64 / 8.0, s.a.1 + (s.b.1 - s.a.1) * k as f64 / 8.0)).collect();
        let fondo = m.fondo_de(&pts);
        s.color = vivo(s.color, fondo);
    }
    for a in d.arcos.iter_mut().filter(|a| a.grosor <= 5.0) {
        let pts: Vec<P> = (0..=8)
            .map(|k| {
                let t = a.inicio + a.barrido * k as f64 / 8.0;
                (a.c.0 + a.r * t.cos(), a.c.1 + a.r * t.sin())
            })
            .collect();
        a.color = vivo(a.color, m.fondo_de(&pts));
    }
    for r in d.recuadros.iter_mut().filter(|r| r.grosor <= 5.0) {
        r.color = vivo(r.color, m.fondo_de(&[(r.x0, r.y0), (r.x1, r.y0), (r.x1, r.y1), (r.x0, r.y1)]));
    }

    // Un «punto» palido es un trozo de una marca de color translucida, no un
    // punto (los puntos se pintan con tinta plena).
    d.puntos.retain(|p| *p.color.iter().min().unwrap() < 150 || mascara::dist_color([p.color[0] as f32, p.color[1] as f32, p.color[2] as f32], papel) >= 120.0);

    // 7. Regiones (lo relleno que no fue punto ni punta).
    let mut region = m.relleno.clone();
    for (k, comp) in pequenas.iter().enumerate() {
        if usadas.contains(&k) {
            for &i in comp {
                region[i] = false;
            }
        }
    }
    d.regiones = regiones(&m, &region, hm);
    formas_rellenas(&mut d, hm);

    // 8. Lo que explican las figuras.
    d.explica = explicado(&m, &d, &region, &glifos_px);
    d.de_relleno = (0..w * h).filter(|&i| region[i] && m.tinta[i] && !m.texto[i]).count() as f64 / d.tinta.max(1) as f64;
    let mut largos: Vec<f64> = d.segmentos.iter().map(Segmento::largo).chain(d.arcos.iter().map(|a| a.r * a.barrido)).collect();
    largos.sort_by(f64::total_cmp);
    d.largo_mediano = largos.get(largos.len() / 2).copied().unwrap_or(0.0);
    m.linea.clear();
    d
}

/// El color medio de los pixeles mas entintados de una mancha.
fn media_mas_tinta(m: &Mapas, comp: &[usize]) -> [f32; 3] {
    let mut v: Vec<(f32, usize)> = comp.iter().map(|&i| (m.dpap[i], i)).collect();
    v.sort_by(|a, b| b.0.total_cmp(&a.0));
    v.truncate((v.len() / 2).max(1));
    mascara::media(v.iter().map(|&(_, i)| m.img.px[i]))
}

// ---------------------------------------------------------------------------
// Rectas

/// Un grupo de tramos rectos que son el mismo segmento.
struct GrupoRecta {
    puntos: Vec<P>,
    recta: Recta,
    t0: f64,
    t1: f64,
}

impl GrupoRecta {
    fn nuevo(puntos: Vec<P>, recta: Recta) -> GrupoRecta {
        let (t0, t1) = intervalo(&recta, &puntos);
        GrupoRecta { puntos, recta, t0, t1 }
    }
    fn largo(&self) -> f64 {
        self.t1 - self.t0
    }
}

fn intervalo(r: &Recta, p: &[P]) -> (f64, f64) {
    let mut t0 = f64::MAX;
    let mut t1 = f64::MIN;
    for &q in p {
        let t = r.t(q);
        t0 = t0.min(t);
        t1 = t1.max(t);
    }
    (t0, t1)
}

fn juntar_rectas(m: &Mapas, tramos: &[Tramo], hm: f64) -> Vec<GrupoRecta> {
    let mut rectos: Vec<(&[P], Recta)> = tramos
        .iter()
        .filter_map(|t| match t {
            Tramo::Recto { puntos, recta } => Some((puntos.as_slice(), *recta)),
            _ => None,
        })
        .collect();
    rectos.sort_by(|a, b| ajuste::largo(b.0).total_cmp(&ajuste::largo(a.0)));
    let mut grupos: Vec<GrupoRecta> = Vec::new();
    for (pts, r) in rectos {
        let largo = ajuste::largo(pts);
        let (a, b) = (pts[0], pts[pts.len() - 1]);
        let mut mejor: Option<(usize, f64)> = None;
        for (gi, g) in grupos.iter().enumerate() {
            // Paralelas: el angulo entre las dos, con holgura en lo corto.
            let cruz = (r.d.0 * g.recta.d.1 - r.d.1 * g.recta.d.0).abs();
            let tol_ang = (1.5f64.to_radians()).max((2.0 / largo.max(1.0)).atan().min(12f64.to_radians()));
            if cruz > tol_ang.sin() {
                continue;
            }
            // Sobre la misma recta.
            let tol_n = 1.6 + 0.02 * g.largo().min(300.0) / 10.0;
            let (na, nb) = (g.recta.normal(a).abs(), g.recta.normal(b).abs());
            if na.max(nb) > tol_n {
                continue;
            }
            // Y sin un hueco grande en medio (o con el hueco tapado).
            let (u0, u1) = {
                let (x, y) = (g.recta.t(a), g.recta.t(b));
                (x.min(y), x.max(y))
            };
            let hueco = (u0 - g.t1).max(g.t0 - u1).max(0.0);
            let libre = 10f64.max(0.8 * hm);
            let vale = hueco <= libre || (hueco <= 5.0 * hm && hueco_tapado(m, &g.recta, if u0 > g.t1 { (g.t1, u0) } else { (u1, g.t0) }));
            if !vale {
                continue;
            }
            let puntuacion = na.max(nb) + hueco * 0.05;
            if mejor.is_none_or(|(_, p)| puntuacion < p) {
                mejor = Some((gi, puntuacion));
            }
        }
        match mejor {
            Some((gi, _)) => {
                let g = &mut grupos[gi];
                g.puntos.extend_from_slice(pts);
                if let Some(nr) = ajuste::recta(&g.puntos) {
                    g.recta = nr;
                }
                let (t0, t1) = intervalo(&g.recta, &g.puntos);
                g.t0 = t0;
                g.t1 = t1;
            }
            None => grupos.push(GrupoRecta::nuevo(pts.to_vec(), r)),
        }
    }
    // Una segunda vuelta: grupos que ya crecidos son el mismo.
    let mut i = 0;
    while i < grupos.len() {
        let mut j = i + 1;
        let mut junto = false;
        while j < grupos.len() {
            let (gi, gj) = (&grupos[i], &grupos[j]);
            let cruz = (gi.recta.d.0 * gj.recta.d.1 - gi.recta.d.1 * gj.recta.d.0).abs();
            let a = gj.recta.en(gj.t0);
            let b = gj.recta.en(gj.t1);
            let (u0, u1) = {
                let (x, y) = (gi.recta.t(a), gi.recta.t(b));
                (x.min(y), x.max(y))
            };
            let hueco = (u0 - gi.t1).max(gi.t0 - u1).max(0.0);
            let cerca = gi.recta.normal(a).abs().max(gi.recta.normal(b).abs()) <= 2.0;
            let tramo_hueco = if u0 > gi.t1 { (gi.t1, u0) } else { (u1, gi.t0) };
            if cruz <= 2f64.to_radians().sin()
                && cerca
                && (hueco <= 10f64.max(0.8 * hm) || (hueco <= 5.0 * hm && hueco_tapado(m, &gi.recta, tramo_hueco)))
            {
                let g = grupos.remove(j);
                let gi = &mut grupos[i];
                gi.puntos.extend(g.puntos);
                if let Some(nr) = ajuste::recta(&gi.puntos) {
                    gi.recta = nr;
                }
                let (t0, t1) = intervalo(&gi.recta, &gi.puntos);
                gi.t0 = t0;
                gi.t1 = t1;
                junto = true;
            } else {
                j += 1;
            }
        }
        if !junto {
            i += 1;
        }
    }
    grupos.retain(|g| g.largo() >= 4.0);
    grupos
}

/// Si el hueco `(desde, hasta)` de la recta lo tapa algo (texto, relleno,
/// otra tinta): una raya que pasa por debajo de un rotulo sigue siendo una.
fn hueco_tapado(m: &Mapas, r: &Recta, (desde, hasta): (f64, f64)) -> bool {
    let n = ((hasta - desde).abs().ceil() as usize).max(1);
    let mut si = 0;
    for k in 0..=n {
        let t = desde + (hasta - desde) * k as f64 / n as f64;
        if m.tapado(r.en(t)) {
            si += 1;
        }
    }
    si as f64 >= 0.7 * (n + 1) as f64
}

/// El segmento de un grupo: extremos, grosor, color y estilo.
fn segmento_de(m: &Mapas, g: &GrupoRecta) -> Option<Segmento> {
    let normal = (-g.recta.d.1, g.recta.d.0);
    let n = ((g.t1 - g.t0) / 2.0).ceil().max(1.0) as usize;
    let muestras: Vec<(P, P)> = (0..=n).map(|k| (g.recta.en(g.t0 + (g.t1 - g.t0) * k as f64 / n as f64), normal)).collect();
    let grosor = m.ancho(&muestras);
    // El esqueleto se queda medio grosor antes de la punta de verdad.
    let (t0, t1) = (g.t0 - grosor / 2.0, g.t1 + grosor / 2.0);
    let (a, b) = (g.recta.en(t0), g.recta.en(t1));
    let color = m.color(&g.puntos, (grosor / 2.0).max(1.0));
    let estilo = estilo_de(m, &g.recta, g.t0, g.t1, grosor);
    Some(Segmento { a, b, grosor, color, estilo, flecha_a: false, flecha_b: false })
}

/// **Discontinua o punteada**: el perfil de la tinta a lo largo, con
/// huecos de verdad (sin nada debajo) que se repiten.
fn estilo_de(m: &Mapas, r: &Recta, t0: f64, t1: f64, grosor: f64) -> Estilo {
    let n = (t1 - t0).ceil() as usize;
    if n < 12 {
        return Estilo::Solido;
    }
    let radio = grosor / 2.0 + 0.8;
    // Solo de lado (a lo ancho del trazo): mirar tambien a lo largo
    // taparia los huecos cortos.
    let normal = (-r.d.1, r.d.0);
    let perfil: Vec<bool> = (0..=n)
        .map(|k| {
            let p = r.en(t0 + k as f64);
            let pasos = (radio * 2.0).ceil() as i64;
            (-pasos..=pasos).any(|q| {
                let s = q as f64 * 0.5;
                m.i((p.0 + normal.0 * s, p.1 + normal.1 * s)).is_some_and(|i| m.linea[i])
            })
        })
        .collect();
    let (mut tramos, mut huecos) = (Vec::new(), 0usize);
    let mut k = 0;
    while k < perfil.len() {
        let v = perfil[k];
        let mut j = k;
        while j < perfil.len() && perfil[j] == v {
            j += 1;
        }
        if v {
            tramos.push(j - k);
        } else if j - k >= 2 && !(k..j).all(|q| m.tapado(r.en(t0 + q as f64))) {
            huecos += 1;
        }
        k = j;
    }
    let cubierto = perfil.iter().filter(|&&b| b).count() as f64 / perfil.len() as f64;
    if std::env::var_os("PIXPIN_DIAGRAMA_DEPURAR").is_some() {
        eprintln!("estilo: n {n} huecos {huecos} cubierto {cubierto:.2} tramos {tramos:?}");
    }
    if huecos >= 3 && cubierto < 0.85 {
        tramos.sort_unstable();
        let medio = tramos.get(tramos.len() / 2).copied().unwrap_or(0) as f64;
        if medio <= 2.5 * grosor + 1.5 { Estilo::Punteado } else { Estilo::Discontinuo }
    } else {
        Estilo::Solido
    }
}

// ---------------------------------------------------------------------------
// Circunferencias

struct GrupoCirc {
    /// Los tramos (cada uno en orden, para el barrido).
    tramos: Vec<Vec<P>>,
    circ: Circ,
}

impl GrupoCirc {
    fn todos(&self) -> Vec<P> {
        self.tramos.iter().flatten().copied().collect()
    }
    fn largo(&self) -> f64 {
        self.tramos.iter().map(|t| ajuste::largo(t)).sum()
    }
}

fn encaja(c: &Circ, p: &[P]) -> bool {
    if p.is_empty() {
        return false;
    }
    let tol = 1.6f64.max(c.r * 0.012);
    let media = p.iter().map(|&q| c.residuo(q).abs()).sum::<f64>() / p.len() as f64;
    let peor = p.iter().map(|&q| c.residuo(q).abs()).fold(0.0, f64::max);
    media <= tol && peor <= 2.2 * tol
}

fn juntar_arcos(tramos: &[Tramo]) -> Vec<GrupoCirc> {
    let mut curvos: Vec<(&[P], Circ)> = tramos
        .iter()
        .filter_map(|t| match t {
            Tramo::Curvo { puntos, circ } => Some((puntos.as_slice(), *circ)),
            _ => None,
        })
        .collect();
    curvos.sort_by(|a, b| ajuste::largo(b.0).total_cmp(&ajuste::largo(a.0)));
    let mut grupos: Vec<GrupoCirc> = Vec::new();
    for (pts, c) in curvos {
        let sitio = grupos.iter().position(|g| {
            let parecida = dist(g.circ.c, c.c) <= 3f64.max(0.06 * g.circ.r) && (g.circ.r - c.r).abs() <= 2.5f64.max(0.05 * g.circ.r);
            parecida || encaja(&g.circ, pts)
        });
        match sitio {
            Some(i) => {
                let g = &mut grupos[i];
                g.tramos.push(pts.to_vec());
                if let Some(nc) = ajuste::circunferencia(&g.todos()) {
                    g.circ = nc;
                }
            }
            None => grupos.push(GrupoCirc { tramos: vec![pts.to_vec()], circ: c }),
        }
    }
    grupos
}

/// Rectas cortas que caen sobre una circunferencia (un trozo entre dos
/// cruces cercanos sale casi recto): son de ella.
fn absorber_cuerdas(cs: &mut [GrupoCirc], ls: &mut Vec<GrupoRecta>) {
    ls.retain(|l| {
        if l.largo() > 60.0 {
            return true;
        }
        for c in cs.iter_mut() {
            if l.largo() <= 0.6 * c.circ.r && encaja(&c.circ, &l.puntos) {
                c.tramos.push(l.puntos.clone());
                return false;
            }
        }
        true
    });
}

/// **Los arcos de una circunferencia**: lo cubierto (con los huecos chicos
/// o tapados puenteados) y partido donde cambia el color.
fn arcos_de(m: &Mapas, g: &GrupoCirc, hm: f64, puntos: &[Punto]) -> Vec<Arco> {
    let Some(c) = ajuste::circunferencia(&g.todos()).or(Some(g.circ)) else { return Vec::new() };
    // Un arquito de nada (un trozo de letra pegada a una raya) no es figura.
    if c.r < 0.6 * hm && g.largo() < 0.75 * TAU * c.r {
        return Vec::new();
    }
    const N: usize = 360;
    let bin = |a: f64| (((a.rem_euclid(TAU)) / TAU * N as f64) as usize).min(N - 1);
    let mut cubierto = [false; N];
    for t in &g.tramos {
        for w in t.windows(2) {
            let (a0, a1) = (c.angulo(w[0]), c.angulo(w[1]));
            let mut da = a1 - a0;
            while da > PI {
                da -= TAU;
            }
            while da < -PI {
                da += TAU;
            }
            let pasos = ((da.abs() / TAU * N as f64).ceil() as usize).max(1);
            for k in 0..=pasos {
                cubierto[bin(a0 + da * k as f64 / pasos as f64)] = true;
            }
        }
    }
    // Puentear los huecos chicos o tapados.
    let grados_px = |px: f64| px / c.r * N as f64 / TAU;
    let libre = grados_px(10f64.max(0.7 * hm)).ceil() as usize;
    let tapado_hasta = grados_px(4.0 * hm).ceil() as usize;
    if cubierto.iter().any(|&b| b) {
        let inicio = (0..N).find(|&k| cubierto[k] && !cubierto[(k + N - 1) % N]);
        if let Some(s) = inicio {
            let mut k = 0;
            while k < N {
                let i = (s + k) % N;
                if cubierto[i] {
                    k += 1;
                    continue;
                }
                let mut j = k;
                while j < N && !cubierto[(s + j) % N] {
                    j += 1;
                }
                let largo = j - k;
                let tapado = largo <= tapado_hasta && {
                    let si = (k..j).filter(|q| m.tapado(c.en(((s + q) % N) as f64 * TAU / N as f64 + TAU / N as f64 / 2.0))).count();
                    si as f64 >= 0.7 * largo as f64
                };
                // Un punto en el hueco (un vertice sobre la circunferencia): el
                // trazo se corto alli a proposito.
                let con_punto = largo <= tapado_hasta && puntos.iter().any(|p| {
                    (k..j).any(|q| dist(c.en(((s + q) % N) as f64 * TAU / N as f64), p.c) <= p.r + 4.0)
                });
                if largo <= libre || tapado || con_punto {
                    for q in k..j {
                        cubierto[(s + q) % N] = true;
                    }
                }
                k = j;
            }
        }
    }
    let radial = |a: f64| (c.en(a), (a.cos(), a.sin()));
    let grosor = m.ancho(&g.todos().iter().map(|&p| radial(c.angulo(p))).collect::<Vec<_>>());
    // El color de cada grado.
    let clase_en = |k: usize| -> Option<(usize, [f32; 3])> {
        let p = c.en((k as f64 + 0.5) * TAU / N as f64);
        let r = (grosor / 2.0).max(1.0);
        let ri = r.ceil() as i64;
        let mut mejor: Option<(f32, usize)> = None;
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                if let Some(i) = m.i((p.0 + dx as f64, p.1 + dy as f64))
                    && m.tinta[i]
                    && !m.texto[i]
                    && mejor.is_none_or(|b| m.dpap[i] > b.0)
                {
                    mejor = Some((m.dpap[i], i));
                }
            }
        }
        mejor.map(|(_, i)| (mascara::clase(&m.centros, m.papel, m.img.px[i]), m.img.px[i]))
    };
    let colores: Vec<Option<(usize, [f32; 3])>> = (0..N).map(|k| if cubierto[k] { clase_en(k) } else { None }).collect();
    // Tramos seguidos de lo cubierto: [desde, hasta) en grados.
    let todos = cubierto.iter().all(|&b| b);
    let mut corridas: Vec<(usize, usize)> = Vec::new();
    if todos {
        corridas.push((0, N));
    } else if let Some(s) = (0..N).find(|&k| cubierto[k] && !cubierto[(k + N - 1) % N]) {
        let mut k = 0;
        while k < N {
            if !cubierto[(s + k) % N] {
                k += 1;
                continue;
            }
            let mut j = k;
            while j < N && cubierto[(s + j) % N] {
                j += 1;
            }
            corridas.push((s + k, s + j));
            k = j;
        }
    }
    let mut salida = Vec::new();
    let minimo = grados_px(12f64.max(0.5 * hm)).max(6.0) as usize;
    for (desde, hasta) in corridas {
        if hasta - desde < minimo {
            continue;
        }
        // Partir por color: la clase de cada grado, con las corridas cortas
        // (menos de 6 grados) tragadas por la vecina.
        let mut clases: Vec<Option<usize>> = (desde..hasta).map(|k| colores[k % N].map(|x| x.0)).collect();
        rellenar_huecos(&mut clases);
        suavizar(&mut clases, 6);
        let mut k = 0;
        let mut piezas: Vec<(usize, usize, Option<usize>)> = Vec::new();
        while k < clases.len() {
            let mut j = k;
            while j < clases.len() && clases[j] == clases[k] {
                j += 1;
            }
            piezas.push((desde + k, desde + j, clases[k]));
            k = j;
        }
        let entera = todos && piezas.len() == 1;
        if std::env::var_os("PIXPIN_DIAGRAMA_DEPURAR").is_some() {
            eprintln!("arco r {:.1}: todos {todos} corrida {desde}..{hasta} piezas {:?}", c.r, piezas);
        }
        for (a, b, _) in piezas {
            let cs: Vec<[f32; 3]> = (a..b).filter_map(|k| colores[k % N].map(|x| x.1)).collect();
            let color = mediana_color(&cs);
            let puntos_pieza: Vec<P> = (a..b).map(|k| c.en((k as f64 + 0.5) * TAU / N as f64)).collect();
            let grosor_p = m.ancho(&(a..b).map(|k| radial((k as f64 + 0.5) * TAU / N as f64)).collect::<Vec<_>>()).max(1.0).min(grosor * 2.5);
            let _ = &puntos_pieza;
            let inicio = a as f64 * TAU / N as f64;
            let barrido = if entera { TAU } else { (b - a) as f64 * TAU / N as f64 };
            salida.push(Arco { c: c.c, r: c.r, inicio, barrido, grosor: grosor_p, color, estilo: Estilo::Solido });
        }
    }
    salida
}

fn rellenar_huecos(v: &mut [Option<usize>]) {
    let mut ultimo = v.iter().find_map(|x| *x);
    for x in v.iter_mut() {
        match x {
            Some(c) => ultimo = Some(*c),
            None => *x = ultimo,
        }
    }
}

/// Las corridas de menos de `min` se las queda la vecina mas larga.
fn suavizar(v: &mut [Option<usize>], min: usize) {
    for _ in 0..v.len() {
        let mut corridas: Vec<(usize, usize)> = Vec::new();
        let mut k = 0;
        while k < v.len() {
            let mut j = k;
            while j < v.len() && v[j] == v[k] {
                j += 1;
            }
            corridas.push((k, j));
            k = j;
        }
        if corridas.len() <= 1 {
            return;
        }
        let mut cambio = false;
        for (ci, &(a, b)) in corridas.iter().enumerate() {
            if b - a >= min {
                continue;
            }
            let izq = ci.checked_sub(1).map(|p| corridas[p]);
            let der = corridas.get(ci + 1).copied();
            let vecina = match (izq, der) {
                (Some(l), Some(r)) => {
                    if l.1 - l.0 >= r.1 - r.0 {
                        l
                    } else {
                        r
                    }
                }
                (Some(l), None) => l,
                (None, Some(r)) => r,
                _ => continue,
            };
            let valor = v[vecina.0];
            for x in v.iter_mut().take(b).skip(a) {
                *x = valor;
            }
            cambio = true;
            break;
        }
        if !cambio {
            return;
        }
    }
}

/// Quita lo que queda de una letra pegada a una raya: segmentos cortitos
/// que no tocan nada y arcos muy pequenos.
fn limpiar_sueltos(d: &mut Diagrama, hm: f64) {
    let corto = 6f64.max(0.45 * hm);
    let extremos: Vec<P> = d.segmentos.iter().flat_map(|s| [s.a, s.b]).chain(d.arcos.iter().flat_map(|a| {
        let (p, q) = a.extremos();
        [p, q]
    })).collect();
    let toca = |p: P, yo: P, yo2: P| {
        extremos.iter().any(|&q| dist(q, p) <= 4.0 && dist(q, yo) > 0.01 && dist(q, yo2) > 0.01)
    };
    let segs = std::mem::take(&mut d.segmentos);
    let mut quedan = Vec::new();
    for s in segs {
        let l = s.largo();
        if l >= corto || toca(s.a, s.a, s.b) || toca(s.b, s.a, s.b) || d.arcos.iter().any(|a| (dist(s.a, a.c) - a.r).abs() < 3.0 || (dist(s.b, a.c) - a.r).abs() < 3.0) {
            if l >= 3.0 {
                quedan.push(s);
            }
        }
    }
    d.segmentos = quedan;
    d.arcos.retain(|a| a.r * a.barrido >= 5.0);
}

// ---------------------------------------------------------------------------
// Flechas

/// Las manchas pequenas rellenas junto a la punta de un segmento: puntas
/// de flecha. Devuelve cuales se usaron.
fn flechas(d: &mut Diagrama, pequenas: &[Vec<usize>], colores: &[[u8; 3]], w: usize, hm: f64) -> Vec<usize> {
    let mut usadas = Vec::new();
    for (k, comp) in pequenas.iter().enumerate() {
        let (x0, y0, x1, y1) = mascara::caja_de(comp, w);
        let tam = ((x1 - x0 + 1).max(y1 - y0 + 1)) as f64;
        if tam > 1.8 * hm.max(10.0) {
            continue;
        }
        let area = comp.len() as f64;
        let n = area;
        let g = (
            comp.iter().map(|&i| (i % w) as f64 + 0.5).sum::<f64>() / n,
            comp.iter().map(|&i| (i / w) as f64 + 0.5).sum::<f64>() / n,
        );
        let mut mejor: Option<(usize, bool, f64)> = None;
        for (si, s) in d.segmentos.iter().enumerate() {
            if s.largo() < 2.0 * tam {
                continue;
            }
            for fin in [true, false] {
                let (e, otro) = if fin { (s.b, s.a) } else { (s.a, s.b) };
                let de = dist(g, e);
                let dir = ((e.0 - otro.0) / s.largo(), (e.1 - otro.1) / s.largo());
                // La mancha esta delante de la punta o encima de ella, en su eje.
                let delante = (g.0 - e.0) * dir.0 + (g.1 - e.1) * dir.1;
                let lado = ((g.0 - e.0) * -dir.1 + (g.1 - e.1) * dir.0).abs();
                if de <= tam + 3.0 && delante > -tam && lado <= tam * 0.6 + 2.0 && mejor.is_none_or(|m| de < m.2) {
                    mejor = Some((si, fin, de));
                }
            }
        }
        // Un vertice (otra raya pasa por la mancha, o es un punto) no es la
        // punta de una flecha: es el nudo donde se juntan.
        let vertice = |si: usize| {
            d.segmentos.iter().enumerate().any(|(j, t)| {
                let ala = dist(t.a, g) <= tam * 1.3 + 2.0 && dist(t.b, g) <= tam * 1.3 + 2.0;
                j != si && !ala && ajuste::dist_a_segmento(g, t.a, t.b) <= tam * 0.5 + 1.5
            })
                || d.puntos.iter().any(|p| dist(p.c, g) <= p.r + tam * 0.5)
                || d.arcos.iter().any(|a| (dist(g, a.c) - a.r).abs() <= tam * 0.5 && a.r > 2.0 * tam)
        };
        // Y la punta es del color de su raya (una marca de angulo gris junto
        // a una raya azul no es su punta).
        let mismo_color = |si: usize| {
            let (a, b) = (colores[k], d.segmentos[si].color);
            let croma = |c: [u8; 3]| *c.iter().max().unwrap() as i32 - *c.iter().min().unwrap() as i32;
            let gris_oscuro = |c: [u8; 3]| croma(c) < 40 && *c.iter().max().unwrap() < 140;
            (gris_oscuro(a) && gris_oscuro(b)) || mascara::dist_color([a[0] as f32, a[1] as f32, a[2] as f32], [b[0] as f32, b[1] as f32, b[2] as f32]) < 90.0
        };
        if let Some((si, fin, _)) = mejor.filter(|m| !vertice(m.0) && mismo_color(m.0)) {
            let s = &mut d.segmentos[si];
            let (e, otro) = if fin { (s.b, s.a) } else { (s.a, s.b) };
            let l = dist(e, otro).max(1e-9);
            let dir = ((e.0 - otro.0) / l, (e.1 - otro.1) / l);
            // La punta: lo mas adelantado de la mancha y de sus bordes finos
            // (lo que no aguanto la erosion salio como rayitas, que sobran).
            let alcance = tam * 1.3 + 2.0;
            let alas: Vec<usize> = (0..d.segmentos.len()).filter(|&j| j != si && dist(d.segmentos[j].a, g) <= alcance && dist(d.segmentos[j].b, g) <= alcance).collect();
            let proy = |p: P| (p.0 - otro.0) * dir.0 + (p.1 - otro.1) * dir.1;
            let mut mas = comp.iter().map(|&i| proy(((i % w) as f64 + 0.5, (i / w) as f64 + 0.5))).fold(f64::MIN, f64::max);
            for &j in &alas {
                mas = mas.max(proy(d.segmentos[j].a)).max(proy(d.segmentos[j].b));
            }
            // Y la puntita fina suelta, que salio como «letra».
            let sueltas: Vec<usize> = (0..d.glifos.len()).filter(|&j| dist((d.glifos[j].caja.cx(), d.glifos[j].caja.cy()), g) <= alcance).collect();
            for &j in &sueltas {
                for p in d.glifos[j].trazos.iter().flatten() {
                    mas = mas.max(proy(*p));
                }
            }
            for &j in sueltas.iter().rev() {
                d.glifos.remove(j);
            }
            let punta = (otro.0 + dir.0 * mas, otro.1 + dir.1 * mas);
            let s = &mut d.segmentos[si];
            if fin {
                s.b = punta;
                s.flecha_b = true;
            } else {
                s.a = punta;
                s.flecha_a = true;
            }
            usadas.push(k);
            let mut j = 0;
            d.segmentos.retain(|_| {
                j += 1;
                !alas.contains(&(j - 1))
            });
        }
    }
    usadas
}

/// Las puntas abiertas (dos rayitas en V en el extremo de un segmento):
/// se quitan las rayitas y el segmento pasa a flecha.
fn flechas_abiertas(d: &mut Diagrama, hm: f64) {
    let corto = 1.3 * hm.max(10.0);
    let n = d.segmentos.len();
    let mut quitar = vec![false; n];
    for i in 0..n {
        let s = d.segmentos[i].clone();
        if s.largo() < 2.0 * corto {
            continue;
        }
        for fin in [true, false] {
            let (e, otro) = if fin { (s.b, s.a) } else { (s.a, s.b) };
            let dir = ((e.0 - otro.0) / s.largo(), (e.1 - otro.1) / s.largo());
            let mut alas = Vec::new();
            for j in 0..n {
                if j == i || quitar[j] {
                    continue;
                }
                let t = &d.segmentos[j];
                // (Un ala puede ser larga si la raya lo es: las del diagrama de flujo.)
                if t.largo() > corto.max(0.45 * s.largo()).min(2.5 * hm.max(10.0)) || t.largo() < 3.0 {
                    continue;
                }
                let (cerca, lejos) = if dist(t.a, e) < dist(t.b, e) { (t.a, t.b) } else { (t.b, t.a) };
                if dist(cerca, e) > 4f64.max(0.8 * hm) {
                    continue;
                }
                // El ala vuelve hacia atras, abierta 15-60 grados.
                let v = ((lejos.0 - cerca.0) / t.largo(), (lejos.1 - cerca.1) / t.largo());
                let cos = -(v.0 * dir.0 + v.1 * dir.1);
                if (0.5..=0.97).contains(&cos) {
                    alas.push(j);
                }
            }
            if alas.len() >= 2 {
                for j in alas {
                    quitar[j] = true;
                }
                d.puntas_abiertas.push(e);
                if fin {
                    d.segmentos[i].flecha_b = true;
                } else {
                    d.segmentos[i].flecha_a = true;
                }
            }
        }
    }
    let mut k = 0;
    d.segmentos.retain(|_| {
        k += 1;
        !quitar[k - 1]
    });
}

// ---------------------------------------------------------------------------
// Topologia

/// **Que las rectas acaben donde acaban**: en los puntos, sobre la
/// circunferencia y en el cruce con otra recta.
fn topologia(d: &mut Diagrama, hm: f64) {
    let n = d.segmentos.len();
    let rectas: Vec<(P, P)> = d.segmentos.iter().map(|s| (s.a, s.b)).collect();
    for i in 0..n {
        for fin in [false, true] {
            let s = &d.segmentos[i];
            if (fin && s.flecha_b) || (!fin && s.flecha_a) {
                continue;
            }
            let e = if fin { s.b } else { s.a };
            let otro = if fin { s.a } else { s.b };
            let tol = s.grosor / 2.0 + 4.0;
            let mut nuevo: Option<P> = None;
            // 1. Un punto.
            // (O uno un poco mas alla, en la prolongacion de la recta: el
            // esqueleto se pierde donde la recta llega al punto junto con
            // otras rayas.)
            let l = dist(e, otro).max(1e-9);
            let dir = ((e.0 - otro.0) / l, (e.1 - otro.1) / l);
            let alcance = 12f64.max(3.0 * hm);
            let en_su_recta = |p: &Punto| {
                let (vx, vy) = (p.c.0 - e.0, p.c.1 - e.1);
                let delante = vx * dir.0 + vy * dir.1;
                let lado = (vx * -dir.1 + vy * dir.0).abs();
                delante > -(p.r + tol) && delante <= alcance && lado <= p.r + s.grosor / 2.0 + 1.5
            };
            if let Some(p) = d.puntos.iter().filter(|p| dist(p.c, e) <= p.r + tol || en_su_recta(p)).min_by(|a, b| dist(a.c, e).total_cmp(&dist(b.c, e))) {
                nuevo = Some(p.c);
            }
            // 2. El cruce con otra recta (esquina o T).
            if nuevo.is_none() {
                let mut mejor: Option<(P, f64)> = None;
                for (j, &(a, b)) in rectas.iter().enumerate() {
                    if j == i {
                        continue;
                    }
                    if ajuste::dist_a_segmento(e, a, b) > tol + 2.0 {
                        continue;
                    }
                    if let Some(x) = cruce((otro, e), (a, b))
                        && dist(x, e) <= tol + 4.0
                        && mejor.is_none_or(|m| dist(x, e) < m.1)
                    {
                        mejor = Some((x, dist(x, e)));
                    }
                }
                nuevo = mejor.map(|m| m.0);
            }
            // 3. La circunferencia.
            if nuevo.is_none()
                && let Some(a) = d.arcos.iter().filter(|a| (dist(e, a.c) - a.r).abs() <= tol).min_by(|x, y| (dist(e, x.c) - x.r).abs().total_cmp(&(dist(e, y.c) - y.r).abs()))
            {
                let l = dist(e, a.c).max(1e-9);
                nuevo = Some((a.c.0 + (e.0 - a.c.0) / l * a.r, a.c.1 + (e.1 - a.c.1) / l * a.r));
            }
            if let Some(x) = nuevo {
                let s = &mut d.segmentos[i];
                if fin {
                    s.b = x;
                } else {
                    s.a = x;
                }
            }
        }
    }
    // Los extremos de los arcos, a los puntos.
    for a in &mut d.arcos {
        if a.entera() {
            continue;
        }
        let (p, q) = a.extremos();
        let tol = a.grosor / 2.0 + 5.0;
        let fin_ang = a.inicio + a.barrido;
        let mut ini = a.inicio;
        let mut fin = fin_ang;
        if let Some(pt) = d.puntos.iter().filter(|x| dist(x.c, p) <= x.r + tol).min_by(|x, y| dist(x.c, p).total_cmp(&dist(y.c, p))) {
            ini = (pt.c.1 - a.c.1).atan2(pt.c.0 - a.c.0);
        }
        if let Some(pt) = d.puntos.iter().filter(|x| dist(x.c, q) <= x.r + tol).min_by(|x, y| dist(x.c, q).total_cmp(&dist(y.c, q))) {
            fin = (pt.c.1 - a.c.1).atan2(pt.c.0 - a.c.0);
        }
        // Mismo sentido y cerca del barrido de antes.
        let mut barrido = (fin - ini).rem_euclid(TAU);
        if (barrido - a.barrido).abs() > PI {
            barrido = a.barrido;
            ini = a.inicio;
        }
        if barrido > 1e-3 {
            a.inicio = ini.rem_euclid(TAU);
            a.barrido = barrido;
        }
    }
}

/// El cruce de las rectas (no segmentos) `a` y `b`; `None` si casi paralelas.
fn cruce(a: (P, P), b: (P, P)) -> Option<P> {
    let (d1, d2) = ((a.1.0 - a.0.0, a.1.1 - a.0.1), (b.1.0 - b.0.0, b.1.1 - b.0.1));
    let den = d1.0 * d2.1 - d1.1 * d2.0;
    let l = (d1.0.hypot(d1.1) * d2.0.hypot(d2.1)).max(1e-9);
    if (den / l).abs() < 10f64.to_radians().sin() {
        return None;
    }
    let t = ((b.0.0 - a.0.0) * d2.1 - (b.0.1 - a.0.1) * d2.0) / den;
    Some((a.0.0 + t * d1.0, a.0.1 + t * d1.1))
}

/// Cuatro segmentos que cierran un rectangulo (horizontal) son un recuadro.
fn recuadros(d: &mut Diagrama) {
    let tol = 5.0;
    let es_h = |s: &Segmento| (s.a.1 - s.b.1).abs() <= 1.5 && s.largo() >= 8.0;
    let es_v = |s: &Segmento| (s.a.0 - s.b.0).abs() <= 1.5 && s.largo() >= 8.0;
    let rango_x = |s: &Segmento| (s.a.0.min(s.b.0), s.a.0.max(s.b.0), (s.a.1 + s.b.1) / 2.0);
    let rango_y = |s: &Segmento| (s.a.1.min(s.b.1), s.a.1.max(s.b.1), (s.a.0 + s.b.0) / 2.0);
    let n = d.segmentos.len();
    let mut usado = vec![false; n];
    let mut nuevos = Vec::new();
    for i in 0..n {
        if usado[i] || !es_h(&d.segmentos[i]) || d.segmentos[i].estilo != Estilo::Solido {
            continue;
        }
        let (ax0, ax1, ay) = rango_x(&d.segmentos[i]);
        for j in 0..n {
            if j == i || usado[j] || usado[i] || !es_h(&d.segmentos[j]) {
                continue;
            }
            let (bx0, bx1, by) = rango_x(&d.segmentos[j]);
            if by <= ay + 6.0 || (ax0 - bx0).abs() > tol || (ax1 - bx1).abs() > tol {
                continue;
            }
            let (x0, x1) = ((ax0 + bx0) / 2.0, (ax1 + bx1) / 2.0);
            let lado = |x: f64, usado: &Vec<bool>| {
                (0..n).find(|&k| {
                    k != i && k != j && !usado[k] && es_v(&d.segmentos[k]) && {
                        let (y0, y1, xv) = rango_y(&d.segmentos[k]);
                        (xv - x).abs() <= tol && (y0 - ay).abs() <= tol && (y1 - by).abs() <= tol
                    }
                })
            };
            if let (Some(l), Some(r)) = (lado(x0, &usado), lado(x1, &usado))
                && l != r
            {
                let s = &d.segmentos;
                let grosor = (s[i].grosor + s[j].grosor + s[l].grosor + s[r].grosor) / 4.0;
                nuevos.push(Recuadro { x0, y0: ay, x1, y1: by, grosor, color: s[i].color, relleno: None });
                for k in [i, j, l, r] {
                    usado[k] = true;
                }
            }
        }
    }
    let mut k = 0;
    d.segmentos.retain(|_| {
        k += 1;
        !usado[k - 1]
    });
    d.recuadros.extend(nuevos);
}

// ---------------------------------------------------------------------------
// Regiones

fn dentro(pol: &[P], (px, py): P) -> bool {
    let mut c = false;
    let n = pol.len();
    let mut j = n.wrapping_sub(1);
    for i in 0..n {
        let (xi, yi) = pol[i];
        let (xj, yj) = pol[j];
        if (yi > py) != (yj > py) && px < (xj - xi) * (py - yi) / (yj - yi) + xi {
            c = !c;
        }
        j = i;
    }
    c
}

/// **Las regiones de color**: clase a clase, cerradas (sin las rendijas
/// por donde las cruzaba una raya), con su color medio. Un agujero tapado
/// por otro relleno no se corta (las bandas de un degradado no dejan
/// rendijas: la de dentro se pinta encima).
fn regiones(m: &Mapas, region: &[bool], hm: f64) -> Vec<Region> {
    let (w, h) = (m.w, m.h);
    let mut salida = Vec::new();
    for k in 0..m.centros.len() {
        let mk: Vec<bool> = (0..w * h).map(|i| region[i] && m.etiqueta[i] == k as u8).collect();
        let n = mk.iter().filter(|&&b| b).count();
        if n < 25 {
            continue;
        }
        let cerrada = en_su_caja(&mk, w, h, 4, |sub, bw, bh| {
            let g = mascara::dilatar(sub, bw, bh, 2.2);
            mascara::erosionar(&g, bw, bh, 2.2)
        });
        let color = a_u8(mascara::media((0..w * h).filter(|&i| mk[i]).map(|i| m.img.px[i])));
        for (contorno, huecos) in crate::calcar::regiones(&cerrada, w, h, 20.0) {
            let contorno = crate::calcar::curva(&contorno, true, 0.6, 0.3);
            if contorno.len() < 3 {
                continue;
            }
            let huecos: Vec<Vec<P>> = huecos
                .into_iter()
                .filter(|hq| {
                    // Lo que hay dentro del agujero: si es relleno de otro color, no se corta.
                    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
                    for &(x, y) in hq {
                        x0 = x0.min(x);
                        y0 = y0.min(y);
                        x1 = x1.max(x);
                        y1 = y1.max(y);
                    }
                    let (mut dentro_n, mut relleno_n) = (0usize, 0usize);
                    let paso = ((x1 - x0).max(y1 - y0) / 30.0).max(1.0);
                    let mut y = y0;
                    while y <= y1 {
                        let mut x = x0;
                        while x <= x1 {
                            if dentro(hq, (x, y)) {
                                dentro_n += 1;
                                if m.i((x, y)).is_some_and(|i| region[i]) {
                                    relleno_n += 1;
                                }
                            }
                            x += paso;
                        }
                        y += paso;
                    }
                    dentro_n > 0 && (relleno_n as f64) < 0.4 * dentro_n as f64
                })
                .map(|hq| crate::calcar::curva(&hq, true, 0.6, 0.3))
                // El agujero de una letra (la X sobre el relleno) no: la letra va encima.
                .filter(|hq| hq.len() >= 3 && crate::calcar::area(hq).abs() >= 9.0 * hm * hm)
                .collect();
            let area = crate::calcar::area(&contorno).abs();
            // Casi del color del papel (el borde blanco de un degradado): nada.
            if mascara::dist_color([color[0] as f32, color[1] as f32, color[2] as f32], m.papel) < 25.0 {
                continue;
            }
            salida.push(Region { contorno, huecos, color, area });
        }
    }
    salida.sort_by(|a, b| b.area.total_cmp(&a.area));
    salida
}

// ---------------------------------------------------------------------------
// Lo explicado

fn pintar_disco(mk: &mut [bool], w: usize, h: usize, c: P, r: f64) {
    let ri = r.ceil() as i64;
    let (cx, cy) = (c.0.floor() as i64, c.1.floor() as i64);
    for dy in -ri..=ri {
        for dx in -ri..=ri {
            if ((dx * dx + dy * dy) as f64) > r * r + 0.5 {
                continue;
            }
            let (x, y) = (cx + dx, cy + dy);
            if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h {
                mk[y as usize * w + x as usize] = true;
            }
        }
    }
}

fn pintar_trazo(mk: &mut [bool], w: usize, h: usize, a: P, b: P, r: f64) {
    let n = (dist(a, b).ceil() as usize).max(1);
    for k in 0..=n {
        let t = k as f64 / n as f64;
        pintar_disco(mk, w, h, (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t), r);
    }
}

/// Que parte de la tinta (sin el texto) cae sobre lo que se dibujara.
fn explicado(m: &Mapas, d: &Diagrama, region: &[bool], glifos: &[Vec<usize>]) -> f64 {
    let (w, h) = (m.w, m.h);
    let mut mk = vec![false; w * h];
    for s in &d.segmentos {
        pintar_trazo(&mut mk, w, h, s.a, s.b, s.grosor / 2.0 + 2.5);
    }
    for a in &d.arcos {
        let n = ((a.r * a.barrido).ceil() as usize).max(2);
        let mut prev = None;
        for k in 0..=n {
            let t = a.inicio + a.barrido * k as f64 / n as f64;
            let p = (a.c.0 + a.r * t.cos(), a.c.1 + a.r * t.sin());
            if let Some(q) = prev {
                pintar_trazo(&mut mk, w, h, q, p, a.grosor / 2.0 + 2.5);
            }
            prev = Some(p);
        }
    }
    for r in &d.recuadros {
        let c = [(r.x0, r.y0), (r.x1, r.y0), (r.x1, r.y1), (r.x0, r.y1), (r.x0, r.y0)];
        for k in 0..4 {
            pintar_trazo(&mut mk, w, h, c[k], c[k + 1], r.grosor / 2.0 + 2.5);
        }
    }
    for p in &d.puntos {
        pintar_disco(&mut mk, w, h, p.c, p.r + 2.5);
    }
    for comp in glifos {
        for &i in comp {
            mk[i] = true;
        }
    }
    let (mut total, mut si) = (0usize, 0usize);
    for i in 0..w * h {
        if !m.tinta[i] || m.texto[i] {
            continue;
        }
        total += 1;
        if mk[i] || region[i] {
            si += 1;
        }
    }
    if total == 0 { 0.0 } else { si as f64 / total as f64 }
}

// ---------------------------------------------------------------------------
// Letras sueltas

/// **Lee las letras sueltas** de una vez: se ponen en fila, grandes y en
/// negro sobre blanco, en una hoja aparte, y se le pasa al lector; lo que
/// lee cada celda es la letra de ese glifo.
pub(super) fn leer_glifos(d: &mut Diagrama, img: &Img, lector: &Lector, o: &Opciones) {
    if d.glifos.is_empty() {
        return;
    }
    let papel = mascara::fondo(img);
    let alto_celda = 64.0f64;
    // Mucho aire entre letras: si no, el OCR las lee como una palabra.
    let hueco = 2.2 * alto_celda;
    let ancho_max = 1800.0f64;
    // (glifo, x, y, escala) en la hoja.
    let mut sitios: Vec<(usize, f64, f64, f64)> = Vec::new();
    let (mut x, mut y, mut alto_fila) = (hueco, hueco, 0.0f64);
    for (k, g) in d.glifos.iter().enumerate().take(80) {
        let esc = (alto_celda * 0.7 / g.caja.h.max(1.0)).clamp(1.0, 8.0);
        let (gw, gh) = (g.caja.w * esc, g.caja.h * esc);
        if x + gw + hueco > ancho_max {
            x = hueco;
            y += alto_fila + hueco;
            alto_fila = 0.0;
        }
        sitios.push((k, x, y, esc));
        x += gw + hueco;
        alto_fila = alto_fila.max(gh);
    }
    let (sw, sh) = (ancho_max as usize, (y + alto_fila + hueco).ceil() as usize);
    let mut hoja = vec![255u8; sw * sh * 4];
    for &(k, ox, oy, esc) in &sitios {
        let g = &d.glifos[k];
        let (gw, gh) = ((g.caja.w * esc).ceil() as usize, (g.caja.h * esc).ceil() as usize);
        // Solo sus pixeles (y su antialias): lo de alrededor (un fondo de
        // color, otra raya) no es la letra.
        let suyos: std::collections::HashSet<(i64, i64)> = g
            .pixeles
            .iter()
            .flat_map(|&(x, y)| (-1..=1).flat_map(move |dy| (-1..=1).map(move |dx| (x as i64 + dx, y as i64 + dy))))
            .collect();
        for yy in 0..gh {
            for xx in 0..gw {
                let (sx, sy) = (g.caja.x + (xx as f64 + 0.5) / esc, g.caja.y + (yy as f64 + 0.5) / esc);
                let (ix, iy) = (sx.floor() as i64, sy.floor() as i64);
                if !suyos.contains(&(ix, iy)) {
                    continue;
                }
                let Some(c) = img.en(ix, iy) else { continue };
                let tinta = (mascara::dist_color(c, papel) / 160.0).clamp(0.0, 1.0);
                let v = (255.0 * (1.0 - tinta)) as u8;
                let (px, py) = (ox as usize + xx, oy as usize + yy);
                if px < sw && py < sh {
                    let i = (py * sw + px) * 4;
                    hoja[i] = v;
                    hoja[i + 1] = v;
                    hoja[i + 2] = v;
                }
            }
        }
    }
    let leidas = lector(&hoja, sw as u32, sh as u32);
    let mut textos: Vec<String> = vec![String::new(); d.glifos.len()];
    for p in leidas {
        // A que celdas cae la palabra: si cubre varias con tantas letras como celdas, una a cada una.
        let celdas: Vec<usize> = sitios
            .iter()
            .filter(|&&(k, ox, oy, esc)| {
                let g = &d.glifos[k];
                let c = Caja::nueva(ox - hueco * 0.4, oy - hueco * 0.4, g.caja.w * esc + hueco * 0.8, g.caja.h * esc + hueco * 0.8);
                c.solape_x(&p.caja) > 0.0 && c.solape_y(&p.caja) > 0.0
            })
            .map(|s| s.0)
            .collect();
        let letras: Vec<char> = p.texto.chars().filter(|c| !c.is_whitespace()).collect();
        if celdas.len() == 1 {
            textos[celdas[0]].push_str(&p.texto);
        } else if celdas.len() > 1 && letras.len() == celdas.len() {
            for (c, l) in celdas.iter().zip(letras) {
                textos[*c].push(l);
            }
        }
    }
    let glifos = std::mem::take(&mut d.glifos);
    for (g, t) in glifos.into_iter().zip(textos) {
        let t = t.trim().to_string();
        let valido = !t.is_empty() && t.chars().count() <= 4 && t.chars().all(|c| c.is_alphanumeric() || "°'´θα β()+-=.,∢".contains(c));
        if valido {
            d.rotulos.push(Rotulo { texto: t, caja: g.caja, color: Some(g.color), negrita: g.grosor >= 0.2 * o.alto_letra.max(6.0), letra_suelta: true });
        } else {
            d.glifos.push(g);
        }
    }
}

/// Lo que dijo la nube de los rotulos: corrige al vecino o lee el glifo; lo
/// que no cae cerca de nada (una «a» sobre un sector de color, que se fue
/// con la region) se anade en su sitio, del alto de las otras letras y con
/// el color de la tinta de alli (si hay imagen).
pub(super) fn corregir_rotulos(d: &mut Diagrama, de_la_nube: &[(String, f64, f64)], hm: f64, img: Option<&Img>) {
    let (w, h) = (d.ancho as f64, d.alto as f64);
    // El alto de las letras sueltas que ya hay (el de una nueva).
    let mut altos: Vec<f64> = d.rotulos.iter().filter(|r| r.letra_suelta).map(|r| r.caja.h).collect();
    altos.sort_by(f64::total_cmp);
    let alto_letra = altos.get(altos.len() / 2).copied().unwrap_or(0.75 * hm).max(6.0);
    for (t, fx, fy) in de_la_nube {
        let t = t.trim();
        if t.is_empty() || t.chars().count() > 12 || t.contains(['?', '¿']) {
            continue;
        }
        let p = (fx * w, fy * h);
        let alcance = 1.6 * hm;
        // Un rotulo que ya esta: si dice casi lo mismo se queda (el OCR lee la
        // posicion exacta); si la nube dice otra cosa corta (una letra), gana.
        let r = (0..d.rotulos.len())
            .filter(|&k| dist((d.rotulos[k].caja.cx(), d.rotulos[k].caja.cy()), p) <= alcance + d.rotulos[k].caja.w.max(d.rotulos[k].caja.h) / 2.0)
            .min_by(|&a, &b| dist((d.rotulos[a].caja.cx(), d.rotulos[a].caja.cy()), p).total_cmp(&dist((d.rotulos[b].caja.cx(), d.rotulos[b].caja.cy()), p)));
        if let Some(k) = r {
            let r = &mut d.rotulos[k];
            // (O uno largo que la nube lee igual pero bien: «iN mod 2 O?» -> «¿N mod 2 = 0?».)
            let corto_y_corto = r.texto.chars().count() <= 3 && t.chars().count() <= 8;
            let mismo_largo = !corto_y_corto && se_parecen(&r.texto, t);
            if (corto_y_corto || mismo_largo) && r.texto != t {
                // Lo que crece el texto («2» -> «2a», «e» -> «θ=45°»): la caja
                // tambien (del alto de las letras), y lo que tapa ya es de el
                // (glifos y rotulos sueltos mal leidos, que se pintaban encima).
                let n1 = t.chars().count() as f64;
                let ancho = (alto_letra * 0.6 * n1).max(r.caja.w);
                if ancho > r.caja.w {
                    r.caja = Caja::nueva(r.caja.x, r.caja.y, ancho, r.caja.h);
                }
                r.texto = t.to_string();
                let c = r.caja.crecer(2.0);
                d.glifos.retain(|g| g.caja.comun(&c) <= 0.3 * g.caja.area().max(1.0));
                let mut j = 0;
                d.rotulos.retain(|o| {
                    j += 1;
                    j - 1 == k || o.texto.chars().count() > 2 || !c.contiene(o.caja.cx(), o.caja.cy())
                });
            }
            continue;
        }
        if let Some(k) = (0..d.glifos.len())
            .filter(|&k| dist((d.glifos[k].caja.cx(), d.glifos[k].caja.cy()), p) <= alcance + d.glifos[k].caja.w.max(d.glifos[k].caja.h) / 2.0)
            .min_by(|&a, &b| dist((d.glifos[a].caja.cx(), d.glifos[a].caja.cy()), p).total_cmp(&dist((d.glifos[b].caja.cx(), d.glifos[b].caja.cy()), p)))
        {
            let g = d.glifos.remove(k);
            // Los glifos de al lado son del mismo rotulo («45°» en tres trozos).
            let mut caja = g.caja;
            let ancho_t = alto_letra * 0.62 * t.chars().count() as f64;
            let zona = Caja::nueva(caja.x - ancho_t, caja.y - 0.3 * alto_letra, caja.w + 2.0 * ancho_t, caja.h + 0.6 * alto_letra);
            let mut k = 0;
            while k < d.glifos.len() {
                if t.chars().count() > 1 && d.glifos[k].caja.comun(&zona) > 0.5 * d.glifos[k].caja.area() {
                    let o = d.glifos.remove(k);
                    caja = Caja::de_esquinas(caja.x.min(o.caja.x), caja.y.min(o.caja.y), caja.x1().max(o.caja.x1()), caja.y1().max(o.caja.y1()));
                } else {
                    k += 1;
                }
            }
            d.rotulos.push(Rotulo { texto: t.to_string(), caja, color: Some(g.color), negrita: false, letra_suelta: true });
            continue;
        }
        // Nada cerca: se anade, si no pisa otro rotulo ni cae fuera.
        if !(0.0..=1.0).contains(fx) || !(0.0..=1.0).contains(fy) {
            continue;
        }
        let ancho = alto_letra * 0.62 * t.chars().count() as f64;
        let caja = Caja::nueva(p.0 - ancho / 2.0, p.1 - alto_letra / 2.0, ancho, alto_letra);
        if d.rotulos.iter().any(|r| r.caja.comun(&caja) > 0.25 * caja.area()) {
            continue;
        }
        // Solo un rotulo de verdad (corto, sin dudas) y donde hay tinta: la
        // nube a veces inventa («2a=?» sobre un recuadro vacio).
        if t.chars().count() > 5 || t.contains(['?', '¿']) {
            continue;
        }
        let color = match img {
            Some(i) => match color_de_tinta(i, &caja) {
                Some(c) => Some(c),
                None => continue,
            },
            None => None,
        };
        d.rotulos.push(Rotulo { texto: t.to_string(), caja, color, negrita: false, letra_suelta: true });
    }
}

/// El color de la tinta de una caja (lo mas entintado: la letra, no el
/// sector de color sobre el que va), vivo si es de color.
fn color_de_tinta(img: &Img, caja: &Caja) -> Option<[u8; 3]> {
    let papel = [255f32, 255.0, 255.0];
    let mut v: Vec<(f32, [f32; 3])> = Vec::new();
    let c = caja.crecer(2.0);
    for y in c.y.floor() as i64..c.y1().ceil() as i64 {
        for x in c.x.floor() as i64..c.x1().ceil() as i64 {
            if let Some(p) = img.en(x, y) {
                v.push((mascara::dist_color(p, papel), p));
            }
        }
    }
    if v.len() < 4 {
        return None;
    }
    v.sort_by(|a, b| b.0.total_cmp(&a.0));
    if v[0].0 < 60.0 {
        return None;
    }
    v.truncate((v.len() / 8).max(2));
    let m = mascara::media(v.iter().map(|x| x.1));
    Some(vivo(a_u8(m), [255, 255, 255]))
}

/// **El color de verdad de un trazo fino**: en una captura reducida, el
/// centro de una raya roja de 1 px queda marron (mezclado con el papel y
/// con lo que la reduccion oscurece) y el borde rosa. El tono esta en lo
/// que se aparta del fondo; se lleva en esa direccion hasta un color pleno.
/// Lo gris (negro, gris de una punteada) se queda como esta.
pub(super) fn vivo(c: [u8; 3], fondo: [u8; 3]) -> [u8; 3] {
    let d: Vec<f64> = (0..3).map(|q| c[q] as f64 - fondo[q] as f64).collect();
    let m = d.iter().map(|x| x.abs()).fold(0.0, f64::max);
    if m < 20.0 {
        return c;
    }
    let u: Vec<f64> = d.iter().map(|x| x / m).collect();
    // Gris o negro sobre un fondo de color: el trazo sale del tono del fondo
    // (oscurecido) y no de otro. Una punteada gris sobre el amarillo salia
    // oliva.
    let tono = |x: [u8; 3]| -> Option<f64> {
        let (r, g, b) = (x[0] as f64, x[1] as f64, x[2] as f64);
        let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
        if mx < 1.0 || (mx - mn) / mx < 0.15 {
            return None;
        }
        let h = if mx == r {
            ((g - b) / (mx - mn)).rem_euclid(6.0)
        } else if mx == g {
            (b - r) / (mx - mn) + 2.0
        } else {
            (r - g) / (mx - mn) + 4.0
        };
        Some(h * 60.0)
    };
    if let (Some(tc), Some(tf)) = (tono(c), tono(fondo)) {
        let dt = (tc - tf).rem_euclid(360.0);
        let lum = |x: [u8; 3]| 0.3 * x[0] as f64 + 0.59 * x[1] as f64 + 0.11 * x[2] as f64;
        // (Solo lo oscuro: una raya roja fina sobre el amarillo tambien tira a
        // su tono, pero es mas clara.)
        if dt.min(360.0 - dt) < 20.0 && lum(c) < 0.5 * lum(fondo) {
            let g = (255.0 * lum(c) / lum(fondo).max(1.0)).round().clamp(0.0, 255.0) as u8;
            return [g, g, g];
        }
    }
    // Lo pleno ya es su color.
    {
        let (mx, mn) = (*c.iter().max().unwrap() as f64, *c.iter().min().unwrap() as f64);
        if mx >= 1.0 && (mx - mn) / mx >= 0.6 {
            return c;
        }
    }
    // Gris o negro: oscurece lo mismo, en proporcion, cada canal del fondo
    // (una punteada gris sobre el amarillo salia oliva): un gris.
    if (0..3).all(|q| c[q] as f64 <= fondo[q] as f64 + 5.0) {
        let osc: Vec<f64> = (0..3).map(|q| 1.0 - c[q] as f64 / (fondo[q] as f64).max(1.0)).collect();
        let (omax, omin) = (osc.iter().copied().fold(f64::MIN, f64::max), osc.iter().copied().fold(f64::MAX, f64::min));
        if omax > 0.0 && (omax - omin) / omax < 0.1 {
            let k = 1.0 - osc.iter().sum::<f64>() / 3.0;
            let g = (255.0 * k).round().clamp(0.0, 255.0) as u8;
            return [g, g, g];
        }
    }
    // De color de verdad: unos canales se apartan del fondo mas que otros.
    let (umax, umin) = (u.iter().copied().fold(f64::MIN, f64::max), u.iter().copied().fold(f64::MAX, f64::min));
    if umax - umin < 0.2 {
        return c;
    }
    // Hasta donde se puede ir antes de que un canal se salga (el 90 %).
    let mut t = f64::MAX;
    for q in 0..3 {
        if u[q].abs() > 0.05 {
            let sitio = if u[q] < 0.0 { fondo[q] as f64 } else { 255.0 - fondo[q] as f64 };
            t = t.min(sitio / u[q].abs());
        }
    }
    let t = (0.9 * t).max(m);
    let mut out = [0u8; 3];
    for q in 0..3 {
        out[q] = (fondo[q] as f64 + u[q] * t).round().clamp(0.0, 255.0) as u8;
    }
    // Y luminoso: lo que la reduccion oscurecio no es del color (el rojo
    // fino salia granate).
    let mx = *out.iter().max().unwrap_or(&0) as f64;
    if mx > 1.0 && mx < 190.0 {
        for o in out.iter_mut() {
            *o = (*o as f64 * 190.0 / mx).round().min(255.0) as u8;
        }
    }
    out
}

/// Dos segmentos en la misma recta que se tocan (o casi) son uno: al
/// juntar tramos por cruces quedan trocitos sueltos junto a un vertice.
fn juntar_colineales(d: &mut Diagrama) {
    loop {
        let mut junto = false;
        'fuera: for i in 0..d.segmentos.len() {
            for j in i + 1..d.segmentos.len() {
                let (a, b) = (&d.segmentos[i], &d.segmentos[j]);
                let discontinuas = a.estilo != Estilo::Solido && b.estilo != Estilo::Solido;
                if a.estilo != b.estilo && !discontinuas {
                    continue;
                }
                let (la, lb) = (a.largo().max(1e-9), b.largo().max(1e-9));
                let da = ((a.b.0 - a.a.0) / la, (a.b.1 - a.a.1) / la);
                let db = ((b.b.0 - b.a.0) / lb, (b.b.1 - b.a.1) / lb);
                if (da.0 * db.1 - da.1 * db.0).abs() > 3f64.to_radians().sin() {
                    continue;
                }
                let lado = |p: P| ((p.0 - a.a.0) * -da.1 + (p.1 - a.a.1) * da.0).abs();
                if lado(b.a).max(lado(b.b)) > 2.0 + a.grosor.max(b.grosor) / 2.0 {
                    continue;
                }
                let t = |p: P| (p.0 - a.a.0) * da.0 + (p.1 - a.a.1) * da.1;
                let (u0, u1) = (t(b.a).min(t(b.b)), t(b.a).max(t(b.b)));
                let hueco = (u0 - la).max(-u1).max(0.0);
                // Una discontinua se corta donde la cruza otra raya o un punto:
                // sus trozos se juntan con mas hueco.
                if hueco > if discontinuas { 40.0 } else { 3.0 } {
                    continue;
                }
                // Los extremos: los mas alejados (manteniendo los que ya
                // estaban pegados a algo, que son los de fuera).
                let pts = [(a.a, a.flecha_a), (a.b, a.flecha_b), (b.a, b.flecha_a), (b.b, b.flecha_b)];
                let (mut pmin, mut pmax) = (pts[0], pts[0]);
                for &p in &pts {
                    if t(p.0) < t(pmin.0) {
                        pmin = p;
                    }
                    if t(p.0) > t(pmax.0) {
                        pmax = p;
                    }
                }
                let grosor = (a.grosor * la + b.grosor * lb) / (la + lb);
                let color = if la >= lb { a.color } else { b.color };
                let estilo = if la >= lb { a.estilo } else { b.estilo };
                d.segmentos[i] = Segmento { a: pmin.0, b: pmax.0, grosor, color, estilo, flecha_a: pmin.1, flecha_b: pmax.1 };
                d.segmentos.remove(j);
                junto = true;
                break 'fuera;
            }
        }
        if !junto {
            break;
        }
    }
}

/// Dos grupos que son la misma circunferencia (el primero se ajusto con un
/// trozo corto y quedo un poco movido): si juntos se ajustan bien, uno.
fn fundir_circunferencias(gs: &mut Vec<GrupoCirc>) {
    loop {
        let mut junto = false;
        'fuera: for i in 0..gs.len() {
            for j in i + 1..gs.len() {
                let (a, b) = (&gs[i].circ, &gs[j].circ);
                let r = a.r.max(b.r);
                if dist(a.c, b.c) > 0.15 * r || (a.r - b.r).abs() > 0.12 * r {
                    continue;
                }
                let mut todos = gs[i].todos();
                todos.extend(gs[j].todos());
                let Some(c) = ajuste::circunferencia(&todos) else { continue };
                if encaja(&c, &todos) {
                    let g = gs.remove(j);
                    gs[i].tramos.extend(g.tramos);
                    gs[i].circ = c;
                    junto = true;
                    break 'fuera;
                }
            }
        }
        if !junto {
            break;
        }
    }
}

// ---------------------------------------------------------------------------
// Discontinuas y punteadas

/// Un trozo suelto de trazo: un guion (con su direccion) o una mota.
struct Guion {
    c: P,
    d: Option<P>,
    l: f64,
    px: Vec<usize>,
}

/// **Encadena los guiones** alineados y seguidos (cada uno en la
/// prolongacion del anterior, con huecos parecidos): cada cadena de 3 o mas
/// es una discontinua. Devuelve las cadenas y los guiones que no entraron
/// en ninguna (vuelven al trazo: una marca suelta).
fn encadenar(g: &[Guion], hm: f64) -> (Vec<Vec<usize>>, Vec<usize>) {
    let n = g.len();
    let mut usado = vec![false; n];
    let mut cadenas = Vec::new();
    let max_hueco = 12f64.max(0.9 * hm);
    let mut orden: Vec<usize> = (0..n).collect();
    orden.sort_by(|a, b| g[*b].l.total_cmp(&g[*a].l));
    // El siguiente desde `e` en la direccion `dir`.
    let siguiente = |e: usize, dir: P, en: &[usize], usado: &[bool]| -> Option<usize> {
        let mut mejor: Option<(usize, f64)> = None;
        for j in 0..n {
            if usado[j] || en.contains(&j) {
                continue;
            }
            let v = (g[j].c.0 - g[e].c.0, g[j].c.1 - g[e].c.1);
            let a = v.0 * dir.0 + v.1 * dir.1;
            let lado = (v.0 * -dir.1 + v.1 * dir.0).abs();
            if a <= 0.5 || a > g[e].l / 2.0 + g[j].l / 2.0 + max_hueco || lado > 2.0 + 0.06 * a {
                continue;
            }
            if let Some(dj) = g[j].d
                && (dj.0 * dir.1 - dj.1 * dir.0).abs() > 15f64.to_radians().sin()
            {
                continue;
            }
            if mejor.is_none_or(|m| a < m.1) {
                mejor = Some((j, a));
            }
        }
        mejor.map(|m| m.0)
    };
    for &i in &orden {
        if usado[i] {
            continue;
        }
        // La direccion: la suya o, si es una mota, hacia la vecina mas cercana.
        let dir = match g[i].d {
            Some(d) => d,
            None => {
                let Some(j) = (0..n).filter(|&j| j != i && !usado[j]).min_by(|&a, &b| dist(g[a].c, g[i].c).total_cmp(&dist(g[b].c, g[i].c))) else { continue };
                let l = dist(g[j].c, g[i].c);
                if l > max_hueco + g[i].l + g[j].l || l < 0.5 {
                    continue;
                }
                ((g[j].c.0 - g[i].c.0) / l, (g[j].c.1 - g[i].c.1) / l)
            }
        };
        let mut cadena = vec![i];
        for sentido in [1.0, -1.0] {
            let mut dir_s = (dir.0 * sentido, dir.1 * sentido);
            loop {
                let fin = if sentido > 0.0 { *cadena.last().unwrap() } else { cadena[0] };
                let Some(j) = siguiente(fin, dir_s, &cadena, &usado) else { break };
                if sentido > 0.0 {
                    cadena.push(j);
                } else {
                    cadena.insert(0, j);
                }
                // La direccion, de punta a punta de lo que ya hay.
                let (a, b) = (g[cadena[0]].c, g[*cadena.last().unwrap()].c);
                let l = dist(a, b);
                if l > 1.0 {
                    dir_s = ((b.0 - a.0) / l * sentido, (b.1 - a.1) / l * sentido);
                }
            }
        }
        if cadena.len() >= 3 {
            for &k in &cadena {
                usado[k] = true;
            }
            cadenas.push(cadena);
        }
    }
    let sueltos = (0..n).filter(|&k| !usado[k]).collect();
    (cadenas, sueltos)
}

/// La discontinua de una cadena de guiones: la recta por todos sus
/// pixeles, de punta a punta.
fn segmento_discontinuo(m: &Mapas, g: &[Guion], cadena: &[usize]) -> Option<Segmento> {
    let pts: Vec<P> = cadena.iter().flat_map(|&k| g[k].px.iter().map(|&i| ((i % m.w) as f64 + 0.5, (i / m.w) as f64 + 0.5))).collect();
    let r = ajuste::recta(&pts)?;
    let (t0, t1) = intervalo(&r, &pts);
    let normal = (-r.d.1, r.d.0);
    let muestras: Vec<(P, P)> = cadena.iter().map(|&k| (g[k].c, normal)).collect();
    let grosor = m.ancho(&muestras);
    let color = m.color(&cadena.iter().map(|&k| g[k].c).collect::<Vec<_>>(), (grosor / 2.0).max(1.0));
    let medio = cadena.iter().map(|&k| g[k].l).sum::<f64>() / cadena.len() as f64;
    let estilo = if medio <= 1.8 * grosor + 1.0 { Estilo::Punteado } else { Estilo::Discontinuo };
    Some(Segmento { a: r.en(t0 - 0.5), b: r.en(t1 + 0.5), grosor, color, estilo, flecha_a: false, flecha_b: false })
}

/// **Un arco partido en trozos casi rectos** (los cruces lo cortan, y un
/// trozo corto de una circunferencia grande se ajusta mejor con una recta):
/// trozos vecinos que juntos curvan y caen en una misma circunferencia son
/// un arco. Asi el arco AB de un cuadrante, cruzado por cinco rayas, sale
/// entero y no como una quebrada.
fn arcos_de_trozos(ls: &mut Vec<GrupoRecta>, cs: &mut Vec<GrupoCirc>, hm: f64, lado: f64) {
    // Los trozos: rectas cortas y arcos de poco barrido.
    struct Trozo {
        puntos: Vec<P>,
        recta: Option<usize>,
        circ: Option<usize>,
    }
    let mut trozos: Vec<Trozo> = Vec::new();
    for (k, l) in ls.iter().enumerate() {
        if l.largo() < 0.3 * lado && l.largo() >= 4.0 {
            let mut p = l.puntos.clone();
            p.sort_by(|a, b| l.recta.t(*a).total_cmp(&l.recta.t(*b)));
            trozos.push(Trozo { puntos: p, recta: Some(k), circ: None });
        }
    }
    for (k, c) in cs.iter().enumerate() {
        if c.largo() < 0.33 * TAU * c.circ.r {
            trozos.push(Trozo { puntos: c.todos(), recta: None, circ: Some(k) });
        }
    }
    let cerca = |a: &[P], b: &[P]| {
        a.iter().step_by(2).any(|p| b.iter().step_by(2).any(|q| dist(*p, *q) <= 12.0))
    };
    let mut usado = vec![false; trozos.len()];
    let mut orden: Vec<usize> = (0..trozos.len()).collect();
    orden.sort_by(|a, b| ajuste::largo(&trozos[*b].puntos).total_cmp(&ajuste::largo(&trozos[*a].puntos)));
    let mut nuevos: Vec<(Vec<usize>, Circ)> = Vec::new();
    for &s in &orden {
        if usado[s] || ajuste::largo(&trozos[s].puntos) < 8.0 {
            continue;
        }
        let mut miembros = vec![s];
        let mut pts = trozos[s].puntos.clone();
        let mut circ: Option<Circ> = None;
        loop {
            let mut anadido = false;
            for &j in &orden {
                if usado[j] || miembros.contains(&j) || !cerca(&pts, &trozos[j].puntos) {
                    continue;
                }
                let mut prueba = pts.clone();
                prueba.extend_from_slice(&trozos[j].puntos);
                let Some(c) = ajuste::circunferencia(&prueba) else { continue };
                if c.r > 3.0 * lado || c.r < 0.8 * hm {
                    continue;
                }
                // Que curve de verdad (si no, es una recta partida).
                if ajuste::recta(&prueba).is_none_or(|r| r.error <= 2.0) {
                    continue;
                }
                let media = prueba.iter().map(|&q| c.residuo(q).abs()).sum::<f64>() / prueba.len() as f64;
                let peor = prueba.iter().map(|&q| c.residuo(q).abs()).fold(0.0, f64::max);
                if media > 1.0 || peor > 2.6 {
                    continue;
                }
                miembros.push(j);
                pts = prueba;
                circ = Some(c);
                anadido = true;
            }
            if !anadido {
                break;
            }
        }
        if let Some(c) = circ
            && miembros.len() >= 2
        {
            // Que barra algo (15 grados): no una quebrada casi recta.
            let angs: Vec<f64> = pts.iter().map(|&p| c.angulo(p)).collect();
            let a0 = angs[0];
            let rel: Vec<f64> = angs.iter().map(|a| (a - a0 + PI).rem_euclid(TAU) - PI).collect();
            let barrido = rel.iter().copied().fold(f64::MIN, f64::max) - rel.iter().copied().fold(f64::MAX, f64::min);
            if barrido < 15f64.to_radians() {
                continue;
            }
            for &k in &miembros {
                usado[k] = true;
            }
            nuevos.push((miembros, c));
        }
    }
    if nuevos.is_empty() {
        return;
    }
    let mut quitar_l: Vec<usize> = Vec::new();
    let mut quitar_c: Vec<usize> = Vec::new();
    for (miembros, c) in nuevos {
        let tramos: Vec<Vec<P>> = miembros.iter().map(|&k| trozos[k].puntos.clone()).collect();
        for &k in &miembros {
            if let Some(l) = trozos[k].recta {
                quitar_l.push(l);
            }
            if let Some(ci) = trozos[k].circ {
                quitar_c.push(ci);
            }
        }
        cs.push(GrupoCirc { tramos, circ: c });
    }
    let mut k = 0;
    ls.retain(|_| {
        k += 1;
        !quitar_l.contains(&(k - 1))
    });
    let mut k = 0;
    cs.retain(|_| {
        k += 1;
        !quitar_c.contains(&(k - 1))
    });
}

/// **Las espuelas**: rayitas cortas con una punta suelta (no toca nada)
/// pegadas a un punto o a otra raya; son el trozo de una letra que toca el
/// dibujo (la P junto a su punto), no geometria.
fn quitar_espuelas(d: &mut Diagrama, hm: f64) {
    let corto = 10f64.max(1.0 * hm);
    let n = d.segmentos.len();
    let toca = |p: P, yo: usize, d: &Diagrama| {
        d.segmentos.iter().enumerate().any(|(j, t)| j != yo && ajuste::dist_a_segmento(p, t.a, t.b) <= 3.0 + t.grosor / 2.0)
            || d.arcos.iter().any(|a| (dist(p, a.c) - a.r).abs() <= 3.0 + a.grosor / 2.0)
            || d.puntos.iter().any(|q| dist(p, q.c) <= q.r + 3.0)
            || d.recuadros.iter().any(|r| {
                let dentro_x = p.0 >= r.x0 - 3.0 && p.0 <= r.x1 + 3.0;
                let dentro_y = p.1 >= r.y0 - 3.0 && p.1 <= r.y1 + 3.0;
                (dentro_x && ((p.1 - r.y0).abs() <= 3.0 || (p.1 - r.y1).abs() <= 3.0)) || (dentro_y && ((p.0 - r.x0).abs() <= 3.0 || (p.0 - r.x1).abs() <= 3.0))
            })
    };
    let quitar: Vec<bool> = (0..n)
        .map(|i| {
            let s = &d.segmentos[i];
            if s.largo() > corto || s.estilo != Estilo::Solido || s.flecha_a || s.flecha_b {
                return false;
            }
            let (ta, tb) = (toca(s.a, i, d), toca(s.b, i, d));
            ta != tb
        })
        .collect();
    let mut k = 0;
    d.segmentos.retain(|_| {
        k += 1;
        !quitar[k - 1]
    });
}

/// Un trazo corto cerca de un cruce mide de mas (el cruce engorda la
/// tinta): los cortos no pasan de vez y media el grosor tipico del dibujo.
fn normalizar_grosores(d: &mut Diagrama, hm: f64) {
    let mut g: Vec<f64> = d.segmentos.iter().filter(|s| s.largo() >= 3.0 * hm).map(|s| s.grosor).collect();
    if g.len() < 3 {
        return;
    }
    g.sort_by(f64::total_cmp);
    let tipico = g[g.len() / 2];
    for s in d.segmentos.iter_mut() {
        if s.largo() < 3.0 * hm && s.grosor > 1.5 * tipico {
            s.grosor = tipico;
        }
    }
    for a in d.arcos.iter_mut() {
        if a.r * a.barrido < 3.0 * hm && a.grosor > 1.5 * tipico {
            a.grosor = tipico;
        }
    }
}

/// La punta de una discontinua: un trazo corto y macizo justo detras de su
/// ultimo guion, en su direccion (la punta en V, chica, sale como rayita).
fn puntas_de_discontinuas(d: &mut Diagrama, hm: f64) {
    let corto = 1.2 * hm.max(10.0);
    let mut quitar: Vec<usize> = Vec::new();
    for i in 0..d.segmentos.len() {
        if d.segmentos[i].estilo == Estilo::Solido {
            continue;
        }
        for fin in [true, false] {
            let s = &d.segmentos[i];
            if (fin && s.flecha_b) || (!fin && s.flecha_a) {
                continue;
            }
            let (e, otro) = if fin { (s.b, s.a) } else { (s.a, s.b) };
            let l = dist(e, otro).max(1e-9);
            let dir = ((e.0 - otro.0) / l, (e.1 - otro.1) / l);
            let mut mejor: Option<(usize, P)> = None;
            for (j, t) in d.segmentos.iter().enumerate() {
                if j == i || quitar.contains(&j) || t.estilo != Estilo::Solido || t.largo() > corto {
                    continue;
                }
                let (cerca, lejos) = if dist(t.a, e) < dist(t.b, e) { (t.a, t.b) } else { (t.b, t.a) };
                let adelante = (lejos.0 - e.0) * dir.0 + (lejos.1 - e.1) * dir.1;
                let lado = ((lejos.0 - e.0) * -dir.1 + (lejos.1 - e.1) * dir.0).abs();
                if dist(cerca, e) <= 8.0 && adelante > 2.0 && lado <= 0.5 * adelante + 2.0 {
                    mejor = Some((j, lejos));
                }
            }
            if let Some((j, lejos)) = mejor {
                let adelante = (lejos.0 - e.0) * dir.0 + (lejos.1 - e.1) * dir.1;
                let punta = (e.0 + dir.0 * adelante, e.1 + dir.1 * adelante);
                let s = &mut d.segmentos[i];
                if fin {
                    s.b = punta;
                    s.flecha_b = true;
                } else {
                    s.a = punta;
                    s.flecha_a = true;
                }
                quitar.push(j);
            }
        }
    }
    let mut k = 0;
    d.segmentos.retain(|_| {
        k += 1;
        !quitar.contains(&(k - 1))
    });
}

/// **Cada arco en su circunferencia de verdad** (15-oct, la lamina del
/// usuario: el arco grueso AB del cuadrante salio en tres trozos, dos de
/// ellos ajustados con circunferencias que no eran —r 154 y r 48 en vez de
/// 261—, y se veia como un trazo a mano). Un arco cuyos puntos caen sobre
/// una circunferencia mayor ya encontrada es de ella (con sus angulos); y
/// los arcos de una misma circunferencia, del mismo color y grosor, que se
/// tocan o tienen entre medias un hueco con tinta (cruces, un punto), uno.
fn consolidar_arcos(m: &Mapas, d: &mut Diagrama, hm: f64) {
    if d.arcos.len() < 2 {
        return;
    }
    let muestras = |a: &Arco| -> Vec<P> {
        let n = ((a.r * a.barrido / 2.0).ceil() as usize).clamp(3, 400);
        (0..=n).map(|k| {
            let t = a.inicio + a.barrido * k as f64 / n as f64;
            (a.c.0 + a.r * t.cos(), a.c.1 + a.r * t.sin())
        }).collect()
    };
    // 1. A su circunferencia: de las mas largas a las mas cortas.
    let mut orden: Vec<usize> = (0..d.arcos.len()).collect();
    orden.sort_by(|&a, &b| (d.arcos[b].r * d.arcos[b].barrido).total_cmp(&(d.arcos[a].r * d.arcos[a].barrido)));
    let mut refs: Vec<(P, f64)> = Vec::new();
    let mut ref_colores: Vec<[u8; 3]> = Vec::new();
    let parecido = |x: [u8; 3], y: [u8; 3]| mascara::dist_color([x[0] as f32, x[1] as f32, x[2] as f32], [y[0] as f32, y[1] as f32, y[2] as f32]) < 110.0;
    for &i in &orden {
        let a = d.arcos[i].clone();
        let pts = muestras(&a);
        // Un trozo corto ajustado con otra curvatura se aparta unos pixeles
        // en medio aunque su tinta este sobre la circunferencia.
        let tol = 4.5f64.max(1.2 * a.grosor);
        // O, aunque su ajuste se aparte, su tinta va por esa circunferencia:
        // los extremos cerca de ella y tinta casi seguida a lo largo de ella
        // entre los dos (un arco de 39 grados ajustado con r 154 en vez de
        // 261: era el mismo trazo).
        let por_tinta = |c: P, r: f64| -> bool {
            let (p0, p1) = (pts[0], pts[pts.len() - 1]);
            let tol_e = 6f64.max(0.04 * r);
            if (dist(p0, c) - r).abs() > tol_e || (dist(p1, c) - r).abs() > tol_e {
                return false;
            }
            let medio = pts[pts.len() / 2];
            let am = (medio.1 - c.1).atan2(medio.0 - c.0);
            let rel = |p: P| {
                let mut x = ((p.1 - c.1).atan2(p.0 - c.0) - am).rem_euclid(TAU);
                if x > PI {
                    x -= TAU;
                }
                x
            };
            let (t0, t1) = (rel(p0).min(rel(p1)), rel(p0).max(rel(p1)));
            let n = (((t1 - t0) * r).ceil() as usize).max(2);
            let si = (0..=n)
                .filter(|k| {
                    let t = am + t0 + (t1 - t0) * *k as f64 / n as f64;
                    [-1.5f64, -0.5, 0.5, 1.5].iter().any(|dr| m.i((c.0 + (r + dr) * t.cos(), c.1 + (r + dr) * t.sin())).is_some_and(|i| m.tinta[i]))
                })
                .count();
            si as f64 >= 0.85 * (n + 1) as f64
        };
        let suya = refs.iter().copied().zip(ref_colores.iter().copied()).filter(|&(_, col)| parecido(col, a.color)).map(|(x, _)| x).filter(|&(c, r)| {
            r >= 0.8 * a.r && (dist(c, a.c) > 1.0 || (r - a.r).abs() > 1.0) && (pts.iter().all(|&p| (dist(p, c) - r).abs() <= tol) || por_tinta(c, r))
        }).min_by(|x, y| {
            let e = |(c, r): (P, f64)| pts.iter().map(|&p| (dist(p, c) - r).abs()).sum::<f64>();
            e(*x).total_cmp(&e(*y))
        });
        match suya {
            Some((c, r)) => {
                // Lo que abarcan sus puntos vistos desde el centro nuevo,
                // alrededor del de en medio (un arquito que se dobla no da
                // la vuelta entera).
                let medio = pts[pts.len() / 2];
                let am = (medio.1 - c.1).atan2(medio.0 - c.0);
                let (mut lo, mut hi) = (0f64, 0f64);
                for p in &pts {
                    let mut x = ((p.1 - c.1).atan2(p.0 - c.0) - am).rem_euclid(TAU);
                    if x > PI {
                        x -= TAU;
                    }
                    lo = lo.min(x);
                    hi = hi.max(x);
                }
                let x = &mut d.arcos[i];
                x.c = c;
                x.r = r;
                x.inicio = (am + lo).rem_euclid(TAU);
                x.barrido = (hi - lo).max(1e-3);
            }
            None => {
                refs.push((a.c, a.r));
                ref_colores.push(a.color);
            }
        }
    }
    // 2. Juntar los de una circunferencia.
    let tinta_en = |c: P, r: f64, desde: f64, hasta: f64| -> f64 {
        let largo = (hasta - desde) * r;
        let n = (largo.ceil() as usize).max(1);
        let si = (0..=n).filter(|k| {
            let t = desde + (hasta - desde) * *k as f64 / n as f64;
            let p = (c.0 + r * t.cos(), c.1 + r * t.sin());
            (-1..=1).any(|dr| {
                let q = (c.0 + (r + dr as f64) * t.cos(), c.1 + (r + dr as f64) * t.sin());
                m.i(q).is_some_and(|i| m.tinta[i]) || m.i(p).is_some_and(|i| m.texto[i])
            })
        }).count();
        si as f64 / (n + 1) as f64
    };
    loop {
        let mut junto = false;
        'fuera: for i in 0..d.arcos.len() {
            for j in 0..d.arcos.len() {
                if i == j {
                    continue;
                }
                let (a, b) = (&d.arcos[i], &d.arcos[j]);
                if a.entera() || b.entera() || dist(a.c, b.c) > 1.0 || (a.r - b.r).abs() > 1.0 || a.estilo != b.estilo {
                    continue;
                }
                let color = mascara::dist_color([a.color[0] as f32, a.color[1] as f32, a.color[2] as f32], [b.color[0] as f32, b.color[1] as f32, b.color[2] as f32]);
                let (g0, g1) = (a.grosor.min(b.grosor), a.grosor.max(b.grosor));
                if color > 80.0 || g1 > 1.7 * g0 + 0.6 {
                    continue;
                }
                // Donde empieza b visto desde el principio de a (0..2pi): b
                // tiene que empezar dentro de a o poco despues de su final.
                let fin_a = a.inicio + a.barrido;
                let rel = (b.inicio - a.inicio).rem_euclid(TAU);
                if rel > PI + a.barrido / 2.0 {
                    continue;
                }
                let hueco = rel - a.barrido;
                let tapado = hueco <= 0.0 || hueco * a.r <= 10f64.max(0.7 * hm) || (hueco <= 40f64.to_radians() && tinta_en(a.c, a.r, fin_a, fin_a + hueco) >= 0.6);
                if !tapado {
                    continue;
                }
                let barrido = a.barrido.max(rel + b.barrido).min(TAU);
                let (la, lb) = (a.barrido, b.barrido);
                let grosor = (a.grosor * la + b.grosor * lb) / (la + lb);
                let color = if la >= lb { a.color } else { b.color };
                let nuevo = Arco { c: a.c, r: a.r, inicio: a.inicio, barrido: if barrido >= TAU - 0.05 { TAU } else { barrido }, grosor, color, estilo: a.estilo };
                d.arcos[i] = nuevo;
                d.arcos.remove(j);
                junto = true;
                break 'fuera;
            }
        }
        if !junto {
            break;
        }
    }
}

/// **Un arco grueso sobre una circunferencia fina** es un arco con su
/// grosor (el AB del cuadrante, repasado, y su prolongacion fina hasta el
/// punto de la izquierda): el grosor de cada tramo de 3 grados, y donde
/// cambia de verdad (vez y media, al menos 12 grados seguidos), dos arcos.
fn partir_por_grosor(m: &Mapas, d: &mut Diagrama) {
    let mut nuevos: Vec<Arco> = Vec::new();
    for a in std::mem::take(&mut d.arcos) {
        if a.r * a.barrido < 40.0 {
            nuevos.push(a);
            continue;
        }
        let paso = 3f64.to_radians();
        let n = ((a.barrido / paso).floor() as usize).max(1);
        let anchos: Vec<f64> = (0..n)
            .map(|k| {
                let t0 = a.inicio + a.barrido * k as f64 / n as f64;
                let muestras: Vec<(P, P)> = (0..5)
                    .map(|q| {
                        let t = t0 + a.barrido / n as f64 * (q as f64 + 0.5) / 5.0;
                        ((a.c.0 + a.r * t.cos(), a.c.1 + a.r * t.sin()), (t.cos(), t.sin()))
                    })
                    .collect();
                m.ancho(&muestras)
            })
            .collect();
        // Mediana de 3 (un cruce engorda un tramo).
        let suave: Vec<f64> = (0..n)
            .map(|k| {
                let mut v = [anchos[k.saturating_sub(1)], anchos[k], anchos[(k + 1).min(n - 1)]];
                v.sort_by(f64::total_cmp);
                v[1]
            })
            .collect();
        let (mn, mx) = (suave.iter().copied().fold(f64::MAX, f64::min), suave.iter().copied().fold(f64::MIN, f64::max));
        if mx < 1.5 * mn || n < 8 {
            nuevos.push(a);
            continue;
        }
        let corte = (mn + mx) / 2.0;
        let mut clase: Vec<bool> = suave.iter().map(|&g| g >= corte).collect();
        // Las corridas de menos de 12 grados, con la vecina.
        let minimo = 4;
        loop {
            let mut cambio = false;
            let mut k = 0;
            while k < n {
                let mut j = k;
                while j < n && clase[j] == clase[k] {
                    j += 1;
                }
                if j - k < minimo && !(k == 0 && j == n) {
                    let v = if k > 0 { clase[k - 1] } else { clase[j.min(n - 1)] };
                    if v != clase[k] {
                        for c in clase.iter_mut().take(j).skip(k) {
                            *c = v;
                        }
                        cambio = true;
                    }
                }
                k = j;
            }
            if !cambio {
                break;
            }
        }
        let mut k = 0;
        while k < n {
            let mut j = k;
            while j < n && clase[j] == clase[k] {
                j += 1;
            }
            let mut g: Vec<f64> = anchos[k..j].to_vec();
            g.sort_by(f64::total_cmp);
            let inicio = a.inicio + a.barrido * k as f64 / n as f64;
            let barrido = a.barrido * (j - k) as f64 / n as f64 + if j == n { a.barrido - a.barrido * n as f64 / n as f64 } else { 0.0 };
            // Su color, el de su propia tinta (la parte fina de un arco verde
            // grueso era la circunferencia negra).
            let pts: Vec<P> = (0..=12).map(|q| {
                let t = inicio + barrido * q as f64 / 12.0;
                (a.c.0 + a.r * t.cos(), a.c.1 + a.r * t.sin())
            }).collect();
            let color = m.color(&pts, (g[g.len() / 2] / 2.0).max(1.0));
            nuevos.push(Arco { inicio: inicio.rem_euclid(TAU), barrido, grosor: g[g.len() / 2], color, ..a.clone() });
            k = j;
        }
    }
    d.arcos = nuevos;
}

/// **Las marcas de angulo palidas** (15-oct, la verde de 45° en P): una
/// marca translucida y fina no aguanta la erosion de los rellenos y no sale
/// como region. Se busca directamente: en cada vertice (donde se cruzan o
/// acaban las rayas largas), en cada hueco entre dos rayas, el anillo de
/// color que no es papel ni el relleno grande de alrededor. Si lo hay, el
/// sector exacto con ese color y el radio hasta donde llega.
pub(super) fn marcas_por_color(img: &Img, d: &mut Diagrama, hm: f64) {
    let papel = mascara::fondo(img);
    let r_max = (2.5 * hm).clamp(12.0, 60.0);
    let largas: Vec<(P, P)> = d.segmentos.iter().filter(|s| s.largo() >= 2.0 * r_max).map(|s| (s.a, s.b)).collect();
    // Los vertices: extremos y cruces de las rayas largas.
    let mut vertices: Vec<P> = Vec::new();
    let anadir = |p: P, v: &mut Vec<P>| {
        if !v.iter().any(|q| dist(*q, p) <= 3.0) {
            v.push(p);
        }
    };
    for (i, &(a, b)) in largas.iter().enumerate() {
        anadir(a, &mut vertices);
        anadir(b, &mut vertices);
        for &(c, e) in &largas[i + 1..] {
            if let Some(x) = cruce((a, b), (c, e))
                && ajuste::dist_a_segmento(x, a, b) <= 1.5
                && ajuste::dist_a_segmento(x, c, e) <= 1.5
            {
                anadir(x, &mut vertices);
            }
        }
    }
    // Los colores de los rellenos grandes (una marca no es del color del cuadrante).
    let grandes: Vec<[f32; 3]> = d.regiones.iter().filter(|r| r.area > 4.0 * r_max * r_max).map(|r| [r.color[0] as f32, r.color[1] as f32, r.color[2] as f32]).collect();
    let saturado = |c: [f32; 3]| {
        let (mx, mn) = (c[0].max(c[1]).max(c[2]), c[0].min(c[1]).min(c[2]));
        mx > 1.0 && (mx - mn) / mx >= 0.12
    };
    let mut nuevas: Vec<super::Marca> = Vec::new();
    for &v in &vertices {
        // Sus rayas: hacia los extremos de las que acaban o pasan por el.
        let mut dirs: Vec<f64> = Vec::new();
        for &(a, b) in &largas {
            if ajuste::dist_a_segmento(v, a, b) > 1.5 {
                continue;
            }
            for e in [a, b] {
                if dist(e, v) > r_max {
                    let t = (e.1 - v.1).atan2(e.0 - v.0);
                    if !dirs.iter().any(|&x| ((x - t + PI).rem_euclid(TAU) - PI).abs() < 0.05) {
                        dirs.push(t);
                    }
                }
            }
        }
        if dirs.len() < 2 {
            continue;
        }
        dirs.sort_by(f64::total_cmp);
        for k in 0..dirs.len() {
            let a0 = dirs[k];
            let a1 = if k + 1 < dirs.len() { dirs[k + 1] } else { dirs[0] + TAU };
            let barrido = a1 - a0;
            if !(15f64.to_radians()..=150f64.to_radians()).contains(&barrido) {
                continue;
            }
            // Ya hay marca en ese hueco.
            let medio = a0 + barrido / 2.0;
            if d.marcas.iter().chain(nuevas.iter()).any(|m| dist(m.v, v) <= 4.0 && ((m.inicio + m.barrido / 2.0 - medio + PI).rem_euclid(TAU) - PI).abs() < barrido / 2.0) {
                continue;
            }
            // Anillo a anillo: lo teñido (de color, no papel, no el relleno grande).
            let margen = 6f64.to_radians().min(barrido / 4.0);
            let anillo = |r: f64| -> Vec<[f32; 3]> {
                let n = ((barrido - 2.0 * margen) * r / 1.5).ceil().max(3.0) as usize;
                (0..=n)
                    .filter_map(|q| {
                        let t = a0 + margen + (barrido - 2.0 * margen) * q as f64 / n as f64;
                        img.en((v.0 + r * t.cos()).floor() as i64, (v.1 + r * t.sin()).floor() as i64)
                    })
                    .collect()
            };
            // (De otro tono que el relleno grande: el degradado naranja del cuadrante amarillo no es una marca.)
            let otro_tono = |c: [f32; 3], g: [f32; 3]| match (tono_de(c), tono_de(g)) {
                (Some(x), Some(y)) => { let dt = (x - y).rem_euclid(360.0); dt.min(360.0 - dt) >= 25.0 }
                _ => true,
            };
            let tenido = |c: [f32; 3]| saturado(c) && mascara::dist_color(c, papel) >= 25.0 && grandes.iter().all(|g| mascara::dist_color(c, *g) >= 35.0 && otro_tono(c, *g));
            let mut color_ref: Option<[f32; 3]> = None;
            let mut radio = 0.0;
            let mut seguidos = 0;
            let mut r = 5.0;
            while r <= r_max {
                let px = anillo(r);
                let ten: Vec<[f32; 3]> = px.iter().copied().filter(|&c| tenido(c)).collect();
                let frac = ten.len() as f64 / px.len().max(1) as f64;
                if color_ref.is_none() && frac >= 0.6 {
                    color_ref = Some(mascara::media(ten.iter().copied()));
                }
                let parecido = color_ref.is_some_and(|cr| frac >= 0.55 && mascara::dist_color(mascara::media(ten.iter().copied()), cr) < 45.0);
                if parecido {
                    seguidos += 1;
                    radio = r;
                } else if color_ref.is_some() {
                    break;
                }
                r += 1.5;
            }
            let Some(color) = color_ref else { continue };
            if seguidos < 4 || radio < 8.0 {
                continue;
            }
            nuevas.push(super::Marca { v, r: radio + 1.0, inicio: a0.rem_euclid(TAU), barrido, color: a_u8(color) });
        }
    }
    if nuevas.is_empty() {
        return;
    }
    // Lo que quedaba del borde de la marca y de su numero: rayitas cortas
    // dentro de ella.
    for m in &nuevas {
        let r = m.r;
        d.segmentos.retain(|s| !(s.largo() < 1.2 * r && dist(s.a, m.v) <= 1.4 * r && dist(s.b, m.v) <= 1.4 * r && dist(s.a, m.v) > 2.0));
    }
    d.marcas.extend(nuevas);
}

/// El tono (0-360) de un color con algo de color; `None` si es gris.
fn tono_de(c: [f32; 3]) -> Option<f32> {
    let (r, g, b) = (c[0], c[1], c[2]);
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    if mx < 1.0 || (mx - mn) / mx < 0.1 {
        return None;
    }
    let h = if mx == r {
        ((g - b) / (mx - mn)).rem_euclid(6.0)
    } else if mx == g {
        (b - r) / (mx - mn) + 2.0
    } else {
        (r - g) / (mx - mn) + 4.0
    };
    Some(h * 60.0)
}

/// **Las formas rellenas** (16-oct, el diagrama de flujo): un relleno cuyo
/// borde es un ovalo es un ovalo (con su trazo: los arcos que lo partian en
/// trozos «Inicio» y «Fin» se quitan), y el relleno de dentro de un
/// recuadro es el relleno del recuadro (las cajas salian blancas).
fn formas_rellenas(d: &mut Diagrama, hm: f64) {
    let mut quitar_reg: Vec<usize> = Vec::new();
    for (ri, r) in d.regiones.iter().enumerate() {
        if r.contorno.len() < 3 {
            continue;
        }
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for &(x, y) in &r.contorno {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
        // El relleno de un recuadro.
        if let Some(q) = d.recuadros.iter_mut().find(|q| {
            let t = q.grosor + 5.0;
            x0 >= q.x0 - t && x1 <= q.x1 + t && y0 >= q.y0 - t && y1 <= q.y1 + t && (x1 - x0) * (y1 - y0) >= 0.6 * (q.x1 - q.x0) * (q.y1 - q.y0)
        }) {
            if q.relleno.is_none() {
                q.relleno = Some(r.color);
            }
            quitar_reg.push(ri);
            continue;
        }
        // Un ovalo.
        let (c, a, b) = (((x0 + x1) / 2.0, (y0 + y1) / 2.0), (x1 - x0) / 2.0, (y1 - y0) / 2.0);
        if a < 0.6 * hm || b < 0.6 * hm {
            continue;
        }
        let mut e: Vec<f64> = r.contorno.iter().map(|&(x, y)| ((((x - c.0) / a).powi(2) + ((y - c.1) / b).powi(2)).sqrt() - 1.0).abs()).collect();
        e.sort_by(f64::total_cmp);
        let (med, p90) = (e[e.len() / 2], e[e.len() * 9 / 10]);
        // Un rombo (la decision): |x|/a + |y|/b = 1.
        let mut er: Vec<f64> = r.contorno.iter().map(|&(x, y)| ((x - c.0).abs() / a + (y - c.1).abs() / b - 1.0).abs()).collect();
        er.sort_by(f64::total_cmp);
        if er[er.len() / 2] <= 0.03 && er[er.len() * 9 / 10] <= 0.08 && a >= hm && b >= 0.7 * hm {
            let en_borde = |p: P, g: f64| {
                let (ax, by) = (a + g, b + g);
                (((p.0 - c.0).abs() / ax + (p.1 - c.1).abs() / by) - 1.0).abs() * a.min(b) <= g + 4.0
            };
            let mut grosores: Vec<f64> = Vec::new();
            let mut color: Option<[u8; 3]> = None;
            d.segmentos.retain(|s| {
                let m = ((s.a.0 + s.b.0) / 2.0, (s.a.1 + s.b.1) / 2.0);
                let suyo = s.largo() <= 1.2 * a.hypot(b) && en_borde(s.a, s.grosor) && en_borde(s.b, s.grosor) && en_borde(m, s.grosor);
                if suyo {
                    grosores.push(s.grosor);
                    color.get_or_insert(s.color);
                }
                !suyo
            });
            if grosores.len() >= 2 {
                grosores.sort_by(f64::total_cmp);
                let g = grosores[grosores.len() / 2];
                d.ovalos.push(super::Ovalo { c, rx: a + g / 2.0, ry: b + g / 2.0, grosor: g, color: color.unwrap_or([30, 30, 30]), relleno: Some(r.color), rombo: true });
                quitar_reg.push(ri);
                continue;
            }
        }
        if med > 0.03 || p90 > 0.08 || r.contorno.len() < 12 {
            continue;
        }
        // Su trazo: los arcos y rayas que van por su borde.
        let en_borde = |p: P, g: f64| {
            let (ax, by) = (a + g / 2.0, b + g / 2.0);
            let k = (((p.0 - c.0) / ax).powi(2) + ((p.1 - c.1) / by).powi(2)).sqrt();
            (k - 1.0).abs() * a.min(b) <= g / 2.0 + 3.5
        };
        let mut grosores: Vec<f64> = Vec::new();
        let mut color: Option<[u8; 3]> = None;
        let mut largo_max = 0.0;
        d.arcos.retain(|x| {
            let pts: Vec<P> = (0..=8).map(|k| {
                let t = x.inicio + x.barrido * k as f64 / 8.0;
                (x.c.0 + x.r * t.cos(), x.c.1 + x.r * t.sin())
            }).collect();
            let suyo = pts.iter().all(|&p| en_borde(p, x.grosor));
            if suyo {
                grosores.push(x.grosor);
                if x.r * x.barrido > largo_max {
                    largo_max = x.r * x.barrido;
                    color = Some(x.color);
                }
            }
            !suyo
        });
        d.segmentos.retain(|s| {
            let suyo = s.largo() < a.max(b) && en_borde(s.a, s.grosor) && en_borde(s.b, s.grosor) && en_borde(((s.a.0 + s.b.0) / 2.0, (s.a.1 + s.b.1) / 2.0), s.grosor);
            if suyo {
                grosores.push(s.grosor);
            }
            !suyo
        });
        grosores.sort_by(f64::total_cmp);
        // Sin trazo alrededor no es un ovalo dibujado (una marca de angulo rellena).
        if grosores.is_empty() {
            continue;
        }
        let grosor = grosores[grosores.len() / 2];
        d.ovalos.push(super::Ovalo { c, rx: a + grosor / 2.0, ry: b + grosor / 2.0, grosor, color: color.unwrap_or([30, 30, 30]), relleno: Some(r.color), rombo: false });
        quitar_reg.push(ri);
    }
    let mut k = 0;
    d.regiones.retain(|_| {
        k += 1;
        !quitar_reg.contains(&(k - 1))
    });
}

/// **Las alas de una punta abierta** (16-oct, el diagrama de flujo): la «V»
/// de una flecha, al esqueletizarse con su raya, sale a veces como un
/// arquito y no como dos rayitas. Un arco pequeno que nace en la punta de
/// una raya larga y por el otro lado no toca nada es un ala: su cuerda.
fn arquitos_a_alas(d: &mut Diagrama, hm: f64) {
    let largas: Vec<(P, P)> = d.segmentos.iter().filter(|s| s.largo() >= 3.0 * hm).map(|s| (s.a, s.b)).collect();
    let mut alas: Vec<Segmento> = Vec::new();
    d.arcos.retain(|a| {
        if a.entera() || a.r > 2.0 * hm || a.barrido > 2.2 {
            return true;
        }
        let (p, q) = a.extremos();
        let alcance = 4f64.max(0.8 * hm);
        let en_punta = |x: P| largas.iter().any(|&(s0, s1)| dist(x, s0) <= alcance || dist(x, s1) <= alcance);
        let cuerda = dist(p, q);
        if cuerda > 2.5 * hm || !(en_punta(p) ^ en_punta(q)) {
            return true;
        }
        alas.push(Segmento { a: p, b: q, grosor: a.grosor, color: a.color, estilo: Estilo::Solido, flecha_a: false, flecha_b: false });
        false
    });
    d.segmentos.extend(alas);
}

/// Los «puntos» en la punta de una flecha son el nudo de la V, no puntos.
fn sin_puntos_en_las_puntas(d: &mut Diagrama) {
    let puntas: Vec<P> = d.segmentos.iter().flat_map(|s| {
        let mut v = Vec::new();
        if s.flecha_a {
            v.push(s.a);
        }
        if s.flecha_b {
            v.push(s.b);
        }
        v
    }).collect();
    d.puntos.retain(|p| !puntas.iter().any(|&q| dist(p.c, q) <= p.r + 6.0));
}

/// **Las palabras de un renglon, un rotulo** (16-oct, el rombo «¿N mod 2 =
/// 0?»: salia en cinco trozos de tamanos distintos, con huecos). Las
/// palabras seguidas a la misma altura y del mismo color van juntas.
fn juntar_renglones(mut v: Vec<Rotulo>) -> Vec<Rotulo> {
    v.sort_by(|a, b| a.caja.cy().total_cmp(&b.caja.cy()).then(a.caja.x.total_cmp(&b.caja.x)));
    let mut salida: Vec<Rotulo> = Vec::new();
    let mut usado = vec![false; v.len()];
    for i in 0..v.len() {
        if usado[i] {
            continue;
        }
        usado[i] = true;
        let mut r = v[i].clone();
        loop {
            let h = r.caja.h;
            let siguiente = (0..v.len()).filter(|&j| !usado[j]).filter(|&j| {
                let o = &v[j];
                let hueco = o.caja.x - r.caja.x1();
                let mismo_color = match (o.color, r.color) {
                    (Some(a), Some(b)) => a.iter().zip(b.iter()).all(|(x, y)| (*x as i32 - *y as i32).abs() < 60),
                    _ => true,
                };
                (o.caja.cy() - r.caja.cy()).abs() <= 0.4 * h.max(o.caja.h) && hueco >= -0.3 * h && hueco <= 0.9 * h.max(o.caja.h) && mismo_color
            }).min_by(|&a, &b| v[a].caja.x.total_cmp(&v[b].caja.x));
            let Some(j) = siguiente else { break };
            usado[j] = true;
            let o = &v[j];
            r.texto = format!("{} {}", r.texto, o.texto);
            r.caja = Caja::de_esquinas(r.caja.x.min(o.caja.x), r.caja.y.min(o.caja.y), r.caja.x1().max(o.caja.x1()), r.caja.y1().max(o.caja.y1()));
            r.negrita |= o.negrita;
        }
        salida.push(r);
    }
    salida
}

/// Si dos textos dicen casi lo mismo (lo que lee el OCR y lo que lee la
/// nube): pares de letras en comun.
fn se_parecen(a: &str, b: &str) -> bool {
    let limpio = |s: &str| -> Vec<char> { s.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).map(|c| if c == 'o' { '0' } else { c }).collect() };
    let (x, y) = (limpio(a), limpio(b));
    if x.len() < 2 || y.len() < 2 || x.len().max(y.len()) > 2 * x.len().min(y.len()) {
        return false;
    }
    let pares = |v: &[char]| -> Vec<(char, char)> { v.windows(2).map(|w| (w[0], w[1])).collect() };
    let (px, mut py) = (pares(&x), pares(&y));
    let mut comun = 0;
    for p in &px {
        if let Some(k) = py.iter().position(|q| q == p) {
            py.remove(k);
            comun += 1;
        }
    }
    2.0 * comun as f64 / (px.len() + pares(&y).len()) as f64 >= 0.5
}
