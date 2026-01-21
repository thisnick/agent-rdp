//! OCR (Optical Character Recognition) module.
//!
//! Provides text detection and recognition from screenshots using the ocrs library.

mod engine;

pub use engine::{find_models_dir, OcrService};
