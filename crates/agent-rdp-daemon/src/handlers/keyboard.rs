//! Keyboard input handler.

use std::collections::HashMap;
use std::sync::Arc;

use agent_rdp_protocol::{ErrorCode, KeyboardRequest, Response};
use ironrdp::pdu::input::fast_path::{FastPathInputEvent, KeyboardFlags};
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use tracing::debug;

use crate::rdp_session::RdpSession;

/// Handle a keyboard request.
pub async fn handle(
    rdp_session: &Arc<Mutex<Option<RdpSession>>>,
    action: KeyboardRequest,
) -> Response {
    // For typing text, send one character at a time with delays for reliability
    if let KeyboardRequest::Type { ref text } = action {
        debug!("Typing {} characters: {:?}", text.len(), text);

        const CHAR_DELAY_MS: u64 = 100;

        for ch in text.chars() {
            let code = ch as u16;
            let events = vec![
                FastPathInputEvent::UnicodeKeyboardEvent(KeyboardFlags::empty(), code),
                FastPathInputEvent::UnicodeKeyboardEvent(KeyboardFlags::RELEASE, code),
            ];

            {
                let session = rdp_session.lock().await;
                let rdp = match session.as_ref() {
                    Some(rdp) => rdp,
                    None => {
                        return Response::error(
                            ErrorCode::NotConnected,
                            "Not connected to an RDP server",
                        );
                    }
                };
                if let Err(e) = rdp.send_input(events).await {
                    return Response::error(ErrorCode::InternalError, e.to_string());
                }
            }
            sleep(Duration::from_millis(CHAR_DELAY_MS)).await;
        }
        return Response::ok();
    }

    // For key combinations, release lock between each key event
    if let KeyboardRequest::Press { ref keys } = action {
        debug!("Pressing key combination: {}", keys);
        let key_infos = match parse_key_combination(keys) {
            Ok(infos) => infos,
            Err(e) => {
                return Response::error(ErrorCode::InvalidRequest, e);
            }
        };

        // Press all keys down
        for info in &key_infos {
            debug!(
                "Key down: scancode=0x{:02X}, extended={}",
                info.scancode, info.extended
            );
            let event = create_key_event_ext(info.scancode, info.extended, false);
            {
                let session = rdp_session.lock().await;
                let rdp = match session.as_ref() {
                    Some(rdp) => rdp,
                    None => {
                        return Response::error(
                            ErrorCode::NotConnected,
                            "Not connected to an RDP server",
                        );
                    }
                };
                if let Err(e) = rdp.send_input(vec![event]).await {
                    return Response::error(ErrorCode::InternalError, e.to_string());
                }
            }
            sleep(Duration::from_millis(10)).await;
        }

        // Small delay before releasing
        sleep(Duration::from_millis(50)).await;

        // Release all keys in reverse order
        for info in key_infos.iter().rev() {
            debug!(
                "Key up: scancode=0x{:02X}, extended={}",
                info.scancode, info.extended
            );
            let event = create_key_event_ext(info.scancode, info.extended, true);
            {
                let session = rdp_session.lock().await;
                let rdp = match session.as_ref() {
                    Some(rdp) => rdp,
                    None => {
                        return Response::error(
                            ErrorCode::NotConnected,
                            "Not connected to an RDP server",
                        );
                    }
                };
                if let Err(e) = rdp.send_input(vec![event]).await {
                    return Response::error(ErrorCode::InternalError, e.to_string());
                }
            }
            sleep(Duration::from_millis(10)).await;
        }

        return Response::ok();
    }

    // For single key operations (KeyDown/KeyUp), use a scoped lock
    let session = rdp_session.lock().await;
    let rdp = match session.as_ref() {
        Some(rdp) => rdp,
        None => {
            return Response::error(ErrorCode::NotConnected, "Not connected to an RDP server");
        }
    };

    let events = match action {
        KeyboardRequest::Type { .. } | KeyboardRequest::Press { .. } => {
            // Handled above
            unreachable!()
        }

        KeyboardRequest::KeyDown { key } => match key_to_scancode(&key) {
            Some((scancode, extended)) => {
                vec![create_key_event_ext(scancode, extended, false)]
            }
            None => {
                return Response::error(
                    ErrorCode::InvalidRequest,
                    format!("Unknown key: {}", key),
                );
            }
        },

        KeyboardRequest::KeyUp { key } => match key_to_scancode(&key) {
            Some((scancode, extended)) => {
                vec![create_key_event_ext(scancode, extended, true)]
            }
            None => {
                return Response::error(
                    ErrorCode::InvalidRequest,
                    format!("Unknown key: {}", key),
                );
            }
        },
    };

    match rdp.send_input(events).await {
        Ok(()) => Response::ok(),
        Err(e) => Response::error(ErrorCode::InternalError, e.to_string()),
    }
}


