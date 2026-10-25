//! Decode PDF image XObjects into 8-bit pixels.

use super::{ColorKind, RawImage};
use lopdf::{Dictionary, Document, Object, Stream};

#[derive(Debug, Clone)]
pub enum Cs {
    Gray,
    Rgb,
    Cmyk,
    Other3,
    Indexed { base: Box<Cs>, lookup: Vec<u8> },
    Unsupported,
}

impl Cs {
    pub fn components(&self) -> Option<u8> {
        match self {
            Cs::Gray => Some(1),
            Cs::Rgb | Cs::Other3 => Some(3),
            Cs::Cmyk => Some(4),
            Cs::Indexed { .. } => Some(1),
            Cs::Unsupported => None,
        }
    }
}

pub fn parse_colorspace(doc: &Document, obj: Option<&Object>) -> Cs {
    let obj = match obj {
        Some(o) => o,
        None => return Cs::Unsupported,
    };
    let (_, obj) = match doc.dereference(obj) {
        Ok(x) => x,
        Err(_) => return Cs::Unsupported,
    };
    match obj {
        Object::Name(n) => match n.as_slice() {
            b"DeviceGray" | b"G" | b"CalGray" => Cs::Gray,
            b"DeviceRGB" | b"RGB" | b"CalRGB" => Cs::Rgb,
            b"DeviceCMYK" | b"CMYK" => Cs::Cmyk,
            _ => Cs::Unsupported,
        },
        Object::Array(a) => {
            let head = a.first().and_then(|o| o.as_name().ok()).unwrap_or(b"");
            match head {
                b"ICCBased" => {
                    let n = a
                        .get(1)
                        .and_then(|o| doc.dereference(o).ok())
                        .and_then(|(_, o)| o.as_stream().ok())
                        .and_then(|s| s.dict.get(b"N").ok())
                        .and_then(|o| o.as_i64().ok());
                    match n {
                        Some(1) => Cs::Gray,
                        Some(3) => Cs::Rgb,
                        Some(4) => Cs::Cmyk,
                        _ => Cs::Unsupported,
                    }
                }
                b"CalRGB" => Cs::Rgb,
                b"CalGray" => Cs::Gray,
                b"Lab" => Cs::Other3,
                b"Indexed" | b"I" => {
                    let base = parse_colorspace(doc, a.get(1));
                    if matches!(base, Cs::Unsupported | Cs::Indexed { .. }) {
                        return Cs::Unsupported;
                    }
                    let lookup = match a.get(3).and_then(|o| doc.dereference(o).ok()) {
                        Some((_, Object::String(s, _))) => s.clone(),
                        Some((_, Object::Stream(s))) => match s.get_plain_content() {
                            Ok(c) => c,
                            Err(_) => return Cs::Unsupported,
                        },
                        _ => return Cs::Unsupported,
                    };
                    Cs::Indexed {
                        base: Box::new(base),
                        lookup,
                    }
                }
                _ => Cs::Unsupported,
            }
        }
        _ => Cs::Unsupported,
    }
}

const OUTER: [&[u8]; 6] = [
    b"FlateDecode",
    b"LZWDecode",
    b"ASCII85Decode",
    b"Fl",
    b"LZW",
    b"A85",
];

