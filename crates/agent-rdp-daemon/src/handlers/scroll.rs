//! Scroll input handler.

use std::sync::Arc;

use agent_rdp_protocol::{ErrorCode, Response, ScrollDirection, ScrollRequest};
use ironrdp::pdu::input::fast_path::FastPathInputEvent;
use ironrdp::pdu::input::mouse::{MousePdu, PointerFlags};
use tokio::sync::Mutex;

use crate::rdp_session::RdpSession;

/// Handle a scroll request.
pub async fn handle(
    rdp_session: &Arc<Mutex<Option<RdpSession>>>,
    params: ScrollRequest,
) -> Response {
    let session = rdp_session.lock().await;

    let rdp = match session.as_ref() {
        Some(rdp) => rdp,
        None => {
            return Response::error(ErrorCode::NotConnected, "Not connected to an RDP server");
        }
    };

    // Use specified position or default to center of screen
    let x = params.x.unwrap_or(rdp.width() / 2);
    let y = params.y.unwrap_or(rdp.height() / 2);

    let events = match params.direction {
        ScrollDirection::Up | ScrollDirection::Down => {
            create_vertical_scroll_events(x, y, params.direction, params.amount)
        }
        ScrollDirection::Left | ScrollDirection::Right => {
            create_horizontal_scroll_events(x, y, params.direction, params.amount)
        }
    };

    match rdp.send_input(events).await {
        Ok(()) => Response::ok(),
        Err(e) => Response::error(ErrorCode::InternalError, e.to_string()),
    }
}

/// Create vertical scroll events.
fn create_vertical_scroll_events(
    x: u16,
    y: u16,
    direction: ScrollDirection,
    amount: u32,
) -> Vec<FastPathInputEvent> {
    let mut events = Vec::new();

    // Each scroll notch is 120 wheel delta units
    let delta_per_notch: i16 = 120;
    let wheel_units = match direction {
        ScrollDirection::Up => delta_per_notch,
        ScrollDirection::Down => -delta_per_notch,
        _ => 0,
    };

    for _ in 0..amount {
        let mut flags = PointerFlags::VERTICAL_WHEEL;
        if wheel_units < 0 {
            flags |= PointerFlags::WHEEL_NEGATIVE;
        }

        events.push(FastPathInputEvent::MouseEvent(MousePdu {
            flags,
            number_of_wheel_rotation_units: wheel_units,
            x_position: x,
            y_position: y,
        }));
    }

    events
}

/// Create horizontal scroll events.
fn create_horizontal_scroll_events(
    x: u16,
    y: u16,
    direction: ScrollDirection,
    amount: u32,
) -> Vec<FastPathInputEvent> {
    let mut events = Vec::new();

    let delta_per_notch: i16 = 120;
    let wheel_units = match direction {
        ScrollDirection::Right => delta_per_notch,
        ScrollDirection::Left => -delta_per_notch,
        _ => 0,
    };

    for _ in 0..amount {
        let mut flags = PointerFlags::HORIZONTAL_WHEEL;
        if wheel_units < 0 {
            flags |= PointerFlags::WHEEL_NEGATIVE;
        }

        events.push(FastPathInputEvent::MouseEvent(MousePdu {
            flags,
            number_of_wheel_rotation_units: wheel_units,
            x_position: x,
            y_position: y,
        }));
    }

    events
}
