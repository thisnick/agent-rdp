#!/bin/bash
# Open Windows Explorer via RDP session
#
# Usage: ./scripts/open-explorer.sh [session]
#
# This script opens a Windows Explorer window using the Win+E keyboard shortcut.

set -e

SESSION="${1:-default}"
AGENT_RDP="./target/release/agent-rdp"

# Check if binary exists
if [ ! -f "$AGENT_RDP" ]; then
    echo "Error: agent-rdp binary not found. Run 'cargo build --release' first."
    exit 1
fi

echo "Opening Windows Explorer on session '$SESSION'..."

# Press Win+E to open Explorer
$AGENT_RDP --session "$SESSION" keyboard press "win+e"

echo "Done. Explorer window should now be open."