/// Apply the "outer" (generic) filters only, returning bytes for the final
/// image codec (or the raw samples when there is none).
// Tupla interna de un solo uso (bytes, parametros, dict del codec); un alias no aclara nada.
#[allow(clippy::type_complexity)]
fn outer_decoded(stream: &Stream) -> Option<(Vec<u8>, Option<Vec<u8>>, Option<Dictionary>)> {
    let filters: Vec<Vec<u8>> = stream
        .filters()
        .map(|v| v.iter().map(|f| f.to_vec()).collect())
        .unwrap_or_default();
    let parms: Vec<Option<Dictionary>> = match stream
        .dict
        .get(b"DecodeParms")
        .or_else(|_| stream.dict.get(b"DP"))
    {
        Ok(Object::Dictionary(d)) => vec![Some(d.clone())],
        Ok(Object::Array(a)) => a.iter().map(|o| o.as_dict().ok().cloned()).collect(),
        _ => vec![],
    };
    let split = filters.iter().position(|f| !OUTER.contains(&f.as_slice()));
    let (outer, codec): (Vec<Vec<u8>>, Option<(Vec<u8>, Option<Dictionary>)>) = match split {
        Some(i) => {
            if i != filters.len() - 1 {
                return None; // codec followed by more filters: not a real-world layout
            }
            (
                filters[..i].to_vec(),
                Some((filters[i].clone(), parms.get(i).cloned().flatten())),
            )
        }
        None => (filters.clone(), None),
    };
    let data = if outer.is_empty() {
        stream.content.clone()
    } else {
        // lopdf only honours a *dictionary* DecodeParms, so hand it the one
        // that carries a predictor (there is at most one in practice).
        let mut tmp = stream.clone();
        tmp.dict.set(
            "Filter",
            Object::Array(outer.iter().map(|f| Object::Name(f.clone())).collect()),
        );
        tmp.dict.remove(b"DP");
        let pred_parms = (0..outer.len())
            .filter_map(|i| parms.get(i).cloned().flatten())
            .find(|d| d.has(b"Predictor") || d.has(b"EarlyChange"));
        // Predictors are undone here (lopdf's PNG un-filtering is not exact).
        let mut png = None;
        match pred_parms {
            Some(mut d) => {
                let predictor = d.get(b"Predictor").and_then(Object::as_i64).unwrap_or(1);
                if predictor >= 10 {
                    let colors = d
                        .get(b"Colors")
                        .and_then(Object::as_i64)
                        .unwrap_or(1)
                        .max(1) as usize;
                    let bits = d
                        .get(b"BitsPerComponent")
                        .and_then(Object::as_i64)
                        .unwrap_or(8)
                        .max(1) as usize;
                    let columns = d
                        .get(b"Columns")
                        .and_then(Object::as_i64)
                        .unwrap_or(1)
                        .max(1) as usize;
                    png = Some((colors, bits, columns));
                    d.remove(b"Predictor");
                } else if predictor == 2 {
                    return None; // TIFF predictor: rare, leave untouched
                }
                tmp.dict.set("DecodeParms", Object::Dictionary(d));
            }
            None => {
                tmp.dict.remove(b"DecodeParms");
            }
        }
        let raw = tmp.decompressed_content().ok()?;
        match png {
            Some((colors, bits, columns)) => png_unpredict(&raw, colors, bits, columns)?,
            None => raw,
        }
    };
    match codec {
        Some((name, p)) => Some((data, Some(name), p)),
        None => Some((data, None, None)),
    }
}

fn decode_array(doc: &Document, dict: &Dictionary) -> Option<Vec<f32>> {
    let (_, o) = doc
        .dereference(dict.get(b"Decode").or_else(|_| dict.get(b"D")).ok()?)
        .ok()?;
    let a = o.as_array().ok()?;
    Some(
        a.iter()
            .filter_map(|x| match x {
                Object::Integer(i) => Some(*i as f32),
                Object::Real(r) => Some(*r),
                _ => None,
            })
            .collect(),
    )
}

