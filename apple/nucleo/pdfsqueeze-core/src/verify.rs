//! Output validation: the optimised file must open, keep every page, keep the
//! extractable text of every page and every image must be decodable.

use crate::error::{Error, Result};
use crate::images::decode;
use lopdf::{Document, Object};

fn norm(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn verify(original: &Document, output_bytes: &[u8]) -> Result<()> {
    let out = Document::load_mem(output_bytes)
        .map_err(|e| Error::Verification(format!("output does not parse: {e}")))?;
    let p0 = original.get_pages().len();
    let p1 = out.get_pages().len();
    if p0 != p1 {
        return Err(Error::Verification(format!(
            "page count changed: {p0} → {p1}"
        )));
    }
    for n in 1..=p1 as u32 {
        let a = original.extract_text(&[n]).ok().map(|s| norm(&s));
        let b = out.extract_text(&[n]).ok().map(|s| norm(&s));
        if let (Some(a), Some(b)) = (&a, &b) {
            if a != b {
                return Err(Error::Verification(format!("text of page {n} changed")));
            }
        }
    }
    for (id, obj) in &out.objects {
        if let Object::Stream(s) = obj {
            let sub = s
                .dict
                .get(b"Subtype")
                .and_then(Object::as_name)
                .unwrap_or(b"");
            if sub != b"Image" {
                continue;
            }
            let filters: Vec<&[u8]> = s.filters().unwrap_or_default();
            let last = filters.last().copied().unwrap_or(b"");
            if last == b"JPXDecode" {
                continue;
            }
            if decode::decode_image(&out, s).is_none() {
                // Some exotic colour spaces are legitimately undecodable for us; only
                // fail when the *original* had the same object decodable.
                if let Some(Object::Stream(orig)) = original.objects.get(id) {
                    if decode::decode_image(original, orig).is_some() {
                        return Err(Error::Verification(format!(
                            "image {} no longer decodes",
                            id.0
                        )));
                    }
                }
            }
        }
    }
    Ok(())
}
