//! **¿Se parece lo pintado al papel?** La comprobacion final, zona a zona.
//!
//! No se comparan pixeles uno a uno: la letra del lienzo no es la del papel
//! y una diferencia de un pixel no dice nada. Se compara **donde hay tinta**
//! a grano grueso: los perfiles de filas y de columnas (los renglones caen a
//! la misma altura, las columnas de una tabla en el mismo sitio) y una
//! rejilla de densidades. Da un numero de 0 (nada que ver) a 1 (igual); por
//! debajo de `Opciones::rehacer_bajo` la zona se rehace una vez.

/// Lado mayor de la rejilla con que se compara.
const REJILLA: usize = 32;

/// La tinta de una imagen RGBA en una rejilla de `cw x ch` celdas (0-1 por
/// celda), con su propio umbral (el papel puede ser gris).
fn rejilla(rgba: &[u8], w: u32, h: u32, cw: usize, ch: usize) -> Vec<f64> {
    let (w, h) = (w as usize, h as usize);
    let mut d = vec![0.0; cw * ch];
    if w == 0 || h == 0 || rgba.len() < w * h * 4 {
        return d;
    }
    let gris = |i: usize| {
        let a = rgba[i * 4 + 3] as u32;
        let g = (rgba[i * 4] as u32 * 299
            + rgba[i * 4 + 1] as u32 * 587
            + rgba[i * 4 + 2] as u32 * 114)
            / 1000;
        ((g * a + 255 * (255 - a)) / 255) as u8
    };
    let mut hist = [0usize; 256];
    for i in 0..w * h {
        hist[gris(i) as usize] += 1;
    }
    let fondo = (100..256).max_by_key(|&g| (hist[g], g)).unwrap_or(255) as f64;
    let umbral = (fondo * 0.7) as u8;
    let mut n = vec![0usize; cw * ch];
    for y in 0..h {
        let cy = (y * ch / h).min(ch - 1);
        for x in 0..w {
            let cx = (x * cw / w).min(cw - 1);
            n[cy * cw + cx] += 1;
            if gris(y * w + x) < umbral {
                d[cy * cw + cx] += 1.0;
            }
        }
    }
    for (v, k) in d.iter_mut().zip(&n) {
        *v /= (*k).max(1) as f64;
    }
    d
}

/// La correlacion de dos perfiles; dos planos iguales valen 1.
fn correlacion(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len());
    if n == 0 {
        return 1.0;
    }
    let ma = a.iter().take(n).sum::<f64>() / n as f64;
    let mb = b.iter().take(n).sum::<f64>() / n as f64;
    let (mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0);
    for i in 0..n {
        let (x, y) = (a[i] - ma, b[i] - mb);
        sab += x * y;
        saa += x * x;
        sbb += y * y;
    }
    if saa < 1e-9 && sbb < 1e-9 {
        return 1.0;
    }
    if saa < 1e-9 || sbb < 1e-9 {
        return 0.0;
    }
    (sab / (saa * sbb).sqrt()).clamp(-1.0, 1.0)
}

/// **El parecido** entre el recorte del papel `a` y el de lo pintado `b`
/// (los dos de la misma zona, del tamano que sea). 0-1.
pub fn parecido(a: (&[u8], u32, u32), b: (&[u8], u32, u32)) -> f64 {
    let (aw, ah) = (a.1.max(1) as f64, a.2.max(1) as f64);
    let (cw, ch) = if aw >= ah {
        (
            REJILLA,
            ((REJILLA as f64 * ah / aw).round() as usize).clamp(6, REJILLA),
        )
    } else {
        (
            ((REJILLA as f64 * aw / ah).round() as usize).clamp(6, REJILLA),
            REJILLA,
        )
    };
    let ra = rejilla(a.0, a.1, a.2, cw, ch);
    let rb = rejilla(b.0, b.1, b.2, cw, ch);
    let total_a: f64 = ra.iter().sum();
    let total_b: f64 = rb.iter().sum();
    if total_a < 1e-6 && total_b < 1e-6 {
        return 1.0;
    }
    if total_a < 1e-6 || total_b < 1e-6 {
        return 0.0;
    }
    let filas = |r: &[f64]| {
        (0..ch)
            .map(|y| r[y * cw..(y + 1) * cw].iter().sum::<f64>())
            .collect::<Vec<_>>()
    };
    let cols = |r: &[f64]| {
        (0..cw)
            .map(|x| (0..ch).map(|y| r[y * cw + x]).sum::<f64>())
            .collect::<Vec<_>>()
    };
    let pf = correlacion(&filas(&ra), &filas(&rb)).max(0.0);
    let pc = correlacion(&cols(&ra), &cols(&rb)).max(0.0);
    // La rejilla, normalizada por la tinta de cada una (una letra mas gruesa
    // no es otra cosa).
    let na: Vec<f64> = ra.iter().map(|v| v / total_a).collect();
    let nb: Vec<f64> = rb.iter().map(|v| v / total_b).collect();
    let dif: f64 = na.iter().zip(&nb).map(|(x, y)| (x - y).abs()).sum::<f64>() / 2.0;
    0.45 * pf + 0.25 * pc + 0.30 * (1.0 - dif)
}
