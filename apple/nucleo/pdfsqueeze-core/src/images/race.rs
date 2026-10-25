//! Codec racing: encode an image every plausible way, measure, keep the
//! smallest candidate that satisfies the quality constraint.

use super::classify::{self, Kind, Stats};
use super::encode::{self, Encoded};
use super::quality;
use super::resample;
use super::RawImage;
use crate::deflate::Effort;
use crate::options::Options;
use lopdf::Object;

#[derive(Clone)]
pub struct RaceContext<'a> {
    pub opts: &'a Options,
    pub effort: Effort,
    /// Effective DPI at the largest placement, when known.
    pub effective_dpi: Option<f32>,
    /// The image's current /ColorSpace object (kept for exact candidates).
    pub orig_cs: Option<Object>,
    /// Colour-key masking (`/Mask [..]`) depends on exact sample values.
    pub color_key_mask: bool,
    /// The image is an SMask (soft mask): always gray, never palette/bilevel.
    pub is_smask: bool,
    /// The source is already in a lossy codec (DCT/JPX): exact Flate
    /// re-encodes of continuous-tone content cannot win and are skipped.
    pub source_lossy: bool,
    /// A candidate already in hand (e.g. MRC layers) sets the size a lossy
    /// JPEG must beat; the search aborts as soon as that is impossible.
    pub size_budget: Option<usize>,
}

#[derive(Debug)]
pub struct RaceResult {
    pub best: Encoded,
    pub ssim: Option<f64>,
    pub candidates: u32,
    pub kind: Kind,
    pub stats: Stats,
    pub action: String,
}

fn device_cs(kind: super::ColorKind) -> Object {
    encode::cs_object(kind)
}

