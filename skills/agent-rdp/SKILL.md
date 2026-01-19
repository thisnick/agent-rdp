---
name: agent-rdp
description: Controls Windows Remote Desktop sessions for automation, testing, and remote administration. Use when the user needs to connect to Windows machines via RDP, take screenshots, click, type, or interact with remote Windows desktops.
allowed-tools: Bash(agent-rdp:*)
---

# Windows Remote Desktop Control with agent-rdp

## Quick start

```bash
agent-rdp connect --host <ip> -u <user> -p <pass>   # Connect to RDP
agent-rdp screenshot --output desktop.png            # Take screenshot
agent-rdp mouse click 500 300                        # Click at position
agent-rdp keyboard type "Hello"                      # Type text
agent-rdp disconnect                                 # Disconnect
```

## Core workflow

1. Connect: `agent-rdp connect --host <ip> --username <user> --password <pass>`
2. Screenshot: `agent-rdp screenshot --base64` (returns base64 image)
3. Interact using mouse/keyboard commands with screen coordinates
4. Re-screenshot after actions to verify results

## Commands

### Connection
```bash
agent-rdp connect --host 192.168.1.100 -u Admin -p secret
agent-rdp connect --host 192.168.1.100 -u Admin --password-stdin  # Read password from stdin
agent-rdp connect --host 192.168.1.100 --width 1920 --height 1080
agent-rdp connect --host 192.168.1.100 --drive /tmp/share:Share   # Map local directory
agent-rdp disconnect
```

### Screenshot
```bash
agent-rdp screenshot                      # Save to ./screenshot.png
agent-rdp screenshot --output desktop.png # Save to specific file
agent-rdp screenshot --base64             # Output as base64
agent-rdp screenshot --format jpeg        # JPEG format
agent-rdp --json screenshot --base64      # JSON output with base64
```

### Mouse
```bash
agent-rdp mouse click 500 300             # Left click at (500, 300)
agent-rdp mouse right-click 500 300       # Right click
agent-rdp mouse double-click 500 300      # Double click
agent-rdp mouse move 100 200              # Move cursor
agent-rdp mouse drag 100 100 500 500      # Drag from (100,100) to (500,500)
```

### Keyboard
```bash
agent-rdp keyboard type "Hello World"     # Type text (supports Unicode)
agent-rdp keyboard press "ctrl+c"         # Key combination
agent-rdp keyboard press "alt+tab"        # Switch windows
agent-rdp keyboard press "ctrl+shift+esc" # Task manager
agent-rdp keyboard press "win+r"          # Run dialog
agent-rdp keyboard key enter              # Single key
agent-rdp keyboard key escape
agent-rdp keyboard key f5
```

### Scroll
```bash
agent-rdp scroll up 3                     # Scroll up 3 notches
agent-rdp scroll down 5                   # Scroll down 5 notches
agent-rdp scroll left
agent-rdp scroll right
```

### Clipboard
```bash
agent-rdp clipboard set "Text to paste"   # Set clipboard (paste on Windows)
agent-rdp clipboard get                   # Get clipboard (after copy on Windows)
```

### Drive mapping
```bash
# Map at connect time
agent-rdp connect --host <ip> -u <user> -p <pass> --drive /local/path:DriveName

# List mapped drives
agent-rdp drive list
```

### Session management
```bash
agent-rdp session list                    # List active sessions
agent-rdp session info                    # Current session info
agent-rdp --session work connect ...      # Named session
agent-rdp --session work screenshot       # Use named session
```

### Wait
```bash
agent-rdp wait 2000                       # Wait 2 seconds
```

## JSON output

Add `--json` for machine-readable output:
```bash
agent-rdp --json screenshot --base64
agent-rdp --json clipboard get
agent-rdp --json session info
```

## Example: Open PowerShell and run command

```bash
agent-rdp connect --host 192.168.1.100 -u Admin -p secret
agent-rdp wait 3000                       # Wait for desktop
agent-rdp keyboard press "win+r"          # Open Run dialog
agent-rdp wait 1000
agent-rdp keyboard type "powershell"
agent-rdp keyboard key enter
agent-rdp wait 2000                       # Wait for PowerShell
agent-rdp keyboard type "Get-Process"
agent-rdp keyboard key enter
agent-rdp screenshot --output result.png
agent-rdp disconnect
```

## Example: File transfer via mapped drive

```bash
# Connect with local directory mapped
agent-rdp connect --host 192.168.1.100 -u Admin -p secret --drive /tmp/transfer:Transfer

# On Windows, access files at \\tsclient\Transfer
agent-rdp keyboard press "win+r"
agent-rdp wait 500
agent-rdp keyboard type "\\\\tsclient\\Transfer"
agent-rdp keyboard key enter
```

## Environment variables

```bash
export AGENT_RDP_USERNAME=Administrator
export AGENT_RDP_PASSWORD=secret
export AGENT_RDP_SESSION=default
agent-rdp connect --host 192.168.1.100    # Uses env vars for credentials
```

## Debugging with WebSocket streaming

```bash
# Enable streaming viewer on port 9224
agent-rdp --stream-port 9224 connect --host 192.168.1.100 -u Admin -p secret

# Open browser to view stream (requires separate viewer)
# WebSocket at ws://localhost:9224 broadcasts JPEG frames
```
