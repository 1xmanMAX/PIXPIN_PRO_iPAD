//! Orchestration of all stages with timing and a size non-regression guarantee.

use crate::analyze;
use crate::deflate::Effort;
use crate::error::{Error, Result};
use crate::images::{decode, placement, race};
use crate::lossless;
use crate::mrc;
use crate::options::Options;
use crate::report::{ImageDecision, Report, StageStat};
use crate::writer::{self, WriteOptions};
use lopdf::{Document, Object, ObjectId};
use rayon::prelude::*;
use std::collections::HashSet;
use std::time::Instant;

// Una decision por imagen, consumida enseguida; meterla en Box no gana nada.
#[allow(clippy::large_enum_variant)]
enum Decision {
    Keep,
    Replace(crate::images::encode::Encoded),
    Mrc(mrc::MrcLayers, crate::images::ColorKind),
}

/// Progress event: `percent` 0..=100 and the stage name. Emitted from the
/// calling thread for stage boundaries and from worker threads while images
/// are processed. Return `false` to cancel.
#[derive(Debug, Clone, Copy)]
pub struct Progress<'a> {
    pub percent: u8,
    pub stage: &'a str,
    pub images_done: u32,
    pub images_total: u32,
}

pub type ProgressFn<'a> = dyn Fn(Progress) -> bool + Sync + 'a;

pub fn compress(input: &[u8], opts: &Options) -> Result<(Vec<u8>, Report)> {
    compress_with(input, opts, &|_| true)
}

