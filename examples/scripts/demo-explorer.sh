#!/bin/bash
# Demo script: Connect to RDP and open Windows Explorer
#
# Usage: ./scripts/demo-explorer.sh --host <IP> -u <USER> -p <PASS>
#
# Example:
#   ./scripts/demo-explorer.sh --host 192.168.1.100 -u admin -p secret
#   ./scripts/demo-explorer.sh --host 192.168.1.100 -u admin -p secret --drive /tmp:Shared

set -e

AGENT_RDP="./target/release/agent-rdp"

# Check if binary exists
if [ ! -f "$AGENT_RDP" ]; then
    echo "Error: agent-rdp binary not found. Run 'cargo build --release' first."
    exit 1
fi

# Pass all arguments to connect
echo "Connecting to RDP server..."
$AGENT_RDP connect "$@"

echo "Waiting for desktop to load..."
$AGENT_RDP wait 2000

echo "Opening Windows Explorer (Win+E)..."
$AGENT_RDP keyboard press "win+e"

echo "Waiting for Explorer to open..."
$AGENT_RDP wait 1000

echo "Taking screenshot..."
$AGENT_RDP screenshot --output ./explorer-screenshot.png

echo "Done! Screenshot saved to ./explorer-screenshot.png"
