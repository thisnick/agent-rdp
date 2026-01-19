//! WebSocket input translation to RDP FastPath events.
//!
//! Translates input messages from the WebSocket viewer (matching agent-browser protocol)
//! to RDP FastPathInputEvent format.

use std::collections::HashMap;

use ironrdp::pdu::input::fast_path::{FastPathInputEvent, KeyboardFlags};
use ironrdp::pdu::input::mouse::{MousePdu, PointerFlags};
use serde::Deserialize;

/// Mouse input message from WebSocket client.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename = "input_mouse")]
pub struct MouseInputMessage {
    #[serde(rename = "eventType")]
    pub event_type: String,
    pub x: u16,
    pub y: u16,
    #[serde(default)]
    pub button: Option<String>,
    #[serde(rename = "deltaX", default)]
    pub delta_x: Option<i32>,
    #[serde(rename = "deltaY", default)]
    pub delta_y: Option<i32>,
}

/// Keyboard input message from WebSocket client.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename = "input_keyboard")]
pub struct KeyboardInputMessage {
    #[serde(rename = "eventType")]
    pub event_type: String,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
}

/// Generic WebSocket input message (for dispatching).
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum WsInputMessage {
    #[serde(rename = "input_mouse")]
    Mouse(MouseInputPayload),
    #[serde(rename = "input_keyboard")]
    Keyboard(KeyboardInputPayload),
}

/// Mouse input payload (fields only, without type tag).
#[derive(Debug, Deserialize)]
pub struct MouseInputPayload {
    #[serde(rename = "eventType")]
    pub event_type: String,
    pub x: u16,
    pub y: u16,
    #[serde(default)]
    pub button: Option<String>,
    #[serde(rename = "deltaX", default)]
    pub delta_x: Option<i32>,
    #[serde(rename = "deltaY", default)]
    pub delta_y: Option<i32>,
}

/// Keyboard input payload (fields only, without type tag).
#[derive(Debug, Deserialize)]
pub struct KeyboardInputPayload {
    #[serde(rename = "eventType")]
    pub event_type: String,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
}

/// Convert a mouse input message to FastPath events.
pub fn mouse_to_fastpath(msg: &MouseInputPayload) -> Vec<FastPathInputEvent> {
    match msg.event_type.as_str() {
        "mousePressed" => {
            let button_flags = button_str_to_flags(msg.button.as_deref());
            vec![create_mouse_event(
                msg.x,
                msg.y,
                button_flags | PointerFlags::DOWN,
            )]
        }
        "mouseReleased" => {
            let button_flags = button_str_to_flags(msg.button.as_deref());
            // Release is sent WITHOUT the DOWN flag
            vec![create_mouse_event(msg.x, msg.y, button_flags)]
        }
        "mouseMoved" => {
            vec![create_mouse_event(msg.x, msg.y, PointerFlags::MOVE)]
        }
        "mouseWheel" => {
            // Handle vertical scroll
            if let Some(delta_y) = msg.delta_y {
                // RDP wheel rotation: positive = scroll up, negative = scroll down
                // The delta is typically in pixels, we need to convert to wheel units
                // Standard wheel rotation is 120 units per notch
                let wheel_delta = if delta_y < 0 {
                    // Scroll down (towards user) - negative delta in browser
                    -((-delta_y).min(32767) as i16)
                } else {
                    // Scroll up (away from user) - positive delta in browser
                    delta_y.min(32767) as i16
                };

                vec![FastPathInputEvent::MouseEvent(MousePdu {
                    flags: PointerFlags::VERTICAL_WHEEL,
                    number_of_wheel_rotation_units: wheel_delta,
                    x_position: msg.x,
                    y_position: msg.y,
                })]
            } else if let Some(delta_x) = msg.delta_x {
                // Horizontal scroll
                let wheel_delta = if delta_x < 0 {
                    -((-delta_x).min(32767) as i16)
                } else {
                    delta_x.min(32767) as i16
                };

                vec![FastPathInputEvent::MouseEvent(MousePdu {
                    flags: PointerFlags::HORIZONTAL_WHEEL,
                    number_of_wheel_rotation_units: wheel_delta,
                    x_position: msg.x,
                    y_position: msg.y,
                })]
            } else {
                vec![]
            }
        }
        _ => vec![],
    }
}

