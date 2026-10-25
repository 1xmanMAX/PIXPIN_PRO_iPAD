//! JBIG2 (ITU-T T.88) generic-region encoder and decoder, PDF-embedded
//! stream organisation. Lossless: the decoded bitmap is bit-exact.
//!
//! Only what we emit is decoded: page-info + immediate (lossless) generic
//! region, template 0, arithmetic coding, optional TPGDON. Anything else
//! (MMR, symbol dictionaries, refinement, halftone) returns `None` and the
//! caller leaves the stream untouched.

// ---------------------------------------------------------------------------
// MQ arithmetic coder (T.88 Annex E, software conventions).
// ---------------------------------------------------------------------------

#[rustfmt::skip]
const QE: [(u16, u8, u8, u8); 47] = [
    (0x5601, 1, 1, 1), (0x3401, 2, 6, 0), (0x1801, 3, 9, 0), (0x0AC1, 4, 12, 0), (0x0521, 5, 29, 0), (0x0221, 38, 33, 0),
    (0x5601, 7, 6, 1), (0x5401, 8, 14, 0), (0x4801, 9, 14, 0), (0x3801, 10, 14, 0), (0x3001, 11, 17, 0), (0x2401, 12, 18, 0),
    (0x1C01, 13, 20, 0), (0x1601, 29, 21, 0), (0x5601, 15, 14, 1), (0x5401, 16, 14, 0), (0x5101, 17, 15, 0), (0x4801, 18, 16, 0),
    (0x3801, 19, 17, 0), (0x3401, 20, 18, 0), (0x3001, 21, 19, 0), (0x2801, 22, 19, 0), (0x2401, 23, 20, 0), (0x2201, 24, 21, 0),
    (0x1C01, 25, 22, 0), (0x1801, 26, 23, 0), (0x1601, 27, 24, 0), (0x1401, 28, 25, 0), (0x1201, 29, 26, 0), (0x1101, 30, 27, 0),
    (0x0AC1, 31, 28, 0), (0x09C1, 32, 29, 0), (0x08A1, 33, 30, 0), (0x0521, 34, 31, 0), (0x0441, 35, 32, 0), (0x02A1, 36, 33, 0),
    (0x0221, 37, 34, 0), (0x0141, 38, 35, 0), (0x0111, 39, 36, 0), (0x0085, 40, 37, 0), (0x0049, 41, 38, 0), (0x0025, 42, 39, 0),
    (0x0015, 43, 40, 0), (0x0009, 44, 41, 0), (0x0005, 45, 42, 0), (0x0001, 45, 43, 0), (0x5601, 46, 46, 0),
];

#[derive(Clone, Copy, Default)]
struct Cx {
    index: u8,
    mps: u8,
}

pub struct MqEncoder {
    out: Vec<u8>,
    a: u32,
    c: u32,
    ct: i32,
    b: u8,
    first: bool,
}

impl Default for MqEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl MqEncoder {
    pub fn new() -> Self {
        MqEncoder {
            out: Vec::new(),
            a: 0x8000,
            c: 0,
            ct: 12,
            b: 0,
            first: true,
        }
    }

    fn emit(&mut self) {
        if self.first {
            self.first = false;
        } else {
            self.out.push(self.b);
        }
    }

    fn byte_out(&mut self) {
        if self.b == 0xFF {
            self.emit();
            self.b = (self.c >> 20) as u8;
            self.c &= 0xF_FFFF;
            self.ct = 7;
        } else {
            if self.c >= 0x800_0000 {
                self.b = self.b.wrapping_add(1);
                if self.b == 0xFF {
                    self.c &= 0x7FF_FFFF;
                    self.emit();
                    self.b = (self.c >> 20) as u8;
                    self.c &= 0xF_FFFF;
                    self.ct = 7;
                    return;
                }
            }
            self.emit();
            self.b = (self.c >> 19) as u8;
            self.c &= 0x7_FFFF;
            self.ct = 8;
        }
    }