/// [`compress`] with a progress/cancellation callback.
pub fn compress_with(
    input: &[u8],
    opts: &Options,
    progress: &ProgressFn,
) -> Result<(Vec<u8>, Report)> {
    let t0 = Instant::now();
    let tick = |percent: u8, stage: &str| -> Result<()> {
        if progress(Progress {
            percent,
            stage,
            images_done: 0,
            images_total: 0,
        }) {
            Ok(())
        } else {
            Err(Error::Cancelled)
        }
    };
    tick(0, "load")?;
    if opts.threads > 0 {
        let _ = rayon::ThreadPoolBuilder::new()
            .num_threads(opts.threads)
            .build_global();
    }
    let effort = Effort {
        zopfli_iterations: opts.zopfli_iterations,
        zopfli_max_bytes: opts.zopfli_max_bytes,
    };
    let (mut doc, load_note) = load_lenient(input)?;
    if doc.is_encrypted() {
        return Err(Error::Encrypted);
    }
    let original = doc.clone();
    let mut report = Report {
        input_bytes: input.len() as u64,
        pages: doc.get_pages().len() as u32,
        profile: format!("{:?}", opts.profile).to_lowercase(),
        ..Default::default()
    };
    let before = analyze::analyze(&doc, input.len() as u64);
    report.budget_before = before.budget.clone();
    report.warnings.extend(load_note);

    // ---- lossless structure pass -----------------------------------------
    tick(3, "cleanup")?;
    let t = Instant::now();
    let (stripped, notes) = lossless::strip(&mut doc, opts.strip_private, opts.strip_metadata);
    report.warnings.extend(notes);
    let removed = lossless::gc(&mut doc);
    let (merged, dedup_saved) = lossless::dedup_streams(&mut doc);
    report.stages.push(StageStat { name: "cleanup".into(), detail: format!("{removed} unreachable objects dropped, {merged} duplicate streams merged, private data stripped"), bytes_saved: stripped as i64 + dedup_saved as i64, elapsed_ms: t.elapsed().as_millis() });

    tick(6, "content")?;
    if opts.minify_content {
        let t = Instant::now();
        let (n, saved) = lossless::minify_content_streams(&mut doc, effort);
        report.stages.push(StageStat {
            name: "content".into(),
            detail: format!("{n} content streams minified"),
            bytes_saved: saved,
            elapsed_ms: t.elapsed().as_millis(),
        });
    }

    tick(8, "recompress")?;
    let t = Instant::now();
    let (n, saved) = lossless::recompress_streams(&mut doc, effort, true);
    report.stages.push(StageStat {
        name: "recompress".into(),
        detail: format!("{n} streams re-encoded (zopfli={})", opts.zopfli_iterations),
        bytes_saved: saved,
        elapsed_ms: t.elapsed().as_millis(),
    });

    let t = Instant::now();
    let (n, saved) = lossless::strip_jpeg_metadata(&mut doc);
    report.stages.push(StageStat {
        name: "jpeg-metadata".into(),
        detail: format!("{n} JPEG streams stripped of EXIF/XMP/comments"),
        bytes_saved: saved,
        elapsed_ms: t.elapsed().as_millis(),
    });

    // ---- images -------------------------------------------------------------
    tick(15, "images")?;
    let t = Instant::now();
    let placements = placement::scan(&doc);
    let mut smask_ids: HashSet<ObjectId> = HashSet::new();
    let mut image_ids: Vec<ObjectId> = Vec::new();
    for (&id, obj) in &doc.objects {
        if let Object::Stream(s) = obj {
            if s.dict
                .get(b"Subtype")
                .and_then(Object::as_name)
                .unwrap_or(b"")
                == b"Image"
            {
                image_ids.push(id);
                if let Ok(r) = s.dict.get(b"SMask").and_then(Object::as_reference) {
                    smask_ids.insert(r);
                }
            }
        }
    }
    let doc_ref = &doc;
    // Progress inside the parallel section: weight images by their size.
    let total_img_bytes: u64 = image_ids
        .iter()
        .map(|id| match doc_ref.get_object(*id) {
            Ok(Object::Stream(s)) => s.content.len() as u64,
            _ => 0,
        })
        .sum::<u64>()
        .max(1);
    let done_bytes = std::sync::atomic::AtomicU64::new(0);
    let done_count = std::sync::atomic::AtomicU32::new(0);
    let cancelled = std::sync::atomic::AtomicBool::new(false);
    let images_total = image_ids.len() as u32;
    let results: Vec<(ObjectId, Decision, ImageDecision)> = image_ids
        .par_iter()
        .map(|&id| {
            if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                return (id, Decision::Keep, ImageDecision::default());
            }
            let r = process_one(doc_ref, id, opts, effort, &placements, &smask_ids);
            let bytes = match doc_ref.get_object(id) {
                Ok(Object::Stream(s)) => s.content.len() as u64,
                _ => 0,
            };
            let db = done_bytes.fetch_add(bytes, std::sync::atomic::Ordering::Relaxed) + bytes;
            let dc = done_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            let percent = 15 + (70.0 * db as f64 / total_img_bytes as f64) as u8;
            if !progress(Progress {
                percent: percent.min(85),
                stage: "images",
                images_done: dc,
                images_total,
            }) {
                cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            r
        })
        .collect();
    if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(Error::Cancelled);
    }
    let mut img_saved = 0i64;
    let mut replaced = 0;
    for (id, decision, dec) in results {
        match decision {
            Decision::Keep => {
                if dec.action.starts_with("keep (colour-key") {
                    if let Some(Object::Stream(s)) = doc.objects.get_mut(&id) {
                        img_saved += lossless::recompress_stream(s, effort);
                    }
                }
            }
            Decision::Replace(enc) => {
                apply_encoded(&mut doc, id, &enc);
                replaced += 1;
            }
            Decision::Mrc(layers, kind) => {
                mrc::apply(&mut doc, id, &layers, kind);
                replaced += 1;
            }
        }
        img_saved += dec.before_bytes as i64 - dec.after_bytes as i64;
        if dec.width > 0 {
            report.images.push(dec);
        }
    }
    report.stages.push(StageStat {
        name: "images".into(),
        detail: format!("{replaced}/{} images re-encoded", image_ids.len()),
        bytes_saved: img_saved,
        elapsed_ms: t.elapsed().as_millis(),
    });

    // ---- serialize ------------------------------------------------------------
    tick(88, "serialize")?;
    let t = Instant::now();
    doc.renumber_objects();
    let mut out = writer::write_document(
        &doc,
        &WriteOptions {
            object_streams: opts.object_streams,
            effort,
            objstm_chunk: 200,
        },
    );
    report.stages.push(StageStat {
        name: "serialize".into(),
        detail: format!("object streams={}, xref stream", opts.object_streams),
        bytes_saved: 0,
        elapsed_ms: t.elapsed().as_millis(),
    });

    if opts.verify {
        tick(94, "verify")?;
        let t = Instant::now();
        match crate::verify::verify(&original, &out) {
            Ok(()) => report.verified = true,
            Err(e) => {
                report
                    .warnings
                    .push(format!("verification failed, returning original: {e}"));
                out = input.to_vec();
                report.returned_original = true;
            }
        }
        report.stages.push(StageStat {
            name: "verify".into(),
            detail: if report.verified {
                "pages, text and images verified".into()
            } else {
                "failed".into()
            },
            bytes_saved: 0,
            elapsed_ms: t.elapsed().as_millis(),
        });
    }
    if opts.no_regression && out.len() >= input.len() && !report.returned_original {
        report
            .warnings
            .push("result was not smaller than the input; original returned".into());
        out = input.to_vec();
        report.returned_original = true;
    }
    if !report.returned_original {
        if let Ok(after_doc) = Document::load_mem(&out) {
            report.budget_after = analyze::analyze(&after_doc, out.len() as u64).budget;
        }
    } else {
        report.budget_after = report.budget_before.clone();
    }
    report.output_bytes = out.len() as u64;
    report.ratio = out.len() as f64 / input.len().max(1) as f64;
    report.elapsed_ms = t0.elapsed().as_millis();
    let _ = progress(Progress {
        percent: 100,
        stage: "done",
        images_done: images_total,
        images_total,
    });
    Ok((out, report))
}

