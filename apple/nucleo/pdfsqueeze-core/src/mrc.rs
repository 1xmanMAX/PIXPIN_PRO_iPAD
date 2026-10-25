//! Mixed Raster Content: split a scanned page into a bilevel *mask* (where the
//! ink is), a low-resolution *foreground* (the ink colour) and a
//! low-resolution *background* (paper, photos). Text stays crisp at full
//! resolution while the expensive continuous-tone layers drop to ~⅓ / ¼ res.
//! Emitted as a Form XObject so the page content stream is untouched.

use crate::deflate::Effort;
use crate::images::encode::{self, Encoded};
use crate::images::quality;
use crate::images::resample;
use crate::images::{ColorKind, RawImage};
use crate::options::Options;
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};

pub struct MrcLayers {
    pub mask: Encoded,
    pub fg: Encoded,
    pub bg: Encoded,
    pub ssim: f64,
    pub bg_scale: u32,
    pub fg_scale: u32,
    /// What a reader will show, as luma at the gate's reference scale
    /// (`ref_w`×`ref_h`): lets callers and tests inspect regions.
    pub recomposed_ref: Vec<u8>,
    pub ref_w: u32,
    pub ref_h: u32,
}

impl MrcLayers {
    pub fn total_len(&self) -> usize {
        self.mask.len() + self.fg.len() + self.bg.len() + 160
    }
}

// El indice `t` es tambien el umbral que se devuelve: el bucle por rango es lo claro.
#[allow(clippy::needless_range_loop)]
fn otsu(hist: &[u64; 256]) -> u8 {
    let total: u64 = hist.iter().sum();
    let sum_all: f64 = hist
        .iter()
        .enumerate()
        .map(|(i, &c)| i as f64 * c as f64)
        .sum();
    let (mut w_b, mut sum_b, mut best, mut thr) = (0u64, 0f64, 0f64, 128u8);
    for t in 0..256 {
        w_b += hist[t];
        if w_b == 0 {
            continue;
        }
        let w_f = total - w_b;
        if w_f == 0 {
            break;
        }
        sum_b += t as f64 * hist[t] as f64;
        let m_b = sum_b / w_b as f64;
        let m_f = (sum_all - sum_b) / w_f as f64;
        let between = w_b as f64 * w_f as f64 * (m_b - m_f).powi(2);
        if between > best {
            best = between;
            thr = t as u8;
        }
    }
    thr
}

struct Integral {
    w: usize,
    t: Vec<u32>,
}
impl Integral {
    fn new(src: impl Fn(usize, usize) -> u32, w: usize, h: usize) -> Self {
        let mut t = vec![0u32; (w + 1) * (h + 1)];
        for y in 0..h {
            let mut row = 0u32;
            for x in 0..w {
                row += src(x, y);
                t[(y + 1) * (w + 1) + x + 1] = t[y * (w + 1) + x + 1] + row;
            }
        }
        Integral { w, t }
    }
    fn sum(&self, x0: usize, y0: usize, x1: usize, y1: usize) -> u32 {
        let w = self.w + 1;
        self.t[y1 * w + x1] - self.t[y0 * w + x1] - self.t[y1 * w + x0] + self.t[y0 * w + x0]
    }
}

