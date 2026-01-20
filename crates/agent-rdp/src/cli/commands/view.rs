//! View command implementation - opens the web viewer in a browser.

use crate::cli::ViewArgs;
use crate::output::Output;

/// GitHub Pages URL for the viewer
const VIEWER_URL: &str = "https://thisnick.github.io/agent-rdp/viewer.html";

pub async fn run(args: ViewArgs, output: &Output) -> anyhow::Result<()> {
    let url = format!("{}?ws=ws://localhost:{}", VIEWER_URL, args.port);

    if output.is_json() {
        println!(r#"{{"url":"{}"}}"#, url);
    } else {
        println!("Opening viewer at: {}", url);
    }

    if let Err(e) = open::that(&url) {
        output.print_error("open_failed", &format!("Failed to open browser: {}", e));
        std::process::exit(1);
    }

    Ok(())
}
