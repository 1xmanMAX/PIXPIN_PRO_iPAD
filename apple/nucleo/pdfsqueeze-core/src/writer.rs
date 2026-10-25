//! Compact PDF serializer.
//!
//! Differences from a naive writer that matter for size:
//! * non-stream objects are packed into object streams (`/ObjStm`), which are
//!   Flate-compressed as a block — PDF syntax is highly repetitive and this
//!   typically shaves 30–60% off the "structure" budget;
//! * a cross-reference *stream* replaces the classic 20-bytes-per-entry table;
//! * numbers are written with the shortest exact representation;
//! * no cosmetic whitespace.

use crate::deflate::{self, Effort};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream, StringFormat};
use std::collections::BTreeMap;
use std::io::Write;

pub struct WriteOptions {
    pub object_streams: bool,
    pub effort: Effort,
    /// Max objects per object stream.
    pub objstm_chunk: usize,
}

impl Default for WriteOptions {
    fn default() -> Self {
        WriteOptions {
            object_streams: true,
            effort: Effort::fast(),
            objstm_chunk: 200,
        }
    }
}

/// Serialize `doc` into a fresh, single-revision PDF.
pub fn write_document(doc: &Document, opts: &WriteOptions) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::with_capacity(1 << 20);
    let version = pick_version(&doc.version, opts.object_streams);
    write!(out, "%PDF-{}\n%\u{e2}\u{e3}\u{cf}\u{d3}\n", version).unwrap();
    // The comment line above must be raw bytes > 127; `write!` would UTF-8
    // encode them, so fix it up explicitly.
    out.truncate(out.len() - 9);
    out.extend_from_slice(b"%\xE2\xE3\xCF\xD3\n");

    let mut max_id = 0u32;
    let mut offsets: BTreeMap<u32, XrefEntry> = BTreeMap::new();
    let mut packable: Vec<(ObjectId, &Object)> = Vec::new();

    for (&id, obj) in &doc.objects {
        if is_leftover(obj) {
            continue;
        }
        max_id = max_id.max(id.0);
        let must_be_direct = matches!(obj, Object::Stream(_)) || id.1 != 0;
        if opts.object_streams && !must_be_direct {
            packable.push((id, obj));
        } else {
            offsets.insert(
                id.0,
                XrefEntry::Direct {
                    offset: out.len() as u64,
                    gen: id.1,
                },
            );
            write_indirect(&mut out, id, obj);
        }
    }

    // Object streams: build all chunks, compress them in parallel, then emit.
    let mut next_id = max_id + 1;
    let chunk_size = opts.objstm_chunk.max(1);
    let chunks: Vec<(u32, &[(ObjectId, &Object)])> = packable
        .chunks(chunk_size)
        .map(|c| {
            let id = next_id;
            next_id += 1;
            (id, c)
        })
        .collect();
    let built: Vec<(u32, usize, usize, Vec<u8>)> = {
        use rayon::prelude::*;
        chunks
            .par_iter()
            .map(|(stm_id, chunk)| {
                let mut header = Vec::new();
                let mut body = Vec::new();
                for (id, obj) in chunk.iter() {
                    write!(header, "{} {} ", id.0, body.len()).unwrap();
                    write_object(&mut body, obj);
                    body.push(b'\n');
                }
                let first = header.len();
                header.extend_from_slice(&body);
                (
                    *stm_id,
                    chunk.len(),
                    first,
                    deflate::zlib(&header, opts.effort),
                )
            })
            .collect()
    };
    for ((stm_id, chunk), (_, n, first, compressed)) in chunks.iter().zip(built) {
        for (idx, (id, _)) in chunk.iter().enumerate() {
            offsets.insert(
                id.0,
                XrefEntry::InStream {
                    stream: *stm_id,
                    index: idx as u32,
                },
            );
        }
        let mut dict = Dictionary::new();
        dict.set("Type", Object::Name(b"ObjStm".to_vec()));
        dict.set("N", n as i64);
        dict.set("First", first as i64);
        dict.set("Filter", Object::Name(b"FlateDecode".to_vec()));
        let stream = Stream::new(dict, compressed);
        offsets.insert(
            *stm_id,
            XrefEntry::Direct {
                offset: out.len() as u64,
                gen: 0,
            },
        );
        write_indirect(&mut out, (*stm_id, 0), &Object::Stream(stream));
    }

    // Cross-reference stream.
    let xref_id = next_id;
    let size = xref_id + 1;
    let xref_offset = out.len() as u64;
    offsets.insert(
        xref_id,
        XrefEntry::Direct {
            offset: xref_offset,
            gen: 0,
        },
    );
    let mut rows = Vec::with_capacity(size as usize * 7);
    for n in 0..size {
        match offsets.get(&n) {
            None => rows.extend_from_slice(&[0, 0, 0, 0, 0, 0xFF, 0xFF]),
            Some(XrefEntry::Direct { offset, gen }) => {
                rows.push(1);
                rows.extend_from_slice(&(*offset as u32).to_be_bytes());
                rows.extend_from_slice(&gen.to_be_bytes());
            }
            Some(XrefEntry::InStream { stream, index }) => {
                rows.push(2);
                rows.extend_from_slice(&stream.to_be_bytes());
                rows.extend_from_slice(&(*index as u16).to_be_bytes());
            }
        }
    }
    let mut dict = Dictionary::new();
    dict.set("Type", Object::Name(b"XRef".to_vec()));
    dict.set("Size", size as i64);
    dict.set("W", Object::Array(vec![1.into(), 4.into(), 2.into()]));
    for key in [b"Root".as_slice(), b"Info", b"ID"] {
        if let Ok(v) = doc.trailer.get(key) {
            dict.set(key, v.clone());
        }
    }
    dict.set("Filter", Object::Name(b"FlateDecode".to_vec()));
    let stream = Stream::new(dict, deflate::zlib(&rows, opts.effort));
    write_indirect(&mut out, (xref_id, 0), &Object::Stream(stream));
    write!(out, "startxref\n{}\n%%EOF\n", xref_offset).unwrap();
    out
}