/// Ink mask: 1 = ink. Adaptive: darker than both the global Otsu threshold
/// and the local mean minus a margin (kills paper texture and shading).
pub fn segment(luma: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut hist = [0u64; 256];
    luma.iter().for_each(|&v| hist[v as usize] += 1);
    let t = otsu(&hist) as i32;
    let integ = Integral::new(|x, y| luma[y * w + x] as u32, w, h);
    const R: usize = 12;
    let mut mask = vec![0u8; w * h];
    for y in 0..h {
        let (y0, y1) = (y.saturating_sub(R), (y + R + 1).min(h));
        for x in 0..w {
            let (x0, x1) = (x.saturating_sub(R), (x + R + 1).min(w));
            let n = ((x1 - x0) * (y1 - y0)) as i32;
            let mean = integ.sum(x0, y0, x1, y1) as i32 / n.max(1);
            let v = luma[y * w + x] as i32;
            if v < t + 8 && v < mean - 18 {
                mask[y * w + x] = 1;
            }
        }
    }
    // Remove isolated specks (single pixels with no ink neighbours).
    let mut clean = mask.clone();
    for y in 1..h.saturating_sub(1) {
        for x in 1..w.saturating_sub(1) {
            if mask[y * w + x] == 1 {
                let n = mask[(y - 1) * w + x]
                    + mask[(y + 1) * w + x]
                    + mask[y * w + x - 1]
                    + mask[y * w + x + 1];
                if n == 0 {
                    clean[y * w + x] = 0;
                }
            }
        }
    }
    clean
}

