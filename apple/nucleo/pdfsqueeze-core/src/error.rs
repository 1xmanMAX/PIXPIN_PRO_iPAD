use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("PDF parse error: {0}")]
    Pdf(#[from] lopdf::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("encrypted PDFs are not supported (decrypt first)")]
    Encrypted,
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("cancelled")]
    Cancelled,
    #[error("verification failed: {0}")]
    Verification(String),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;
