use serde::{Deserialize, Serialize};

/// Named presets. Everything a preset sets can be overridden individually.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Profile {
    /// Bit-exact content: structure cleanup, dedup, stream recompression only.
    Lossless,
    /// Re-encodes images but every image must stay perceptually near-identical.
    #[default]
    Balanced,
    /// Visible-quality target for screen reading; enables MRC for scans.
    Small,
    /// Aggressive: lowest DPI / SSIM still readable. Archive-grade for text scans.
    Extreme,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Options {
    pub profile: Profile,
    /// Allow lossy re-encoding of images at all.
    pub allow_lossy: bool,
    /// Minimum structural similarity (luma, 0..1) any lossy candidate must keep
    /// against the decoded original. Compared after resampling back to the
    /// original size, so downsampling is penalised honestly.
    pub min_ssim: f64,
    /// Downsample images whose *effective* DPI (pixels / drawn size) exceeds this.
    pub max_dpi: Option<f32>,
    /// Never downsample below this factor of the original (safety for tiny images).
    pub min_scale: f32,
    /// Lowest JPEG quality the search may pick.
    pub jpeg_min_quality: u8,
    /// Highest JPEG quality the search may pick.
    pub jpeg_max_quality: u8,
    /// Granularity of the JPEG quality search (larger = fewer encodes).
    pub jpeg_quality_step: u8,
    /// Enable Mixed Raster Content layering for full-page scans.
    pub mrc: bool,
    /// Extra SSIM slack allowed for MRC recompositions (binarised text edges
    /// score lower on SSIM while being sharper to a reader).
    pub mrc_ssim_slack: f64,
    /// Prefer the MRC layering over a plain JPEG of a text page as long as it
    /// is at most this factor larger (text stays at full resolution). 1.0 =
    /// pick strictly by size.
    pub mrc_size_tolerance: f64,
    /// Coarsest background downscale the MRC search may pick (2..=8). Paper
    /// texture and vignetting vanish above 4; on flat paper 8 is invisible.
    pub mrc_max_bg_scale: u32,
    /// Coarsest foreground (ink colour) downscale (4 or 8).
    pub mrc_max_fg_scale: u32,
    /// Remove XMP metadata and non-essential Info entries.
    pub strip_metadata: bool,
    /// Remove application-private data (PieceInfo, page thumbnails, ...).
    pub strip_private: bool,
    /// Zopfli iterations for Flate streams. 0 = plain deflate (fast).
    pub zopfli_iterations: u8,
    /// Streams larger than this fall back to plain deflate regardless.
    pub zopfli_max_bytes: usize,
    /// Re-serialize content streams compactly (numbers are re-formatted, so
    /// rasterization can differ by a sub-pixel; off in the lossless profile).
    pub minify_content: bool,
    /// Pack non-stream objects into object streams (PDF 1.5).
    pub object_streams: bool,
    /// Return the original bytes if the result is not smaller.
    pub no_regression: bool,
    /// Verify output (page count, text, image decodability) before returning.
    pub verify: bool,
    /// Number of worker threads (0 = all cores).
    pub threads: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options::from_profile(Profile::Balanced)
    }
}

impl Options {
    pub fn from_profile(profile: Profile) -> Self {
        let base = Options {
            profile,
            allow_lossy: true,
            min_ssim: 0.97,
            max_dpi: Some(200.0),
            min_scale: 0.2,
            jpeg_min_quality: 35,
            jpeg_max_quality: 95,
            jpeg_quality_step: 4,
            mrc: false,
            mrc_ssim_slack: 0.06,
            mrc_size_tolerance: 1.3,
            mrc_max_bg_scale: 8,
            mrc_max_fg_scale: 4,
            strip_metadata: false,
            strip_private: true,
            zopfli_iterations: 5,
            zopfli_max_bytes: 1 << 20,
            minify_content: true,
            object_streams: true,
            no_regression: true,
            verify: true,
            threads: 0,
        };
        match profile {
            Profile::Lossless => Options {
                allow_lossy: false,
                max_dpi: None,
                mrc: false,
                zopfli_iterations: 8,
                // Big Flate images are the whole game here: worth the minutes.
                zopfli_max_bytes: 8 << 20,
                minify_content: false,
                ..base
            },
            Profile::Balanced => base,
            Profile::Small => Options {
                min_ssim: 0.93,
                max_dpi: Some(150.0),
                mrc: true,
                strip_metadata: true,
                jpeg_min_quality: 25,
                ..base
            },
            Profile::Extreme => Options {
                mrc_max_fg_scale: 8,
                min_ssim: 0.88,
                max_dpi: Some(110.0),
                mrc: true,
                mrc_ssim_slack: 0.10,
                strip_metadata: true,
                jpeg_min_quality: 15,
                zopfli_iterations: 3,
                ..base
            },
        }
    }
}
