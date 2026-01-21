//! Automation handler for Windows UI Automation.

use std::sync::Arc;

use agent_rdp_protocol::{
    AccessibilityElement, AccessibilitySnapshot, AutomateRequest, AutomationStatus, ElementBounds,
    ElementValue, ErrorCode, Response, ResponseData, RunResult, WindowInfo,
};
use tokio::sync::Mutex;
use tracing::error;

use crate::automation::SharedAutomationState;
use crate::rdp_session::RdpSession;

/// Handle an automation request.
pub async fn handle(
    rdp_session: &Arc<Mutex<Option<RdpSession>>>,
    automation_state: &SharedAutomationState,
    request: AutomateRequest,
) -> Response {
    // Check if connected
    {
        let session = rdp_session.lock().await;
        if session.is_none() {
            return Response::error(ErrorCode::NotConnected, "Not connected to RDP server");
        }
    }

    // Check if automation is enabled and agent is ready
    let mut state = automation_state.lock().await;
    if !state.enabled {
        return Response::error(
            ErrorCode::AutomationNotEnabled,
            "Automation not enabled. Use --enable-win-automation when connecting",
        );
    }

    // If agent not ready, try checking for a late handshake
    if !state.agent_ready {
        if let Some(ref ipc) = state.ipc {
            match ipc.check_handshake().await {
                Ok(Some(handshake)) => {
                    // Agent is now ready!
                    state.agent_ready = true;
                    state.agent_pid = Some(handshake.agent_pid);
                    tracing::info!(
                        "Automation agent became ready: PID={}, version={}",
                        handshake.agent_pid,
                        handshake.version
                    );
                }
                Ok(None) => {
                    return Response::error(
                        ErrorCode::AutomationError,
                        "Automation agent not ready. Agent may still be starting or failed to launch",
                    );
                }
                Err(e) => {
                    return Response::error(
                        ErrorCode::AutomationError,
                        format!("Failed to check automation agent status: {}", e),
                    );
                }
            }
        } else {
            return Response::error(
                ErrorCode::AutomationError,
                "Automation IPC not initialized",
            );
        }
    }

    let ipc = match state.ipc.as_ref() {
        Some(ipc) => ipc,
        None => {
            return Response::error(
                ErrorCode::AutomationError,
                "Automation IPC not initialized",
            );
        }
    };

    // Send request to PowerShell agent
    match ipc.send_request(&request).await {
        Ok(data) => convert_response(request, data),
        Err(e) => {
            error!("Automation request failed: {}", e);
            Response::error(ErrorCode::AutomationError, e.to_string())
        }
    }
}

/// Convert the JSON response from PowerShell agent to protocol response.
fn convert_response(request: AutomateRequest, data: serde_json::Value) -> Response {
    match request {
        AutomateRequest::Snapshot { .. } => {
            match parse_snapshot_response(data) {
                Ok(snapshot) => Response::success(ResponseData::Snapshot(snapshot)),
                Err(e) => {
                    error!("Failed to parse snapshot response: {}", e);
                    Response::error(ErrorCode::AutomationError, e.to_string())
                }
            }
        }

        AutomateRequest::Get { .. } => {
            match parse_element_response(data) {
                Ok(element) => Response::success(ResponseData::Element(element)),
                Err(e) => {
                    error!("Failed to parse element response: {}", e);
                    Response::error(ErrorCode::AutomationError, e.to_string())
                }
            }
        }

        AutomateRequest::Window { action, .. } => {
            if action == agent_rdp_protocol::WindowAction::List {
                match parse_window_list_response(data) {
                    Ok(windows) => Response::success(ResponseData::WindowList { windows }),
                    Err(e) => {
                        error!("Failed to parse window list response: {}", e);
                        Response::error(ErrorCode::AutomationError, e.to_string())
                    }
                }
            } else {
                Response::ok()
            }
        }

        AutomateRequest::Run { wait, .. } => {
            if wait {
                match parse_run_response(data) {
                    Ok(result) => Response::success(ResponseData::RunResult(result)),
                    Err(e) => {
                        error!("Failed to parse run response: {}", e);
                        Response::error(ErrorCode::AutomationError, e.to_string())
                    }
                }
            } else {
                match parse_run_response(data) {
                    Ok(result) => Response::success(ResponseData::RunResult(result)),
                    Err(_) => Response::ok(),
                }
            }
        }

        AutomateRequest::Status => {
            match parse_status_response(data) {
                Ok(status) => Response::success(ResponseData::AutomationStatus(status)),
                Err(e) => {
                    error!("Failed to parse status response: {}", e);
                    Response::error(ErrorCode::AutomationError, e.to_string())
                }
            }
        }

        // All other actions return simple Ok
        _ => Response::ok(),
    }
}