    fn renorm(&mut self) {
        loop {
            if self.ct == 0 {
                self.byte_out();
            }
            self.a <<= 1;
            self.c <<= 1;
            self.ct -= 1;
            if self.a & 0x8000 != 0 {
                break;
            }
        }
    }

    fn encode(&mut self, cx: &mut Cx, d: u8) {
        let (qe, nmps, nlps, switch) = QE[cx.index as usize];
        let qe = qe as u32;
        if d == cx.mps {
            // CODEMPS
            self.a -= qe;
            if self.a & 0x8000 == 0 {
                if self.a < qe {
                    self.a = qe;
                } else {
                    self.c += qe;
                }
                cx.index = nmps;
                self.renorm();
            } else {
                self.c += qe;
            }
        } else {
            // CODELPS
            self.a -= qe;
            if self.a < qe {
                self.c += qe;
            } else {
                self.a = qe;
            }
            if switch == 1 {
                cx.mps = 1 - cx.mps;
            }
            cx.index = nlps;
            self.renorm();
        }
    }

    fn flush(mut self) -> Vec<u8> {
        // SETBITS
        let tempc = self.c + self.a;
        self.c |= 0xFFFF;
        if self.c >= tempc {
            self.c -= 0x8000;
        }
        self.c <<= self.ct;
        self.byte_out();
        self.c <<= self.ct;
        self.byte_out();
        if self.b != 0xFF {
            self.emit();
        }
        self.out.push(0xFF);
        self.out.push(0xAC);
        self.out
    }
}

pub struct MqDecoder<'a> {
    data: &'a [u8],
    bp: usize,
    chigh: u32,
    clow: u32,
    a: u32,
    ct: i32,
}

impl<'a> MqDecoder<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        let mut d = MqDecoder {
            data,
            bp: 0,
            chigh: 0,
            clow: 0,
            a: 0,
            ct: 0,
        };
        d.chigh = ((d.byte(0) as u32) << 8) | d.byte(1) as u32;
        // INITDEC (software conventions, as in pdf.js): C = B<<16; BYTEIN; C <<= 7; CT -= 7; A = 0x8000
        d.chigh = d.byte(0) as u32;
        d.clow = 0;
        d.byte_in();
        d.chigh = ((d.chigh << 7) & 0xFFFF) | ((d.clow >> 9) & 0x7F);
        d.clow = (d.clow << 7) & 0xFFFF;
        d.ct -= 7;
        d.a = 0x8000;
        d
    }

    fn byte(&self, i: usize) -> u8 {
        self.data.get(i).copied().unwrap_or(0xFF)
    }

    fn byte_in(&mut self) {
        if self.byte(self.bp) == 0xFF {
            if self.byte(self.bp + 1) > 0x8F {
                self.clow += 0xFF00;
                self.ct = 8;
            } else {
                self.bp += 1;
                self.clow += (self.byte(self.bp) as u32) << 9;
                self.ct = 7;
            }
        } else {
            self.bp += 1;
            self.clow += if self.bp < self.data.len() {
                (self.byte(self.bp) as u32) << 8
            } else {
                0xFF00
            };
            self.ct = 8;
        }
        if self.clow > 0xFFFF {
            self.chigh += self.clow >> 16;
            self.clow &= 0xFFFF;
        }
    }

    fn decode(&mut self, cx: &mut Cx) -> u8 {
        let (qe, nmps, nlps, switch) = QE[cx.index as usize];
        let qe = qe as u32;
        let d;
        self.a = self.a.wrapping_sub(qe);
        if self.chigh < qe {
            if self.a < qe {
                self.a = qe;
                d = cx.mps;
                cx.index = nmps;
            } else {
                self.a = qe;
                d = 1 ^ cx.mps;
                if switch == 1 {
                    cx.mps = d;
                }
                cx.index = nlps;
            }
        } else {
            self.chigh -= qe;
            if self.a & 0x8000 != 0 {
                return cx.mps;
            }
            if self.a < qe {
                d = 1 ^ cx.mps;
                if switch == 1 {
                    cx.mps = d;
                }
                cx.index = nlps;
            } else {
                d = cx.mps;
                cx.index = nmps;
            }
        }
        loop {
            if self.ct == 0 {
                self.byte_in();
            }
            self.a <<= 1;
            self.chigh = ((self.chigh << 1) & 0xFFFF) | ((self.clow >> 15) & 1);
            self.clow = (self.clow << 1) & 0xFFFF;
            self.ct -= 1;
            if self.a & 0x8000 != 0 {
                break;
            }
        }
        d
    }
}

