//! Connect command implementation.

use std::io::{self, BufRead};
use std::path::Path;

use agent_rdp_protocol::{ConnectRequest, DriveMapping, Request};

use crate::cli::ConnectArgs;
use crate::output::Output;
use crate::session_manager::SessionManager;

pub async fn run(
    session: &str,
    args: ConnectArgs,
    output: &Output,
    timeout_ms: u64,
) -> anyhow::Result<()> {
    // Get password from args, env, or stdin
    let password = get_password(&args, output)?;

    // Parse drive mapping (single drive only)
    let drives = parse_drive_mapping(&args.drive, output)?;

    let manager = SessionManager::new(session.to_string());
    let mut client = manager.ensure_daemon().await?;

    let request = Request::Connect(ConnectRequest {
        host: args.host,
        port: args.port,
        username: args.username,
        password,
        domain: args.domain,
        width: args.width,
        height: args.height,
        drives,
    });

    let response = client.send(&request, timeout_ms).await?;
    output.print_response(&response);

    if !response.success {
        std::process::exit(1);
    }

    Ok(())
}

/// Parse a single drive mapping string (format: /path:DriveName) into a DriveMapping.
fn parse_drive_mapping(drive: &Option<String>, output: &Output) -> anyhow::Result<Vec<DriveMapping>> {
    let Some(drive_spec) = drive else {
        return Ok(vec![]);
    };

    // Find the last colon to split path from name
    if let Some(colon_pos) = drive_spec.rfind(':') {
        let path = &drive_spec[..colon_pos];
        let name = &drive_spec[colon_pos + 1..];

        if path.is_empty() {
            output.print_error(
                "invalid_drive",
                &format!("Invalid drive mapping '{}': path cannot be empty", drive_spec),
            );
            std::process::exit(1);
        }

        if name.is_empty() {
            output.print_error(
                "invalid_drive",
                &format!("Invalid drive mapping '{}': name cannot be empty", drive_spec),
            );
            std::process::exit(1);
        }

        // Expand ~ to home directory and verify path exists
        let expanded_path = shellexpand::tilde(path);
        let path_ref = Path::new(expanded_path.as_ref());

        if !path_ref.exists() {
            output.print_error(
                "invalid_drive",
                &format!("Drive path '{}' does not exist", expanded_path),
            );
            std::process::exit(1);
        }

        if !path_ref.is_dir() {
            output.print_error(
                "invalid_drive",
                &format!("Drive path '{}' is not a directory", expanded_path),
            );
            std::process::exit(1);
        }

        Ok(vec![DriveMapping {
            path: expanded_path.into_owned(),
            name: name.to_string(),
        }])
    } else {
        output.print_error(
            "invalid_drive",
            &format!(
                "Invalid drive mapping '{}': expected format /path:DriveName",
                drive_spec
            ),
        );
        std::process::exit(1);
    }
}

/// Get password from command line, environment, or stdin.
fn get_password(args: &ConnectArgs, output: &Output) -> anyhow::Result<String> {
    // Priority: --password-stdin > --password/env
    if args.password_stdin {
        let stdin = io::stdin();
        let mut line = String::new();
        stdin.lock().read_line(&mut line)?;
        return Ok(line.trim_end().to_string());
    }

    if let Some(ref password) = args.password {
        return Ok(password.clone());
    }

    // No password provided
    output.print_error(
        "missing_password",
        "Password required. Use --password, AGENT_RDP_PASSWORD env var, or --password-stdin",
    );
    std::process::exit(1);
}