/// Parse a key combination like "ctrl+c" into key info for sending.
fn parse_key_combination(keys: &str) -> Result<Vec<KeyInfo>, String> {
    let parts: Vec<String> = keys.split('+').map(|s| s.trim().to_lowercase()).collect();

    let mut key_infos = Vec::new();

    for key in &parts {
        let (scancode, extended) = key_to_scancode(&key)
            .ok_or_else(|| format!("Unknown key: {}", key))?;
        key_infos.push(KeyInfo { scancode, extended });
    }

    Ok(key_infos)
}

/// Key information including scancode and extended flag.
struct KeyInfo {
    scancode: u8,
    extended: bool,
}

/// Convert a key name to a scancode and extended flag.
fn key_to_scancode(key: &str) -> Option<(u8, bool)> {
    // Common key mappings (US keyboard layout scancodes)
    // (scancode, needs_extended_flag)
    let key_lower = key.to_lowercase();
    let key_map: HashMap<&str, (u8, bool)> = [
        // Modifier keys
        ("ctrl", (0x1D, false)),
        ("control", (0x1D, false)),
        ("lctrl", (0x1D, false)),
        ("rctrl", (0x1D, true)),  // Right ctrl needs extended flag
        ("alt", (0x38, false)),
        ("lalt", (0x38, false)),
        ("ralt", (0x38, true)),   // Right alt needs extended flag
        ("shift", (0x2A, false)),
        ("lshift", (0x2A, false)),
        ("rshift", (0x36, false)),
        ("win", (0x5B, true)),    // Windows key needs extended flag
        ("windows", (0x5B, true)),
        ("lwin", (0x5B, true)),
        ("rwin", (0x5C, true)),
        ("super", (0x5B, true)),
        ("meta", (0x5B, true)),

        // Function keys (no extended flag needed)
        ("esc", (0x01, false)),
        ("escape", (0x01, false)),
        ("f1", (0x3B, false)),
        ("f2", (0x3C, false)),
        ("f3", (0x3D, false)),
        ("f4", (0x3E, false)),
        ("f5", (0x3F, false)),
        ("f6", (0x40, false)),
        ("f7", (0x41, false)),
        ("f8", (0x42, false)),
        ("f9", (0x43, false)),
        ("f10", (0x44, false)),
        ("f11", (0x57, false)),
        ("f12", (0x58, false)),

        // Navigation keys
        ("tab", (0x0F, false)),
        ("enter", (0x1C, false)),
        ("return", (0x1C, false)),
        ("backspace", (0x0E, false)),
        ("space", (0x39, false)),
        ("capslock", (0x3A, false)),
        ("caps", (0x3A, false)),

        // Arrow keys (need extended flag)
        ("up", (0x48, true)),
        ("down", (0x50, true)),
        ("left", (0x4B, true)),
        ("right", (0x4D, true)),

        // Other navigation (need extended flag)
        ("insert", (0x52, true)),
        ("delete", (0x53, true)),
        ("home", (0x47, true)),
        ("end", (0x4F, true)),
        ("pageup", (0x49, true)),
        ("pgup", (0x49, true)),
        ("pagedown", (0x51, true)),
        ("pgdn", (0x51, true)),

        // Printscreen/scroll/pause
        ("printscreen", (0x37, true)),
        ("prtsc", (0x37, true)),
        ("scrolllock", (0x46, false)),
        ("pause", (0x45, false)),
        ("break", (0x45, false)),

        // Number row
        ("1", (0x02, false)),
        ("2", (0x03, false)),
        ("3", (0x04, false)),
        ("4", (0x05, false)),
        ("5", (0x06, false)),
        ("6", (0x07, false)),
        ("7", (0x08, false)),
        ("8", (0x09, false)),
        ("9", (0x0A, false)),
        ("0", (0x0B, false)),

        // Letter keys
        ("a", (0x1E, false)),
        ("b", (0x30, false)),
        ("c", (0x2E, false)),
        ("d", (0x20, false)),
        ("e", (0x12, false)),
        ("f", (0x21, false)),
        ("g", (0x22, false)),
        ("h", (0x23, false)),
        ("i", (0x17, false)),
        ("j", (0x24, false)),
        ("k", (0x25, false)),
        ("l", (0x26, false)),
        ("m", (0x32, false)),
        ("n", (0x31, false)),
        ("o", (0x18, false)),
        ("p", (0x19, false)),
        ("q", (0x10, false)),
        ("r", (0x13, false)),
        ("s", (0x1F, false)),
        ("t", (0x14, false)),
        ("u", (0x16, false)),
        ("v", (0x2F, false)),
        ("w", (0x11, false)),
        ("x", (0x2D, false)),
        ("y", (0x15, false)),
        ("z", (0x2C, false)),

        // Punctuation
        ("minus", (0x0C, false)),
        ("-", (0x0C, false)),
        ("equals", (0x0D, false)),
        ("=", (0x0D, false)),
        ("leftbracket", (0x1A, false)),
        ("[", (0x1A, false)),
        ("rightbracket", (0x1B, false)),
        ("]", (0x1B, false)),
        ("backslash", (0x2B, false)),
        ("\\", (0x2B, false)),
        ("semicolon", (0x27, false)),
        (";", (0x27, false)),
        ("quote", (0x28, false)),
        ("'", (0x28, false)),
        ("grave", (0x29, false)),
        ("`", (0x29, false)),
        ("comma", (0x33, false)),
        (",", (0x33, false)),
        ("period", (0x34, false)),
        (".", (0x34, false)),
        ("slash", (0x35, false)),
        ("/", (0x35, false)),
    ]
    .into_iter()
    .collect();

    key_map.get(key_lower.as_str()).copied()
}

/// Create a keyboard event with proper flags.
fn create_key_event_ext(scancode: u8, extended: bool, release: bool) -> FastPathInputEvent {
    let mut flags = KeyboardFlags::empty();
    if release {
        flags |= KeyboardFlags::RELEASE;
    }
    if extended {
        flags |= KeyboardFlags::EXTENDED;
    }
    FastPathInputEvent::KeyboardEvent(flags, scancode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_to_scancode() {
        assert_eq!(key_to_scancode("a"), Some((0x1E, false)));
        assert_eq!(key_to_scancode("A"), Some((0x1E, false)));
        assert_eq!(key_to_scancode("ctrl"), Some((0x1D, false)));
        assert_eq!(key_to_scancode("enter"), Some((0x1C, false)));
        assert_eq!(key_to_scancode("up"), Some((0x48, true))); // Extended key
        assert_eq!(key_to_scancode("unknown"), None);
    }

    #[test]
    fn test_parse_key_combination() {
        let key_infos = parse_key_combination("ctrl+c").unwrap();
        // Should have: ctrl and c
        assert_eq!(key_infos.len(), 2);
        assert_eq!(key_infos[0].scancode, 0x1D); // ctrl
        assert_eq!(key_infos[1].scancode, 0x2E); // c
    }
}
