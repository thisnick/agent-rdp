# agent-rdp

A CLI tool for AI agents to control Windows Remote Desktop sessions, built on [IronRDP](https://github.com/Devolutions/IronRDP).

## Features

- **Connect to RDP servers** - Full RDP protocol support with TLS and CredSSP authentication
- **Take screenshots** - Capture the remote desktop as PNG or JPEG
- **Mouse control** - Click, double-click, right-click, drag, scroll
- **Keyboard input** - Type text, press key combinations (Ctrl+C, Alt+Tab, etc.)
- **Clipboard sync** - Copy/paste text between local machine and remote Windows
- **Drive mapping** - Map local directories as network drives on the remote machine
- **JSON output** - Structured output for AI agent consumption
- **Session management** - Multiple named sessions with automatic daemon lifecycle

## Installation

### From npm

```bash
npm install agent-rdp
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

# Press single keys
agent-rdp keyboard key enter
agent-rdp keyboard key escape
agent-rdp keyboard key f5
```

### Scroll

```bash
agent-rdp scroll up --amount 3
agent-rdp scroll down --amount 5
agent-rdp scroll left
agent-rdp scroll right
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
});

// Screenshot
const { base64, width, height } = await rdp.screenshot({ format: 'png' });

// Mouse
await rdp.mouse.click(100, 200);
await rdp.mouse.rightClick(100, 200);
await rdp.mouse.doubleClick(100, 200);
await rdp.mouse.move(150, 250);
await rdp.mouse.drag(100, 100, 500, 500);

// Keyboard
await rdp.keyboard.type('Hello World');
await rdp.keyboard.press('ctrl+c');
await rdp.keyboard.key('enter');

// Scroll
await rdp.scroll.up(3);
await rdp.scroll.down(5);

// Clipboard
await rdp.clipboard.set('text to copy');
const text = await rdp.clipboard.get();

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