/// Returns `None` for anything we cannot decode exactly enough to re-encode
/// (JPX, JBIG2, DeviceN, ...). Those streams are simply left untouched.
pub fn decode_image(doc: &Document, stream: &Stream) -> Option<RawImage> {
    let d = &stream.dict;
    let width = d
        .get(b"Width")
        .or_else(|_| d.get(b"W"))
        .ok()
        .and_then(|o| doc.dereference(o).ok())
        .and_then(|(_, o)| o.as_i64().ok())? as u32;
    let height = d
        .get(b"Height")
        .or_else(|_| d.get(b"H"))
        .ok()
        .and_then(|o| doc.dereference(o).ok())
        .and_then(|(_, o)| o.as_i64().ok())? as u32;
    if width == 0 || height == 0 || width > 20000 || height > 20000 {
        return None;
    }
    let is_mask = d
        .get(b"ImageMask")
        .or_else(|_| d.get(b"IM"))
        .ok()
        .and_then(|o| o.as_bool().ok())
        .unwrap_or(false);
    let bpc = if is_mask {
        1
    } else {
        d.get(b"BitsPerComponent")
            .or_else(|_| d.get(b"BPC"))
            .ok()
            .and_then(|o| doc.dereference(o).ok())
            .and_then(|(_, o)| o.as_i64().ok())
            .unwrap_or(8) as u8
    };
    let cs = if is_mask {
        Cs::Gray
    } else {
        parse_colorspace(doc, d.get(b"ColorSpace").or_else(|_| d.get(b"CS")).ok())
    };
    let ncomp = cs.components()?;
    let decode = decode_array(doc, d);
    let (data, codec, parms) = outer_decoded(stream)?;

    let npix = (width as usize) * (height as usize);
    let (mut pixels, channels, kind): (Vec<u8>, u8, ColorKind) = match codec.as_deref() {
        None => {
            let unpacked = unpack(&data, width, height, ncomp, bpc)?;
            expand_cs(unpacked, &cs, bpc)?
        }
        Some(b"DCTDecode") | Some(b"DCT") => {
            if bpc != 8 {
                return None;
            }
            // jpeg-decoder (a libjpeg port) is used on purpose: zune-jpeg 0.4/0.5
            // silently mis-decodes 4:2:0 streams with optimised Huffman tables.
            let mut dec = jpeg_decoder::Decoder::new(std::io::Cursor::new(&data));
            dec.read_info().ok()?;
            let info = dec.info()?;
            if info.width as u32 != width || info.height as u32 != height {
                return None;
            }
            let ch = match info.pixel_format {
                jpeg_decoder::PixelFormat::L8 => 1usize,
                jpeg_decoder::PixelFormat::RGB24 => 3,
                // CMYK/YCCK: Adobe inversion rules are too fragile to re-encode blind.
                _ => return None,
            };
            let px = dec.decode().ok()?;
            if px.len() < npix * ch {
                return None;
            }
            let kind = match (ch, &cs) {
                (1, _) => ColorKind::Gray,
                (3, Cs::Rgb) => ColorKind::Rgb,
                (3, Cs::Other3) => ColorKind::Other3,
                _ => return None,
            };
            (px, ch as u8, kind)
        }
        Some(b"CCITTFaxDecode") | Some(b"CCF") => {
            let p = parms.unwrap_or_default();
            let k = p.get(b"K").and_then(Object::as_i64).unwrap_or(0);
            let cols = p.get(b"Columns").and_then(Object::as_i64).unwrap_or(1728) as u32;
            let black_is_1 = p
                .get(b"BlackIs1")
                .and_then(Object::as_bool)
                .unwrap_or(false);
            let aligned = p
                .get(b"EncodedByteAlign")
                .and_then(Object::as_bool)
                .unwrap_or(false);
            if k >= 0 || aligned || cols != width || width > u16::MAX as u32 {
                return None;
            }
            let mut rows: Vec<Vec<u16>> = Vec::with_capacity(height as usize);
            fax::decoder::decode_g4(
                data.iter().copied(),
                width as u16,
                Some(height as u16),
                |t| rows.push(t.to_vec()),
            )?;
            if rows.len() < height as usize {
                return None;
            }
            let mut px = Vec::with_capacity(npix);
            for row in rows.iter().take(height as usize) {
                for c in fax::decoder::pels(row, width as u16) {
                    // Filter output bit: BlackIs1=false → black=0. DeviceGray 1-bit: 0=black.
                    let bit_black = if black_is_1 { 1u8 } else { 0u8 };
                    let bit = if c == fax::Color::Black {
                        bit_black
                    } else {
                        1 - bit_black
                    };
                    px.push(if bit == 1 { 255 } else { 0 });
                }
            }
            (px, 1, ColorKind::Gray)
        }
        Some(b"JBIG2Decode") => {
            if bpc != 1 || ncomp != 1 {
                return None;
            }
            let p = parms.unwrap_or_default();
            if p.has(b"JBIG2Globals") {
                return None; // symbol dictionaries live there: not our subset
            }
            let bm = crate::jbig2::decode_pdf_stream(&data, width, height)?;
            // Filter output is the inverse of JBIG2 (1 = black) → DeviceGray 0 = black.
            let px: Vec<u8> = bm
                .bits
                .iter()
                .map(|&b| if b == 1 { 0 } else { 255 })
                .collect();
            (px, 1, ColorKind::Gray)
        }
        _ => return None,
    };

    // /Decode arrays: only the plain inversion form is honoured; anything
    // exotic makes us leave the image alone.
    if let Some(dec) = decode {
        if !matches!(cs, Cs::Indexed { .. }) && !dec.is_empty() {
            let inverted = dec
                .chunks(2)
                .all(|c| c.len() == 2 && c[0] == 1.0 && c[1] == 0.0);
            let identity = dec
                .chunks(2)
                .all(|c| c.len() == 2 && c[0] == 0.0 && c[1] == 1.0);
            if inverted {
                pixels.iter_mut().for_each(|p| *p = 255 - *p);
            } else if !identity {
                return None;
            }
        }
    }
    if pixels.len() < npix * channels as usize {
        return None;
    }
    pixels.truncate(npix * channels as usize);
    Some(RawImage {
        width,
        height,
        channels,
        data: pixels,
        cs: kind,
        is_mask,
        orig_bpc: bpc,
        lossy_decode: bpc == 16,
    })
}

