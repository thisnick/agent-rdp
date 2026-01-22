# Windows UI Automation - Implementation Specification

This document describes the architecture, protocols, and implementation details of agent-rdp's UI automation system.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Host Machine                             │
│                                                                   │
│  CLI ──► Daemon ──► FileIpc ──► RDPDR ──► \\TSCLIENT\agent-auto │
│                                                                   │
└─────────────────────────────────────────────────────────────────┘
                              │ RDP Protocol (RDPDR Channel)
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Remote Windows Machine                        │
│                                                                   │
│  agent.ps1 ◄──► File Polling ◄──► Windows UI Automation API     │
│                                                                   │
└─────────────────────────────────────────────────────────────────┘
```

## RDPDR (RDP Drive Redirection)

Communication between host and remote uses **RDPDR** - the RDP protocol's drive redirection channel. This maps a local host directory as a network drive visible on the Windows machine as `\\TSCLIENT\agent-automation`.

### Backend State

The RDPDR backend maintains several mappings to track open files:

| Map | Purpose |
|-----|---------|
| `drive_paths` | device_id → local base path |
| `file_map` | file_id → file handle (None for directories) |
| `file_path_map` | file_id → full filesystem path |
| `file_device_map` | file_id → device_id |
| `file_dir_map` | file_id → directory iteration state |
| `delete_on_close` | file_id → deletion flag |

### File Operations

| Operation | RDPDR Request | Backend Action |
|-----------|---------------|----------------|
| Open/Create | `DeviceCreateRequest` | Assign file_id, open handle, store in all maps |
| Read | `DeviceReadRequest` | Lookup handle by file_id, read bytes |
| Write | `DeviceWriteRequest` | Lookup handle by file_id, write bytes, flush |
| Rename | `SetInformation(Rename)` | Call `fs::rename`, update `file_path_map` |
| Delete | `SetInformation(Disposition)` | Set `delete_on_close[file_id] = true` |
| Close | `DeviceCloseRequest` | Sync file, clean all maps, perform deferred deletion |

### Delete-on-Close Semantics

Windows uses deferred deletion. Files are marked for deletion via `FileDispositionInformation`, then actually deleted when the handle is closed:

```
1. Open file → assigns file_id, stores in all maps
2. SetInformation(Disposition) → sets delete_on_close flag
3. Close → sync_all(), remove from all maps, then delete file
```

This ordering is critical - cleaning up the maps before deletion prevents stale file_id references.

## File-based IPC Protocol

### Directory Structure

```
<session-dir>/automation-<uuid>/
├── handshake.json              # Agent ready signal
├── agent.log                   # Agent debug log
├── scripts/
│   └── agent.ps1               # PowerShell automation agent
├── requests/
│   └── req_<id>.json           # Commands from daemon
└── responses/
    └── res_<id>.json           # Responses from agent
