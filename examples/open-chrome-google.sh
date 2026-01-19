#!/bin/bash
# Script to open Chrome and navigate to google.com via RDP

set -e

AGENT_RDP="./target/release/agent-rdp"
SCREENSHOT_DIR="/tmp/rdp-screenshots"

mkdir -p "$SCREENSHOT_DIR"

echo "=== Step 1: Opening Chrome via Win+R ==="
# Press Win+R to open Run dialog
$AGENT_RDP keyboard press "win+r"
$AGENT_RDP wait 500

$AGENT_RDP screenshot --output "$SCREENSHOT_DIR/01-run-dialog.png"
echo "Screenshot saved: $SCREENSHOT_DIR/01-run-dialog.png"

echo "=== Step 2: Type 'chrome' and press Enter ==="
$AGENT_RDP keyboard type "chrome"
$AGENT_RDP wait 300

$AGENT_RDP screenshot --output "$SCREENSHOT_DIR/02-typed-chrome.png"
echo "Screenshot saved: $SCREENSHOT_DIR/02-typed-chrome.png"

$AGENT_RDP keyboard press "enter"
echo "Waiting for Chrome to open..."
$AGENT_RDP wait 3000

$AGENT_RDP screenshot --output "$SCREENSHOT_DIR/03-chrome-opened.png"
echo "Screenshot saved: $SCREENSHOT_DIR/03-chrome-opened.png"

echo "=== Step 3: Navigate to google.com ==="
# Press Ctrl+L to focus address bar
$AGENT_RDP keyboard press "ctrl+l"
$AGENT_RDP wait 300

$AGENT_RDP keyboard type "google.com"
$AGENT_RDP wait 300

$AGENT_RDP screenshot --output "$SCREENSHOT_DIR/04-typed-url.png"
echo "Screenshot saved: $SCREENSHOT_DIR/04-typed-url.png"

$AGENT_RDP keyboard press "enter"
echo "Waiting for page to load..."
$AGENT_RDP wait 3000

$AGENT_RDP screenshot --output "$SCREENSHOT_DIR/05-google-loaded.png"
echo "Screenshot saved: $SCREENSHOT_DIR/05-google-loaded.png"

echo ""
echo "=== Done! Screenshots saved to $SCREENSHOT_DIR ==="
ls -la "$SCREENSHOT_DIR"
