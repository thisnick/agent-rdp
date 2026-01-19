//! Clipboard handler.
//!
//! Stores clipboard content in the daemon's memory. This provides a simple
//! clipboard that AI agents can use to transfer text between commands.
//!
//! Note: This is NOT synchronized with the remote RDP clipboard. For that,
//! the full CLIPRDR virtual channel would need to be implemented.

use std::sync::Arc;

use agent_rdp_protocol::{ClipboardRequest, Response, ResponseData};
use tokio::sync::Mutex;

/// Handle a clipboard request.
pub async fn handle(clipboard: &Arc<Mutex<String>>, action: ClipboardRequest) -> Response {
    match action {
        ClipboardRequest::Get => {
            let text = clipboard.lock().await.clone();
            Response::success(ResponseData::Clipboard { text })
        }

        ClipboardRequest::Set { text } => {
            *clipboard.lock().await = text;
            Response::ok()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_clipboard_set_and_get() {
        let clipboard = Arc::new(Mutex::new(String::new()));

        // Initially empty
        let response = handle(&clipboard, ClipboardRequest::Get).await;
        assert!(response.success);
        if let Some(ResponseData::Clipboard { text }) = response.data {
            assert_eq!(text, "");
        } else {
            panic!("Expected Clipboard response");
        }

        // Set some text
        let response = handle(
            &clipboard,
            ClipboardRequest::Set {
                text: "hello world".to_string(),
            },
        )
        .await;
        assert!(response.success);

        // Get the text back
        let response = handle(&clipboard, ClipboardRequest::Get).await;
        assert!(response.success);
        if let Some(ResponseData::Clipboard { text }) = response.data {
            assert_eq!(text, "hello world");
        } else {
            panic!("Expected Clipboard response");
        }

        // Set different text
        let response = handle(
            &clipboard,
            ClipboardRequest::Set {
                text: "new content".to_string(),
            },
        )
        .await;
        assert!(response.success);

        // Verify it changed
        let response = handle(&clipboard, ClipboardRequest::Get).await;
        assert!(response.success);
        if let Some(ResponseData::Clipboard { text }) = response.data {
            assert_eq!(text, "new content");
        } else {
            panic!("Expected Clipboard response");
        }
    }
}
