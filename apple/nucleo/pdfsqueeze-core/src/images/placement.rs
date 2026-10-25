//! Where and how large each image XObject is drawn.
//!
//! Walks every page content stream (and nested Form XObjects) tracking the CTM,
//! so the *effective* resolution of an image is known from the size it is
//! actually painted at, not guessed from `/Width`.

use lopdf::content::Content;
use lopdf::{Dictionary, Document, Object, ObjectId};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Default)]
pub struct Placement {
    /// Largest drawn width/height in points (1/72 in) across all placements.
    pub max_w_pt: f32,
    pub max_h_pt: f32,
    pub count: u32,
    /// Fraction of the page area covered by the largest placement (0..1).
    pub max_page_coverage: f32,
    /// Page (object id) where the largest placement occurs.
    pub page: Option<ObjectId>,
    /// Resource name used for that placement in its page/form resources.
    pub resource_name: Option<Vec<u8>>,
    /// True when the image is only ever drawn from a page content stream
    /// directly (not from inside a Form XObject).
    pub only_direct: bool,
}

impl Placement {
    pub fn effective_dpi(&self, width_px: u32, height_px: u32) -> Option<f32> {
        if self.max_w_pt <= 0.0 || self.max_h_pt <= 0.0 {
            return None;
        }
        let dx = width_px as f32 * 72.0 / self.max_w_pt;
        let dy = height_px as f32 * 72.0 / self.max_h_pt;
        Some(dx.min(dy))
    }
}

type Ctm = [f32; 6];

fn mul(m: &Ctm, n: &Ctm) -> Ctm {
    // m × n (apply m first, then n) as in PDF `cm` semantics.
    [
        m[0] * n[0] + m[1] * n[2],
        m[0] * n[1] + m[1] * n[3],
        m[2] * n[0] + m[3] * n[2],
        m[2] * n[1] + m[3] * n[3],
        m[4] * n[0] + m[5] * n[2] + n[4],
        m[4] * n[1] + m[5] * n[3] + n[5],
    ]
}

fn num(o: &Object) -> Option<f32> {
    match o {
        Object::Integer(i) => Some(*i as f32),
        Object::Real(r) => Some(*r),
        _ => None,
    }
}

