# agent-rdp

A CLI tool for AI agents to control Windows Remote Desktop sessions, built on [IronRDP](https://github.com/Devolutions/IronRDP).

## Features

- **Connect to RDP servers** - Full RDP protocol support with TLS and CredSSP authentication
- **Take screenshots** - Capture the remote desktop as PNG or JPEG
- **Mouse control** - Click, double-click, right-click, drag, scroll
- **Keyboard input** - Type text, press key combinations (Ctrl+C, Alt+Tab, etc.)
- **Clipboard sync** - Copy/paste text between local machine and remote Windows
- **Drive mapping** - Map local directories as network drives on the remote machine
- **UI Automation** - Interact with Windows applications via accessibility API using native patterns (invoke, select, toggle, expand)
- **OCR text location** - Find text on screen using OCR when UI Automation isn't available
- **JSON output** - Structured output for AI agent consumption
- **Session management** - Multiple named sessions with automatic daemon lifecycle

## Installation

### From npm

```bash
npm install agent-rdp
```

### As a Claude Code skill

```bash
npx add-skill https://github.com/thisnick/agent-rdp
```

### From source

```bash
git clone https://github.com/thisnick/agent-rdp
cd agent-rdp
pnpm install
pnpm build      # Build native binary
pnpm build:ts   # Build TypeScript
```

## Usage

### Connect to an RDP Server

```bash
# Using command line (password visible in process list - not recommended)
agent-rdp connect --host 192.168.1.100 --username Administrator --password 'secret'

# Using environment variables (recommended)
export AGENT_RDP_USERNAME=Administrator
export AGENT_RDP_PASSWORD=secret
agent-rdp connect --host 192.168.1.100

# Using stdin (most secure)
echo 'secret' | agent-rdp connect --host 192.168.1.100 --username Administrator --password-stdin
```

### Take a Screenshot

```bash
# Save to file
agent-rdp screenshot --output desktop.png

# Output as base64 (for AI agents)
agent-rdp screenshot --base64

# With JSON output
agent-rdp --json screenshot --base64
```

### Mouse Operations

```bash
# Click at position
agent-rdp mouse click 500 300

# Right-click
agent-rdp mouse right-click 500 300

# Double-click
agent-rdp mouse double-click 500 300

# Move cursor
agent-rdp mouse move 100 200

# Drag from (100,100) to (500,500)
agent-rdp mouse drag 100 100 500 500
```

### Keyboard Operations

```bash
# Type text (supports Unicode)
agent-rdp keyboard type "Hello, World!"

# Press key combinations
agent-rdp keyboard press "ctrl+c"
agent-rdp keyboard press "alt+tab"
agent-rdp keyboard press "ctrl+shift+esc"

# Press single keys (use press command)
agent-rdp keyboard press enter
agent-rdp keyboard press escape
agent-rdp keyboard press f5
```

### Scroll

```bash
agent-rdp scroll up --amount 3
agent-rdp scroll down --amount 5
agent-rdp scroll left
agent-rdp scroll right
```

### Locate (OCR)

Find text on screen using OCR (powered by [ocrs](https://github.com/robertknight/ocrs)). Useful when UI Automation can't access certain elements (WebView content, some dialogs).

```bash
# Find lines containing text
agent-rdp locate "Cancel"

# Pattern matching (glob-style)
agent-rdp locate "Save*" --pattern

# Get all text on screen
agent-rdp locate --all

# JSON output
agent-rdp locate "OK" --json
```

Returns text lines with coordinates for clicking:
```
Found 1 line(s) containing 'Cancel':
  'Cancel Button' at (650, 420) size 80x14 - center: (690, 427)

To click the first match: agent-rdp mouse click 690 427
```

### Clipboard

```bash
# Set clipboard text (available when you paste on Windows)
agent-rdp clipboard set "Hello from CLI"

# Get clipboard text (after copying on Windows)
agent-rdp clipboard get

# With JSON output
agent-rdp --json clipboard get
```

### Drive Mapping

Map local directories as network drives on the remote Windows machine. Drives must be mapped at connect time. Multiple drives can be specified.

```bash
# Map local directories during connection
agent-rdp connect --host 192.168.1.100 -u Administrator -p secret \
  --drive /home/user/documents:Documents \
  --drive /tmp/shared:Shared

# List mapped drives
agent-rdp drive list
```

On the remote Windows machine, mapped drives appear in File Explorer as network locations.

### UI Automation

Interact with Windows applications programmatically via the Windows UI Automation API using native patterns (InvokePattern, SelectionItemPattern, TogglePattern, etc.). When enabled, a PowerShell agent is injected into the remote session that captures the accessibility tree and performs actions. Communication between the CLI and the agent uses a mapped drive as an IPC channel.

For detailed documentation, see [docs/AUTOMATION.md](docs/AUTOMATION.md).

```bash
# Connect with automation enabled
agent-rdp connect --host 192.168.1.100 -u Admin -p secret --enable-win-automation

# Take an accessibility tree snapshot (refs are always included)
agent-rdp automate snapshot

# Snapshot filtering options (like agent-browser)
agent-rdp automate snapshot -i              # Interactive elements only
agent-rdp automate snapshot -c              # Compact (remove empty structural elements)
agent-rdp automate snapshot -d 3            # Limit depth to 3 levels
agent-rdp automate snapshot -s "~*Notepad*" # Scope to a window/element
agent-rdp automate snapshot -i -c -d 5      # Combine options

# Pattern-based element operations (refs use @eN format)
agent-rdp automate invoke "#SaveButton"    # Invoke button (InvokePattern)
agent-rdp automate invoke "@e5"            # By ref number from snapshot
agent-rdp automate select "@e10"           # Select item (SelectionItemPattern)
agent-rdp automate toggle "@e7"            # Toggle checkbox (TogglePattern)
agent-rdp automate expand "@e3"            # Expand menu (ExpandCollapsePattern)
agent-rdp automate context-menu "@e5"      # Open context menu (Shift+F10)

# Fill text fields
agent-rdp automate fill ".Edit" "Hello World"

# Window operations
agent-rdp automate window list
agent-rdp automate window focus "~*Notepad*"

# Run PowerShell commands
agent-rdp automate run "Get-Process" --wait
```

**Selector Types:**
- `@e5` or `@5` - Reference number from snapshot (e prefix recommended)
- `#SaveButton` - Automation ID
- `.Edit` - Win32 class name
- `~*pattern*` - Wildcard name match
- `File` - Element name (exact match)

**Snapshot Output Format:**
```
- Window "Notepad" [ref=e1, id=Notepad]
  - MenuBar "Application" [ref=e2]
    - MenuItem "File" [ref=e3]
  - Edit "Text Editor" [ref=e5, value="Hello"]
```

### Session Management

```bash
# List active sessions
agent-rdp session list

# Get current session info
agent-rdp session info

# Close a session
agent-rdp session close

# Use a named session
agent-rdp --session work connect --host work-pc.local ...
agent-rdp --session work screenshot
```

### Disconnect

```bash
agent-rdp disconnect
```

### Web Viewer

Open the web-based viewer to see the remote desktop in your browser:

```bash
# Open viewer (connects to default streaming port 9224)
agent-rdp view

# Specify a different port
agent-rdp view --port 9224
```

The viewer requires WebSocket streaming to be enabled. Start a session with streaming:

```bash
agent-rdp --stream-port 9224 connect --host 192.168.1.100 -u Admin -p secret
agent-rdp view
```

## JSON Output

All commands support `--json` for structured output:

```bash
agent-rdp --json screenshot --base64
```

**Success response:**
```json
{
  "success": true,
  "data": {
    "type": "screenshot",
    "width": 1920,
    "height": 1080,
    "format": "png",
    "base64": "iVBORw0KGgo..."
  }
}
```

**Error response:**
```json
{
  "success": false,
  "error": {
    "code": "not_connected",
    "message": "Not connected to an RDP server"
  }
}
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `AGENT_RDP_SESSION` | Session name (default: "default") |
| `AGENT_RDP_USERNAME` | RDP username |
| `AGENT_RDP_PASSWORD` | RDP password |
| `AGENT_RDP_STREAM_PORT` | WebSocket streaming port (0 = disabled) |
| `AGENT_RDP_STREAM_FPS` | Frame rate for streaming (default: 10) |
| `AGENT_RDP_STREAM_QUALITY` | JPEG quality 0-100 (default: 80) |

## Node.js API

Use agent-rdp programmatically from Node.js/TypeScript:

```typescript
import { RdpSession } from 'agent-rdp';

const rdp = new RdpSession({ session: 'default' });

await rdp.connect({
  host: '192.168.1.100',
  username: 'Administrator',
  password: 'secret',
  width: 1280,
  height: 800,
  drives: [{ path: '/tmp/share', name: 'Share' }],
  enableWinAutomation: true,  // Enable UI Automation
});

// Screenshot
const { base64, width, height } = await rdp.screenshot({ format: 'png' });

// Mouse
await rdp.mouse.click({ x: 100, y: 200 });
await rdp.mouse.rightClick({ x: 100, y: 200 });
await rdp.mouse.doubleClick({ x: 100, y: 200 });
await rdp.mouse.move({ x: 150, y: 250 });
await rdp.mouse.drag({ from: { x: 100, y: 100 }, to: { x: 500, y: 500 } });

// Keyboard
await rdp.keyboard.type({ text: 'Hello World' });
await rdp.keyboard.press({ keys: 'ctrl+c' });
await rdp.keyboard.press({ keys: 'enter' });  // Single keys use press()

// Scroll
await rdp.scroll.up();                    // Default amount: 3
await rdp.scroll.down({ amount: 5 });     // Custom amount
await rdp.scroll.up({ x: 500, y: 300 });  // Scroll at position

// Clipboard
await rdp.clipboard.set({ text: 'text to copy' });
const text = await rdp.clipboard.get();

// Locate text using OCR
const matches = await rdp.locate('Cancel');
if (matches.length > 0) {
  await rdp.mouse.click({ x: matches[0].center_x, y: matches[0].center_y });
}

// Get all text on screen
const allText = await rdp.locateAll();

// Automation (requires --enable-win-automation at connect)
const snapshot = await rdp.automation.snapshot({ interactive: true });
await rdp.automation.invoke('@e5');          // Invoke button by ref
await rdp.automation.select('@e10');         // Select item
await rdp.automation.toggle('@e7');          // Toggle checkbox
await rdp.automation.expand('@e3');          // Expand menu
await rdp.automation.contextMenu('@e5');     // Open context menu
await rdp.automation.fill('#input', 'text'); // Fill text field
await rdp.automation.run('notepad.exe');     // Run command
await rdp.automation.waitFor('#SaveButton', { timeout: 5000 });

// Window management
const windows = await rdp.automation.listWindows();
await rdp.automation.focusWindow('~*Notepad*');
await rdp.automation.maximizeWindow();

// Drives
const drives = await rdp.drives.list();

// Session info
const info = await rdp.getInfo();

// Disconnect
await rdp.disconnect();
```

### WebSocket Streaming

Enable WebSocket streaming for real-time screen capture:

```typescript
const rdp = new RdpSession({
  session: 'viewer',
  streamPort: 9224,  // Enable streaming
});

await rdp.connect({...});

// Connect your WebSocket client to receive JPEG frames
const streamUrl = rdp.getStreamUrl(); // "ws://localhost:9224"
```

## Architecture

agent-rdp uses a daemon-per-session architecture:

1. **CLI** (`agent-rdp`) - Parses commands and communicates with the daemon
2. **Daemon** - Maintains the RDP connection and processes commands
3. **IPC** - Unix sockets (macOS/Linux) or TCP (Windows)

The daemon is automatically started on the first command and persists until explicitly closed or the session times out.

## Requirements

- Rust 1.75 or later
- Target RDP server with Network Level Authentication (NLA) enabled

## License

MIT OR Apache-2.0 (same as IronRDP)