// ---------------------------------------------------------------------------
// Generic region, template 0, nominal AT pixels, optional TPGDON.
// ---------------------------------------------------------------------------

/// Bitmap with one byte per pixel (1 = black), row-major.
pub struct Bitmap {
    pub width: u32,
    pub height: u32,
    pub bits: Vec<u8>,
}

/// Context ordering follows the sorted (y, then x) template used by every
/// mainstream decoder: row −2 (x−2..x+2), row −1 (x−3..x+3), row 0 (x−4..x−1).
#[inline]
fn context(bits: &[u8], w: usize, x: usize, y: usize) -> u16 {
    let px = |dx: isize, dy: isize| -> u16 {
        let xx = x as isize + dx;
        let yy = y as isize + dy;
        if xx < 0 || yy < 0 || xx >= w as isize {
            0
        } else {
            bits[yy as usize * w + xx as usize] as u16
        }
    };
    let mut c: u16 = 0;
    for dx in -2..=2 {
        c = (c << 1) | px(dx, -2);
    }
    for dx in -3..=3 {
        c = (c << 1) | px(dx, -1);
    }
    for dx in -4..=-1 {
        c = (c << 1) | px(dx, 0);
    }
    c
}

const TPGDON_CONTEXT: usize = 0x9B25;

pub fn encode_generic(bm: &Bitmap, tpgdon: bool) -> Vec<u8> {
    let (w, h) = (bm.width as usize, bm.height as usize);
    let mut cx = vec![Cx::default(); 1 << 16];
    let mut enc = MqEncoder::new();
    let mut ltp = 0u8;
    for y in 0..h {
        if tpgdon {
            let same = y > 0 && bm.bits[y * w..(y + 1) * w] == bm.bits[(y - 1) * w..y * w];
            let new_ltp = same as u8;
            let sltp = new_ltp ^ ltp;
            enc.encode(&mut cx[TPGDON_CONTEXT], sltp);
            ltp = new_ltp;
            if ltp == 1 {
                continue;
            }
        }
        for x in 0..w {
            let c = context(&bm.bits, w, x, y) as usize;
            enc.encode(&mut cx[c], bm.bits[y * w + x]);
        }
    }
    enc.flush()
}

pub fn decode_generic(data: &[u8], width: u32, height: u32, tpgdon: bool) -> Bitmap {
    let (w, h) = (width as usize, height as usize);
    let mut bits = vec![0u8; w * h];
    let mut cx = vec![Cx::default(); 1 << 16];
    let mut dec = MqDecoder::new(data);
    let mut ltp = 0u8;
    for y in 0..h {
        if tpgdon {
            let sltp = dec.decode(&mut cx[TPGDON_CONTEXT]);
            ltp ^= sltp;
            if ltp == 1 {
                if y > 0 {
                    let (prev, cur) = bits.split_at_mut(y * w);
                    cur[..w].copy_from_slice(&prev[(y - 1) * w..]);
                }
                continue;
            }
        }
        for x in 0..w {
            let c = context(&bits, w, x, y) as usize;
            bits[y * w + x] = dec.decode(&mut cx[c]);
        }
    }
    Bitmap {
        width,
        height,
        bits,
    }
}

