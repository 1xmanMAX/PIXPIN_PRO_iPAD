//! Perceptual quality metrics on 8-bit luma planes.

/// Mean SSIM (Wang et al. 2004) with a 7×7 uniform window computed from
/// integral images. Both planes must be `w×h`.
pub fn ssim(a: &[u8], b: &[u8], w: usize, h: usize) -> f64 {
    const R: usize = 3; // window radius → 7×7
    if w < 2 * R + 1 || h < 2 * R + 1 {
        return if a == b {
            1.0
        } else {
            psnr_to_pseudo_ssim(psnr(a, b))
        };
    }
    let n = (w + 1) * (h + 1);
    let mut ia = vec![0f64; n];
    let mut ib = vec![0f64; n];
    let mut iaa = vec![0f64; n];
    let mut ibb = vec![0f64; n];
    let mut iab = vec![0f64; n];
    for y in 0..h {
        let (mut ra, mut rb, mut raa, mut rbb, mut rab) = (0f64, 0f64, 0f64, 0f64, 0f64);
        for x in 0..w {
            let pa = a[y * w + x] as f64;
            let pb = b[y * w + x] as f64;
            ra += pa;
            rb += pb;
            raa += pa * pa;
            rbb += pb * pb;
            rab += pa * pb;
            let i = (y + 1) * (w + 1) + (x + 1);
            let up = y * (w + 1) + (x + 1);
            ia[i] = ia[up] + ra;
            ib[i] = ib[up] + rb;
            iaa[i] = iaa[up] + raa;
            ibb[i] = ibb[up] + rbb;
            iab[i] = iab[up] + rab;
        }
    }
    let c1 = (0.01f64 * 255.0).powi(2);
    let c2 = (0.03f64 * 255.0).powi(2);
    let win = ((2 * R + 1) * (2 * R + 1)) as f64;
    let sum = |t: &[f64], x0: usize, y0: usize, x1: usize, y1: usize| -> f64 {
        t[y1 * (w + 1) + x1] - t[y0 * (w + 1) + x1] - t[y1 * (w + 1) + x0] + t[y0 * (w + 1) + x0]
    };
    let mut total = 0f64;
    let mut count = 0usize;
    // Stride 2 for speed; the metric is dense enough at this scale.
    let mut y = R;
    while y + R < h {
        let mut x = R;
        while x + R < w {
            let (x0, y0, x1, y1) = (x - R, y - R, x + R + 1, y + R + 1);
            let ma = sum(&ia, x0, y0, x1, y1) / win;
            let mb = sum(&ib, x0, y0, x1, y1) / win;
            let va = sum(&iaa, x0, y0, x1, y1) / win - ma * ma;
            let vb = sum(&ibb, x0, y0, x1, y1) / win - mb * mb;
            let cov = sum(&iab, x0, y0, x1, y1) / win - ma * mb;
            let s = ((2.0 * ma * mb + c1) * (2.0 * cov + c2))
                / ((ma * ma + mb * mb + c1) * (va + vb + c2));
            total += s;
            count += 1;
            x += 2;
        }
        y += 2;
    }
    if count == 0 {
        1.0
    } else {
        (total / count as f64).clamp(0.0, 1.0)
    }
}

/// Mean SSIM plus a low percentile over `tile`×`tile` tiles, so a locally
/// destroyed region (a photo blurred inside a page of flat paper) cannot hide
/// behind a high global average. Returns (mean, p10_of_tiles).
pub fn ssim_mean_and_p10(a: &[u8], b: &[u8], w: usize, h: usize, tile: usize) -> (f64, f64) {
    let mean = ssim(a, b, w, h);
    let tile = tile.max(16);
    let mut tiles = Vec::new();
    let mut y = 0;
    while y < h {
        let th = (h - y).min(tile);
        let mut x = 0;
        while x < w {
            let tw = (w - x).min(tile);
            if tw >= 8 && th >= 8 {
                let mut ta = Vec::with_capacity(tw * th);
                let mut tb = Vec::with_capacity(tw * th);
                for yy in y..y + th {
                    ta.extend_from_slice(&a[yy * w + x..yy * w + x + tw]);
                    tb.extend_from_slice(&b[yy * w + x..yy * w + x + tw]);
                }
                // Flat paper tiles are trivially identical; only tiles with
                // content in the original are informative.
                let (mn, mx) = ta
                    .iter()
                    .fold((255u8, 0u8), |(mn, mx), &v| (mn.min(v), mx.max(v)));
                if mx - mn >= 24 {
                    tiles.push(ssim(&ta, &tb, tw, th));
                }
            }
            x += tile;
        }
        y += tile;
    }
    if tiles.is_empty() {
        return (mean, mean);
    }
    tiles.sort_by(|p, q| p.partial_cmp(q).unwrap());
    let p10 = tiles[(tiles.len() as f64 * 0.10) as usize];
    (mean, p10)
}

pub fn psnr(a: &[u8], b: &[u8]) -> f64 {
    let n = a.len().min(b.len());
    if n == 0 {
        return 0.0;
    }
    let mse: f64 = a
        .iter()
        .zip(b)
        .map(|(&x, &y)| {
            let d = x as f64 - y as f64;
            d * d
        })
        .sum::<f64>()
        / n as f64;
    if mse <= 1e-9 {
        99.0
    } else {
        10.0 * (255.0f64 * 255.0 / mse).log10()
    }
}

fn psnr_to_pseudo_ssim(p: f64) -> f64 {
    ((p - 20.0) / 30.0).clamp(0.0, 1.0)
}

/// SSIM on planes that may be large: both are box-downscaled to ≤ ~1.5 MP
/// first so memory stays bounded and the cost is predictable.
pub fn ssim_capped(a: &[u8], b: &[u8], w: u32, h: u32) -> f64 {
    const CAP: u64 = 1_500_000;
    let n = w as u64 * h as u64;
    if n <= CAP {
        return ssim(a, b, w as usize, h as usize);
    }
    let f = ((n as f64 / CAP as f64).sqrt()).ceil() as u32;
    let (nw, nh) = ((w / f).max(8), (h / f).max(8));
    let a2 = super::resample::resize(a, w, h, 1, nw, nh, image::imageops::FilterType::Triangle);
    let b2 = super::resample::resize(b, w, h, 1, nw, nh, image::imageops::FilterType::Triangle);
    ssim(&a2, &b2, nw as usize, nh as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identical_is_one() {
        let a: Vec<u8> = (0..64 * 64).map(|i| (i % 251) as u8).collect();
        assert!((ssim(&a, &a, 64, 64) - 1.0).abs() < 1e-9);
    }
    #[test]
    fn noise_lowers_ssim() {
        let a: Vec<u8> = (0..64 * 64).map(|i| ((i / 64) * 4) as u8).collect();
        let b: Vec<u8> = a
            .iter()
            .enumerate()
            .map(|(i, &v)| v.wrapping_add(((i * 7919) % 41) as u8))
            .collect();
        let s = ssim(&a, &b, 64, 64);
        assert!(s < 0.9, "{}", s);
    }
}
