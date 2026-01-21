//! Screenshot command implementation.

use std::fs::File;
use std::io::Write;
use std::path::Path;

use agent_rdp_protocol::{ImageFormat, Request, ResponseData, ScreenshotRequest};
use base64::Engine;

use crate::cli::ScreenshotArgs;
use crate::output::Output;
use crate::session_manager::SessionManager;

pub async fn run(
    session: &str,
    args: ScreenshotArgs,
    output: &Output,
    timeout_ms: u64,
) -> anyhow::Result<()> {
    let manager = SessionManager::new(session.to_string());

    if !manager.is_daemon_alive() {
        output.print_error("daemon_not_running", "No daemon running for this session");
        std::process::exit(1);
    }

    let mut client = manager.ensure_daemon().await?;

    let format = match args.format.to_lowercase().as_str() {
        "png" => ImageFormat::Png,
        "jpeg" | "jpg" => ImageFormat::Jpeg,
        _ => {
            output.print_error("invalid_format", "Format must be 'png' or 'jpeg'");
            std::process::exit(1);
        }
    };

    let request = Request::Screenshot(ScreenshotRequest { format });
    let response = client.send(&request, timeout_ms).await?;

    if !response.success {
        output.print_response(&response);
        std::process::exit(1);
    }

    // Handle the screenshot data - save to file
    if let Some(ResponseData::Screenshot { width, height, base64, .. }) = response.data {
        let image_data = base64::engine::general_purpose::STANDARD.decode(&base64)?;

        let path = Path::new(&args.output);
        let mut file = File::create(path)?;
        file.write_all(&image_data)?;

        if output.is_json() {
            println!(
                r#"{{"success":true,"data":{{"type":"screenshot","path":"{}","width":{},"height":{}}}}}"#,
                path.display(),
                width,
                height
            );
        } else {
            println!("Screenshot saved to {} ({}x{})", path.display(), width, height);
        }
    }

    Ok(())
}
