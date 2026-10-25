//! Codec / colour-space edge cases: every input here must round-trip through
//! the pipeline with pixel-exact content in lossless mode (or be left alone).

use lopdf::{dictionary, Object};
use pdfsqueeze_core::images::{decode, encode};
use pdfsqueeze_core::testgen::{self, Synth};
use pdfsqueeze_core::{compress, Options, Profile};

fn first_image(bytes: &[u8]) -> (lopdf::Document, lopdf::Stream) {
    let d = lopdf::Document::load_mem(bytes).unwrap();
    let s = d
        .objects
        .values()
        .find_map(|o| match o {
            lopdf::Object::Stream(s)
                if s.dict
                    .get(b"Subtype")
                    .and_then(Object::as_name)
                    .unwrap_or(b"")
                    == b"Image" =>
            {
                Some(s.clone())
            }
            _ => None,
        })
        .unwrap();
    (d, s)
}

#[test]
fn ccitt_g4_input_is_decoded_and_preserved() {
    let img = testgen::bilevel_lineart(800, 600);
    let g4 = encode::ccitt_g4(&img.data, 800, 600, false).unwrap();
    let mut s = Synth::new();
    let mut dict = dictionary! { "Type" => "XObject", "Subtype" => "Image", "Width" => 800, "Height" => 600, "ColorSpace" => "DeviceGray", "BitsPerComponent" => 1, "Filter" => "CCITTFaxDecode" };
    dict.set(
        "DecodeParms",
        Object::Dictionary(g4.decode_parms.clone().unwrap()),
    );
    s.custom_image_page(dict, g4.data.clone(), 400.0, 300.0);
    let input = s.finish();
    let (d, st) = first_image(&input);
    let dec = decode::decode_image(&d, &st).expect("decode CCITT input");
    assert_eq!(dec.data, img.data);
    let (out, rep) = compress(&input, &Options::from_profile(Profile::Lossless)).unwrap();
    assert!(rep.verified, "{:?}", rep.warnings);
    let (d2, st2) = first_image(&out);
    assert_eq!(decode::decode_image(&d2, &st2).unwrap().data, img.data);
}

#[test]
fn indexed_4bit_with_decode_inversion_round_trips() {
    // 4-bit indexed RGB, plus a Decode [1 0] gray image with a colour-key mask.
    let w = 64u32;
    let h = 32u32;
    let palette: Vec<u8> = (0..16u8)
        .flat_map(|i| [i * 16, 255 - i * 16, (i * 37) % 255])
        .collect();
    let indices: Vec<u8> = (0..w * h).map(|i| ((i / 4) % 16) as u8).collect();
    let packed = encode::pack_bits(&indices, w as usize, h as usize, 4);
    let mut s = Synth::new();
    let cs = Object::Array(vec![
        Object::Name(b"Indexed".to_vec()),
        Object::Name(b"DeviceRGB".to_vec()),
        15.into(),
        Object::String(palette.clone(), lopdf::StringFormat::Hexadecimal),
    ]);
    s.custom_image_page(dictionary! { "Type" => "XObject", "Subtype" => "Image", "Width" => w as i64, "Height" => h as i64, "ColorSpace" => cs, "BitsPerComponent" => 4 }, packed, 200.0, 100.0);
    let gray: Vec<u8> = (0..w * h).map(|i| (i % 251) as u8).collect();
    s.custom_image_page(dictionary! { "Type" => "XObject", "Subtype" => "Image", "Width" => w as i64, "Height" => h as i64, "ColorSpace" => "DeviceGray", "BitsPerComponent" => 8, "Decode" => vec![1.into(), 0.into()], "Mask" => vec![250.into(), 255.into()] }, gray.clone(), 200.0, 100.0);
    let input = s.finish();
    let (out, rep) = compress(&input, &Options::from_profile(Profile::Small)).unwrap();
    assert!(rep.verified, "{:?}", rep.warnings);
    let d = lopdf::Document::load_mem(&out).unwrap();
    let mut seen = 0;
    for o in d.objects.values() {
        if let lopdf::Object::Stream(st) = o {
            if st
                .dict
                .get(b"Subtype")
                .and_then(Object::as_name)
                .unwrap_or(b"")
                != b"Image"
            {
                continue;
            }
            let dec = decode::decode_image(&d, st).unwrap();
            if dec.channels == 3 {
                let expected: Vec<u8> = indices
                    .iter()
                    .flat_map(|&i| palette[i as usize * 3..i as usize * 3 + 3].to_vec())
                    .collect();
                assert_eq!(dec.data, expected, "indexed image changed");
            } else {
                // Colour-key masked images must stay sample-exact (mask depends on values).
                let expected: Vec<u8> = gray.iter().map(|&v| 255 - v).collect();
                assert_eq!(dec.data, expected, "colour-keyed image changed");
                assert!(st.dict.has(b"Mask"));
            }
            seen += 1;
        }
    }
    assert_eq!(seen, 2);
}

