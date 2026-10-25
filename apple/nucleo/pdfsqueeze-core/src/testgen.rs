//! Synthetic PDF generator used by tests and the benchmark corpus. Produces
//! the archetypes a compressor meets in the wild: born-digital text, photos,
//! flat graphics, scanned text pages and files bloated with duplicates.

use crate::deflate;
use crate::images::{ColorKind, RawImage};
use lopdf::{dictionary, Dictionary, Document, Object, Stream};

pub struct Synth {
    pub doc: Document,
    pages_id: lopdf::ObjectId,
    page_ids: Vec<lopdf::ObjectId>,
    font_id: lopdf::ObjectId,
}

impl Default for Synth {
    fn default() -> Self {
        Self::new()
    }
}

impl Synth {
    pub fn new() -> Self {
        let mut doc = Document::with_version("1.4");
        let pages_id = doc.new_object_id();
        let font_id = doc.add_object(
            dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" },
        );
        Synth {
            doc,
            pages_id,
            page_ids: vec![],
            font_id,
        }
    }

    fn add_page(&mut self, content: Vec<u8>, xobjects: Dictionary) -> lopdf::ObjectId {
        let content_id = self.doc.add_object(Stream::new(dictionary! {}, content));
        let mut fonts = Dictionary::new();
        fonts.set("F1", Object::Reference(self.font_id));
        let resources = dictionary! { "Font" => fonts, "XObject" => xobjects };
        let page_id = self.doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => self.pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            "Contents" => content_id,
            "Resources" => resources,
        });
        self.page_ids.push(page_id);
        page_id
    }

    /// Page of Latin text lines.
    pub fn text_page(&mut self, lines: usize) -> lopdf::ObjectId {
        let page_no = self.page_ids.len() + 1;
        let mut c = String::from("BT /F1 11 Tf 14 TL 50 740 Td\n");
        let words = [
            "lorem",
            "ipsum",
            "dolor",
            "sit",
            "amet",
            "consectetur",
            "adipiscing",
            "elit",
            "sed",
            "do",
            "eiusmod",
            "tempor",
            "incididunt",
            "ut",
            "labore",
            "et",
            "dolore",
            "magna",
            "aliqua",
        ];
        let mut seed = page_no as u64 * 7919;
        for i in 0..lines {
            let mut line = format!("p{page_no} l{i}:");
            for _ in 0..11 {
                line.push(' ');
                line.push_str(words[(lcg(&mut seed) % words.len() as u32) as usize]);
            }
            c.push_str(&format!("({line}) '\n"));
        }
        c.push_str("ET");
        self.add_page(c.into_bytes(), Dictionary::new())
    }

    fn image_object(
        &mut self,
        img: &RawImage,
        filter: &str,
        data: Vec<u8>,
        bpc: u8,
    ) -> lopdf::ObjectId {
        let cs = match img.cs {
            ColorKind::Gray => "DeviceGray",
            ColorKind::Rgb | ColorKind::Other3 => "DeviceRGB",
            ColorKind::Cmyk => "DeviceCMYK",
        };
        let mut d = dictionary! {
            "Type" => "XObject", "Subtype" => "Image",
            "Width" => img.width as i64, "Height" => img.height as i64,
            "ColorSpace" => cs, "BitsPerComponent" => bpc as i64,
        };
        if !filter.is_empty() {
            d.set("Filter", filter);
        }
        self.doc.add_object(Stream::new(d, data))
    }

    /// Page drawing `img` scaled to `w_pt × h_pt` points at (x, y).
    pub fn image_page(
        &mut self,
        img: &RawImage,
        encoding: ImgEnc,
        x: f32,
        y: f32,
        w_pt: f32,
        h_pt: f32,
    ) -> (lopdf::ObjectId, lopdf::ObjectId) {
        let (filter, data, bpc) = match encoding {
            ImgEnc::Raw => ("", img.data.clone(), 8),
            ImgEnc::Flate => ("FlateDecode", deflate::zlib_best(&img.data), 8),
            ImgEnc::Jpeg(q) => {
                let mut buf = Vec::new();
                let ct = if img.channels == 1 {
                    jpeg_encoder::ColorType::Luma
                } else {
                    jpeg_encoder::ColorType::Rgb
                };
                jpeg_encoder::Encoder::new(&mut buf, q)
                    .encode(&img.data, img.width as u16, img.height as u16, ct)
                    .unwrap();
                ("DCTDecode", buf, 8)
            }
        };
        let img_id = self.image_object(img, filter, data, bpc);
        let mut xo = Dictionary::new();
        xo.set("Im1", Object::Reference(img_id));
        let content = format!(
            "q {w_pt} 0 0 {h_pt} {x} {y} cm /Im1 Do Q\nBT /F1 9 Tf 50 20 Td (caption) Tj ET"
        );
        let page = self.add_page(content.into_bytes(), xo);
        (page, img_id)
    }

    /// Page drawing an arbitrary pre-built image XObject stream (for codec /
    /// colour-space edge cases).
    pub fn custom_image_page(
        &mut self,
        dict: Dictionary,
        data: Vec<u8>,
        w_pt: f32,
        h_pt: f32,
    ) -> (lopdf::ObjectId, lopdf::ObjectId) {
        let img_id = self.doc.add_object(Stream::new(dict, data));
        let mut xo = Dictionary::new();
        xo.set("Im1", Object::Reference(img_id));
        let content = format!("q {w_pt} 0 0 {h_pt} 40 300 cm /Im1 Do Q");
        let page = self.add_page(content.into_bytes(), xo);
        (page, img_id)
    }

    pub fn finish(mut self) -> Vec<u8> {
        let kids: Vec<Object> = self
            .page_ids
            .iter()
            .map(|id| Object::Reference(*id))
            .collect();
        let count = kids.len() as i64;
        self.doc.objects.insert(
            self.pages_id,
            Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count }),
        );
        let catalog = self
            .doc
            .add_object(dictionary! { "Type" => "Catalog", "Pages" => self.pages_id });
        let info = self.doc.add_object(dictionary! { "Producer" => Object::string_literal("synth"), "Title" => Object::string_literal("Synthetic") });
        self.doc.trailer.set("Root", catalog);
        self.doc.trailer.set("Info", info);
        let mut out = Vec::new();
        self.doc.save_to(&mut out).unwrap();
        out
    }
}

