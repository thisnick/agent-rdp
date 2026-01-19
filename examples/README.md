# Examples

## scripts/

Shell script examples demonstrating common agent-rdp workflows.

| Script | Description |
|--------|-------------|
| `demo-explorer.sh` | Demo opening Windows Explorer |
| `open-explorer.sh` | Open Windows Explorer to a specific path |
| `open-chrome-google.sh` | Launch Chrome and navigate to Google |
| `list-drives.sh` | List available drives on the remote system |

**Usage:**
```bash
# Set credentials
export AGENT_RDP_USERNAME=your_user
export AGENT_RDP_PASSWORD=your_pass

# Run a script
./examples/scripts/open-chrome-google.sh --host 192.168.1.100
```

## viewer/

Browser-based viewer for debugging and interactive control.

| File | Description |
|------|-------------|
| `viewer.html` | WebSocket-based desktop viewer with mouse/keyboard support |

**Usage:**

1. Start agent-rdp with streaming enabled:
   ```bash
   agent-rdp --stream-port 9224 connect --host <ip> -u <user> -p <pass>
   ```

2. Open `viewer.html` in a browser

3. Connect to `ws://localhost:9224`

**Features:**
- Live desktop frame streaming (JPEG)
- Mouse input (click, move, drag, scroll)
- Keyboard input (typing, special keys)
- Fullscreen mode

**Configuration via environment:**
| Variable | Default | Description |
|----------|---------|-------------|
| `AGENT_RDP_STREAM_PORT` | 0 (disabled) | WebSocket server port |
| `AGENT_RDP_STREAM_FPS` | 10 | Frame rate |
| `AGENT_RDP_STREAM_QUALITY` | 80 | JPEG quality (0-100) |
