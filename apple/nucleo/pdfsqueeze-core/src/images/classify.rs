//! Cheap content statistics that drive candidate selection.

use super::RawImage;
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Exactly two values (0/255) per channel — line art, masks, scans.
    Bilevel,
    /// ≤256 distinct colours — logos, charts, screenshots of UIs.
    Palette,
    /// Continuous tone — photographs, gradients, scans of photos.
    Continuous,
}

#[derive(Debug, Clone)]
pub struct Stats {
    pub kind: Kind,
    pub unique_colors: usize,
    /// True when R≈G≈B everywhere (within `gray_tolerance`).
    pub is_gray: bool,
    /// True when R==G==B exactly (a lossless gray conversion is possible).
    pub is_gray_exact: bool,
    /// Fraction of luma samples that read as paper/background (>= 190).
    pub white_frac: f32,
    /// Fraction of luma samples near black (<= 40).
    pub dark_frac: f32,
    /// Looks like a scanned/typed text page: bright background, sparse dark ink.
    pub text_like: bool,
    /// Fraction of pixels that are exactly 0 or 255 in luma.
    pub extreme_frac: f32,
}

pub fn stats(img: &RawImage) -> Stats {
    let npix = (img.width * img.height) as usize;
    let ch = img.channels as usize;
    let stride = (npix / 200_000).max(1); // sample at most ~200k pixels
    let mut uniq: HashSet<u32> = HashSet::new();
    let mut is_gray = true;
    let mut is_gray_exact = true;
    let mut white = 0usize;
    let mut dark = 0usize;
    let mut extreme = 0usize;
    let mut sampled = 0usize;
    let luma = img.luma();
    let mut i = 0;
    while i < npix {
        let p = &img.data[i * ch..i * ch + ch];
        let key = match ch {
            1 => p[0] as u32,
            3 => (p[0] as u32) << 16 | (p[1] as u32) << 8 | p[2] as u32,
            _ => (p[0] as u32) << 24 | (p[1] as u32) << 16 | (p[2] as u32) << 8 | p[3] as u32,
        };
        if uniq.len() <= 4096 {
            uniq.insert(key);
        }
        if ch == 3 {
            let (r, g, b) = (p[0] as i32, p[1] as i32, p[2] as i32);
            let spread = (r - g).abs().max((g - b).abs()).max((r - b).abs());
            if spread > 0 {
                is_gray_exact = false;
            }
            if spread > 6 {
                is_gray = false;
            }
        } else if ch == 4 {
            is_gray = false;
            is_gray_exact = false;
        }
        let l = luma[i];
        if l >= 190 {
            white += 1;
        }
        if l <= 40 {
            dark += 1;
        }
        if l == 0 || l == 255 {
            extreme += 1;
        }
        sampled += 1;
        i += stride;
    }
    let sampled = sampled.max(1) as f32;
    let white_frac = white as f32 / sampled;
    let dark_frac = dark as f32 / sampled;
    let extreme_frac = extreme as f32 / sampled;
    let unique_colors = uniq.len();
    let bilevel = unique_colors <= 2 && img.data.iter().all(|&v| v == 0 || v == 255);
    let kind = if bilevel {
        Kind::Bilevel
    } else if unique_colors <= 256 && stride == 1 {
        Kind::Palette
    } else {
        Kind::Continuous
    };
    let text_like = white_frac > 0.50
        && dark_frac > 0.005
        && dark_frac < 0.35
        && img.width >= 600
        && img.height >= 600;
    Stats {
        kind,
        unique_colors,
        is_gray: is_gray && ch != 4,
        is_gray_exact: is_gray_exact && ch == 3,
        white_frac,
        dark_frac,
        text_like,
        extreme_frac,
    }
}