#[derive(Clone, Copy)]
pub enum ImgEnc {
    Raw,
    Flate,
    Jpeg(u8),
}

fn lcg(seed: &mut u64) -> u32 {
    *seed = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (*seed >> 33) as u32
}

/// Smooth, photo-like RGB image (gradients + soft blobs + mild noise).
pub fn photo(w: u32, h: u32) -> RawImage {
    photo_seeded(w, h, 42)
}

pub fn photo_seeded(w: u32, h: u32, seed: u64) -> RawImage {
    let mut data = Vec::with_capacity((w * h * 3) as usize);
    let mut seed = seed.wrapping_mul(0x9E3779B97F4A7C15) | 1;
    let sx = 0.3 + (seed % 5) as f32 * 0.1;
    let sy = 0.3 + (seed % 7) as f32 * 0.06;
    for y in 0..h {
        for x in 0..w {
            let fx = x as f32 / w as f32;
            let fy = y as f32 / h as f32;
            let blob = (-(((fx - sx).powi(2) + (fy - sy).powi(2)) * 18.0)).exp();
            let blob2 = (-(((fx - 0.75).powi(2) + (fy - 0.3).powi(2)) * 30.0)).exp();
            let n = (lcg(&mut seed) % 9) as f32 - 4.0;
            let r = (200.0 * fx + 55.0 * blob + n).clamp(0.0, 255.0);
            let g = (120.0 + 100.0 * blob2 - 60.0 * fy + n).clamp(0.0, 255.0);
            let b = (240.0 * (1.0 - fx) * (1.0 - fy) + 40.0 * blob + n).clamp(0.0, 255.0);
            data.extend_from_slice(&[r as u8, g as u8, b as u8]);
        }
    }
    RawImage {
        width: w,
        height: h,
        channels: 3,
        data,
        cs: ColorKind::Rgb,
        is_mask: false,
        orig_bpc: 8,
        lossy_decode: false,
    }
}

/// Flat-colour chart with a handful of colours.
pub fn flat_graphic(w: u32, h: u32) -> RawImage {
    flat_graphic_seeded(w, h, 0)
}

pub fn flat_graphic_seeded(w: u32, h: u32, seed: u64) -> RawImage {
    let palette: [[u8; 3]; 6] = [
        [255, 255, 255],
        [30, 60, 200],
        [220, 40, 40],
        [40, 180, 80],
        [250, 200, 30],
        [20, 20, 20],
    ];
    let mut data = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            let bar = (x * 6 / w) as usize;
            let height = (bar * 37 + 20 + seed as usize * 13) % 90 + 5;
            let c = if y > h * (100 - height as u32) / 100 {
                palette[bar]
            } else if y % 40 == 0 {
                palette[5]
            } else {
                palette[0]
            };
            data.extend_from_slice(&c);
        }
    }
    RawImage {
        width: w,
        height: h,
        channels: 3,
        data,
        cs: ColorKind::Rgb,
        is_mask: false,
        orig_bpc: 8,
        lossy_decode: false,
    }
}