/// Unpack `bpc`-bit samples (rows padded to bytes) to one byte per sample,
/// scaled to 0..255 (for indexed images the raw index is kept).
fn unpack(data: &[u8], w: u32, h: u32, ncomp: u8, bpc: u8) -> Option<Vec<u8>> {
    let samples_per_row = w as usize * ncomp as usize;
    let row_bytes = (samples_per_row * bpc as usize).div_ceil(8);
    if data.len() < row_bytes * h as usize {
        return None;
    }
    let mut out = Vec::with_capacity(samples_per_row * h as usize);
    match bpc {
        8 => {
            for y in 0..h as usize {
                out.extend_from_slice(&data[y * row_bytes..y * row_bytes + samples_per_row]);
            }
        }
        16 => {
            for y in 0..h as usize {
                let row = &data[y * row_bytes..(y + 1) * row_bytes];
                out.extend(row.chunks_exact(2).map(|c| c[0]));
            }
        }
        1 | 2 | 4 => {
            let max = ((1u16 << bpc) - 1) as u8;
            for y in 0..h as usize {
                let row = &data[y * row_bytes..(y + 1) * row_bytes];
                for i in 0..samples_per_row {
                    let bit = i * bpc as usize;
                    let byte = row[bit / 8];
                    let shift = 8 - bpc as usize - (bit % 8);
                    let v = (byte >> shift) & max;
                    out.push(v);
                }
            }
        }
        _ => return None,
    }
    Some(out)
}

fn expand_cs(samples: Vec<u8>, cs: &Cs, bpc: u8) -> Option<(Vec<u8>, u8, ColorKind)> {
    let scale = |v: u8| -> u8 {
        match bpc {
            8 | 16 => v,
            1 => {
                if v != 0 {
                    255
                } else {
                    0
                }
            }
            2 => v * 85,
            4 => v * 17,
            _ => v,
        }
    };
    match cs {
        Cs::Gray => Some((samples.into_iter().map(scale).collect(), 1, ColorKind::Gray)),
        Cs::Rgb => Some((samples.into_iter().map(scale).collect(), 3, ColorKind::Rgb)),
        Cs::Other3 => Some((
            samples.into_iter().map(scale).collect(),
            3,
            ColorKind::Other3,
        )),
        Cs::Cmyk => Some((samples.into_iter().map(scale).collect(), 4, ColorKind::Cmyk)),
        Cs::Indexed { base, lookup } => {
            let n = base.components()? as usize;
            let kind = match **base {
                Cs::Gray => ColorKind::Gray,
                Cs::Rgb => ColorKind::Rgb,
                Cs::Cmyk => ColorKind::Cmyk,
                Cs::Other3 => ColorKind::Other3,
                _ => return None,
            };
            let mut out = Vec::with_capacity(samples.len() * n);
            for idx in samples {
                let off = idx as usize * n;
                if off + n <= lookup.len() {
                    out.extend_from_slice(&lookup[off..off + n]);
                } else {
                    out.extend(std::iter::repeat_n(0, n));
                }
            }
            Some((out, n as u8, kind))
        }
        Cs::Unsupported => None,
    }
}

/// Undo PNG row filters (Predictor 10–15): each row is a filter-type byte
/// followed by `row_len` filtered bytes.
pub fn png_unpredict(data: &[u8], colors: usize, bits: usize, columns: usize) -> Option<Vec<u8>> {
    let bpp = (colors * bits).div_ceil(8).max(1);
    let row_len = (colors * bits * columns).div_ceil(8);
    let rows = data.len() / (row_len + 1);
    let mut out = vec![0u8; rows * row_len];
    for r in 0..rows {
        let ft = data[r * (row_len + 1)];
        let src = &data[r * (row_len + 1) + 1..(r + 1) * (row_len + 1)];
        let (prev_rows, cur_rows) = out.split_at_mut(r * row_len);
        let prev: &[u8] = if r == 0 {
            &[]
        } else {
            &prev_rows[(r - 1) * row_len..]
        };
        let cur = &mut cur_rows[..row_len];
        for i in 0..row_len {
            let a = if i >= bpp { cur[i - bpp] } else { 0 };
            let b = if r == 0 { 0 } else { prev[i] };
            let c = if r == 0 || i < bpp { 0 } else { prev[i - bpp] };
            let pred = match ft {
                0 => 0,
                1 => a,
                2 => b,
                3 => ((a as u16 + b as u16) / 2) as u8,
                4 => {
                    let p = a as i16 + b as i16 - c as i16;
                    let (pa, pb, pc) = (
                        (p - a as i16).abs(),
                        (p - b as i16).abs(),
                        (p - c as i16).abs(),
                    );
                    if pa <= pb && pa <= pc {
                        a
                    } else if pb <= pc {
                        b
                    } else {
                        c
                    }
                }
                _ => return None,
            };
            cur[i] = src[i].wrapping_add(pred);
        }
    }
    Some(out)
}
