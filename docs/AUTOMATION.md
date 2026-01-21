# Windows UI Automation

agent-rdp provides UI automation capabilities for Windows Remote Desktop sessions, enabling AI agents to interact with Windows applications programmatically through the Windows UI Automation API.

## Overview

UI Automation allows you to:
- **Inspect the UI** - Get an accessibility tree snapshot of all visible elements
- **Interact with elements** - Click, fill text, select items by selector
- **Manage windows** - List, focus, minimize, maximize, close windows
- **Run commands** - Execute PowerShell commands on the remote machine

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Host Machine                             │
│  CLI/TS ──▶ Daemon ──▶ RDPDR Channel ──▶ \\TSCLIENT\agent-auto  │
└─────────────────────────────────────────────────────────────────┘
                              │ RDP Protocol
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Remote Windows Machine                        │
│  PowerShell Agent ◀──▶ File-based IPC ◀──▶ UI Automation API   │
└─────────────────────────────────────────────────────────────────┘
```

The automation system works by:

1. **Drive Mapping**: When connecting with `--enable-win-automation`, an automation directory is mapped as a network drive to the Windows machine via RDPDR (RDP Drive Redirection)

2. **PowerShell Agent**: A PowerShell script is launched on the remote Windows machine that uses the Windows UI Automation API

3. **File-based IPC**: Commands are sent via JSON files written to the mapped drive. The PowerShell agent reads requests and writes responses

4. **Automatic Cleanup**: When the RDP session disconnects, the mapped drive disappears, and the PowerShell agent exits gracefully

## Quick Start

```bash
# Connect with automation enabled
agent-rdp connect --host 192.168.1.100 -u Admin -p secret --enable-win-automation

# Take an accessibility tree snapshot
agent-rdp automate snapshot

# Click an element by automation ID
agent-rdp automate click "#SaveButton"

# Fill text in an input field
agent-rdp automate fill ".Edit" "Hello World"

# List all windows
agent-rdp automate window list
```

## Commands

### Snapshot

Get an accessibility tree of the entire desktop or a specific window:

```bash
# Full desktop snapshot
agent-rdp automate snapshot

# Include reference numbers for elements
agent-rdp automate snapshot --refs

# Limit tree depth
agent-rdp automate snapshot --max-depth 5

# Snapshot a specific window
agent-rdp automate snapshot --scope window --window "#Notepad"
```

The snapshot returns a hierarchical tree:

```json
{
  "snapshot_id": "abc123",
  "ref_count": 42,
  "root": {
    "ref": 1,
    "role": "window",
    "name": "Notepad",
    "automation_id": "Notepad",
    "bounds": { "x": 100, "y": 100, "width": 800, "height": 600 },
    "states": ["focusable", "focused"],
    "patterns": ["window", "transform"],
    "children": [...]
  }
}
```

### Element Operations

```bash
# Click an element
agent-rdp automate click <selector>
agent-rdp automate click "#SaveButton"
agent-rdp automate click "@5"              # By ref number from snapshot

# Double-click
agent-rdp automate double-click <selector>

# Right-click
agent-rdp automate right-click <selector>

# Focus an element
agent-rdp automate focus <selector>

# Get element properties
agent-rdp automate get <selector>

# Fill text (clears existing content first)
agent-rdp automate fill <selector> "text to fill"

# Clear text
agent-rdp automate clear <selector>

# Select item in ComboBox/ListBox
agent-rdp automate select <selector> "Item Name"

# Check/uncheck CheckBox
agent-rdp automate check <selector>
agent-rdp automate check <selector> --uncheck

# Scroll an element
agent-rdp automate scroll <selector> --direction down --amount 3
```

### Window Operations

```bash
# List all windows
agent-rdp automate window list

# Focus a window
agent-rdp automate window focus "#Notepad"

# Maximize/minimize/restore
agent-rdp automate window maximize
agent-rdp automate window minimize "#Notepad"
agent-rdp automate window restore

# Close a window
agent-rdp automate window close "#Notepad"
```

### Run Commands

Execute PowerShell commands on the remote machine:

```bash
# Run and wait for result
agent-rdp automate run "Get-Process" --wait

# Run in background
agent-rdp automate run "notepad.exe"