/// One image with the panic guard: a decoder panic on one exotic image must
/// never take the document (or the host app) down.
fn process_one(
    doc_ref: &Document,
    id: ObjectId,
    opts: &Options,
    effort: Effort,
    placements: &std::collections::HashMap<ObjectId, placement::Placement>,
    smask_ids: &HashSet<ObjectId>,
) -> (ObjectId, Decision, ImageDecision) {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        process_image(doc_ref, id, opts, effort, placements, smask_ids)
    })) {
        Ok(r) => r,
        Err(_) => {
            let mut dec = ImageDecision {
                object: id.0,
                ..Default::default()
            };
            if let Ok(Object::Stream(s)) = doc_ref.get_object(id) {
                dec.width = analyze::dict_int(doc_ref, &s.dict, b"Width").unwrap_or(0) as u32;
                dec.height = analyze::dict_int(doc_ref, &s.dict, b"Height").unwrap_or(0) as u32;
                dec.before_bytes = s.content.len() as u64;
                dec.after_bytes = dec.before_bytes;
            }
            dec.action = "keep (decoder failure)".into();
            (id, Decision::Keep, dec)
        }
    }
}

fn apply_encoded(doc: &mut Document, id: ObjectId, enc: &crate::images::encode::Encoded) {
    if let Some(Object::Stream(s)) = doc.objects.get_mut(&id) {
        s.dict.set("Width", enc.width as i64);
        s.dict.set("Height", enc.height as i64);
        for k in [
            b"W".as_slice(),
            b"H",
            b"BPC",
            b"CS",
            b"F",
            b"DP",
            b"D",
            b"DL",
            b"Decode",
            b"DecodeParms",
            b"Filter",
            b"Interpolate",
        ] {
            s.dict.remove(k);
        }
        let is_mask = s
            .dict
            .get(b"ImageMask")
            .and_then(Object::as_bool)
            .unwrap_or(false);
        if !is_mask {
            s.dict.set("BitsPerComponent", enc.bpc as i64);
            if let Some(cs) = &enc.colorspace {
                s.dict.set("ColorSpace", cs.clone());
            }
        }
        if let Some(f) = enc.filter {
            s.dict.set("Filter", Object::Name(f.as_bytes().to_vec()));
        }
        if let Some(p) = &enc.decode_parms {
            s.dict.set("DecodeParms", Object::Dictionary(p.clone()));
        }
        s.set_content(enc.data.clone());
    }
}

/// Parse a PDF, tolerating the two most common real-world corruptions:
/// junk appended after `%%EOF` (mail gateways, truncated downloads that were
/// "fixed" by appending) and junk before the `%PDF-` header.
pub fn load_lenient(input: &[u8]) -> Result<(Document, Vec<String>)> {
    match Document::load_mem(input) {
        Ok(d) => Ok((d, vec![])),
        Err(first) => {
            let mut notes = Vec::new();
            let start = find(input, b"%PDF-").unwrap_or(0);
            let end = rfind(input, b"%%EOF").map(|i| i + 5).unwrap_or(input.len());
            if start > 0 || end < input.len() {
                if let Ok(d) = Document::load_mem(&input[start..end]) {
                    notes.push(format!(
                        "input had {} junk bytes before the header / after %%EOF; repaired",
                        start + (input.len() - end)
                    ));
                    return Ok((d, notes));
                }
            }
            Err(first.into())
        }
    }
}

fn find(h: &[u8], n: &[u8]) -> Option<usize> {
    h.windows(n.len()).position(|w| w == n)
}
fn rfind(h: &[u8], n: &[u8]) -> Option<usize> {
    h.windows(n.len()).rposition(|w| w == n)
}

