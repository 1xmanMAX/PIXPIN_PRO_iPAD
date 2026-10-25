//! pdfsqueeze-core: content-aware PDF optimizer.
//!
//! The pipeline is: analyze → lossless structure pass → per-image codec racing
//! under a perceptual quality constraint → optional MRC layering for scans →
//! custom serializer (object streams + xref stream + zopfli) → verification
//! with a size non-regression guarantee.

pub mod analyze;
pub mod deflate;
pub mod error;
pub mod images;
pub mod jbig2;
pub mod lossless;
pub mod mrc;
pub mod options;
pub mod pipeline;
pub mod report;
pub mod testgen;
pub mod verify;
pub mod writer;

pub use error::{Error, Result};
pub use options::{Options, Profile};
pub use pipeline::compress;
pub use report::Report;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const PRODUCER: &str = concat!("pdfsqueeze ", env!("CARGO_PKG_VERSION"));