/// Parse snapshot response from PowerShell agent.
fn parse_snapshot_response(data: serde_json::Value) -> anyhow::Result<AccessibilitySnapshot> {
    let snapshot_id = data["snapshot_id"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();
    let ref_count = data["ref_count"].as_u64().unwrap_or(0) as u32;
    let root_data = &data["root"];

    let root = parse_element(root_data)?;

    Ok(AccessibilitySnapshot {
        snapshot_id,
        ref_count,
        root,
    })
}

/// Parse a single element from the accessibility tree.
fn parse_element(data: &serde_json::Value) -> anyhow::Result<AccessibilityElement> {
    let r#ref = data["ref"].as_u64().map(|v| v as u32);
    let role = data["role"].as_str().unwrap_or("unknown").to_string();
    let name = data["name"].as_str().map(|s| s.to_string());
    let automation_id = data["automation_id"].as_str().map(|s| s.to_string());
    let class_name = data["class_name"].as_str().map(|s| s.to_string());
    let value = data["value"].as_str().map(|s| s.to_string());

    let bounds = if let Some(bounds_data) = data.get("bounds") {
        Some(ElementBounds {
            x: bounds_data["x"].as_i64().unwrap_or(0) as i32,
            y: bounds_data["y"].as_i64().unwrap_or(0) as i32,
            width: bounds_data["width"].as_i64().unwrap_or(0) as i32,
            height: bounds_data["height"].as_i64().unwrap_or(0) as i32,
        })
    } else {
        None
    };

    let states = data["states"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let patterns = data["patterns"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let children = data["children"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| parse_element(v).ok())
                .collect()
        })
        .unwrap_or_default();

    Ok(AccessibilityElement {
        r#ref,
        role,
        name,
        automation_id,
        class_name,
        bounds,
        states,
        value,
        patterns,
        children,
    })
}

/// Parse element value response from PowerShell agent.
fn parse_element_response(data: serde_json::Value) -> anyhow::Result<ElementValue> {
    let name = data["name"].as_str().map(|s| s.to_string());
    let value = data["value"].as_str().map(|s| s.to_string());

    let states = data["states"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let bounds = if let Some(bounds_data) = data.get("bounds") {
        Some(ElementBounds {
            x: bounds_data["x"].as_i64().unwrap_or(0) as i32,
            y: bounds_data["y"].as_i64().unwrap_or(0) as i32,
            width: bounds_data["width"].as_i64().unwrap_or(0) as i32,
            height: bounds_data["height"].as_i64().unwrap_or(0) as i32,
        })
    } else {
        None
    };

    Ok(ElementValue {
        name,
        value,
        states,
        bounds,
    })
}

/// Parse window list response from PowerShell agent.
fn parse_window_list_response(data: serde_json::Value) -> anyhow::Result<Vec<WindowInfo>> {
    let windows_data = data["windows"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Missing windows array"))?;

    let windows = windows_data
        .iter()
        .map(|w| {
            let title = w["title"].as_str().unwrap_or("").to_string();
            let process_name = w["process_name"].as_str().map(|s| s.to_string());
            let process_id = w["process_id"].as_u64().map(|v| v as u32);

            let bounds = if let Some(bounds_data) = w.get("bounds") {
                Some(ElementBounds {
                    x: bounds_data["x"].as_i64().unwrap_or(0) as i32,
                    y: bounds_data["y"].as_i64().unwrap_or(0) as i32,
                    width: bounds_data["width"].as_i64().unwrap_or(0) as i32,
                    height: bounds_data["height"].as_i64().unwrap_or(0) as i32,
                })
            } else {
                None
            };

            let minimized = w["minimized"].as_bool().unwrap_or(false);
            let maximized = w["maximized"].as_bool().unwrap_or(false);

            WindowInfo {
                title,
                process_name,
                process_id,
                bounds,
                minimized,
                maximized,
            }
        })
        .collect();

    Ok(windows)
}

/// Parse run command response from PowerShell agent.
fn parse_run_response(data: serde_json::Value) -> anyhow::Result<RunResult> {
    let exit_code = data["exit_code"].as_i64().map(|v| v as i32);
    let stdout = data["stdout"].as_str().map(|s| s.to_string());
    let stderr = data["stderr"].as_str().map(|s| s.to_string());
    let pid = data["pid"].as_u64().map(|v| v as u32);

    Ok(RunResult {
        exit_code,
        stdout,
        stderr,
        pid,
    })
}

/// Parse status response from PowerShell agent.
fn parse_status_response(data: serde_json::Value) -> anyhow::Result<AutomationStatus> {
    let agent_running = data["agent_running"].as_bool().unwrap_or(false);
    let agent_pid = data["agent_pid"].as_u64().map(|v| v as u32);
    let version = data["version"].as_str().map(|s| s.to_string());

    let capabilities = data["capabilities"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    Ok(AutomationStatus {
        agent_running,
        agent_pid,
        capabilities,
        version,
    })
}