// ---------------------------------------------------------------------------
// PDF embedded stream organisation (segment headers, no file header).
// ---------------------------------------------------------------------------

fn segment_header(out: &mut Vec<u8>, number: u32, seg_type: u8, page: u8, data_len: u32) {
    out.extend_from_slice(&number.to_be_bytes());
    out.push(seg_type & 0x3F); // page association size 1 byte, not deferred
    out.push(0); // no referred-to segments
    out.push(page);
    out.extend_from_slice(&data_len.to_be_bytes());
}

/// Encode a bilevel bitmap as a PDF `/JBIG2Decode` stream (embedded
/// organisation: page info + immediate lossless generic region).
pub fn encode_pdf_stream(bm: &Bitmap, tpgdon: bool) -> Vec<u8> {
    let coded = encode_generic(bm, tpgdon);
    let mut out = Vec::with_capacity(coded.len() + 64);
    // Page information segment (type 48).
    let mut page = Vec::with_capacity(19);
    page.extend_from_slice(&bm.width.to_be_bytes());
    page.extend_from_slice(&bm.height.to_be_bytes());
    page.extend_from_slice(&0u32.to_be_bytes()); // x resolution (unknown)
    page.extend_from_slice(&0u32.to_be_bytes()); // y resolution
    page.push(0x01); // eventually lossless; default pixel 0; combination OR
    page.extend_from_slice(&0u16.to_be_bytes()); // no striping
    segment_header(&mut out, 0, 48, 1, page.len() as u32);
    out.extend_from_slice(&page);
    // Immediate lossless generic region segment (type 39).
    let mut region = Vec::with_capacity(coded.len() + 26);
    region.extend_from_slice(&bm.width.to_be_bytes());
    region.extend_from_slice(&bm.height.to_be_bytes());
    region.extend_from_slice(&0u32.to_be_bytes()); // x
    region.extend_from_slice(&0u32.to_be_bytes()); // y
    region.push(0); // external combination operator OR
    region.push(if tpgdon { 0x08 } else { 0x00 }); // MMR=0, GBTEMPLATE=0, TPGDON bit 3
    for (x, y) in [(3i8, -1i8), (-3, -1), (2, -2), (-2, -2)] {
        region.push(x as u8);
        region.push(y as u8);
    }
    region.extend_from_slice(&coded);
    segment_header(&mut out, 1, 39, 1, region.len() as u32);
    out.extend_from_slice(&region);
    out
}

