#!/bin/bash
# List mapped drive contents via PowerShell
#
# Usage: ./scripts/list-drives.sh --host <IP> -u <USER> -p <PASS> --drive /path:DriveName
#
# Example:
#   ./scripts/list-drives.sh --host 192.168.1.100 -u admin -p secret --drive /tmp:Shared

set -e

AGENT_RDP="./target/release/agent-rdp"

# Check if binary exists
if [ ! -f "$AGENT_RDP" ]; then
    echo "Error: agent-rdp binary not found. Run 'cargo build --release' first."
    exit 1
fi

# Extract drive name from arguments (format: --drive /path:DriveName)
DRIVE_NAME=""
for i in "$@"; do
    if [[ "$prev" == "--drive" ]]; then
        # Extract the part after the colon
        DRIVE_NAME="${i##*:}"
        break
    fi
    prev="$i"
done

if [ -z "$DRIVE_NAME" ]; then
    echo "Error: --drive argument required. Use --drive /path:DriveName"
    exit 1
fi

# Pass all arguments to connect
echo "Connecting to RDP server..."
$AGENT_RDP connect "$@"

echo "Waiting for desktop to load..."
$AGENT_RDP wait 3000

echo "Opening PowerShell..."
$AGENT_RDP keyboard press "super+r"
$AGENT_RDP wait 500
$AGENT_RDP keyboard type "powershell"
$AGENT_RDP keyboard key "enter"
$AGENT_RDP wait 2000

echo "Maximizing window..."
$AGENT_RDP keyboard press "super+up"
$AGENT_RDP wait 500

echo "Listing drive contents (\\\\TSCLIENT\\$DRIVE_NAME)..."
$AGENT_RDP keyboard type "Get-ChildItem \"\\\\TSCLIENT\\$DRIVE_NAME\""
$AGENT_RDP keyboard key "enter"
$AGENT_RDP wait 2000

echo "Taking screenshot..."
SCREENSHOT_PATH="/tmp/drives-screenshot.png"
if $AGENT_RDP screenshot --output "$SCREENSHOT_PATH"; then
    echo "Done! Screenshot saved to $SCREENSHOT_PATH"
else
    echo "Screenshot failed. Trying with base64 output..."
    $AGENT_RDP screenshot --base64
fi
