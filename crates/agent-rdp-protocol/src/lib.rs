//! Protocol types for agent-rdp IPC communication.
//!
//! This crate defines the request and response types used for
//! communication between the CLI and the daemon over IPC.

mod request;
mod response;

pub use request::*;
pub use response::*;
