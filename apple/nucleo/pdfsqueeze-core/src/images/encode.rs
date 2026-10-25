//! Candidate encoders. Every candidate is a complete, standards-compliant
//! image XObject payload (filter + params + colour space + bit depth).

use super::{ColorKind, RawImage};
use crate::deflate::{self, Effort};
use lopdf::{Dictionary, Object};

#[derive(Debug, Clone)]
pub struct Encoded {
    pub label: &'static str,
    pub data: Vec<u8>,
    pub filter: Option<&'static str>,
    pub decode_parms: Option<Dictionary>,
    /// `None` keeps the original ColorSpace object; `Some` replaces it.
    pub colorspace: Option<Object>,
    pub bpc: u8,
    pub width: u32,
    pub height: u32,
    pub lossy: bool,
    /// Pixels as the reader will see them (for quality evaluation); `None`
    /// for lossless candidates (identical to the source).
    pub decoded: Option<RawImage>,
    /// Uncompressed payload for Flate candidates, so the winner can be
    /// re-squeezed with zopfli after the race has been decided cheaply.
    pub raw: Option<Vec<u8>>,
}

impl Encoded {
    pub fn len(&self) -> usize {
        self.data.len()
            + self
                .decode_parms
                .as_ref()
                .map(|d| crate::writer::serialized_len(&Object::Dictionary(d.clone())))
                .unwrap_or(0)
    }
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

pub fn cs_object(kind: ColorKind) -> Object {
    Object::Name(match kind {
        ColorKind::Gray => b"DeviceGray".to_vec(),
        ColorKind::Rgb => b"DeviceRGB".to_vec(),
        ColorKind::Cmyk => b"DeviceCMYK".to_vec(),
        ColorKind::Other3 => b"DeviceRGB".to_vec(),
    })
}

/// Flate with the best PNG predictor per row (like optipng's heuristic).
pub fn flate(img: &RawImage, effort: Effort, cs: Option<Object>) -> Vec<Encoded> {
    let mut out = Vec::new();
    let raw = deflate::zlib(&img.data, effort);
    out.push(Encoded {
        label: "flate",
        raw: Some(img.data.clone()),
        data: raw,
        filter: Some("FlateDecode"),
        decode_parms: None,
        colorspace: cs.clone(),
        bpc: 8,
        width: img.width,
        height: img.height,
        lossy: false,
        decoded: None,
    });
    // PNG predictors help photos/gradients a lot and cost nothing to try.
    let predicted = png_predict(&img.data, img.width as usize, img.channels as usize);
    let pred = deflate::zlib(&predicted, effort);
    let mut parms = Dictionary::new();
    parms.set("Predictor", 15);
    parms.set("Colors", img.channels as i64);
    parms.set("BitsPerComponent", 8);
    parms.set("Columns", img.width as i64);
    out.push(Encoded {
        label: "flate+png",
        raw: Some(predicted),
        data: pred,
        filter: Some("FlateDecode"),
        decode_parms: Some(parms),
        colorspace: cs.clone(),
        bpc: 8,
        width: img.width,
        height: img.height,
        lossy: false,
        decoded: None,
    });
    out
}

/// Choose per row among None/Sub/Up/Average/Paeth by minimum sum of absolute
/// residuals; emit PNG-style rows (filter byte + data).
pub fn png_predict(data: &[u8], width: usize, bpp: usize) -> Vec<u8> {
    let row_len = width * bpp;
    let rows = data.len() / row_len.max(1);
    let mut out = Vec::with_capacity(data.len() + rows);
    let zero = vec![0u8; row_len];
    let mut buf = vec![0u8; row_len];
    for r in 0..rows {
        let cur = &data[r * row_len..(r + 1) * row_len];
        let prev = if r == 0 {
            &zero[..]
        } else {
            &data[(r - 1) * row_len..r * row_len]
        };
        let mut best_score = u64::MAX;
        let mut best_type = 0u8;
        for t in 0..5u8 {
            filter_row(t, cur, prev, bpp, &mut buf);
            let score: u64 = buf.iter().map(|&v| (v as i8).unsigned_abs() as u64).sum();
            if score < best_score {
                best_score = score;
                best_type = t;
            }
        }
        filter_row(best_type, cur, prev, bpp, &mut buf);
        out.push(best_type);
        out.extend_from_slice(&buf);
    }
    out
}

fn filter_row(t: u8, cur: &[u8], prev: &[u8], bpp: usize, out: &mut [u8]) {
    for i in 0..cur.len() {
        let a = if i >= bpp { cur[i - bpp] } else { 0 };
        let b = prev[i];
        let c = if i >= bpp { prev[i - bpp] } else { 0 };
        let pred = match t {
            0 => 0,
            1 => a,
            2 => b,
            3 => ((a as u16 + b as u16) / 2) as u8,
            _ => paeth(a, b, c),
        };
        out[i] = cur[i].wrapping_sub(pred);
    }
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = a as i16 + b as i16 - c as i16;
    let pa = (p - a as i16).abs();
    let pb = (p - b as i16).abs();
    let pc = (p - c as i16).abs();
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

/// Gray conversion (lossless when exact, otherwise lossy) followed by Flate.
pub fn to_gray(img: &RawImage) -> RawImage {
    RawImage {
        width: img.width,
        height: img.height,
        channels: 1,
        data: img.luma(),
        cs: ColorKind::Gray,
        is_mask: false,
        orig_bpc: 8,
        lossy_decode: false,
    }
}

/// Indexed palette (≤256 colours) at the smallest bit depth that fits.
pub fn indexed(img: &RawImage, effort: Effort, base: Object) -> Option<Encoded> {
    if img.channels == 1 || img.channels > 4 {
        return None;
    }
    let ch = img.channels as usize;
    let mut palette: Vec<[u8; 4]> = Vec::new();
    let mut indices: Vec<u8> = Vec::with_capacity(img.data.len() / ch);
    let mut lut = std::collections::HashMap::<u32, u8>::new();
    for p in img.data.chunks_exact(ch) {
        let key = p.iter().fold(0u32, |k, &v| (k << 8) | v as u32);
        let idx = match lut.get(&key) {
            Some(&i) => i,
            None => {
                if palette.len() >= 256 {
                    return None;
                }
                let mut c = [0u8; 4];
                c[..ch].copy_from_slice(p);
                palette.push(c);
                let i = (palette.len() - 1) as u8;
                lut.insert(key, i);
                i
            }
        };
        indices.push(idx);
    }
    let bpc: u8 = if palette.len() <= 2 {
        1
    } else if palette.len() <= 4 {
        2
    } else if palette.len() <= 16 {
        4
    } else {
        8
    };
    let packed = pack_bits(&indices, img.width as usize, img.height as usize, bpc);
    let data = deflate::zlib(&packed, effort);
    let raw = Some(packed);
    let lookup: Vec<u8> = palette.iter().flat_map(|c| c[..ch].to_vec()).collect();
    let cs = Object::Array(vec![
        Object::Name(b"Indexed".to_vec()),
        base,
        Object::Integer(palette.len() as i64 - 1),
        Object::String(lookup, lopdf::StringFormat::Hexadecimal),
    ]);
    Some(Encoded {
        label: "indexed",
        data,
        filter: Some("FlateDecode"),
        decode_parms: None,
        colorspace: Some(cs),
        bpc,
        width: img.width,
        height: img.height,
        lossy: false,
        decoded: None,
        raw,
    })
}

pub fn pack_bits(samples: &[u8], w: usize, h: usize, bpc: u8) -> Vec<u8> {
    if bpc == 8 {
        return samples.to_vec();
    }
    let row_bytes = (w * bpc as usize).div_ceil(8);
    let mut out = vec![0u8; row_bytes * h];
    for y in 0..h {
        for x in 0..w {
            let v = samples[y * w + x];
            let bit = x * bpc as usize;
            let shift = 8 - bpc as usize - (bit % 8);
            out[y * row_bytes + bit / 8] |= (v & ((1u16 << bpc) - 1) as u8) << shift;
        }
    }
    out
}

/// 1-bit DeviceGray, Flate-compressed (exact for bilevel input).
pub fn bilevel_flate(gray: &[u8], w: u32, h: u32, effort: Effort, is_mask: bool) -> Encoded {
    let bits: Vec<u8> = gray.iter().map(|&v| if v >= 128 { 1 } else { 0 }).collect();
    let packed = pack_bits(&bits, w as usize, h as usize, 1);
    Encoded {
        label: "bilevel-flate",
        raw: Some(packed.clone()),
        data: deflate::zlib(&packed, effort),
        filter: Some("FlateDecode"),
        decode_parms: None,
        colorspace: if is_mask {
            None
        } else {
            Some(Object::Name(b"DeviceGray".to_vec()))
        },
        bpc: 1,
        width: w,
        height: h,
        lossy: false,
        decoded: None,
    }
}

/// CCITT Group 4 (ITU-T T.6), the classic fax/scan codec: exact for bilevel input.
pub fn ccitt_g4(gray: &[u8], w: u32, h: u32, is_mask: bool) -> Option<Encoded> {
    if w > u16::MAX as u32 || h > u16::MAX as u32 {
        return None;
    }
    let mut enc = fax::encoder::Encoder::new(fax::VecWriter::new());
    for y in 0..h as usize {
        let row = &gray[y * w as usize..(y + 1) * w as usize];
        // BlackIs1=false (default): 0 bits are black. Gray 0 → black.
        let pels = row.iter().map(|&v| {
            if v < 128 {
                fax::Color::Black
            } else {
                fax::Color::White
            }
        });
        enc.encode_line(pels, w as u16).ok()?;
    }
    let data = enc.finish().ok()?.finish();
    let mut parms = Dictionary::new();
    parms.set("K", -1);
    parms.set("Columns", w as i64);
    parms.set("Rows", h as i64);
    Some(Encoded {
        label: "ccitt-g4",
        data,
        filter: Some("CCITTFaxDecode"),
        decode_parms: Some(parms),
        colorspace: if is_mask {
            None
        } else {
            Some(Object::Name(b"DeviceGray".to_vec()))
        },
        bpc: 1,
        width: w,
        height: h,
        lossy: false,
        decoded: None,
        raw: None,
    })
}

/// JBIG2 generic region (arithmetic, TPGDON): exact for bilevel input and
/// usually well under half the size of CCITT G4 on text.
pub fn jbig2(gray: &[u8], w: u32, h: u32, is_mask: bool) -> Option<Encoded> {
    if w == 0 || h == 0 || (w as u64) * (h as u64) > 50_000_000 {
        return None;
    }
    // JBIG2 1 = black; the /JBIG2Decode filter inverts, so DeviceGray 0 (black) ↔ JBIG2 1.
    let bits: Vec<u8> = gray.iter().map(|&v| (v < 128) as u8).collect();
    let bm = crate::jbig2::Bitmap {
        width: w,
        height: h,
        bits,
    };
    let data = crate::jbig2::encode_pdf_stream(&bm, true);
    Some(Encoded {
        label: "jbig2",
        data,
        filter: Some("JBIG2Decode"),
        decode_parms: None,
        colorspace: if is_mask {
            None
        } else {
            Some(Object::Name(b"DeviceGray".to_vec()))
        },
        bpc: 1,
        width: w,
        height: h,
        lossy: false,
        decoded: None,
        raw: None,
    })
}

/// Baseline JPEG at `quality` with optimised Huffman tables.
pub fn jpeg(img: &RawImage, quality: u8, subsample_420: bool, cs: Object) -> Option<Encoded> {
    if img.width > u16::MAX as u32 || img.height > u16::MAX as u32 {
        return None;
    }
    let ct = match img.channels {
        1 => jpeg_encoder::ColorType::Luma,
        3 => jpeg_encoder::ColorType::Rgb,
        4 => jpeg_encoder::ColorType::Cmyk,
        _ => return None,
    };
    let sampling = if subsample_420 && img.channels == 3 {
        jpeg_encoder::SamplingFactor::F_2_2
    } else {
        jpeg_encoder::SamplingFactor::F_1_1
    };
    let mut buf = Vec::with_capacity(img.data.len() / 8);
    let mut decoded = None;
    // Optimised Huffman tables save a few percent, but the encoder occasionally
    // emits a table our decoder rejects; fall back to the standard tables so
    // every emitted JPEG is one we have verified decodes.
    for optimized in [true, false] {
        buf.clear();
        let mut enc = jpeg_encoder::Encoder::new(&mut buf, quality);
        enc.set_optimized_huffman_tables(optimized);
        enc.set_sampling_factor(sampling);
        enc.encode(&img.data, img.width as u16, img.height as u16, ct)
            .ok()?;
        if let Some(d) = decode_jpeg(&buf, img) {
            decoded = Some(d);
            break;
        }
    }
    // Decode back for the quality gate (what the reader will actually show).
    let decoded = decoded?;
    Some(Encoded {
        label: if subsample_420 {
            "jpeg-420"
        } else {
            "jpeg-444"
        },
        data: buf,
        filter: Some("DCTDecode"),
        decode_parms: None,
        colorspace: Some(cs),
        bpc: 8,
        width: img.width,
        height: img.height,
        lossy: true,
        decoded: Some(decoded),
        raw: None,
    })
}

fn decode_jpeg(buf: &[u8], like: &RawImage) -> Option<RawImage> {
    if like.channels == 4 {
        // We don't round-trip CMYK through the decoder; treat as unverifiable.
        return None;
    }
    let mut dec = jpeg_decoder::Decoder::new(std::io::Cursor::new(buf));
    let px = match dec.decode() {
        Ok(p) => p,
        Err(e) => {
            log::debug!(
                "jpeg round-trip decode failed ({}x{} ch={}): {e:?}",
                like.width,
                like.height,
                like.channels
            );
            return None;
        }
    };
    let ch = match dec.info().map(|i| i.pixel_format) {
        Some(jpeg_decoder::PixelFormat::L8) => 1,
        Some(jpeg_decoder::PixelFormat::RGB24) => 3,
        _ => return None,
    };
    if ch != like.channels {
        return None;
    }
    Some(RawImage {
        width: like.width,
        height: like.height,
        channels: ch,
        data: px,
        cs: like.cs,
        is_mask: false,
        orig_bpc: 8,
        lossy_decode: false,
    })
}

/// Re-squeeze a Flate candidate with the (expensive) effort after it has won.
pub fn squeeze(e: &mut Encoded, effort: Effort) {
    if let Some(raw) = &e.raw {
        if e.filter == Some("FlateDecode") {
            let better = deflate::zlib(raw, effort);
            if better.len() < e.data.len() {
                e.data = better;
            }
        }
    }
}
