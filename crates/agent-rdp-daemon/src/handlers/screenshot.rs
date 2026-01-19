//! Screenshot handler.

use std::io::Cursor;
use std::sync::Arc;

use agent_rdp_protocol::{ErrorCode, ImageFormat, Response, ResponseData, ScreenshotRequest};
use base64::Engine;
use image::ImageFormat as ImgFormat;
use tokio::sync::Mutex;

use crate::rdp_session::RdpSession;

/// Handle a screenshot request.
pub async fn handle(
    rdp_session: &Arc<Mutex<Option<RdpSession>>>,
    params: ScreenshotRequest,
) -> Response {
    let session = rdp_session.lock().await;

    let rdp = match session.as_ref() {
        Some(rdp) => rdp,
        None => {
            return Response::error(ErrorCode::NotConnected, "Not connected to an RDP server");
        }
    };

    // Get the current desktop image from the RDP session
    // The background frame processor keeps this up-to-date
    let (width, height, data) = rdp.get_image_data();
    let width = width as u32;
    let height = height as u32;

    // Convert to an image::RgbaImage
    let rgba_image = match image::RgbaImage::from_raw(width, height, data) {
        Some(img) => img,
        None => {
            return Response::error(
                ErrorCode::InternalError,
                "Failed to create image from decoded data",
            );
        }
    };

    // Encode to requested format
    let format = match params.format {
        ImageFormat::Png => ImgFormat::Png,
        ImageFormat::Jpeg => ImgFormat::Jpeg,
    };

    let format_str = match params.format {
        ImageFormat::Png => "png",
        ImageFormat::Jpeg => "jpeg",
    };

    let mut buffer = Cursor::new(Vec::new());
    if let Err(e) = rgba_image.write_to(&mut buffer, format) {
        return Response::error(
            ErrorCode::InternalError,
            format!("Failed to encode image: {}", e),
        );
    }

    let base64_data = base64::engine::general_purpose::STANDARD.encode(buffer.into_inner());

    Response::success(ResponseData::Screenshot {
        width,
        height,
        format: format_str.to_string(),
        base64: base64_data,
    })
}
