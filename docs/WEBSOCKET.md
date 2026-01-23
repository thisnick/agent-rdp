# WebSocket Streaming Protocol

agent-rdp supports real-time WebSocket streaming for interactive viewing and debugging of remote desktop sessions. This document describes the WebSocket protocol used between the daemon and viewer clients.

## Overview

When enabled, the daemon starts a WebSocket server that:
- Broadcasts JPEG frames of the remote desktop at a configurable frame rate
- Accepts mouse and keyboard input from connected clients
- Supports bidirectional clipboard synchronization

## Connection Setup

### Enabling Streaming

Start a session with the `--stream-port` flag:

```bash
agent-rdp --stream-port 9224 connect --host 192.168.1.100 -u Admin -p secret
```

Or use environment variables:

```bash
export AGENT_RDP_STREAM_PORT=9224
export AGENT_RDP_STREAM_FPS=10
export AGENT_RDP_STREAM_QUALITY=80
```

### Accessing the Viewer

The daemon serves both the WebSocket API and an embedded HTML viewer on the same port:

- **HTML Viewer**: `http://localhost:{port}` (e.g., `http://localhost:9224`)
- **WebSocket API**: `ws://localhost:{port}` (e.g., `ws://localhost:9224`)

The port automatically detects whether an incoming request is a WebSocket upgrade or regular HTTP and responds appropriately.

You can also use the CLI to open the viewer in your browser:

```bash
agent-rdp view --port 9224
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `AGENT_RDP_STREAM_PORT` | 0 (disabled) | WebSocket server port |
| `AGENT_RDP_STREAM_FPS` | 10 | Frame broadcast rate (frames per second) |
| `AGENT_RDP_STREAM_QUALITY` | 80 | JPEG compression quality (0-100) |

## Message Types

All messages are JSON-encoded text frames.

### Server to Client

#### `status` - Connection Status

Sent immediately upon client connection.

```json
{
  "type": "status",
  "connected": true,
  "streaming": true,
  "viewportWidth": 1920,
  "viewportHeight": 1080
}
```

| Field | Type | Description |
|-------|------|-------------|
| `connected` | boolean | Whether RDP session is connected |
| `streaming` | boolean | Whether streaming is active |
| `viewportWidth` | number | Remote desktop width in pixels |
| `viewportHeight` | number | Remote desktop height in pixels |

#### `frame` - Desktop Frame

Broadcast periodically at the configured FPS.

```json
{
  "type": "frame",
  "data": "<base64-encoded-jpeg>",
  "metadata": {
    "deviceWidth": 1920,
    "deviceHeight": 1080
  }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `data` | string | Base64-encoded JPEG image data |
| `metadata.deviceWidth` | number | Image width in pixels |
| `metadata.deviceHeight` | number | Image height in pixels |

#### `clipboard_changed` - Remote Clipboard Changed

Sent when the remote Windows clipboard content changes (e.g., user copies text).

```json
{
  "type": "clipboard_changed"
}
```

This is a notification only. To retrieve the actual content, send a `clipboard_get` request.

#### `clipboard_data` - Clipboard Data Response

Response to a `clipboard_get` request from the client.

```json
{
  "type": "clipboard_data",
  "content": {
    "contentType": "text",
    "text": "clipboard content here"
  }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `content.contentType` | string | Content type (`"text"` currently supported) |
| `content.text` | string | The clipboard text content (may be null/empty) |

### Client to Server

#### `input_mouse` - Mouse Input

```json
{
  "type": "input_mouse",
  "eventType": "mousePressed",
  "x": 500,
  "y": 300,
  "button": "left"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `eventType` | string | `"mousePressed"`, `"mouseReleased"`, `"mouseMoved"`, or `"mouseWheel"` |
| `x` | number | X coordinate in remote desktop pixels |
| `y` | number | Y coordinate in remote desktop pixels |
| `button` | string | `"left"`, `"right"`, or `"middle"` (for press/release events) |
| `deltaX` | number | Horizontal scroll amount (for wheel events) |
| `deltaY` | number | Vertical scroll amount (for wheel events) |

#### `input_keyboard` - Keyboard Input

For non-printable keys (modifiers, arrows, function keys):

```json
{
  "type": "input_keyboard",
  "eventType": "keyDown",
  "key": "Control",
  "code": "ControlLeft"
}
```

For printable characters:

```json
{
  "type": "input_keyboard",
  "eventType": "char",
  "text": "a"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `eventType` | string | `"keyDown"`, `"keyUp"`, or `"char"` |
| `key` | string | Key name (e.g., `"Control"`, `"a"`) |
| `code` | string | Physical key code (e.g., `"ControlLeft"`, `"KeyA"`) |
| `text` | string | Character to type (for `"char"` events) |

**Event Types:**
- `keyDown` / `keyUp`: Send scancode-based key press/release. Used for modifiers, arrows, function keys, and key combinations.
- `char`: Send Unicode character input. Used for typing printable characters. Automatically includes key release.

#### `clipboard_get` - Request Remote Clipboard

Request the current remote clipboard content.

```json
{
  "type": "clipboard_get",
  "formats": ["text"]
}
```

The server will respond with a `clipboard_data` message.

#### `clipboard_set` - Set Clipboard Before Paste

Set the clipboard content on the server. The viewer sends this before forwarding a Ctrl+V keypress to ensure the clipboard data is available when the remote app pastes.

```json
{
  "type": "clipboard_set",
  "text": "clipboard content to paste"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `text` | string | The text to set on the clipboard |

## Coordinate System

- All coordinates are in remote desktop pixels (not scaled viewer pixels)
- Origin (0, 0) is the top-left corner
- Clients must scale mouse coordinates from their display to remote desktop coordinates

### Scaling Example

```javascript
function getRemoteCoordinates(clientX, clientY, canvas) {
  const rect = canvas.getBoundingClientRect();
  const scaleX = canvas.width / rect.width;
  const scaleY = canvas.height / rect.height;
  return {
    x: Math.round((clientX - rect.left) * scaleX),
    y: Math.round((clientY - rect.top) * scaleY)
  };
}
```

## Keyboard Scancode Mapping

The `code` field maps to RDP scancodes using the following reference:

| Code | Scancode | Extended |
|------|----------|----------|
| `KeyA`-`KeyZ` | 0x1E-0x2C | No |
| `Digit0`-`Digit9` | 0x02-0x0B | No |
| `F1`-`F12` | 0x3B-0x58 | No |
| `ArrowUp` | 0x48 | Yes |
| `ArrowDown` | 0x50 | Yes |
| `ArrowLeft` | 0x4B | Yes |
| `ArrowRight` | 0x4D | Yes |
| `ControlLeft` | 0x1D | No |
| `ControlRight` | 0x1D | Yes |
| `ShiftLeft` | 0x2A | No |
| `ShiftRight` | 0x36 | No |
| `AltLeft` | 0x38 | No |
| `AltRight` | 0x38 | Yes |
| `MetaLeft` | 0x5B | Yes |
| `MetaRight` | 0x5C | Yes |
| `Enter` | 0x1C | No |
| `Escape` | 0x01 | No |
| `Backspace` | 0x0E | No |
| `Tab` | 0x0F | No |
| `Space` | 0x39 | No |
| `Delete` | 0x53 | Yes |
| `Insert` | 0x52 | Yes |
| `Home` | 0x47 | Yes |
| `End` | 0x4F | Yes |
| `PageUp` | 0x49 | Yes |
| `PageDown` | 0x51 | Yes |

See `ws_input.rs` for the complete mapping table.

## Clipboard Flow

### Remote Copy to Local (Remote user copies text)

```
1. User copies text on remote Windows (Ctrl+C)
2. CLIPRDR protocol notifies daemon of format list change
3. Daemon broadcasts clipboard_changed to all WebSocket clients
4. Client receives clipboard_changed
5. Client sends clipboard_get request
6. Daemon requests format data via CLIPRDR protocol
7. Remote Windows provides clipboard data
8. Daemon sends clipboard_data response to client
9. Client writes text to local clipboard via navigator.clipboard.writeText()
```

### Local Paste to Remote (Viewer user pastes with Ctrl+V)

```
1. User presses Ctrl+V in the viewer
2. Viewer intercepts the keypress before sending
3. Viewer reads local clipboard via navigator.clipboard.readText()
4. Viewer sends clipboard_set message with the text
5. Daemon sets local_text and announces CF_UNICODETEXT to remote
6. Viewer sends the Ctrl+V keypress to remote
7. Remote app receives paste, requests format data via CLIPRDR
8. Daemon responds with local_text content
9. Remote app receives clipboard content
```

**Note:** There is a small race window between clipboard_set and the Ctrl+V keypress reaching the remote. In practice, this works reliably.

## Example Usage

### HTML Viewer

The daemon serves an embedded HTML viewer that demonstrates a complete implementation:

```bash
# Start streaming session
agent-rdp --stream-port 9224 connect --host 192.168.1.100 -u Admin -p secret

# Open viewer (method 1: CLI command)
agent-rdp view --port 9224

# Open viewer (method 2: direct browser access)
# Navigate to http://localhost:9224 in your browser
```

### Programmatic Access

```javascript
const ws = new WebSocket('ws://localhost:9224');

ws.onmessage = async (event) => {
  const msg = JSON.parse(event.data);

  if (msg.type === 'frame') {
    // Display frame
    const img = new Image();
    img.src = 'data:image/jpeg;base64,' + msg.data;
    // Draw to canvas...
  } else if (msg.type === 'clipboard_changed') {
    // Remote clipboard changed - fetch content
    ws.send(JSON.stringify({ type: 'clipboard_get', formats: ['text'] }));
  } else if (msg.type === 'clipboard_data') {
    // Write to local clipboard
    if (msg.content?.text) {
      await navigator.clipboard.writeText(msg.content.text);
    }
  }
};

// Send mouse click
ws.send(JSON.stringify({
  type: 'input_mouse',
  eventType: 'mousePressed',
  x: 500, y: 300,
  button: 'left'
}));

// Type text
ws.send(JSON.stringify({
  type: 'input_keyboard',
  eventType: 'char',
  text: 'Hello'
}));

// Paste from local clipboard (set clipboard then send Ctrl+V)
const text = await navigator.clipboard.readText();
ws.send(JSON.stringify({ type: 'clipboard_set', text }));
ws.send(JSON.stringify({
  type: 'input_keyboard',
  eventType: 'keyDown',
  key: 'Control',
  code: 'ControlLeft'
}));
ws.send(JSON.stringify({
  type: 'input_keyboard',
  eventType: 'keyDown',
  key: 'v',
  code: 'KeyV'
}));
// ... then keyUp events
```

## Security Considerations

- The WebSocket server binds to `0.0.0.0` by default (all interfaces)
- There is no authentication on the WebSocket connection
- For production use, consider:
  - Running behind a reverse proxy with authentication
  - Using SSH tunneling
  - Binding to localhost only and using a local viewer

## Compatibility

The input protocol is designed to be compatible with [agent-browser](https://github.com/anthropics/agent-browser), enabling similar automation patterns across browser and desktop targets.
