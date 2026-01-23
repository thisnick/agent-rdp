//! View command implementation - opens the web viewer served by the daemon.

use crate::cli::ViewArgs;
use crate::output::Output;

pub async fn run(args: ViewArgs, output: &Output) -> anyhow::Result<()> {
    // The daemon serves the viewer HTML on the same port as the WebSocket server
    let url = format!("http://localhost:{}", args.port);

    if output.is_json() {
        println!(r#"{{"url":"{}"}}"#, url);
    } else {
        println!("Opening viewer at: {}", url);
    }

    // Open browser
    if let Err(e) = open::that(&url) {
        output.print_error("open_failed", &format!("Failed to open browser: {}", e));
        std::process::exit(1);
    }

    Ok(())
}