/// A "scanned" text page: off-white textured paper, dark glyph-like blobs in
/// lines, slight vignetting, RGB.
pub fn scanned_text(w: u32, h: u32) -> RawImage {
    scanned_text_seeded(w, h, 7)
}

pub fn scanned_text_seeded(w: u32, h: u32, seed: u64) -> RawImage {
    let mut data = Vec::with_capacity((w * h * 3) as usize);
    let mut seed = seed.wrapping_mul(0x9E3779B97F4A7C15) | 1;
    let line_h = (h / 45).max(12);
    let glyph_w = (w / 90).max(6);
    let mut ink = vec![false; (w * h) as usize];
    // Lay out pseudo glyphs.
    for line in 2..43u32 {
        let y0 = line * line_h + line_h / 3;
        let mut x = w / 12;
        while x + glyph_w < w * 11 / 12 {
            let r = lcg(&mut seed);
            if !r.is_multiple_of(6) {
                let gh = line_h / 2 + (r % 4);
                for yy in y0..(y0 + gh).min(h) {
                    for xx in x..(x + glyph_w * 3 / 4).min(w) {
                        // Vary shape: stems, bowls, crossbars.
                        let on = match r % 3 {
                            0 => xx - x < 2 || yy - y0 < 2,
                            1 => (xx - x) % 4 < 2,
                            _ => (yy - y0) % 5 < 3 && (xx - x) < glyph_w / 2 + 1,
                        };
                        if on {
                            ink[(yy * w + xx) as usize] = true;
                        }
                    }
                }
            }
            x += glyph_w;
        }
    }
    for y in 0..h {
        for x in 0..w {
            let n = (lcg(&mut seed) % 7) as i32 - 3;
            let vign = 1.0
                - 0.12
                    * (((x as f32 / w as f32 - 0.5).powi(2) + (y as f32 / h as f32 - 0.5).powi(2))
                        * 2.0);
            let paper = (243.0 * vign) as i32 + n;
            let (r, g, b) = if ink[(y * w + x) as usize] {
                let d = 25 + (lcg(&mut seed) % 30) as i32;
                (d, d, d + 8)
            } else {
                (paper, paper - 2, paper - 8)
            };
            data.extend_from_slice(&[
                r.clamp(0, 255) as u8,
                g.clamp(0, 255) as u8,
                b.clamp(0, 255) as u8,
            ]);
        }
    }
    RawImage {
        width: w,
        height: h,
        channels: 3,
        data,
        cs: ColorKind::Rgb,
        is_mask: false,
        orig_bpc: 8,
        lossy_decode: false,
    }
}

/// Pure black/white line-art stored as 8-bit gray (wasteful, as scanners emit).
pub fn bilevel_lineart(w: u32, h: u32) -> RawImage {
    bilevel_lineart_seeded(w, h, 0)
}

pub fn bilevel_lineart_seeded(w: u32, h: u32, seed: u64) -> RawImage {
    let s = seed as u32;
    let mut data = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            let on = ((x + s) / 9 + y / 9).is_multiple_of(7)
                || ((x + s * 5) % 97 < 3)
                || (y % (61 + s) < 2);
            data.push(if on { 0 } else { 255 });
        }
    }
    RawImage {
        width: w,
        height: h,
        channels: 1,
        data,
        cs: ColorKind::Gray,
        is_mask: false,
        orig_bpc: 8,
        lossy_decode: false,
    }
}

/// Scanned text page with a photograph pasted in the lower half: the MRC
/// background must keep the photo recognisable, not just the paper.
pub fn scanned_text_with_photo(w: u32, h: u32) -> RawImage {
    let mut page = scanned_text_seeded(w, h, 11);
    let (pw, ph) = (w * 6 / 10, h * 3 / 10);
    let photo = photo_seeded(pw, ph, 5);
    let (ox, oy) = (w / 5, h * 6 / 10);
    let mut seed = 99u64;
    for y in 0..ph {
        for x in 0..pw {
            let src = ((y * pw + x) * 3) as usize;
            let dst = (((oy + y) * w + ox + x) * 3) as usize;
            // Real photographs carry fine texture (grain, fabric, foliage);
            // without it a blurred background would still score well.
            let grain = (lcg(&mut seed) % 41) as i32 - 20;
            let weave = if (x / 3 + y / 3) % 2 == 0 { 14 } else { -14 };
            for c in 0..3 {
                page.data[dst + c] =
                    (photo.data[src + c] as i32 + grain + weave).clamp(0, 255) as u8;
            }
        }
    }
    page
}
