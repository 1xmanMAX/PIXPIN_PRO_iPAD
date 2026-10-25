//! Byte budget: where the size of a PDF actually goes.

use crate::images::placement;
use crate::writer::serialized_len;
use lopdf::{Document, Object, ObjectId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Analysis {
    pub file_bytes: u64,
    pub pages: u32,
    pub version: String,
    pub objects: u32,
    pub budget: BTreeMap<String, u64>,
    pub counts: BTreeMap<String, u32>,
    pub images: Vec<ImageInfo>,
    pub largest: Vec<LargeObject>,
    pub incremental_updates: bool,
    pub has_object_streams: bool,
    pub has_xref_stream: bool,
    pub encrypted: bool,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ImageInfo {
    pub object: u32,
    pub width: u32,
    pub height: u32,
    pub bpc: u32,
    pub colorspace: String,
    pub filter: String,
    pub bytes: u64,
    pub effective_dpi: Option<f32>,
    pub page_coverage: Option<f32>,
    pub has_smask: bool,
    pub is_mask: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LargeObject {
    pub object: u32,
    pub category: String,
    pub bytes: u64,
    pub detail: String,
}

pub fn category_of(
    doc: &Document,
    id: ObjectId,
    obj: &Object,
    fonts: &HashSet<ObjectId>,
    contents: &HashSet<ObjectId>,
) -> &'static str {
    match obj {
        Object::Stream(s) => {
            let ty = s.dict.get(b"Type").and_then(Object::as_name).unwrap_or(b"");
            let sub = s
                .dict
                .get(b"Subtype")
                .and_then(Object::as_name)
                .unwrap_or(b"");
            if sub == b"Image" {
                let filt = filter_name(doc, s);
                return match filt.as_str() {
                    "DCTDecode" => "image/jpeg",
                    "JPXDecode" => "image/jpx",
                    "JBIG2Decode" => "image/jbig2",
                    "CCITTFaxDecode" => "image/ccitt",
                    _ => "image/raw",
                };
            }
            if sub == b"Form" {
                return "form_xobject";
            }
            if fonts.contains(&id) {
                return "fonts";
            }
            if contents.contains(&id) {
                return "content";
            }
            if ty == b"XRef" || ty == b"ObjStm" {
                return "structure";
            }
            if ty == b"Metadata" {
                return "metadata";
            }
            if ty == b"EmbeddedFile" {
                return "attachments";
            }
            if s.dict.has(b"N") && s.dict.len() <= 4 {
                return "icc_profiles";
            }
            if ty == b"CMap" || s.dict.has(b"CIDSystemInfo") {
                return "fonts";
            }
            "other_streams"
        }
        _ => "structure",
    }
}

pub fn filter_name(doc: &Document, s: &lopdf::Stream) -> String {
    let f = s
        .dict
        .get(b"Filter")
        .ok()
        .and_then(|o| doc.dereference(o).ok())
        .map(|(_, o)| o.clone());
    match f {
        Some(Object::Name(n)) => String::from_utf8_lossy(&n).into_owned(),
        Some(Object::Array(a)) => a
            .iter()
            .filter_map(|o| {
                o.as_name()
                    .ok()
                    .map(|n| String::from_utf8_lossy(n).into_owned())
            })
            .collect::<Vec<_>>()
            .join("+"),
        _ => "none".into(),
    }
}

pub fn colorspace_name(doc: &Document, cs: Option<&Object>) -> String {
    let cs = match cs {
        Some(c) => c,
        None => return "?".into(),
    };
    let (_, cs) = match doc.dereference(cs) {
        Ok(x) => x,
        Err(_) => return "?".into(),
    };
    match cs {
        Object::Name(n) => String::from_utf8_lossy(n).into_owned(),
        Object::Array(a) => {
            let head = a
                .first()
                .and_then(|o| o.as_name().ok())
                .map(|n| String::from_utf8_lossy(n).into_owned())
                .unwrap_or_default();
            if head == "ICCBased" {
                let n = a
                    .get(1)
                    .and_then(|o| doc.dereference(o).ok())
                    .and_then(|(_, o)| o.as_stream().ok())
                    .and_then(|s| s.dict.get(b"N").ok())
                    .and_then(|o| o.as_i64().ok())
                    .unwrap_or(0);
                format!("ICCBased({})", n)
            } else if head == "Indexed" {
                format!("Indexed({})", colorspace_name(doc, a.get(1)))
            } else {
                head
            }
        }
        _ => "?".into(),
    }
}

/// Collect font program stream ids and page content stream ids.
pub fn font_and_content_ids(doc: &Document) -> (HashSet<ObjectId>, HashSet<ObjectId>) {
    let mut fonts = HashSet::new();
    let mut contents = HashSet::new();
    for obj in doc.objects.values() {
        let d = match obj {
            Object::Dictionary(d) => d,
            Object::Stream(s) => &s.dict,
            _ => continue,
        };
        for key in [b"FontFile".as_slice(), b"FontFile2", b"FontFile3"] {
            if let Ok(r) = d.get(key).and_then(Object::as_reference) {
                fonts.insert(r);
            }
        }
    }
    for (_, pid) in doc.get_pages() {
        for c in doc.get_page_contents(pid) {
            contents.insert(c);
        }
    }
    (fonts, contents)
}

/// Integer dictionary entry, following indirect references (scanner apps love
/// `/Height 54 0 R`).
pub fn dict_int(doc: &Document, d: &lopdf::Dictionary, key: &[u8]) -> Option<i64> {
    let o = d.get(key).ok()?;
    let (_, o) = doc.dereference(o).ok()?;
    match o {
        Object::Integer(i) => Some(*i),
        Object::Real(r) => Some(*r as i64),
        _ => None,
    }
}

pub fn object_size(obj: &Object) -> u64 {
    match obj {
        Object::Stream(s) => {
            s.content.len() as u64 + serialized_len(&Object::Dictionary(s.dict.clone())) as u64 + 24
        }
        o => serialized_len(o) as u64 + 16,
    }
}

pub fn analyze(doc: &Document, file_bytes: u64) -> Analysis {
    let mut a = Analysis {
        file_bytes,
        pages: doc.get_pages().len() as u32,
        version: doc.version.clone(),
        objects: doc.objects.len() as u32,
        ..Default::default()
    };
    a.encrypted = doc.is_encrypted();
    a.has_xref_stream = matches!(
        doc.reference_table.cross_reference_type,
        lopdf::xref::XrefType::CrossReferenceStream
    );
    let (fonts, contents) = font_and_content_ids(doc);
    let placements = placement::scan(doc);
    let mut large: Vec<LargeObject> = Vec::new();
    for (&id, obj) in &doc.objects {
        let cat = category_of(doc, id, obj, &fonts, &contents);
        let size = object_size(obj);
        *a.budget.entry(cat.to_string()).or_default() += size;
        *a.counts.entry(cat.to_string()).or_default() += 1;
        if let Object::Stream(s) = obj {
            let ty = s.dict.get(b"Type").and_then(Object::as_name).unwrap_or(b"");
            if ty == b"ObjStm" {
                a.has_object_streams = true;
            }
            if cat.starts_with("image/") {
                let w = dict_int(doc, &s.dict, b"Width").unwrap_or(0) as u32;
                let h = dict_int(doc, &s.dict, b"Height").unwrap_or(0) as u32;
                let pl = placements.get(&id);
                a.images.push(ImageInfo {
                    object: id.0,
                    width: w,
                    height: h,
                    bpc: dict_int(doc, &s.dict, b"BitsPerComponent").unwrap_or(0) as u32,
                    colorspace: colorspace_name(doc, s.dict.get(b"ColorSpace").ok()),
                    filter: filter_name(doc, s),
                    bytes: size,
                    effective_dpi: pl.and_then(|p| p.effective_dpi(w, h)),
                    page_coverage: pl.map(|p| p.max_page_coverage),
                    has_smask: s.dict.has(b"SMask"),
                    is_mask: s
                        .dict
                        .get(b"ImageMask")
                        .and_then(Object::as_bool)
                        .unwrap_or(false),
                });
            }
        }
        let detail = match obj {
            Object::Stream(s) => format!(
                "{} {}",
                String::from_utf8_lossy(
                    s.dict
                        .get(b"Subtype")
                        .and_then(Object::as_name)
                        .unwrap_or(b"stream")
                ),
                filter_name(doc, s)
            ),
            Object::Dictionary(d) => {
                String::from_utf8_lossy(d.get(b"Type").and_then(Object::as_name).unwrap_or(b"dict"))
                    .into_owned()
            }
            _ => "object".into(),
        };
        large.push(LargeObject {
            object: id.0,
            category: cat.into(),
            bytes: size,
            detail,
        });
    }
    large.sort_by_key(|x| std::cmp::Reverse(x.bytes));
    large.truncate(15);
    a.largest = large;
    a.images.sort_by_key(|x| std::cmp::Reverse(x.bytes));
    let accounted: u64 = a.budget.values().sum();
    if file_bytes > accounted {
        a.budget
            .insert("unaccounted_or_incremental".into(), file_bytes - accounted);
        if file_bytes > accounted + accounted / 10 + 4096 {
            a.incremental_updates = true;
            a.notes.push("File carries data not reachable from the current revision (incremental updates or junk): rewriting alone will drop it.".into());
        }
    }
    if a.encrypted {
        a.notes
            .push("Document is encrypted; decrypt before optimising.".into());
    }
    a
}
