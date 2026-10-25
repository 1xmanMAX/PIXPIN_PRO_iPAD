use pdfsqueeze_core::testgen::{self, ImgEnc, Synth};
use pdfsqueeze_core::{compress, Options, Profile};

fn text_of(bytes: &[u8]) -> String {
    let d = lopdf::Document::load_mem(bytes).unwrap();
    let n = d.get_pages().len() as u32;
    (1..=n)
        .map(|p| d.extract_text(&[p]).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn lossless_text_document_shrinks_and_keeps_text() {
    let mut s = Synth::new();
    for _ in 0..8 {
        s.text_page(40);
    }
    let input = s.finish();
    let (out, rep) = compress(&input, &Options::from_profile(Profile::Lossless)).unwrap();
    assert!(rep.verified, "{:?}", rep.warnings);
    assert!(out.len() < input.len(), "{} !< {}", out.len(), input.len());
    assert_eq!(text_of(&input), text_of(&out));
    assert!(!rep.returned_original);
}

#[test]
fn raw_photo_is_flate_compressed_losslessly() {
    let mut s = Synth::new();
    let img = testgen::photo(300, 200);
    s.image_page(&img, ImgEnc::Raw, 50.0, 400.0, 300.0, 200.0);
    let input = s.finish();
    let (out, rep) = compress(&input, &Options::from_profile(Profile::Lossless)).unwrap();
    assert!(rep.verified, "{:?}", rep.warnings);
    assert!(
        out.len() < input.len() * 8 / 10,
        "{} vs {}",
        out.len(),
        input.len()
    );
    // Pixel-exact round trip.
    let d = lopdf::Document::load_mem(&out).unwrap();
    let mut found = false;
    for o in d.objects.values() {
        if let lopdf::Object::Stream(st) = o {
            if st
                .dict
                .get(b"Subtype")
                .and_then(lopdf::Object::as_name)
                .unwrap_or(b"")
                == b"Image"
            {
                let dec = pdfsqueeze_core::images::decode::decode_image(&d, st).unwrap();
                assert_eq!(dec.data, img.data);
                found = true;
            }
        }
    }
    assert!(found);
}

#[test]
fn balanced_downsamples_oversized_photo_within_ssim() {
    let mut s = Synth::new();
    let img = testgen::photo(1600, 1200);
    // Drawn at 2×1.5 inches → 800 dpi effective; balanced caps at 200.
    s.image_page(&img, ImgEnc::Jpeg(92), 50.0, 400.0, 144.0, 108.0);
    let input = s.finish();
    let opts = Options::from_profile(Profile::Balanced);
    let (out, rep) = compress(&input, &opts).unwrap();
    assert!(rep.verified, "{:?}", rep.warnings);
    let dec = rep.images.iter().find(|i| i.width == 1600).unwrap();
    assert!(dec.after_width < 1600, "not downsampled: {:?}", dec);
    assert!(dec.ssim.unwrap() >= opts.min_ssim);
    assert!(
        out.len() < input.len() / 3,
        "{} vs {}",
        out.len(),
        input.len()
    );
}

#[test]
fn flat_graphic_goes_indexed() {
    let mut s = Synth::new();
    let img = testgen::flat_graphic(640, 400);
    s.image_page(&img, ImgEnc::Flate, 50.0, 300.0, 400.0, 250.0);
    let input = s.finish();
    let (out, rep) = compress(&input, &Options::from_profile(Profile::Lossless)).unwrap();
    assert!(rep.verified, "{:?}", rep.warnings);
    let dec = &rep.images[0];
    assert!(dec.action.starts_with("indexed"), "{:?}", dec);
    assert!(out.len() < input.len());
}

#[test]
fn bilevel_gray_becomes_ccitt_or_1bit() {
    let mut s = Synth::new();
    let img = testgen::bilevel_lineart(1200, 800);
    s.image_page(&img, ImgEnc::Flate, 30.0, 100.0, 550.0, 370.0);
    let input = s.finish();
    let (_, rep) = compress(&input, &Options::from_profile(Profile::Lossless)).unwrap();
    assert!(rep.verified, "{:?}", rep.warnings);
    let dec = &rep.images[0];
    assert!(
        dec.after_filter == "CCITTFaxDecode" || dec.action.starts_with("bilevel"),
        "{:?}",
        dec
    );
    assert!(dec.after_bytes * 6 < dec.before_bytes, "{:?}", dec);
}

#[test]
fn scanned_page_uses_mrc_in_small_profile() {
    let mut s = Synth::new();
    let img = testgen::scanned_text(1700, 2200);
    s.image_page(&img, ImgEnc::Jpeg(85), 0.0, 0.0, 612.0, 792.0);
    let input = s.finish();
    let (out, rep) = compress(&input, &Options::from_profile(Profile::Small)).unwrap();
    assert!(rep.verified, "{:?}", rep.warnings);
    let dec = &rep.images[0];
    assert!(dec.action.starts_with("mrc"), "expected MRC, got {:?}", dec);
    assert!(
        out.len() < input.len() / 2,
        "{} vs {}",
        out.len(),
        input.len()
    );
    // The page must still render two image layers + a mask.
    let d = lopdf::Document::load_mem(&out).unwrap();
    let forms = d.objects.values().filter(|o| matches!(o, lopdf::Object::Stream(s) if s.dict.get(b"Subtype").and_then(lopdf::Object::as_name).unwrap_or(b"") == b"Form")).count();
    assert_eq!(forms, 1);
}

#[test]
fn duplicate_streams_are_merged() {
    let mut s = Synth::new();
    let img = testgen::photo(200, 150);
    for _ in 0..5 {
        s.image_page(&img, ImgEnc::Jpeg(80), 50.0, 400.0, 200.0, 150.0);
    }
    let input = s.finish();
    let (out, rep) = compress(&input, &Options::from_profile(Profile::Lossless)).unwrap();
    assert!(rep.verified);
    let d = lopdf::Document::load_mem(&out).unwrap();
    let images = d.objects.values().filter(|o| matches!(o, lopdf::Object::Stream(s) if s.dict.get(b"Subtype").and_then(lopdf::Object::as_name).unwrap_or(b"") == b"Image")).count();
    assert_eq!(images, 1);
    assert!(out.len() < input.len() / 3);
}

#[test]
fn never_returns_larger_output() {
    // Already-optimal tiny file: pipeline must hand back the original bytes.
    let mut s = Synth::new();
    s.text_page(1);
    let input = s.finish();
    let (out1, _) = compress(&input, &Options::from_profile(Profile::Lossless)).unwrap();
    let (out2, rep2) = compress(&out1, &Options::from_profile(Profile::Lossless)).unwrap();
    assert!(out2.len() <= out1.len());
    if out2.len() == out1.len() {
        assert!(rep2.returned_original);
    }
}

#[test]
fn trailing_junk_is_repaired() {
    let mut s = Synth::new();
    s.text_page(5);
    let mut input = s.finish();
    input.extend_from_slice(b"\n% mail gateway junk ");
    input.extend(std::iter::repeat_n(b'x', 20000));
    let (out, rep) = compress(&input, &Options::from_profile(Profile::Lossless)).unwrap();
    assert!(rep.verified, "{:?}", rep.warnings);
    assert!(rep.warnings.iter().any(|w| w.contains("repaired")));
    assert!(out.len() < input.len() / 4);
    assert_eq!(
        lopdf::Document::load_mem(&out).unwrap().get_pages().len(),
        1
    );
}

#[test]
fn progress_reports_and_cancellation_works() {
    use pdfsqueeze_core::pipeline::{compress_with, Progress};
    use std::sync::Mutex;
    let mut s = Synth::new();
    for i in 0..4u64 {
        s.image_page(
            &testgen::photo_seeded(600, 400, i),
            ImgEnc::Jpeg(90),
            50.0,
            300.0,
            300.0,
            200.0,
        );
    }
    let input = s.finish();
    let seen: Mutex<Vec<(u8, String)>> = Mutex::new(vec![]);
    let (_, rep) = compress_with(
        &input,
        &Options::from_profile(Profile::Balanced),
        &|p: Progress| {
            seen.lock().unwrap().push((p.percent, p.stage.to_string()));
            true
        },
    )
    .unwrap();
    let seen = seen.into_inner().unwrap();
    assert!(rep.verified);
    assert_eq!(seen.first().unwrap().0, 0);
    assert_eq!(seen.last().unwrap().0, 100);
    assert!(
        seen.windows(2).all(|w| w[0].0 <= w[1].0),
        "monotone: {:?}",
        seen
    );
    assert!(seen.iter().any(|(_, s)| s == "images"));
    // Cancel as soon as image work starts.
    let r = compress_with(
        &input,
        &Options::from_profile(Profile::Balanced),
        &|p: Progress| p.stage != "images",
    );
    assert!(matches!(r, Err(pdfsqueeze_core::Error::Cancelled)));
}