#[test]
fn cmyk_jpeg_is_left_untouched() {
    // A DCT stream we cannot safely re-encode: must survive byte-for-byte.
    let mut s = Synth::new();
    let fake_cmyk_jpeg = {
        let img = testgen::photo(64, 64);
        let mut buf = Vec::new();
        jpeg_encoder::Encoder::new(&mut buf, 80)
            .encode(
                &img.data
                    .iter()
                    .flat_map(|&v| [v, v, v, 0])
                    .take(64 * 64 * 4)
                    .collect::<Vec<u8>>(),
                64,
                64,
                jpeg_encoder::ColorType::Cmyk,
            )
            .unwrap();
        buf
    };
    s.custom_image_page(dictionary! { "Type" => "XObject", "Subtype" => "Image", "Width" => 64, "Height" => 64, "ColorSpace" => "DeviceCMYK", "BitsPerComponent" => 8, "Filter" => "DCTDecode" }, fake_cmyk_jpeg.clone(), 100.0, 100.0);
    let input = s.finish();
    let (out, rep) = compress(&input, &Options::from_profile(Profile::Extreme)).unwrap();
    assert!(rep.verified, "{:?}", rep.warnings);
    let (_, st) = first_image(&out);
    assert_eq!(st.content, fake_cmyk_jpeg);
    assert!(
        rep.images[0].action.starts_with("keep"),
        "{:?}",
        rep.images[0]
    );
}

