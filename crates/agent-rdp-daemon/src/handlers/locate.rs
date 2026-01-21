//! OCR-based text location handler.

use std::io::Cursor;
use std::sync::{Arc, OnceLock};

use agent_rdp_protocol::{ErrorCode, LocateRequest, LocateResult, Response, ResponseData};
use image::ImageFormat;
use tokio::sync::Mutex;
use tracing::debug;

use crate::ocr::{find_models_dir, OcrService};
use crate::rdp_session::RdpSession;

/// Lazily initialized OCR service.
static OCR_SERVICE: OnceLock<Option<OcrService>> = OnceLock::new();

/// Get or initialize the OCR service.
fn get_ocr_service() -> Option<&'static OcrService> {
    OCR_SERVICE.get_or_init(|| {
        match find_models_dir() {
            Ok(models_dir) => match OcrService::new(&models_dir) {
                Ok(service) => Some(service),
                Err(e) => {
                    tracing::error!("Failed to initialize OCR service: {}", e);
                    None
                }
            },
            Err(e) => {
                tracing::error!("Failed to find OCR models: {}", e);
                None
            }
        }
    }).as_ref()
}

/// Handle a locate request.
pub async fn handle(
    rdp_session: &Arc<Mutex<Option<RdpSession>>>,
    params: LocateRequest,
) -> Response {
    debug!("Locate request: text='{}', pattern={}, ignore_case={}, all={}",
           params.text, params.pattern, params.ignore_case, params.all);

    // Get the current screenshot first (this acquires the async lock)
    let image_data = {
        let session = rdp_session.lock().await;
        let rdp = match session.as_ref() {
            Some(rdp) => rdp,
            None => {
                return Response::error(ErrorCode::NotConnected, "Not connected to an RDP server");
            }
        };

        let (width, height, data) = rdp.get_image_data();
        let width = width as u32;
        let height = height as u32;

        // Convert to an image
        let rgba_image = match image::RgbaImage::from_raw(width, height, data) {
            Some(img) => img,
            None => {
                return Response::error(
                    ErrorCode::InternalError,
                    "Failed to create image from desktop data",
                );
            }
        };

        // Encode to PNG for OCR
        let mut buffer = Cursor::new(Vec::new());
        if let Err(e) = rgba_image.write_to(&mut buffer, ImageFormat::Png) {
            return Response::error(
                ErrorCode::InternalError,
                format!("Failed to encode image: {}", e),
            );
        }

        buffer.into_inner()
    }; // session lock is dropped here

    // Get the OCR service (no async operations, just a static reference)
    let ocr = match get_ocr_service() {
        Some(ocr) => ocr,
        None => {
            return Response::error(
                ErrorCode::InternalError,
                "OCR service not available. Make sure OCR models are installed.",
            );
        }
    };

    // Run OCR (this is CPU-bound, not async)
    let result = if params.all {
        // Return all lines without filtering
        ocr.get_all_lines(&image_data)
    } else {
        // Search for matching lines
        ocr.find_text(&image_data, &params.text, params.pattern, params.ignore_case)
    };

    match result {
        Ok((matches, total_lines)) => {
            debug!("Found {} lines out of {} total", matches.len(), total_lines);
            Response::success(ResponseData::LocateResult(LocateResult {
                matches,
                total_words: total_lines, // Now represents total lines
            }))
        }
        Err(e) => Response::error(
            ErrorCode::InternalError,
            format!("OCR failed: {}", e),
        ),
    }
}
