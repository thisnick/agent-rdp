//! Clipboard backend for CLIPRDR integration.
//!
//! This module provides a custom clipboard backend that stores clipboard data
//! and communicates with the frame processor via channels.

use std::sync::Arc;

use ironrdp_cliprdr::backend::{CliprdrBackend, ClipboardMessage, ClipboardMessageProxy};
use ironrdp_cliprdr::pdu::{
    ClipboardGeneralCapabilityFlags, FileContentsRequest, FileContentsResponse,
    FormatDataRequest, FormatDataResponse, LockDataId, OwnedFormatDataResponse,
};
use ironrdp_cliprdr::{Cliprdr, Client};
use ironrdp_svc::impl_as_any;
use parking_lot::Mutex;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

// Re-export types needed by rdp_session
pub use ironrdp_cliprdr::pdu::{ClipboardFormat, ClipboardFormatId};
pub use ironrdp_cliprdr::CliprdrClient;

/// Standard clipboard format ID for Unicode text (CF_UNICODETEXT = 13).
pub fn cf_unicodetext() -> ClipboardFormatId {
    ClipboardFormatId::new(13)
}

/// Messages from backend to frame processor.
#[derive(Debug)]
pub enum BackendMessage {
    /// Backend wants to initiate copy (announce formats).
    InitiateCopy(Vec<ClipboardFormat>),
    /// Backend has format data ready to send.
    FormatData(OwnedFormatDataResponse),
    /// Backend wants to request data from remote.
    InitiatePaste(ClipboardFormatId),
}

/// Proxy that sends messages to the frame processor.
#[derive(Debug, Clone)]
pub struct ChannelProxy {
    tx: mpsc::UnboundedSender<BackendMessage>,
}

impl ChannelProxy {
    pub fn new(tx: mpsc::UnboundedSender<BackendMessage>) -> Self {
        Self { tx }
    }
}

impl ClipboardMessageProxy for ChannelProxy {
    fn send_clipboard_message(&self, message: ClipboardMessage) {
        let backend_msg = match message {
            ClipboardMessage::SendInitiateCopy(formats) => BackendMessage::InitiateCopy(formats),
            ClipboardMessage::SendFormatData(data) => BackendMessage::FormatData(data),
            ClipboardMessage::SendInitiatePaste(format_id) => BackendMessage::InitiatePaste(format_id),
            ClipboardMessage::Error(e) => {
                warn!("Clipboard backend error: {}", e);
                return;
            }
        };
        let _ = self.tx.send(backend_msg);
    }
}

/// Shared state for clipboard data.
#[derive(Debug)]
pub struct ClipboardState {
    /// Text we want to send to remote (set by clipboard set command).
    pub local_text: Option<String>,
    /// Text received from remote.
    pub remote_text: Option<String>,
    /// Formats available on remote clipboard.
    pub remote_formats: Vec<ClipboardFormat>,
    /// Pending text get request response channel.
    pub pending_get: Option<tokio::sync::oneshot::Sender<Result<Option<String>, String>>>,
    /// Notify when remote clipboard changes (for WebSocket integration).
    pub clipboard_changed_tx: Option<mpsc::UnboundedSender<()>>,
}

impl Default for ClipboardState {
    fn default() -> Self {
        Self {
            local_text: None,
            remote_text: None,
            remote_formats: Vec::new(),
            pending_get: None,
            clipboard_changed_tx: None,
        }
    }
}

/// Custom clipboard backend that stores data in memory.
#[derive(Debug)]
pub struct AgentClipboardBackend {
    state: Arc<Mutex<ClipboardState>>,
    proxy: ChannelProxy,
}

impl_as_any!(AgentClipboardBackend);

impl AgentClipboardBackend {
    pub fn new(state: Arc<Mutex<ClipboardState>>, proxy: ChannelProxy) -> Self {
        Self { state, proxy }
    }
}

impl CliprdrBackend for AgentClipboardBackend {
    fn temporary_directory(&self) -> &str {
        ".cliprdr"
    }

    fn client_capabilities(&self) -> ClipboardGeneralCapabilityFlags {
        ClipboardGeneralCapabilityFlags::USE_LONG_FORMAT_NAMES
    }