// Los canales indexan a la vez varios arreglos planos (acc, fg, img.data), y el
// `if n > 0` guarda tambien `filled`: los bucles por indice se leen mejor.
#[allow(clippy::needless_range_loop, clippy::manual_checked_ops)]
pub fn build(img: &RawImage, opts: &Options, effort: Effort) -> Option<MrcLayers> {
    let t0 = std::time::Instant::now();
    if img.channels == 4 || img.is_mask || img.width < 400 || img.height < 400 {
        return None;
    }
    let (w, h) = (img.width as usize, img.height as usize);
    let ch = img.channels as usize;
    let luma = img.luma();
    let mask = segment(&luma, w, h);
    let t_seg = t0.elapsed().as_millis();
    let ink: usize = mask.iter().map(|&m| m as usize).sum();
    let ink_frac = ink as f32 / (w * h) as f32;
    if !(0.003..=0.40).contains(&ink_frac) {
        log::debug!("mrc: ink fraction {ink_frac:.4} out of range");
        return None;
    }

    // Background: paint over the ink with the local paper colour.
    let keep = Integral::new(|x, y| 1 - mask[y * w + x] as u32, w, h);
    let mut chan_integrals = Vec::with_capacity(ch);
    for c in 0..ch {
        chan_integrals.push(Integral::new(
            |x, y| {
                if mask[y * w + x] == 0 {
                    img.data[(y * w + x) * ch + c] as u32
                } else {
                    0
                }
            },
            w,
            h,
        ));
    }
    let mut bg = img.data.clone();
    const R: usize = 10;
    for y in 0..h {
        for x in 0..w {
            if mask[y * w + x] == 0 {
                continue;
            }
            let (x0, y0, x1, y1) = (
                x.saturating_sub(R),
                y.saturating_sub(R),
                (x + R + 1).min(w),
                (y + R + 1).min(h),
            );
            let n = keep.sum(x0, y0, x1, y1);
            for c in 0..ch {
                bg[(y * w + x) * ch + c] = chan_integrals[c]
                    .sum(x0, y0, x1, y1)
                    .checked_div(n)
                    .map_or(255, |v| v as u8);
            }
        }
    }
    let t_bg = t0.elapsed().as_millis();

    // Foreground: ink colour per 4×4 block at full detail; coarser scales are
    // derived from it. Holes are filled by propagation so JPEG has no edges
    // to ring against.
    let fg_base_scale = 4u32;
    let (fw, fh) = (
        (img.width / fg_base_scale).max(8),
        (img.height / fg_base_scale).max(8),
    );
    let mut fg = vec![0u8; (fw * fh) as usize * ch];
    let mut filled = vec![false; (fw * fh) as usize];
    for by in 0..fh as usize {
        for bx in 0..fw as usize {
            let mut acc = vec![0u32; ch];
            let mut n = 0u32;
            for y in by * fg_base_scale as usize..((by + 1) * fg_base_scale as usize).min(h) {
                for x in bx * fg_base_scale as usize..((bx + 1) * fg_base_scale as usize).min(w) {
                    if mask[y * w + x] == 1 {
                        for c in 0..ch {
                            acc[c] += img.data[(y * w + x) * ch + c] as u32;
                        }
                        n += 1;
                    }
                }
            }
            if n > 0 {
                for c in 0..ch {
                    fg[(by * fw as usize + bx) * ch + c] = (acc[c] / n) as u8;
                }
                filled[by * fw as usize + bx] = true;
            }
        }
    }
    let fwu = fw as usize;
    for i in 0..filled.len() {
        if !filled[i] {
            let src = if i % fwu > 0 && filled[i - 1] {
                Some(i - 1)
            } else if i >= fwu && filled[i - fwu] {
                Some(i - fwu)
            } else {
                None
            };
            if let Some(s) = src {
                for c in 0..ch {
                    fg[i * ch + c] = fg[s * ch + c];
                }
                filled[i] = true;
            }
        }
    }
    for i in (0..filled.len()).rev() {
        if !filled[i] {
            let src = if i % fwu < fwu - 1 && filled[i + 1] {
                Some(i + 1)
            } else if i + fwu < filled.len() && filled[i + fwu] {
                Some(i + fwu)
            } else {
                None
            };
            match src {
                Some(s) => {
                    for c in 0..ch {
                        fg[i * ch + c] = fg[s * ch + c];
                    }
                }
                None => {
                    for c in 0..ch {
                        fg[i * ch + c] = 0;
                    }
                }
            }
            filled[i] = true;
        }
    }
    let fg_base = RawImage {
        width: fw,
        height: fh,
        channels: img.channels,
        data: fg,
        cs: img.cs,
        is_mask: false,
        orig_bpc: 8,
        lossy_decode: false,
    };
    let t_fg = t0.elapsed().as_millis();

    // Mask layer: exact bilevel, race JBIG2 vs G4 vs Flate.
    let mask_gray: Vec<u8> = mask.iter().map(|&m| if m == 1 { 255 } else { 0 }).collect();
    let mut mask_enc =
        encode::bilevel_flate(&mask_gray, img.width, img.height, Effort::fast(), false);
    if let Some(g4) = encode::ccitt_g4(&mask_gray, img.width, img.height, false) {
        if g4.len() < mask_enc.len() {
            mask_enc = g4;
        }
    }
    if let Some(jb) = encode::jbig2(&mask_gray, img.width, img.height, false) {
        if jb.len() < mask_enc.len() {
            mask_enc = jb;
        }
    }
    encode::squeeze(&mut mask_enc, effort);
    mask_enc.raw = None;
    log::debug!(
        "mrc {}x{}: segment {} | bg fill {} | fg {} | mask {} ms (cumulative)",
        img.width,
        img.height,
        t_seg,
        t_bg,
        t_fg,
        t0.elapsed().as_millis()
    );

    let cs = encode::cs_object(img.cs);
    let target = (opts.min_ssim - opts.mrc_ssim_slack).max(0.5);
    // Reference plane for the gate (see images::race::Reference).
    let cap = 1_500_000u64;
    let n = img.width as u64 * img.height as u64;
    let f = if n <= cap {
        1
    } else {
        ((n as f64 / cap as f64).sqrt()).ceil() as u32
    };
    let (rw, rh) = ((img.width / f).max(8), (img.height / f).max(8));
    let ref_luma = if f == 1 {
        luma.clone()
    } else {
        resample::resize(
            &luma,
            img.width,
            img.height,
            1,
            rw,
            rh,
            image::imageops::FilterType::Triangle,
        )
    };
    let mask_ref = if f == 1 {
        mask_gray.clone()
    } else {
        resample::resize(
            &mask_gray,
            img.width,
            img.height,
            1,
            rw,
            rh,
            image::imageops::FilterType::Triangle,
        )
    };

    // Layer candidates. Both layers are raced under the recomposition gate by
    // coordinate descent: background first (it dominates the bytes), then
    // foreground given the chosen background.
    let bg_layer = |scale: u32, q: u8| -> Option<(Encoded, Vec<u8>)> {
        let (bw, bh) = ((img.width / scale).max(16), (img.height / scale).max(16));
        let small = RawImage {
            width: bw,
            height: bh,
            channels: img.channels,
            data: resample::resize(
                &bg,
                img.width,
                img.height,
                img.channels,
                bw,
                bh,
                image::imageops::FilterType::Triangle,
            ),
            cs: img.cs,
            is_mask: false,
            orig_bpc: 8,
            lossy_decode: false,
        };
        let e = encode::jpeg(&small, q, true, cs.clone())?;
        let l = resample::upscale(&e.decoded.as_ref()?.luma(), bw, bh, 1, rw, rh);
        Some((e, l))
    };
    let fg_layer = |scale: u32, q: u8| -> Option<(Encoded, Vec<u8>)> {
        let (sw, sh) = ((img.width / scale).max(8), (img.height / scale).max(8));
        let small = if scale == fg_base_scale {
            fg_base.clone()
        } else {
            RawImage {
                width: sw,
                height: sh,
                channels: img.channels,
                data: resample::resize(
                    &fg_base.data,
                    fw,
                    fh,
                    img.channels,
                    sw,
                    sh,
                    image::imageops::FilterType::Triangle,
                ),
                cs: img.cs,
                is_mask: false,
                orig_bpc: 8,
                lossy_decode: false,
            }
        };
        let e = encode::jpeg(&small, q, true, cs.clone())?;
        let l = resample::upscale(&e.decoded.as_ref()?.luma(), sw, sh, 1, rw, rh);
        Some((e, l))
    };
    // Gate: global mean SSIM ≥ target AND the 10th percentile of content
    // tiles ≥ target − 0.08 (the degradation a page-level JPEG would show),
    // so a blurred photo cannot hide behind flat paper.
    let recompose = |bg_l: &[u8], fg_l: &[u8]| -> Vec<u8> {
        let mut rec = vec![0u8; (rw * rh) as usize];
        for i in 0..rec.len() {
            let a = mask_ref[i] as u32;
            rec[i] = ((fg_l[i] as u32 * a + bg_l[i] as u32 * (255 - a)) / 255) as u8;
        }
        rec
    };
    let gate = |bg_l: &[u8], fg_l: &[u8]| -> f64 {
        let rec = recompose(bg_l, fg_l);
        let (mean, p10) = quality::ssim_mean_and_p10(&ref_luma, &rec, rw as usize, rh as usize, 96);
        if p10 < target - 0.08 {
            // Report the local failure as the score so callers reject it.
            p10.min(mean)
        } else {
            mean
        }
    };

    // Pass 1: background, with a safe foreground (scale 4, q 45).
    let (fg0, fg0_l) = fg_layer(fg_base_scale, 45)?;
    let mut best_bg: Option<(u32, Encoded, Vec<u8>)> = None;
    'bg: for scale in [8u32, 6, 4, 3, 2]
        .into_iter()
        .filter(|&s| s <= opts.mrc_max_bg_scale.max(2))
    {
        for q in [30u8, 45, 60] {
            if let Some((e, l)) = bg_layer(scale, q) {
                let s = gate(&l, &fg0_l);
                log::debug!("mrc bg scale {scale} q{q}: {}B ssim {s:.4}", e.len());
                if s >= target {
                    if best_bg
                        .as_ref()
                        .map(|b| e.len() < b.1.len())
                        .unwrap_or(true)
                    {
                        best_bg = Some((scale, e, l));
                    }
                    continue 'bg; // larger scales are always smaller than the next scale: keep first pass
                }
            }
        }
    }
    let (bg_scale, bg_enc, bg_l) = best_bg?;
    // Pass 2: foreground given that background.
    let mut best_fg: Option<(u32, Encoded, Vec<u8>, f64)> = None;
    for (scale, q) in [(8u32, 30u8), (8, 45), (4, 30), (4, 45), (4, 60)]
        .into_iter()
        .filter(|&(s, _)| s <= opts.mrc_max_fg_scale.max(4))
    {
        if let Some((e, l)) = fg_layer(scale, q) {
            let s = gate(&bg_l, &l);
            log::debug!("mrc fg scale {scale} q{q}: {}B ssim {s:.4}", e.len());
            if s >= target
                && best_fg
                    .as_ref()
                    .map(|b| e.len() < b.1.len())
                    .unwrap_or(true)
            {
                best_fg = Some((scale, e, l, s));
            }
        }
    }
    let (fg_scale, fg_enc, fg_l, ssim) = match best_fg {
        Some(b) => b,
        None => {
            let s = gate(&bg_l, &fg0_l);
            (fg_base_scale, fg0, fg0_l, s)
        }
    };
    if ssim < target {
        return None;
    }
    let recomposed_ref = recompose(&bg_l, &fg_l);
    let strip = |mut e: Encoded| {
        e.decoded = None;
        e
    };
    log::debug!(
        "mrc done in {} ms: bg/{bg_scale} fg/{fg_scale} mask {} bg {} fg {} ssim {ssim:.4}",
        t0.elapsed().as_millis(),
        mask_enc.len(),
        bg_enc.len(),
        fg_enc.len()
    );
    Some(MrcLayers {
        mask: mask_enc,
        fg: strip(fg_enc),
        bg: strip(bg_enc),
        ssim,
        bg_scale,
        fg_scale,
        recomposed_ref,
        ref_w: rw,
        ref_h: rh,
    })
}