fn process_image(
    doc_ref: &Document,
    id: ObjectId,
    opts: &Options,
    effort: Effort,
    placements: &std::collections::HashMap<ObjectId, placement::Placement>,
    smask_ids: &HashSet<ObjectId>,
) -> (ObjectId, Decision, ImageDecision) {
    let s = match doc_ref.get_object(id) {
        Ok(Object::Stream(s)) => s,
        _ => return (id, Decision::Keep, ImageDecision::default()),
    };
    let before_bytes = s.content.len() as u64;
    let w = analyze::dict_int(doc_ref, &s.dict, b"Width").unwrap_or(0) as u32;
    let h = analyze::dict_int(doc_ref, &s.dict, b"Height").unwrap_or(0) as u32;
    let mut dec = ImageDecision {
        object: id.0,
        width: w,
        height: h,
        before_filter: analyze::filter_name(doc_ref, s),
        before_bytes,
        after_filter: analyze::filter_name(doc_ref, s),
        after_bytes: before_bytes,
        after_width: w,
        after_height: h,
        action: "keep".into(),
        ..Default::default()
    };
    let pl = placements.get(&id).cloned();
    dec.effective_dpi = pl.as_ref().and_then(|p| p.effective_dpi(w, h));
    let img = match decode::decode_image(doc_ref, s) {
        Some(i) => i,
        None => {
            dec.action = "keep (undecodable/unsupported codec)".into();
            return (id, Decision::Keep, dec);
        }
    };
    if s.dict.has(b"Matte") {
        dec.action = "keep (pre-multiplied matte)".into();
        return (id, Decision::Keep, dec);
    }
    let color_key_mask = matches!(s.dict.get(b"Mask"), Ok(Object::Array(_)));
    if color_key_mask {
        // Colour-key ranges refer to the raw sample values: any change of
        // depth, colour space or Decode would silently move the key.
        dec.action = "keep (colour-key mask; stream re-flated only)".into();
        return (id, Decision::Keep, dec);
    }
    // For Indexed sources the decoder expands to the base space, so
    // candidates must carry the *base* colour space, not the palette.
    let orig_cs = match s
        .dict
        .get(b"ColorSpace")
        .ok()
        .and_then(|o| doc_ref.dereference(o).ok())
    {
        Some((_, Object::Array(a)))
            if a.first()
                .and_then(|o| o.as_name().ok())
                .map(|n| n == b"Indexed" || n == b"I")
                .unwrap_or(false) =>
        {
            a.get(1).cloned()
        }
        Some((_, o)) => Some(o.clone()),
        None => None,
    };
    let source_lossy = matches!(dec.before_filter.as_str(), "DCTDecode" | "JPXDecode")
        || dec.before_filter.ends_with("+DCTDecode");
    let is_smask = smask_ids.contains(&id);
    // MRC first for full-page text scans: its size then bounds the JPEG search.
    let stats = crate::images::classify::stats(&img);
    let full_page = pl
        .as_ref()
        .map(|p| p.max_page_coverage >= 0.80 && p.only_direct)
        .unwrap_or(false);
    let mrc_layers = if opts.mrc
        && opts.allow_lossy
        && full_page
        && stats.text_like
        && stats.kind != crate::images::classify::Kind::Bilevel
        && !is_smask
        && !color_key_mask
    {
        mrc::build(&img, opts, effort).filter(|l| l.total_len() < before_bytes as usize)
    } else {
        None
    };
    let size_budget = mrc_layers
        .as_ref()
        .map(|l| (l.total_len() as f64 / opts.mrc_size_tolerance.max(1.0)) as usize);
    let ctx = race::RaceContext {
        opts,
        effort,
        effective_dpi: dec.effective_dpi,
        orig_cs,
        color_key_mask,
        is_smask,
        source_lossy,
        size_budget,
    };
    let raced = race::race(&img, &ctx);
    let mut decision = Decision::Keep;
    let mut best_len = before_bytes as usize;
    if let Some(r) = raced {
        dec.kind = format!("{:?}", r.kind).to_lowercase();
        dec.candidates_tried = r.candidates;
        if r.best.len() < best_len {
            best_len = r.best.len();
            dec.after_bytes = r.best.data.len() as u64;
            dec.after_filter = r.best.filter.unwrap_or("none").into();
            dec.after_width = r.best.width;
            dec.after_height = r.best.height;
            dec.ssim = r.ssim;
            dec.action = r.action.clone();
            decision = Decision::Replace(r.best);
        }
    } else {
        dec.kind = format!("{:?}", stats.kind).to_lowercase();
    }
    if let Some(layers) = mrc_layers {
        // A text page reads better with its ink at full resolution than
        // as a downsampled JPEG of similar size.
        if (layers.total_len() as f64) < best_len as f64 * opts.mrc_size_tolerance.max(1.0) {
            dec.after_bytes = layers.total_len() as u64;
            dec.after_filter = "MRC(mask+fg+bg)".into();
            dec.after_width = w;
            dec.after_height = h;
            dec.ssim = Some(layers.ssim);
            dec.action = format!("mrc bg/{} fg/{}", layers.bg_scale, layers.fg_scale);
            decision = Decision::Mrc(layers, img.cs);
        }
    }
    (id, decision, dec)
}