    fn on_ready(&mut self) {
        info!("CLIPRDR clipboard ready");
    }

    fn on_request_format_list(&mut self) {
        debug!("Backend: on_request_format_list");
        // During initialization, send our available formats (if any).
        let state = self.state.lock();
        if state.local_text.is_some() {
            let formats = vec![ClipboardFormat::new(cf_unicodetext())];
            self.proxy.send_clipboard_message(ClipboardMessage::SendInitiateCopy(formats));
        } else {
            // Send empty format list to complete initialization.
            self.proxy.send_clipboard_message(ClipboardMessage::SendInitiateCopy(vec![]));
        }
    }

    fn on_process_negotiated_capabilities(&mut self, _capabilities: ClipboardGeneralCapabilityFlags) {
        debug!("Backend: negotiated capabilities");
    }

    fn on_remote_copy(&mut self, available_formats: &[ClipboardFormat]) {
        debug!("Backend: remote copied, formats: {:?}", available_formats);
        let mut state = self.state.lock();
        state.remote_formats = available_formats.to_vec();
        // Clear old remote data since new data is available.
        state.remote_text = None;

        // Notify WebSocket clients that clipboard changed (if channel is set up).
        if let Some(ref tx) = state.clipboard_changed_tx {
            let _ = tx.send(());
        }
    }

    fn on_format_data_request(&mut self, request: FormatDataRequest) {
        debug!("Backend: format data request for {:?}", request.format);
        let state = self.state.lock();

        let response = if request.format == cf_unicodetext() {
            if let Some(ref text) = state.local_text {
                // Convert to UTF-16LE with null terminator.
                let utf16: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
                let bytes: Vec<u8> = utf16.iter().flat_map(|&c| c.to_le_bytes()).collect();
                OwnedFormatDataResponse::new_data(bytes)
            } else {
                OwnedFormatDataResponse::new_error()
            }
        } else {
            OwnedFormatDataResponse::new_error()
        };

        self.proxy.send_clipboard_message(ClipboardMessage::SendFormatData(response));
    }

    fn on_format_data_response(&mut self, response: FormatDataResponse<'_>) {
        debug!("Backend: format data response, is_error={}", response.is_error());

        let mut state = self.state.lock();

        if response.is_error() {
            // Server returned error - clipboard is empty or doesn't have text format.
            // This is normal, not an error condition.
            if let Some(tx) = state.pending_get.take() {
                let _ = tx.send(Ok(None));
            }
            return;
        }

        // Decode UTF-16LE to String.
        let data = response.data();
        if data.len() >= 2 {
            let utf16: Vec<u16> = data
                .chunks_exact(2)
                .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                .collect();
            // Remove null terminator if present.
            let text: String = String::from_utf16_lossy(&utf16)
                .trim_end_matches('\0')
                .to_string();

            debug!("Received clipboard text: {} chars", text.len());
            state.remote_text = Some(text.clone());

            if let Some(tx) = state.pending_get.take() {
                let _ = tx.send(Ok(Some(text)));
            }
        } else if let Some(tx) = state.pending_get.take() {
            let _ = tx.send(Ok(None));
        }
    }

    fn on_file_contents_request(&mut self, _request: FileContentsRequest) {
        debug!("Backend: file contents request (not supported)");
    }

    fn on_file_contents_response(&mut self, _response: FileContentsResponse<'_>) {
        debug!("Backend: file contents response (not supported)");
    }

    fn on_lock(&mut self, _data_id: LockDataId) {
        debug!("Backend: lock");
    }

    fn on_unlock(&mut self, _data_id: LockDataId) {
        debug!("Backend: unlock");
    }
}

/// Create the cliprdr client with our custom backend.
/// Returns the cliprdr client and a receiver for backend messages.
pub fn create_cliprdr(
    state: Arc<Mutex<ClipboardState>>,
) -> (CliprdrClient, mpsc::UnboundedReceiver<BackendMessage>) {
    let (proxy_tx, proxy_rx) = mpsc::unbounded_channel();
    let proxy = ChannelProxy::new(proxy_tx);
    let backend = Box::new(AgentClipboardBackend::new(state, proxy));
    let cliprdr = Cliprdr::<Client>::new(backend);
    (cliprdr, proxy_rx)
}