fn image_stream(e: &Encoded, cs: Object, extra: impl FnOnce(&mut Dictionary)) -> Stream {
    let mut d = Dictionary::new();
    d.set("Type", Object::Name(b"XObject".to_vec()));
    d.set("Subtype", Object::Name(b"Image".to_vec()));
    d.set("Width", e.width as i64);
    d.set("Height", e.height as i64);
    d.set("ColorSpace", cs);
    d.set("BitsPerComponent", e.bpc as i64);
    if let Some(f) = e.filter {
        d.set("Filter", Object::Name(f.as_bytes().to_vec()));
    }
    if let Some(p) = &e.decode_parms {
        d.set("DecodeParms", Object::Dictionary(p.clone()));
    }
    extra(&mut d);
    Stream::new(d, e.data.clone())
}

/// Replace image object `id` with a Form XObject that paints bg then fg∘mask.
pub fn apply(doc: &mut Document, id: ObjectId, layers: &MrcLayers, kind: ColorKind) {
    let cs = encode::cs_object(kind);
    let mask_id = doc.add_object(Object::Stream(image_stream(
        &layers.mask,
        Object::Name(b"DeviceGray".to_vec()),
        |_| {},
    )));
    let fg_id = doc.add_object(Object::Stream(image_stream(&layers.fg, cs.clone(), |d| {
        d.set("SMask", Object::Reference(mask_id))
    })));
    let bg_id = doc.add_object(Object::Stream(image_stream(&layers.bg, cs, |d| {
        d.set("Interpolate", true)
    })));
    let mut xobjs = Dictionary::new();
    xobjs.set("Bg", Object::Reference(bg_id));
    xobjs.set("Fg", Object::Reference(fg_id));
    let mut res = Dictionary::new();
    res.set("XObject", Object::Dictionary(xobjs));
    let mut d = Dictionary::new();
    d.set("Type", Object::Name(b"XObject".to_vec()));
    d.set("Subtype", Object::Name(b"Form".to_vec()));
    d.set(
        "BBox",
        Object::Array(vec![0.into(), 0.into(), 1.into(), 1.into()]),
    );
    d.set("Resources", Object::Dictionary(res));
    let form = Stream::new(d, b"/Bg Do /Fg Do".to_vec());
    doc.objects.insert(id, Object::Stream(form));
}