/// Convert a keyboard input message to FastPath events.
pub fn keyboard_to_fastpath(msg: &KeyboardInputPayload) -> Vec<FastPathInputEvent> {
    match msg.event_type.as_str() {
        "keyDown" => {
            // Try to get scancode from key or code
            if let Some((scancode, extended)) = get_scancode_from_message(msg) {
                vec![create_key_event(scancode, extended, false)]
            } else {
                vec![]
            }
        }
        "keyUp" => {
            if let Some((scancode, extended)) = get_scancode_from_message(msg) {
                vec![create_key_event(scancode, extended, true)]
            } else {
                vec![]
            }
        }
        "char" => {
            // Send unicode character
            if let Some(text) = &msg.text {
                text.chars()
                    .flat_map(|ch| {
                        let code = ch as u16;
                        vec![
                            FastPathInputEvent::UnicodeKeyboardEvent(KeyboardFlags::empty(), code),
                            FastPathInputEvent::UnicodeKeyboardEvent(KeyboardFlags::RELEASE, code),
                        ]
                    })
                    .collect()
            } else {
                vec![]
            }
        }
        _ => vec![],
    }
}

/// Try to get scancode from the message's key or code fields.
fn get_scancode_from_message(msg: &KeyboardInputPayload) -> Option<(u8, bool)> {
    // First try the code field (e.g., "KeyA", "ArrowUp")
    if let Some(code) = &msg.code {
        if let Some(result) = code_to_scancode(code) {
            return Some(result);
        }
    }

    // Fall back to key field (e.g., "a", "Enter")
    if let Some(key) = &msg.key {
        if let Some(result) = key_to_scancode(key) {
            return Some(result);
        }
    }

    None
}

/// Convert button string to PointerFlags.
fn button_str_to_flags(button: Option<&str>) -> PointerFlags {
    match button {
        Some("left") | None => PointerFlags::LEFT_BUTTON,
        Some("right") => PointerFlags::RIGHT_BUTTON,
        Some("middle") => PointerFlags::MIDDLE_BUTTON_OR_WHEEL,
        _ => PointerFlags::LEFT_BUTTON,
    }
}

/// Create a mouse event.
fn create_mouse_event(x: u16, y: u16, flags: PointerFlags) -> FastPathInputEvent {
    FastPathInputEvent::MouseEvent(MousePdu {
        flags,
        number_of_wheel_rotation_units: 0,
        x_position: x,
        y_position: y,
    })
}

/// Create a keyboard event.
fn create_key_event(scancode: u8, extended: bool, release: bool) -> FastPathInputEvent {
    let mut flags = KeyboardFlags::empty();
    if release {
        flags |= KeyboardFlags::RELEASE;
    }
    if extended {
        flags |= KeyboardFlags::EXTENDED;
    }
    FastPathInputEvent::KeyboardEvent(flags, scancode)
}

