//! Mouse input handler.

use std::sync::Arc;

use agent_rdp_protocol::{ErrorCode, MouseButton, MouseRequest, Response};
use ironrdp::pdu::input::fast_path::FastPathInputEvent;
use ironrdp::pdu::input::mouse::{MousePdu, PointerFlags};
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use tracing::debug;

use crate::rdp_session::RdpSession;

/// Handle a mouse request.
pub async fn handle(
    rdp_session: &Arc<Mutex<Option<RdpSession>>>,
    action: MouseRequest,
) -> Response {
    let session = rdp_session.lock().await;

    let rdp = match session.as_ref() {
        Some(rdp) => rdp,
        None => {
            return Response::error(ErrorCode::NotConnected, "Not connected to an RDP server");
        }
    };

    let result = match action {
        MouseRequest::Move { x, y } => {
            debug!("Mouse move to ({}, {})", x, y);
            let events = vec![create_mouse_event(x, y, PointerFlags::MOVE)];
            rdp.send_input(events).await
        }

        MouseRequest::Click { x, y } => {
            debug!("Mouse click at ({}, {})", x, y);
            // Send down event
            let down_event = vec![create_mouse_event(x, y, PointerFlags::LEFT_BUTTON | PointerFlags::DOWN)];
            if let Err(e) = rdp.send_input(down_event).await {
                return Response::error(ErrorCode::InternalError, e.to_string());
            }
            // Small delay between down and up
            sleep(Duration::from_millis(20)).await;
            // Send up event
            let up_event = vec![create_mouse_event(x, y, PointerFlags::LEFT_BUTTON)];
            rdp.send_input(up_event).await
        }

        MouseRequest::RightClick { x, y } => {
            let events = vec![
                create_mouse_event(x, y, PointerFlags::RIGHT_BUTTON | PointerFlags::DOWN),
                create_mouse_event(x, y, PointerFlags::RIGHT_BUTTON),
            ];
            rdp.send_input(events).await
        }

        MouseRequest::DoubleClick { x, y } => {
            let events = vec![
                create_mouse_event(x, y, PointerFlags::LEFT_BUTTON | PointerFlags::DOWN),
                create_mouse_event(x, y, PointerFlags::LEFT_BUTTON),
                create_mouse_event(x, y, PointerFlags::LEFT_BUTTON | PointerFlags::DOWN),
                create_mouse_event(x, y, PointerFlags::LEFT_BUTTON),
            ];
            rdp.send_input(events).await
        }

        MouseRequest::MiddleClick { x, y } => {
            let events = vec![
                create_mouse_event(x, y, PointerFlags::MIDDLE_BUTTON_OR_WHEEL | PointerFlags::DOWN),
                create_mouse_event(x, y, PointerFlags::MIDDLE_BUTTON_OR_WHEEL),
            ];
            rdp.send_input(events).await
        }

        MouseRequest::Drag { from_x, from_y, to_x, to_y } => {
            // Press at start position
            let start_events = vec![
                create_mouse_event(from_x, from_y, PointerFlags::MOVE),
                create_mouse_event(from_x, from_y, PointerFlags::LEFT_BUTTON | PointerFlags::DOWN),
            ];
            if let Err(e) = rdp.send_input(start_events).await {
                return Response::error(ErrorCode::InternalError, e.to_string());
            }

            // Small delay for drag
            sleep(Duration::from_millis(50)).await;

            // Move to end and release
            let end_events = vec![
                create_mouse_event(to_x, to_y, PointerFlags::MOVE),
                create_mouse_event(to_x, to_y, PointerFlags::LEFT_BUTTON),
            ];
            rdp.send_input(end_events).await
        }

        MouseRequest::ButtonDown { button } => {
            let flags = button_to_flags(button) | PointerFlags::DOWN;
            let events = vec![create_mouse_event(0, 0, flags | PointerFlags::MOVE)];
            rdp.send_input(events).await
        }

        MouseRequest::ButtonUp { button } => {
            let flags = button_to_flags(button);
            let events = vec![create_mouse_event(0, 0, flags | PointerFlags::MOVE)];
            rdp.send_input(events).await
        }
    };

    match result {
        Ok(()) => Response::ok(),
        Err(e) => Response::error(ErrorCode::InternalError, e.to_string()),
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

/// Convert MouseButton to PointerFlags.
fn button_to_flags(button: MouseButton) -> PointerFlags {
    match button {
        MouseButton::Left => PointerFlags::LEFT_BUTTON,
        MouseButton::Right => PointerFlags::RIGHT_BUTTON,
        MouseButton::Middle => PointerFlags::MIDDLE_BUTTON_OR_WHEEL,
    }
}