pub fn scan(doc: &Document) -> HashMap<ObjectId, Placement> {
    let mut out: HashMap<ObjectId, Placement> = HashMap::new();
    for (_, page_id) in doc.get_pages() {
        let page = match doc.get_dictionary(page_id) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let media = page_box(doc, page).unwrap_or([0.0, 0.0, 612.0, 792.0]);
        let page_area = ((media[2] - media[0]) * (media[3] - media[1]))
            .abs()
            .max(1.0);
        let content = match doc.get_page_content(page_id) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let resources = resolve_resources(doc, page.get(b"Resources").ok());
        let mut visited = HashSet::new();
        walk(
            doc,
            &content,
            resources.as_ref(),
            [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            page_id,
            page_area,
            true,
            &mut out,
            &mut visited,
            0,
        );
    }
    out
}

fn page_box(doc: &Document, page: &Dictionary) -> Option<[f32; 4]> {
    // MediaBox may be inherited.
    let mut node: Option<&Dictionary> = Some(page);
    let mut guard = 0;
    while let Some(d) = node {
        if let Ok(arr) = d
            .get(b"MediaBox")
            .and_then(|o| doc.dereference(o).map(|(_, o)| o))
            .and_then(Object::as_array)
        {
            if arr.len() == 4 {
                let v: Vec<f32> = arr
                    .iter()
                    .filter_map(|o| doc.dereference(o).ok().and_then(|(_, o)| num(o)))
                    .collect();
                if v.len() == 4 {
                    return Some([v[0], v[1], v[2], v[3]]);
                }
            }
        }
        node = d
            .get(b"Parent")
            .ok()
            .and_then(|p| doc.dereference(p).ok())
            .and_then(|(_, o)| o.as_dict().ok());
        guard += 1;
        if guard > 64 {
            break;
        }
    }
    None
}

fn resolve_resources(doc: &Document, res: Option<&Object>) -> Option<Dictionary> {
    let (_, o) = doc.dereference(res?).ok()?;
    o.as_dict().ok().cloned()
}

#[allow(clippy::too_many_arguments)]
fn walk(
    doc: &Document,
    content: &[u8],
    resources: Option<&Dictionary>,
    base: Ctm,
    page_id: ObjectId,
    page_area: f32,
    direct: bool,
    out: &mut HashMap<ObjectId, Placement>,
    visited: &mut HashSet<ObjectId>,
    depth: u32,
) {
    if depth > 12 {
        return;
    }
    let ops = match Content::decode(content) {
        Ok(c) => c.operations,
        Err(_) => return,
    };
    let mut stack: Vec<Ctm> = Vec::new();
    let mut ctm = base;
    let xobjects = resources
        .and_then(|r| r.get(b"XObject").ok())
        .and_then(|x| doc.dereference(x).ok())
        .and_then(|(_, o)| o.as_dict().ok());
    for op in ops {
        match op.operator.as_str() {
            "q" => stack.push(ctm),
            "Q" => {
                if let Some(c) = stack.pop() {
                    ctm = c;
                }
            }
            "cm" => {
                if op.operands.len() == 6 {
                    let v: Vec<f32> = op.operands.iter().filter_map(num).collect();
                    if v.len() == 6 {
                        ctm = mul(&[v[0], v[1], v[2], v[3], v[4], v[5]], &ctm);
                    }
                }
            }
            "Do" => {
                let name = match op.operands.first().and_then(|o| o.as_name().ok()) {
                    Some(n) => n,
                    None => continue,
                };
                let xd = match xobjects {
                    Some(x) => x,
                    None => continue,
                };
                let id = match xd.get(name).ok().and_then(|o| o.as_reference().ok()) {
                    Some(id) => id,
                    None => continue,
                };
                let stream = match doc.get_object(id).ok().and_then(|o| o.as_stream().ok()) {
                    Some(s) => s,
                    None => continue,
                };
                let subtype = stream
                    .dict
                    .get(b"Subtype")
                    .and_then(Object::as_name)
                    .unwrap_or(b"");
                if subtype == b"Image" {
                    let w = (ctm[0] * ctm[0] + ctm[1] * ctm[1]).sqrt();
                    let h = (ctm[2] * ctm[2] + ctm[3] * ctm[3]).sqrt();
                    let cov = (w * h / page_area).min(1.0);
                    let p = out.entry(id).or_insert(Placement {
                        only_direct: true,
                        ..Default::default()
                    });
                    p.count += 1;
                    if !direct {
                        p.only_direct = false;
                    }
                    if w * h > p.max_w_pt * p.max_h_pt {
                        p.max_w_pt = w;
                        p.max_h_pt = h;
                        p.max_page_coverage = cov;
                        p.page = Some(page_id);
                        p.resource_name = Some(name.to_vec());
                    }
                } else if subtype == b"Form" && !visited.contains(&id) {
                    visited.insert(id);
                    let mut inner = ctm;
                    if let Ok(m) = stream.dict.get(b"Matrix").and_then(Object::as_array) {
                        let v: Vec<f32> = m.iter().filter_map(num).collect();
                        if v.len() == 6 {
                            inner = mul(&[v[0], v[1], v[2], v[3], v[4], v[5]], &ctm);
                        }
                    }
                    let form_res = resolve_resources(doc, stream.dict.get(b"Resources").ok());
                    let res = form_res.as_ref().or(resources);
                    if let Ok(data) = stream
                        .decompressed_content()
                        .or_else(|_| Ok::<_, lopdf::Error>(stream.content.clone()))
                    {
                        walk(
                            doc,
                            &data,
                            res,
                            inner,
                            page_id,
                            page_area,
                            false,
                            out,
                            visited,
                            depth + 1,
                        );
                    }
                    visited.remove(&id);
                }
            }
            _ => {}
        }
    }
}
