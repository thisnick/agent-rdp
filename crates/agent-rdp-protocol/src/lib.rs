//! Protocol types for agent-rdp IPC communication.
//!
//! This crate defines the request and response types used for
//! communication between the CLI and the daemon over IPC.

mod automation;
mod request;
mod response;

pub use automation::*;
pub use request::*;
pub use response::*;