enum XrefEntry {
    Direct { offset: u64, gen: u16 },
    InStream { stream: u32, index: u32 },
}

fn pick_version(v: &str, object_streams: bool) -> String {
    let parsed: f32 = v.trim().parse().unwrap_or(1.4);
    let min = if object_streams { 1.5 } else { 1.4 };
    let v = if parsed < min { min } else { parsed };
    format!("{:.1}", v)
}

/// Leftovers of the *input* file layout that we regenerate ourselves.
fn is_leftover(obj: &Object) -> bool {
    match obj {
        Object::Stream(s) => {
            let t = s.dict.get(b"Type").and_then(Object::as_name).unwrap_or(b"");
            t == b"XRef" || t == b"ObjStm"
        }
        Object::Dictionary(d) => d.has(b"Linearized"),
        _ => false,
    }
}

fn write_indirect(out: &mut Vec<u8>, id: ObjectId, obj: &Object) {
    writeln!(out, "{} {} obj", id.0, id.1).unwrap();
    write_object(out, obj);
    out.extend_from_slice(b"\nendobj\n");
}

pub fn write_object(out: &mut Vec<u8>, obj: &Object) {
    match obj {
        Object::Null => out.extend_from_slice(b"null"),
        Object::Boolean(b) => out.extend_from_slice(if *b { b"true" } else { b"false" }),
        Object::Integer(i) => write!(out, "{}", i).unwrap(),
        Object::Real(r) => write_real(out, *r),
        Object::Name(n) => write_name(out, n),
        Object::String(s, f) => write_string(out, s, *f),
        Object::Array(a) => {
            out.push(b'[');
            let mut prev_needs_sep = false;
            for item in a {
                if prev_needs_sep && needs_separator_before(item) {
                    out.push(b' ');
                }
                write_object(out, item);
                prev_needs_sep = needs_separator_after(item);
            }
            out.push(b']');
        }
        Object::Dictionary(d) => write_dict(out, d),
        Object::Stream(s) => {
            write_dict(out, &s.dict);
            out.extend_from_slice(b"stream\n");
            out.extend_from_slice(&s.content);
            out.extend_from_slice(b"\nendstream");
        }
        Object::Reference(id) => write!(out, "{} {} R", id.0, id.1).unwrap(),
    }
}

fn write_dict(out: &mut Vec<u8>, d: &Dictionary) {
    out.extend_from_slice(b"<<");
    for (k, v) in d.iter() {
        write_name(out, k);
        if needs_separator_before(v) {
            out.push(b' ');
        }
        write_object(out, v);
    }
    out.extend_from_slice(b">>");
}

/// Tokens that start with a delimiter (`/`, `[`, `<`, `(`) need no space after a name.
fn needs_separator_before(o: &Object) -> bool {
    !matches!(
        o,
        Object::Name(_)
            | Object::Array(_)
            | Object::Dictionary(_)
            | Object::String(..)
            | Object::Stream(_)
    )
}
fn needs_separator_after(o: &Object) -> bool {
    !matches!(
        o,
        Object::Array(_) | Object::Dictionary(_) | Object::String(..)
    )
}

fn write_real(out: &mut Vec<u8>, r: f32) {
    if r.fract() == 0.0 && r.abs() < 1e9 {
        write!(out, "{}", r as i64).unwrap();
        return;
    }
    let s = format!("{:.5}", r);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    // "-0" and "0.x" → ".x" are both legal but keep it readable: only drop leading zero.
    let s = if let Some(rest) = s.strip_prefix("0.") {
        format!(".{}", rest)
    } else if let Some(rest) = s.strip_prefix("-0.") {
        format!("-.{}", rest)
    } else {
        s.to_string()
    };
    out.extend_from_slice(s.as_bytes());
}

fn write_name(out: &mut Vec<u8>, name: &[u8]) {
    out.push(b'/');
    for &b in name {
        if b" \t\n\r\x0C()<>[]{}/%#".contains(&b) || !(33..=126).contains(&b) {
            write!(out, "#{:02X}", b).unwrap();
        } else {
            out.push(b);
        }
    }
}

fn write_string(out: &mut Vec<u8>, s: &[u8], f: StringFormat) {
    match f {
        StringFormat::Hexadecimal => {
            out.push(b'<');
            for b in s {
                write!(out, "{:02X}", b).unwrap();
            }
            out.push(b'>');
        }
        StringFormat::Literal => {
            out.push(b'(');
            for &b in s {
                match b {
                    b'(' | b')' | b'\\' => {
                        out.push(b'\\');
                        out.push(b);
                    }
                    b'\r' => out.extend_from_slice(b"\\r"),
                    b'\n' => out.extend_from_slice(b"\\n"),
                    _ => out.push(b),
                }
            }
            out.push(b')');
        }
    }
}

/// Size of an object once serialized (used by the analyzer for non-stream objects).
pub fn serialized_len(obj: &Object) -> usize {
    let mut v = Vec::new();
    write_object(&mut v, obj);
    v.len()
}