/// Decode a stream we (or a compatible encoder) produced. `None` for any
/// feature outside the generic-region subset.
pub fn decode_pdf_stream(data: &[u8], width: u32, height: u32) -> Option<Bitmap> {
    let mut pos = 0usize;
    let mut page: Option<Bitmap> = None;
    while pos + 11 <= data.len() {
        let flags = data[pos + 4];
        let seg_type = flags & 0x3F;
        let page_assoc_4 = flags & 0x40 != 0;
        let mut p = pos + 5;
        let rts = data[p];
        let count = (rts >> 5) as usize;
        if count == 7 {
            return None; // long form referred-to list: not ours
        }
        p += 1;
        let number = u32::from_be_bytes(data[pos..pos + 4].try_into().ok()?);
        let ref_size = if number <= 256 {
            1
        } else if number <= 65536 {
            2
        } else {
            4
        };
        p += count * ref_size;
        p += if page_assoc_4 { 4 } else { 1 };
        let len = u32::from_be_bytes(data.get(p..p + 4)?.try_into().ok()?) as usize;
        p += 4;
        let body = data.get(p..p + len)?;
        match seg_type {
            48 => {
                let w = u32::from_be_bytes(body.get(0..4)?.try_into().ok()?);
                let h = u32::from_be_bytes(body.get(4..8)?.try_into().ok()?);
                let flags = *body.get(16)?;
                let default_pixel = (flags >> 2) & 1;
                if w != width || (h != height && h != 0xFFFF_FFFF) {
                    return None;
                }
                page = Some(Bitmap {
                    width,
                    height,
                    bits: vec![default_pixel; (width * height) as usize],
                });
            }
            36 | 38 | 39 => {
                let w = u32::from_be_bytes(body.get(0..4)?.try_into().ok()?);
                let h = u32::from_be_bytes(body.get(4..8)?.try_into().ok()?);
                let x = u32::from_be_bytes(body.get(8..12)?.try_into().ok()?);
                let y = u32::from_be_bytes(body.get(12..16)?.try_into().ok()?);
                let gflags = *body.get(17)?;
                let mmr = gflags & 1;
                let template = (gflags >> 1) & 3;
                let tpgdon = (gflags >> 3) & 1 == 1;
                if mmr == 1 || template != 0 {
                    return None;
                }
                let at = body.get(18..26)?;
                if at != [3, 0xFF, 0xFD, 0xFF, 2, 0xFE, 0xFE, 0xFE] {
                    return None; // non-nominal AT pixels
                }
                let region = decode_generic(&body[26..], w, h, tpgdon);
                let pg = page.get_or_insert_with(|| Bitmap {
                    width,
                    height,
                    bits: vec![0; (width * height) as usize],
                });
                for ry in 0..h as usize {
                    let py = y as usize + ry;
                    if py >= height as usize {
                        break;
                    }
                    for rx in 0..w as usize {
                        let px = x as usize + rx;
                        if px < width as usize {
                            pg.bits[py * width as usize + px] |= region.bits[ry * w as usize + rx];
                        }
                    }
                }
            }
            49 | 50 | 51 | 62 => {} // end of page/stripe/file, extension: ignore
            _ => return None,
        }
        pos = p + len;
    }
    page
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checker(w: u32, h: u32) -> Bitmap {
        let mut bits = vec![0u8; (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                let on = ((x / 7 + y / 5) % 3 == 0) || (x % 41 < 2) || (y > h / 2 && y % 17 == 0);
                bits[(y * w + x) as usize] = on as u8;
            }
        }
        Bitmap {
            width: w,
            height: h,
            bits,
        }
    }

    #[test]
    fn mq_round_trip_random_bits() {
        let mut cx = [Cx::default(); 4];
        let mut enc = MqEncoder::new();
        let mut seed = 12345u64;
        let mut bits = Vec::new();
        for i in 0..20000 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let b = (seed >> 33).is_multiple_of(7) as u8; // skewed source
            bits.push(b);
            enc.encode(&mut cx[i % 4], b);
        }
        let data = enc.flush();
        let mut cx = [Cx::default(); 4];
        let mut dec = MqDecoder::new(&data);
        for (i, &b) in bits.iter().enumerate() {
            assert_eq!(dec.decode(&mut cx[i % 4]), b, "bit {i}");
        }
        assert!(data.len() < 20000 / 8);
    }

    #[test]
    fn generic_region_round_trip() {
        for tpgdon in [false, true] {
            let bm = checker(203, 117);
            let coded = encode_pdf_stream(&bm, tpgdon);
            let back = decode_pdf_stream(&coded, 203, 117).expect("decodes");
            assert_eq!(back.bits, bm.bits, "tpgdon={tpgdon}");
        }
    }

    #[test]
    fn blank_and_full_rows() {
        let mut bm = checker(64, 40);
        for y in 10..20 {
            for x in 0..64 {
                bm.bits[y * 64 + x] = 1;
            }
        }
        for y in 20..30 {
            for x in 0..64 {
                bm.bits[y * 64 + x] = 0;
            }
        }
        let coded = encode_pdf_stream(&bm, true);
        assert_eq!(decode_pdf_stream(&coded, 64, 40).unwrap().bits, bm.bits);
    }
}