pub fn race(img: &RawImage, ctx: &RaceContext) -> Option<RaceResult> {
    let t0 = std::time::Instant::now();
    let stats = classify::stats(img);
    let opts = ctx.opts;
    // Phase 1 races with plain deflate; phase 2 zopfli-squeezes only the winner.
    let full_effort = ctx.effort;
    let ctx = &RaceContext {
        effort: Effort::fast(),
        ..ctx.clone()
    };
    let mut cands: Vec<(Encoded, Option<f64>)> = Vec::new();
    let orig_cs = ctx.orig_cs.clone().unwrap_or_else(|| device_cs(img.cs));
    let orig_luma = img.luma();
    let reference = Reference::new(&orig_luma, img.width, img.height);
    let exact_ok = !img.lossy_decode;

    // ---- exact candidates -------------------------------------------------
    if exact_ok {
        if img.is_mask
            || (stats.kind == Kind::Bilevel && (img.channels == 1 || stats.is_gray_exact))
        {
            let g = if img.channels == 1 {
                img.data.clone()
            } else {
                orig_luma.clone()
            };
            cands.push((
                encode::bilevel_flate(&g, img.width, img.height, ctx.effort, img.is_mask),
                None,
            ));
            if let Some(c) = encode::ccitt_g4(&g, img.width, img.height, img.is_mask) {
                cands.push((c, None));
            }
            if let Some(c) = encode::jbig2(&g, img.width, img.height, img.is_mask) {
                cands.push((c, None));
            }
        } else if !ctx.color_key_mask {
            if stats.is_gray_exact && !ctx.is_smask {
                let g = encode::to_gray(img);
                for c in encode::flate(&g, ctx.effort, Some(device_cs(super::ColorKind::Gray))) {
                    cands.push((c, None));
                }
                if stats.kind == Kind::Palette {
                    if let Some(c) =
                        encode::indexed(&g, ctx.effort, device_cs(super::ColorKind::Gray))
                    {
                        cands.push((c, None));
                    }
                }
            }
            if stats.kind == Kind::Palette && !ctx.is_smask {
                if let Some(c) = encode::indexed(img, ctx.effort, orig_cs.clone()) {
                    cands.push((c, None));
                }
            }
        }
        if !img.is_mask && !(ctx.source_lossy && stats.kind == Kind::Continuous) {
            for c in encode::flate(img, ctx.effort, None) {
                cands.push((c, None));
            }
        }
    }

    // ---- lossy candidates -------------------------------------------------
    let lossy_allowed = opts.allow_lossy
        && !img.is_mask
        && !ctx.color_key_mask
        && stats.kind != Kind::Bilevel
        && img.cs != super::ColorKind::Other3;
    let mut best_ssim: Option<f64> = None;
    if lossy_allowed {
        // Downsample plan from the effective DPI.
        let mut work = img.clone();
        let mut scaled = false;
        if let (Some(dpi), Some(max)) = (ctx.effective_dpi, opts.max_dpi) {
            if dpi > max * 1.15 {
                let s = (max / dpi).max(opts.min_scale);
                let nw = ((img.width as f32 * s).round() as u32).max(16);
                let nh = ((img.height as f32 * s).round() as u32).max(16);
                if nw < img.width && nh < img.height {
                    work = RawImage {
                        width: nw,
                        height: nh,
                        channels: img.channels,
                        data: resample::downscale(
                            &img.data,
                            img.width,
                            img.height,
                            img.channels,
                            nw,
                            nh,
                        ),
                        ..img.clone()
                    };
                    scaled = true;
                }
            }
        }
        if stats.is_gray && img.channels == 3 {
            work = encode::to_gray(&work);
        }
        // Keep the original colour space object (ICC profiles included) whenever
        // the channel count is unchanged; resampling does not alter colour semantics.
        let work_cs = if work.channels == img.channels {
            orig_cs.clone()
        } else {
            device_cs(work.cs)
        };
        // Gray/downsampled Flate candidates for flat content.
        let wstats = classify::stats(&work);
        if wstats.kind == Kind::Palette && (scaled || work.channels != img.channels) {
            if let Some(c) = encode::indexed(&work, ctx.effort, work_cs.clone()) {
                let s = reference.gate(&work);
                if s >= opts.min_ssim {
                    cands.push((lossy_of(c, work.clone()), Some(s)));
                }
            }
        }
        if (scaled || work.channels != img.channels) && wstats.kind == Kind::Palette {
            for c in encode::flate(&work, ctx.effort, Some(work_cs.clone())) {
                let s = reference.gate(&work);
                if s >= opts.min_ssim {
                    cands.push((lossy_of(c, work.clone()), Some(s)));
                }
            }
        }
        // Near-bilevel gray → true bilevel, gated.
        if work.channels == 1 && wstats.extreme_frac > 0.85 {
            let bits: Vec<u8> = work
                .data
                .iter()
                .map(|&v| if v >= 128 { 255 } else { 0 })
                .collect();
            let bw = RawImage {
                data: bits.clone(),
                ..work.clone()
            };
            let s = reference.gate(&bw);
            if s >= opts.min_ssim {
                cands.push((
                    lossy_of(
                        encode::bilevel_flate(&bits, work.width, work.height, ctx.effort, false),
                        bw.clone(),
                    ),
                    Some(s),
                ));
                if let Some(c) = encode::ccitt_g4(&bits, work.width, work.height, false) {
                    cands.push((lossy_of(c, bw.clone()), Some(s)));
                }
                if let Some(c) = encode::jbig2(&bits, work.width, work.height, false) {
                    cands.push((lossy_of(c, bw), Some(s)));
                }
            }
        }
        // JPEG quality search, 4:2:0 first (cheaper), 4:4:4 as fallback.
        if work.channels != 4 {
            let mut over_budget = false;
            for sub in [true, false] {
                if sub && work.channels == 1 {
                    continue;
                }
                match jpeg_search(
                    &work,
                    &reference,
                    opts,
                    sub,
                    work_cs.clone(),
                    ctx.size_budget,
                ) {
                    Search::Found(c, s) => {
                        best_ssim = Some(best_ssim.map_or(s, |b: f64| b.max(s)));
                        cands.push((c, Some(s)));
                        // Only try 4:4:4 if 4:2:0 was rejected or the image is small
                        // enough that chroma detail matters.
                        if sub && work.width * work.height > 400 * 400 {
                            break;
                        }
                    }
                    // 4:4:4 is never smaller than 4:2:0 at the same quality, and a
                    // full-size JPEG is never smaller than a downsampled one.
                    Search::OverBudget => {
                        over_budget = true;
                        break;
                    }
                    Search::Failed => {}
                }
            }
            // Downsampling might have been what killed quality: retry at full size.
            if scaled && !over_budget && cands.iter().all(|(c, _)| !c.lossy) {
                let full = if stats.is_gray && img.channels == 3 {
                    encode::to_gray(img)
                } else {
                    img.clone()
                };
                if let Search::Found(c, s) = jpeg_search(
                    &full,
                    &reference,
                    opts,
                    true,
                    if full.channels == img.channels {
                        orig_cs.clone()
                    } else {
                        device_cs(full.cs)
                    },
                    ctx.size_budget,
                ) {
                    cands.push((c, Some(s)));
                }
            }
        }
    }

    let candidates = cands.len() as u32;
    let (mut best, ssim) = cands.into_iter().min_by_key(|(c, _)| c.len())?;
    let t_sq = std::time::Instant::now();
    encode::squeeze(&mut best, full_effort);
    best.raw = None;
    log::debug!(
        "race {}x{} kind={:?}: {} candidates, best={} {}B, squeeze {} ms, total {} ms",
        img.width,
        img.height,
        stats.kind,
        candidates,
        best.label,
        best.len(),
        t_sq.elapsed().as_millis(),
        t0.elapsed().as_millis()
    );
    let action = format!(
        "{}{}",
        best.label,
        if best.width != img.width {
            format!(
                " {}x{}→{}x{}",
                img.width, img.height, best.width, best.height
            )
        } else {
            String::new()
        }
    );
    Some(RaceResult {
        best,
        ssim,
        candidates,
        kind: stats.kind,
        stats,
        action,
    })
}