# Run with arguments
agent-rdp automate run "ping" --args "localhost" "-n" "4" --wait
```

### Wait and Status

```bash
# Wait for element to appear
agent-rdp automate wait-for <selector> --timeout 5000

# Wait for specific state
agent-rdp automate wait-for <selector> --state visible
agent-rdp automate wait-for <selector> --state enabled
agent-rdp automate wait-for <selector> --state gone

# Check automation status
agent-rdp automate status
```

## Selector Syntax

| Prefix | Type | Example | Description |
|--------|------|---------|-------------|
| `@` | Reference | `@5` | Element by snapshot ref number |
| `#` | AutomationId | `#SaveButton` | By AutomationId property |
| `.` | ClassName | `.Edit` | By Win32 class name |
| `~` | Pattern | `~Save*` | Name with wildcards |
| (none) | Name | `File` | By Name property (exact match) |

### Selector Tips

- **`@ref` numbers** are only valid for the current snapshot. Take a new snapshot if the UI changes.
- **`#automationId`** selectors are the most reliable as they survive UI changes.
- **`.className`** selectors match Win32 class names like `Edit`, `Button`, `ListBox`.
- **Name selectors** match the visible text/name of elements.

## File-based IPC Protocol

### Directory Structure

When automation is enabled, the following structure is created:

```
\\TSCLIENT\agent-automation\
├── handshake.json          # Agent status and capabilities
├── scripts/
│   └── agent.ps1           # PowerShell automation agent
├── requests/
│   └── req_<uuid>.json     # Commands from daemon
└── responses/
    └── res_<uuid>.json     # Responses from agent
```

### Handshake

The PowerShell agent writes a handshake file on startup:

```json
{
  "version": "1.0.0",
  "agent_pid": 12345,
  "started_at": "2024-01-15T10:30:00Z",
  "capabilities": ["snapshot", "click", "fill", "window", "run", ...],
  "ready": true
}
```

### Request/Response

Requests are JSON files with:
- `id`: Unique request identifier
- `command`: Command name (snapshot, click, fill, etc.)
- `params`: Command parameters

Responses include:
- `id`: Matching request ID
- `success`: Boolean
- `data`: Result data on success
- `error`: Error details on failure

### Atomicity

All file operations use atomic writes:
1. Write to `.tmp` file
2. Rename to final `.json` name
3. Use `.processing` lock files during execution

## Bootstrap Sequence

When `--enable-win-automation` is specified:

1. Create automation directory with unique UUID
2. Write embedded PowerShell script to disk
3. Map directory as `agent-automation` drive via RDPDR
4. Wait for desktop to stabilize (2-3 seconds)
5. Launch PowerShell via Win+R:
   ```
   powershell -ExecutionPolicy Bypass -WindowStyle Hidden -File "\\TSCLIENT\agent-automation\scripts\agent.ps1"
   ```
6. Wait for handshake with exponential backoff

## Cleanup

**Daemon side (on disconnect):**
- Remove the automation directory
- Called in disconnect handler and on daemon shutdown

**PowerShell agent side:**
- Polls for mapped drive availability every 50ms
- If `\\TSCLIENT\agent-automation` disappears, exits gracefully
- Prevents orphaned PowerShell processes

## Limitations

1. **UAC**: The PowerShell agent runs in user context. Elevated (admin) windows may not be automatable.

2. **Single Session**: Only one automation session per RDP connection.

3. **Windows Only**: Currently only Windows UI Automation is implemented. Future versions may add Linux AT-SPI and macOS Accessibility API support.

4. **Performance**: File-based IPC adds ~50-100ms latency per command compared to direct API calls.

## Troubleshooting

### "Automation agent not ready"

The PowerShell agent hasn't started or failed to launch. Check:
- Is the Windows machine logged in with a desktop?
- Did the Win+R command execute successfully?
- Check for `error.log` in the automation directory

### Element not found

- Take a fresh snapshot to get current element refs
- Verify the selector syntax is correct
- The element may be in a different window or not visible

### Slow performance

- Reduce snapshot `--max-depth` for faster responses
- Use specific window scope instead of full desktop
- Prefer `#automationId` selectors over tree traversal