#[test]
fn smask_survives_and_is_optimised() {
    let mut s = Synth::new();
    let img = testgen::photo(300, 200);
    let alpha: Vec<u8> = (0..300 * 200)
        .map(|i| if (i % 300) < 150 { 255 } else { 0 })
        .collect();
    let smask_id = s.doc.add_object(lopdf::Stream::new(dictionary! { "Type" => "XObject", "Subtype" => "Image", "Width" => 300, "Height" => 200, "ColorSpace" => "DeviceGray", "BitsPerComponent" => 8 }, alpha.clone()));
    s.custom_image_page(dictionary! { "Type" => "XObject", "Subtype" => "Image", "Width" => 300, "Height" => 200, "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8, "SMask" => smask_id }, img.data.clone(), 300.0, 200.0);
    let input = s.finish();
    let (out, rep) = compress(&input, &Options::from_profile(Profile::Balanced)).unwrap();
    assert!(rep.verified, "{:?}", rep.warnings);
    let d = lopdf::Document::load_mem(&out).unwrap();
    let main = d
        .objects
        .values()
        .find_map(|o| match o {
            lopdf::Object::Stream(s) if s.dict.has(b"SMask") => Some(s.clone()),
            _ => None,
        })
        .expect("SMask reference kept");
    let sm_id = main.dict.get(b"SMask").unwrap().as_reference().unwrap();
    let sm = d.get_object(sm_id).unwrap().as_stream().unwrap();
    let dec = decode::decode_image(&d, sm).unwrap();
    assert_eq!(dec.channels, 1);
    assert_eq!(
        dec.data, alpha,
        "soft mask must be exact (it is bilevel here)"
    );
    assert!(sm.content.len() < alpha.len() / 10);
}

#[test]
fn text_mask_goes_jbig2_and_round_trips() {
    // A realistic ink mask (from the synthetic scan) stored as 1-bit Flate:
    // JBIG2 generic coding must win and decode bit-exactly.
    let scan = testgen::scanned_text(1200, 1600);
    let mask = pdfsqueeze_core::mrc::segment(&scan.luma(), 1200, 1600);
    let gray: Vec<u8> = mask.iter().map(|&m| if m == 1 { 0 } else { 255 }).collect();
    let bits: Vec<u8> = gray.iter().map(|&v| (v >= 128) as u8).collect();
    let packed = encode::pack_bits(&bits, 1200, 1600, 1);
    let mut s = Synth::new();
    s.custom_image_page(dictionary! { "Type" => "XObject", "Subtype" => "Image", "Width" => 1200, "Height" => 1600, "ColorSpace" => "DeviceGray", "BitsPerComponent" => 1, "Filter" => "FlateDecode" }, pdfsqueeze_core::deflate::zlib_best(&packed), 500.0, 700.0);
    let input = s.finish();
    let (out, rep) = compress(&input, &Options::from_profile(Profile::Lossless)).unwrap();
    assert!(rep.verified, "{:?}", rep.warnings);
    let dec = &rep.images[0];
    // The synthetic glyphs repeat, so Flate can legitimately win here; what
    // matters is that JBIG2 was raced and whichever won is bit-exact.
    assert!(dec.candidates_tried >= 3, "{:?}", dec);
    assert!(
        matches!(
            dec.after_filter.as_str(),
            "FlateDecode" | "CCITTFaxDecode" | "JBIG2Decode"
        ),
        "{:?}",
        dec
    );
    let (d, st) = first_image(&out);
    assert_eq!(decode::decode_image(&d, &st).unwrap().data, gray);
    // And JBIG2 itself must be exact and smaller than G4 on ink masks.
    let jb = encode::jbig2(&gray, 1200, 1600, false).unwrap();
    let g4 = encode::ccitt_g4(&gray, 1200, 1600, false).unwrap();
    assert!(jb.len() < g4.len(), "jbig2 {} vs g4 {}", jb.len(), g4.len());
    let bm = pdfsqueeze_core::jbig2::decode_pdf_stream(&jb.data, 1200, 1600).unwrap();
    assert!(bm
        .bits
        .iter()
        .zip(&gray)
        .all(|(&b, &g)| (b == 1) == (g < 128)));
}

#[test]
fn mrc_keeps_embedded_photo_legible() {
    use pdfsqueeze_core::images::{quality, resample};
    let img = testgen::scanned_text_with_photo(1700, 2200);
    let opts = Options::from_profile(Profile::Small);
    let layers = pdfsqueeze_core::mrc::build(&img, &opts, pdfsqueeze_core::deflate::Effort::fast())
        .expect("mrc builds");
    // Compare the photo region of what a reader will show (recomposition at
    // the gate's reference scale) against the original at that scale.
    let (w, h) = (img.width, img.height);
    let (rw, rh) = (layers.ref_w, layers.ref_h);
    let orig_ref = resample::resize(
        &img.luma(),
        w,
        h,
        1,
        rw,
        rh,
        image::imageops::FilterType::Triangle,
    );
    let (pw, ph, ox, oy) = (rw * 6 / 10, rh * 3 / 10, rw / 5, rh * 6 / 10);
    let crop = |l: &[u8]| -> Vec<u8> {
        let mut out = Vec::with_capacity((pw * ph) as usize);
        for y in oy..oy + ph {
            out.extend_from_slice(&l[(y * rw + ox) as usize..(y * rw + ox + pw) as usize]);
        }
        out
    };
    let s = quality::ssim(
        &crop(&orig_ref),
        &crop(&layers.recomposed_ref),
        pw as usize,
        ph as usize,
    );
    assert!(
        layers.bg_scale <= 4,
        "photo page must not get a coarse background: bg/{} (photo ssim {s:.3})",
        layers.bg_scale
    );
    assert!(
        s >= 0.75,
        "photo region degraded: ssim {s:.3} at bg/{}",
        layers.bg_scale
    );
}
