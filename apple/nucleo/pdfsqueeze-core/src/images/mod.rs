pub mod classify;
pub mod decode;
pub mod encode;
pub mod placement;
pub mod quality;
pub mod race;
pub mod resample;

/// Decoded, 8-bit-per-sample interleaved pixels.
#[derive(Debug, Clone)]
pub struct RawImage {
    pub width: u32,
    pub height: u32,
    /// 1 (gray), 3 (RGB), 4 (CMYK)
    pub channels: u8,
    pub data: Vec<u8>,
    pub cs: ColorKind,
    /// `/ImageMask true` stencil: 1 channel, 0 = paint.
    pub is_mask: bool,
    /// Original BitsPerComponent.
    pub orig_bpc: u8,
    /// True when decoding itself was not exact (16-bit → 8-bit).
    pub lossy_decode: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorKind {
    Gray,
    Rgb,
    Cmyk,
    /// Three-component space we can carry losslessly but must not treat as RGB.
    Other3,
}

impl RawImage {
    pub fn luma(&self) -> Vec<u8> {
        let n = (self.width * self.height) as usize;
        let mut out = Vec::with_capacity(n);
        match self.channels {
            1 => out.extend_from_slice(&self.data[..n]),
            3 => {
                for p in self.data.chunks_exact(3).take(n) {
                    out.push(
                        ((77 * p[0] as u32 + 150 * p[1] as u32 + 29 * p[2] as u32) >> 8) as u8,
                    );
                }
            }
            4 => {
                for p in self.data.chunks_exact(4).take(n) {
                    let k = 255 - p[3] as i32;
                    let r = (255 - p[0] as i32) * k / 255;
                    let g = (255 - p[1] as i32) * k / 255;
                    let b = (255 - p[2] as i32) * k / 255;
                    out.push(((77 * r + 150 * g + 29 * b) >> 8) as u8);
                }
            }
            _ => out.resize(n, 0),
        }
        out
    }
}