fn lossy_of(mut c: Encoded, decoded: RawImage) -> Encoded {
    c.lossy = true;
    c.decoded = Some(decoded);
    c
}

/// Reference plane for the quality gate: the original luma, box-downscaled
/// once to at most ~1.5 MP. Every candidate is brought to the same size, so
/// downsampling is still penalised while the per-candidate cost stays bounded
/// (no 3.6 MP upscale + SSIM per search step on a 300-dpi scan).
struct Reference {
    luma: Vec<u8>,
    w: u32,
    h: u32,
}

impl Reference {
    fn new(orig_luma: &[u8], w: u32, h: u32) -> Self {
        const CAP: u64 = 1_500_000;
        let n = w as u64 * h as u64;
        if n <= CAP {
            return Reference {
                luma: orig_luma.to_vec(),
                w,
                h,
            };
        }
        let f = ((n as f64 / CAP as f64).sqrt()).ceil() as u32;
        let (nw, nh) = ((w / f).max(8), (h / f).max(8));
        Reference {
            luma: resample::resize(
                orig_luma,
                w,
                h,
                1,
                nw,
                nh,
                image::imageops::FilterType::Triangle,
            ),
            w: nw,
            h: nh,
        }
    }

    fn gate(&self, cand: &RawImage) -> f64 {
        let l = cand.luma();
        let l = if cand.width != self.w || cand.height != self.h {
            resample::resize(
                &l,
                cand.width,
                cand.height,
                1,
                self.w,
                self.h,
                image::imageops::FilterType::Triangle,
            )
        } else {
            l
        };
        quality::ssim(&self.luma, &l, self.w as usize, self.h as usize)
    }
}

/// Lowest JPEG quality that still satisfies `min_ssim`, by binary search
/// (SSIM is monotone enough in quality for this to be sound).
// Vive un instante en la pila y se consume enseguida; meterlo en Box no gana nada.
#[allow(clippy::large_enum_variant)]
enum Search {
    Found(Encoded, f64),
    OverBudget,
    Failed,
}

fn jpeg_search(
    work: &RawImage,
    reference: &Reference,
    opts: &Options,
    sub420: bool,
    cs: Object,
    budget: Option<usize>,
) -> Search {
    let t0 = std::time::Instant::now();
    let mut steps = 1u32;
    let (mut lo, mut hi) = (
        opts.jpeg_min_quality.min(opts.jpeg_max_quality),
        opts.jpeg_max_quality,
    );
    // Probe the floor first: the lowest quality is the smallest JPEG we can
    // make. If it passes the gate it is the answer; if it is already larger
    // than the budget, no higher quality can win either.
    let floor = match encode::jpeg(work, lo, sub420, cs.clone()) {
        Some(e) => e,
        None => return Search::Failed,
    };
    if let Some(b) = budget {
        if floor.len() >= b {
            log::debug!(
                "  jpeg-{} {}x{}: floor {}B ≥ budget {}B, skipped",
                if sub420 { "420" } else { "444" },
                work.width,
                work.height,
                floor.len(),
                b
            );
            return Search::OverBudget;
        }
    }
    let s_floor = match floor.decoded.as_ref() {
        Some(d) => reference.gate(d),
        None => return Search::Failed,
    };
    if s_floor >= opts.min_ssim {
        log::debug!(
            "  jpeg-{} {}x{}: floor q{} passes, 1 step",
            if sub420 { "420" } else { "444" },
            work.width,
            work.height,
            lo
        );
        let mut c = floor;
        c.decoded = None;
        return Search::Found(c, s_floor);
    }
    // Then the ceiling; if even that fails, bail early.
    let top = match encode::jpeg(work, hi, sub420, cs.clone()) {
        Some(e) => e,
        None => return Search::Failed,
    };
    steps += 1;
    let s = match top.decoded.as_ref() {
        Some(d) => reference.gate(d),
        None => return Search::Failed,
    };
    if s < opts.min_ssim {
        return Search::Failed;
    }
    let mut best: Option<(Encoded, f64)> = Some((top, s));
    let step = opts.jpeg_quality_step.max(1);
    while lo + step <= hi {
        let mid = (lo + hi) / 2;
        if mid == hi {
            break;
        }
        let c = match encode::jpeg(work, mid, sub420, cs.clone()) {
            Some(e) => e,
            None => break,
        };
        steps += 1;
        let s = match c.decoded.as_ref() {
            Some(d) => reference.gate(d),
            None => break,
        };
        if s >= opts.min_ssim {
            hi = mid;
            best = Some((c, s));
        } else {
            lo = mid + 1;
        }
    }
    log::debug!(
        "  jpeg-{} {}x{}: {} steps, {} ms",
        if sub420 { "420" } else { "444" },
        work.width,
        work.height,
        steps,
        t0.elapsed().as_millis()
    );
    match best {
        Some((mut c, s)) => {
            c.decoded = None;
            Search::Found(c, s)
        }
        None => Search::Failed,
    }
}
