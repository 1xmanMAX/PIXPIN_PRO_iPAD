//! Flate (zlib) encoding with a quality/speed knob. Output is always a standard
//! zlib stream, i.e. a valid `/FlateDecode` payload for any PDF reader.

use std::io::Write;
use std::num::NonZeroU64;

#[derive(Debug, Clone, Copy)]
pub struct Effort {
    pub zopfli_iterations: u8,
    pub zopfli_max_bytes: usize,
}

impl Effort {
    pub fn fast() -> Self {
        Effort {
            zopfli_iterations: 0,
            zopfli_max_bytes: 0,
        }
    }
}

/// Below this size zopfli's fixed per-call cost (~200 ms in the Rust port)
/// buys a few dozen bytes at most: not worth it.
pub const ZOPFLI_MIN_BYTES: usize = 6 * 1024;

pub fn zlib(data: &[u8], effort: Effort) -> Vec<u8> {
    if effort.zopfli_iterations > 0
        && data.len() <= effort.zopfli_max_bytes
        && data.len() >= ZOPFLI_MIN_BYTES
    {
        // Zopfli cost grows super-linearly; scale iterations down for big inputs.
        let iters = if data.len() > (1 << 20) {
            1
        } else if data.len() > (256 << 10) {
            effort.zopfli_iterations.min(3)
        } else {
            effort.zopfli_iterations
        };
        let opts = zopfli::Options {
            iteration_count: NonZeroU64::new(iters as u64).unwrap(),
            ..Default::default()
        };
        let mut out = Vec::with_capacity(data.len() / 2);
        if zopfli::compress(opts, zopfli::Format::Zlib, data, &mut out).is_ok() {
            // Zopfli is not guaranteed to beat zlib-9 on every input; keep the best.
            let plain = zlib_best(data);
            return if plain.len() < out.len() { plain } else { out };
        }
    }
    zlib_best(data)
}

pub fn zlib_best(data: &[u8]) -> Vec<u8> {
    let mut enc = flate2::write::ZlibEncoder::new(
        Vec::with_capacity(data.len() / 2),
        flate2::Compression::best(),
    );
    enc.write_all(data).expect("in-memory write");
    enc.finish().expect("in-memory finish")
}

pub fn inflate(data: &[u8]) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let mut out = Vec::with_capacity(data.len() * 3);
    flate2::read::ZlibDecoder::new(data).read_to_end(&mut out)?;
    Ok(out)
}