/// Convert JavaScript KeyboardEvent.code to scancode.
/// See: https://developer.mozilla.org/en-US/docs/Web/API/KeyboardEvent/code/code_values
fn code_to_scancode(code: &str) -> Option<(u8, bool)> {
    let code_map: HashMap<&str, (u8, bool)> = [
        // Letter keys
        ("KeyA", (0x1E, false)),
        ("KeyB", (0x30, false)),
        ("KeyC", (0x2E, false)),
        ("KeyD", (0x20, false)),
        ("KeyE", (0x12, false)),
        ("KeyF", (0x21, false)),
        ("KeyG", (0x22, false)),
        ("KeyH", (0x23, false)),
        ("KeyI", (0x17, false)),
        ("KeyJ", (0x24, false)),
        ("KeyK", (0x25, false)),
        ("KeyL", (0x26, false)),
        ("KeyM", (0x32, false)),
        ("KeyN", (0x31, false)),
        ("KeyO", (0x18, false)),
        ("KeyP", (0x19, false)),
        ("KeyQ", (0x10, false)),
        ("KeyR", (0x13, false)),
        ("KeyS", (0x1F, false)),
        ("KeyT", (0x14, false)),
        ("KeyU", (0x16, false)),
        ("KeyV", (0x2F, false)),
        ("KeyW", (0x11, false)),
        ("KeyX", (0x2D, false)),
        ("KeyY", (0x15, false)),
        ("KeyZ", (0x2C, false)),
        // Digit keys
        ("Digit0", (0x0B, false)),
        ("Digit1", (0x02, false)),
        ("Digit2", (0x03, false)),
        ("Digit3", (0x04, false)),
        ("Digit4", (0x05, false)),
        ("Digit5", (0x06, false)),
        ("Digit6", (0x07, false)),
        ("Digit7", (0x08, false)),
        ("Digit8", (0x09, false)),
        ("Digit9", (0x0A, false)),
        // Function keys
        ("F1", (0x3B, false)),
        ("F2", (0x3C, false)),
        ("F3", (0x3D, false)),
        ("F4", (0x3E, false)),
        ("F5", (0x3F, false)),
        ("F6", (0x40, false)),
        ("F7", (0x41, false)),
        ("F8", (0x42, false)),
        ("F9", (0x43, false)),
        ("F10", (0x44, false)),
        ("F11", (0x57, false)),
        ("F12", (0x58, false)),
        // Modifier keys
        ("ShiftLeft", (0x2A, false)),
        ("ShiftRight", (0x36, false)),
        ("ControlLeft", (0x1D, false)),
        ("ControlRight", (0x1D, true)),
        ("AltLeft", (0x38, false)),
        ("AltRight", (0x38, true)),
        ("MetaLeft", (0x5B, true)),
        ("MetaRight", (0x5C, true)),
        // Navigation keys
        ("ArrowUp", (0x48, true)),
        ("ArrowDown", (0x50, true)),
        ("ArrowLeft", (0x4B, true)),
        ("ArrowRight", (0x4D, true)),
        ("Home", (0x47, true)),
        ("End", (0x4F, true)),
        ("PageUp", (0x49, true)),
        ("PageDown", (0x51, true)),
        ("Insert", (0x52, true)),
        ("Delete", (0x53, true)),
        // Editing keys
        ("Backspace", (0x0E, false)),
        ("Tab", (0x0F, false)),
        ("Enter", (0x1C, false)),
        ("NumpadEnter", (0x1C, true)),
        ("Escape", (0x01, false)),
        ("Space", (0x39, false)),
        ("CapsLock", (0x3A, false)),
        // Punctuation
        ("Minus", (0x0C, false)),
        ("Equal", (0x0D, false)),
        ("BracketLeft", (0x1A, false)),
        ("BracketRight", (0x1B, false)),
        ("Backslash", (0x2B, false)),
        ("Semicolon", (0x27, false)),
        ("Quote", (0x28, false)),
        ("Backquote", (0x29, false)),
        ("Comma", (0x33, false)),
        ("Period", (0x34, false)),
        ("Slash", (0x35, false)),
        // Lock keys
        ("NumLock", (0x45, false)),
        ("ScrollLock", (0x46, false)),
        // System keys
        ("PrintScreen", (0x37, true)),
        ("Pause", (0x45, false)),
        // Numpad
        ("Numpad0", (0x52, false)),
        ("Numpad1", (0x4F, false)),
        ("Numpad2", (0x50, false)),
        ("Numpad3", (0x51, false)),
        ("Numpad4", (0x4B, false)),
        ("Numpad5", (0x4C, false)),
        ("Numpad6", (0x4D, false)),
        ("Numpad7", (0x47, false)),
        ("Numpad8", (0x48, false)),
        ("Numpad9", (0x49, false)),
        ("NumpadAdd", (0x4E, false)),
        ("NumpadSubtract", (0x4A, false)),
        ("NumpadMultiply", (0x37, false)),
        ("NumpadDivide", (0x35, true)),
        ("NumpadDecimal", (0x53, false)),
    ]
    .into_iter()
    .collect();

    code_map.get(code).copied()
}