```

The `<uuid>` is generated per-connection to ensure fresh state and avoid conflicts.

### Request Lifecycle

```
1. CLI sends Request to Daemon via Unix socket/TCP
2. Daemon generates unique request ID (8-char UUID prefix)
3. Daemon writes req_<id>.json to requests/ directory
4. PS agent polls requests/ every 50ms
5. PS agent reads and parses request JSON
6. PS agent executes UI Automation command
7. PS agent writes res_<id>.json to responses/ directory
8. PS agent deletes req_<id>.json (via RDPDR delete-on-close)
9. Daemon polls responses/ every 50ms
10. Daemon reads and parses response JSON
11. Daemon deletes res_<id>.json (local filesystem)
12. Daemon returns response to CLI
```

### Request Format

```json
{
  "id": "a1b2c3d4",
  "command": "snapshot",
  "params": {
    "include_refs": true,
    "scope": "desktop",
    "max_depth": 10
  }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique request identifier (8 chars) |
| `command` | string | Command name from AutomateRequest enum |
| `params` | object | Command-specific parameters |

### Response Format

Success:
```json
{
  "id": "a1b2c3d4",
  "timestamp": "2024-01-15T10:30:00Z",
  "success": true,
  "data": { ... },
  "error": null
}
```

Failure:
```json
{
  "id": "a1b2c3d4",
  "timestamp": "2024-01-15T10:30:00Z",
  "success": false,
  "data": null,
  "error": {
    "code": "element_not_found",
    "message": "Element not found: #SaveButton"
  }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Matching request ID |
| `timestamp` | string | ISO 8601 timestamp |
| `success` | boolean | Whether command succeeded |
| `data` | object/null | Command result on success |
| `error` | object/null | Error details on failure |

### Handshake

On startup, the PS agent writes `handshake.json`:

```json
{
  "version": "1.0.0",
  "agent_pid": 12345,
  "started_at": "2024-01-15T10:30:00Z",
  "capabilities": ["snapshot", "invoke", "select", "toggle", "expand", "collapse", "context_menu", "fill", "window", "run", ...],
  "ready": true
}
```

The daemon waits for this file with exponential backoff:
- Initial delay: 500ms
- Backoff factor: 1.5x
- Max delay: 5 seconds
- Max attempts: 5

### UTF-8 BOM Handling

PowerShell on Windows writes UTF-8 files with a BOM (`U+FEFF`). The daemon strips this prefix before JSON parsing.

## Bootstrap Sequence

When `--enable-win-automation` is specified:

1. Generate UUID for this connection
2. Create automation directory: `<session>/automation-<uuid>/`
3. Create subdirectories: `requests/`, `responses/`, `scripts/`
4. Write embedded `agent.ps1` to `scripts/agent.ps1`
5. Register drive mapping with RDPDR channel as `agent-automation`
6. Wait 2-3 seconds for Windows desktop stabilization
7. Send Win+R keystroke to open Run dialog
8. Type PowerShell launch command:
   ```
   powershell -ExecutionPolicy Bypass -WindowStyle Hidden -File "\\TSCLIENT\agent-automation\scripts\agent.ps1"
   ```
9. Press Enter
10. Wait for `handshake.json` with exponential backoff
11. Return success or timeout error

## PowerShell Agent

The agent script is embedded in the Rust binary via `include_str!()` and written to disk at runtime.

### Main Loop

1. Check if `\\TSCLIENT\agent-automation` exists (disconnect detection)
2. List files in `requests/` directory using `Where-Object` filter (wildcards don't work on RDPDR)
3. For each `req_*.json` file:
   - Parse JSON request
   - Dispatch to command handler based on `command` field
   - Build response object
   - Write response to `responses/res_<id>.json`
   - Delete request file
4. Sleep 50ms
5. Repeat

### Ref Mapping

For `@ref` selectors, the agent maintains a hashtable mapping ref numbers to `AutomationElement` objects:

- Refs are always assigned during snapshot (no `--refs` flag needed)
- On each `snapshot` command, the ref map is cleared and rebuilt
- Refs are assigned incrementally during tree traversal (depth-first)
- Ref 1 is always the root element
- Refs are only valid until the next snapshot
- Refs are displayed with "e" prefix in output: `ref=e123`
- Both `@e123` and `@123` work as selectors (e prefix recommended)

### Snapshot Filtering

The snapshot command supports filtering options (similar to agent-browser):

| Flag | Name | Description |
|------|------|-------------|
| `-i` | `--interactive` | Include only interactive elements (buttons, inputs, focusable) |
| `-c` | `--compact` | Remove empty structural elements (Pane, Group, Custom) |
| `-d N` | `--depth N` | Limit tree depth to N levels (default: 10) |
| `-s SEL` | `--selector SEL` | Scope to a specific element via selector |

**Interactive elements** are those that:
- Have `IsKeyboardFocusable = true`
- Support interactive patterns: Invoke, Value, Toggle, SelectionItem, ExpandCollapse, RangeValue, Scroll

**Compact mode** removes elements that:
- Have a structural role (Pane, Group, Custom, Document, ScrollBar, Thumb)
- Have no name, no value, and no children

### Selector Resolution

| Prefix | Type | Resolution |
|--------|------|------------|
| `@eN` or `@N` | Reference | Hashtable lookup by ref number |
| `#id` | AutomationId | PropertyCondition on AutomationIdProperty |
| `.class` | ClassName | PropertyCondition on ClassNameProperty |
| `~pattern` | Pattern | Name property with wildcard matching |
| (none) | Name | PropertyCondition on NameProperty (exact match) |

### Pattern-based Commands

Commands use native Windows UI Automation patterns for reliable interaction:

| Command | UI Automation Pattern | Use Case |
|---------|----------------------|----------|
| `invoke` | InvokePattern.Invoke() | Buttons, hyperlinks, menu items |
| `select` | SelectionItemPattern.Select() | List items, radio buttons |
| `toggle` | TogglePattern.Toggle() | Checkboxes |
| `expand` | ExpandCollapsePattern.Expand() | Menus, tree items, combo boxes |
| `collapse` | ExpandCollapsePattern.Collapse() | Menus, tree items, combo boxes |
| `context_menu` | Focus + Shift+F10 (keyboard) | Opening context menus |
| `fill` | ValuePattern.SetValue() | Text fields |

**Why patterns instead of mouse clicks?**
- **Reliability**: Patterns interact directly with the control, not via coordinates
- **Speed**: No need to calculate positions or simulate mouse movement
- **Consistency**: Works regardless of window position or overlapping elements

### Disconnect Detection

The agent polls for drive availability each loop iteration:
- If `\\TSCLIENT\agent-automation` no longer exists, the agent exits gracefully
- This prevents orphaned PowerShell processes when RDP disconnects

## Timeout and Error Handling

### Response Timeout

Default timeout: 30 seconds

The daemon polls for response files every 50ms. If a response file exists but fails to parse (incomplete write), the daemon retries after 100ms.

### Error Codes

| Code | Description |
|------|-------------|
| `element_not_found` | Selector didn't match any element |
| `stale_ref` | @ref number not in current snapshot |
| `command_failed` | UI Automation operation failed |
| `timeout` | Operation exceeded timeout |
| `unknown` | Unspecified error |

## Cleanup

### On Disconnect (Daemon)

The daemon removes the entire automation directory on disconnect or shutdown.

### On Drive Disappearance (Agent)

When the mapped drive disappears, the PS agent:
1. Logs "Mapped drive gone, exiting..."
2. Exits with code 0

## Key Implementation Files

| File | Purpose |
|------|---------|
| `automation/mod.rs` | Module exports |
| `automation/bootstrap.rs` | Agent launch sequence |
| `automation/file_ipc.rs` | IPC client (request/response handling) |
| `automation/scripts/agent.ps1` | Embedded PowerShell agent |
| `rdpdr_backend.rs` | RDPDR filesystem operations |
| `handlers/automate.rs` | CLI command dispatch |

## Limitations

1. **Latency**: File polling adds ~50-100ms round-trip per command
2. **Single Session**: One automation agent per RDP connection
3. **UAC**: Cannot automate elevated (admin) windows from non-elevated context
4. **RDPDR Bandwidth**: Large snapshots may be slow over high-latency connections
5. **Wildcards**: `Get-ChildItem -Filter` doesn't work on RDPDR drives; must use `Where-Object`