/// Convert key name to scancode (US keyboard layout).
fn key_to_scancode(key: &str) -> Option<(u8, bool)> {
    let key_lower = key.to_lowercase();
    let key_map: HashMap<&str, (u8, bool)> = [
        // Modifier keys
        ("ctrl", (0x1D, false)),
        ("control", (0x1D, false)),
        ("alt", (0x38, false)),
        ("shift", (0x2A, false)),
        ("meta", (0x5B, true)),
        // Function keys
        ("escape", (0x01, false)),
        ("esc", (0x01, false)),
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
        // Navigation
        ("tab", (0x0F, false)),
        ("enter", (0x1C, false)),
        ("backspace", (0x0E, false)),
        ("space", (0x39, false)),
        (" ", (0x39, false)),
        ("capslock", (0x3A, false)),
        // Arrow keys
        ("arrowup", (0x48, true)),
        ("arrowdown", (0x50, true)),
        ("arrowleft", (0x4B, true)),
        ("arrowright", (0x4D, true)),
        ("up", (0x48, true)),
        ("down", (0x50, true)),
        ("left", (0x4B, true)),
        ("right", (0x4D, true)),
        // Other navigation
        ("insert", (0x52, true)),
        ("delete", (0x53, true)),
        ("home", (0x47, true)),
        ("end", (0x4F, true)),
        ("pageup", (0x49, true)),
        ("pagedown", (0x51, true)),
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
        ("-", (0x0C, false)),
        ("=", (0x0D, false)),
        ("[", (0x1A, false)),
        ("]", (0x1B, false)),
        ("\\", (0x2B, false)),
        (";", (0x27, false)),
        ("'", (0x28, false)),
        ("`", (0x29, false)),
        (",", (0x33, false)),
        (".", (0x34, false)),
        ("/", (0x35, false)),
    ]
    .into_iter()
    .collect();

    key_map.get(key_lower.as_str()).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mouse_pressed() {
        let msg = MouseInputPayload {
            event_type: "mousePressed".to_string(),
            x: 100,
            y: 200,
            button: Some("left".to_string()),
            delta_x: None,
            delta_y: None,
        };
        let events = mouse_to_fastpath(&msg);
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_mouse_wheel() {
        let msg = MouseInputPayload {
            event_type: "mouseWheel".to_string(),
            x: 100,
            y: 200,
            button: None,
            delta_x: None,
            delta_y: Some(-120),
        };
        let events = mouse_to_fastpath(&msg);
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_keyboard_key_down() {
        let msg = KeyboardInputPayload {
            event_type: "keyDown".to_string(),
            key: Some("a".to_string()),
            code: Some("KeyA".to_string()),
            text: None,
        };
        let events = keyboard_to_fastpath(&msg);
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_keyboard_char() {
        let msg = KeyboardInputPayload {
            event_type: "char".to_string(),
            key: None,
            code: None,
            text: Some("abc".to_string()),
        };
        let events = keyboard_to_fastpath(&msg);
        // 3 chars × 2 events (down + up) = 6 events
        assert_eq!(events.len(), 6);
    }

    #[test]
    fn test_code_to_scancode() {
        assert_eq!(code_to_scancode("KeyA"), Some((0x1E, false)));
        assert_eq!(code_to_scancode("ArrowUp"), Some((0x48, true)));
        assert_eq!(code_to_scancode("ControlRight"), Some((0x1D, true)));
    }
}
