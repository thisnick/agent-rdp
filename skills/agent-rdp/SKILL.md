---
name: agent-rdp
description: Control Windows Remote Desktop sessions from Windows 11 or Ubuntu with tested screenshots, OCR, mouse, keyboard, clipboard, drive mapping, session management, web viewing, WebSocket streaming, and the Node API. Use for remote Windows operation, testing, and administration through agent-rdp.
---

<!-- Version: 1 -->
<!-- Date: 2026-08-08T01:01:56Z -->
<!-- Author: luckygreen (with assistance by 5.6 Sol High) -->
<!-- Purpose: Provide an OS-neutral, path-neutral, and harness-neutral agent-rdp skill. -->
<!-- Usage: Loaded from any supported skill directory by an agent harness that implements SKILL.md discovery. -->

# agent-rdp

## Validation Boundary

### Tested Release

Use this workflow with npm package `agent-rdp` 0.6.5. The upstream source used for comparison was commit `e4c45f9c4ec2a01694c148fd51d358d7277ad42a`.

### Tested Controllers

The native Windows controller was Windows 11 Pro 24H2, 64-bit, OS build `26100.8875`, running PowerShell 7.

The Linux controller was Ubuntu 24.04.4 LTS, Noble Numbat, running as the `Ubuntu-24.04` WSL 2 distribution. The WSL release was `2.7.11.0`. The Linux kernel was `6.18.33.2-microsoft-standard-WSL2`.

Both controllers ran `agent-rdp` 0.6.5 installed globally through npm.

### Tested Target

The target was `NUC-B14`, Windows 11 Pro 24H2, 64-bit, OS build `26100.8655`, registry `CurrentBuildNumber` `26100`, registry `UBR` `8655`, and `BuildLabEx` `26100.1.amd64fre.ge_release.240331-1435`.

### Passed-Only Scope

Use only the surfaces documented below. Every operative surface passed on the tested environment.

# Resolve Runtime Paths

## Windows 11 PowerShell 7

Resolve each executable to an absolute path before use.

```powershell
$AgentRdp = (Get-Command agent-rdp -ErrorAction Stop).Source
$Node = (Get-Command node -ErrorAction Stop).Source
$Npm = (Get-Command npm -ErrorAction Stop).Source
$EvidenceRoot = Join-Path ([System.IO.Path]::GetTempPath()) 'agent-rdp-operator'
$BeforePng = Join-Path $EvidenceRoot 'before.png'
$AfterPng = Join-Path $EvidenceRoot 'after.png'
$ScreenPng = Join-Path $EvidenceRoot 'screen.png'
$FinalPng = Join-Path $EvidenceRoot 'final.png'
$SharePath = Join-Path $EvidenceRoot 'share'

New-Item -ItemType Directory -Force -Path $EvidenceRoot | Out-Null

if (-not [System.IO.Path]::IsPathFullyQualified($AgentRdp)) { throw 'agent-rdp path is not absolute' }

function agent_rdp {
    & $AgentRdp @args
    if ($LASTEXITCODE -ne 0) { throw "agent-rdp failed with exit code $LASTEXITCODE" }
}
```

## Ubuntu Bash

Resolve each executable to an absolute path before use.

```bash
AGENT_RDP="$(command -v agent-rdp)"
NODE="$(command -v node)"
NPM="$(command -v npm)"
MKTEMP="$(command -v mktemp)"
EVIDENCE_ROOT="$("${MKTEMP}" -d)"
BEFORE_PNG="${EVIDENCE_ROOT}/before.png"
AFTER_PNG="${EVIDENCE_ROOT}/after.png"
SCREEN_PNG="${EVIDENCE_ROOT}/screen.png"
FINAL_PNG="${EVIDENCE_ROOT}/final.png"
SHARE_PATH="${EVIDENCE_ROOT}/share"

case "${AGENT_RDP}" in
    /*) ;;
    *) printf '%s\n' 'agent-rdp path is not absolute' >&2; exit 1 ;;
esac

test -x "${AGENT_RDP}"

agent_rdp() {
    "${AGENT_RDP}" "$@"
}
```

# Core Operator Sequence

## Connect a Named Session

### Windows 11 PowerShell 7

```powershell
$env:AGENT_RDP_HOST = '192.0.2.10'
$env:AGENT_RDP_PORT = '3389'
$env:AGENT_RDP_USERNAME = 'Work'
$env:AGENT_RDP_PASSWORD = $Password
$env:AGENT_RDP_SESSION = 'operator'
agent_rdp connect --width 1280 --height 800 --json
```

### Ubuntu Bash

```bash
export AGENT_RDP_HOST='192.0.2.10'
export AGENT_RDP_PORT='3389'
export AGENT_RDP_USERNAME='Work'
export AGENT_RDP_PASSWORD="${PASSWORD}"
export AGENT_RDP_SESSION='operator'
agent_rdp connect --width 1280 --height 800 --json
```

The CLI also passed with `--host`, `--username`, and `--password-stdin`. Do not place credentials directly in command arguments.

## Stabilize the Desktop

Wait five seconds after connection before the first input. Immediate input was ignored on the tested target. Input after this wait passed.

```text
agent_rdp --session operator wait 5000 --json
```

## Observe Before Acting

### Windows 11 PowerShell 7

```powershell
agent_rdp --session operator screenshot --output $BeforePng --format png --json
if (-not (Test-Path -LiteralPath $BeforePng -PathType Leaf)) { throw 'Screenshot missing' }
```

### Ubuntu Bash

```bash
agent_rdp --session operator screenshot --output "${BEFORE_PNG}" --format png --json
test -s "${BEFORE_PNG}"
```

## Act Through OCR or Verified Coordinates

```text
agent_rdp --session operator locate 'Save' --json
agent_rdp --session operator mouse click 672 427 --json
```

Use coordinates from a current OCR result, current screenshot, or already validated layout.

## Verify After Acting

```powershell
agent_rdp --session operator screenshot --output $AfterPng --format png --json
agent_rdp --session operator locate --all --json
```

```bash
agent_rdp --session operator screenshot --output "${AFTER_PNG}" --format png --json
agent_rdp --session operator locate --all --json
```

## Disconnect and Confirm Cleanup

```powershell
agent_rdp --session operator disconnect --json
agent_rdp session list --json
Remove-Item Env:AGENT_RDP_PASSWORD -ErrorAction SilentlyContinue
```

```bash
agent_rdp --session operator disconnect --json
agent_rdp session list --json
unset AGENT_RDP_PASSWORD
```

# Session Management

## List Sessions

```text
agent_rdp session list --json
```

## Inspect a Named Session

```text
agent_rdp --session operator session info --json
```

## Disconnect a Named Session

```text
agent_rdp --session operator disconnect --json
```

# Screenshots

## Save a PNG

```powershell
agent_rdp --session operator screenshot --output $ScreenPng --format png --json
```

```bash
agent_rdp --session operator screenshot --output "${SCREEN_PNG}" --format png --json
```

The tested output was a 1280 by 800 RGBA PNG. Verify that the file exists, is nonempty, and is current before using it as evidence.

# Mouse

## Click, Move, and Drag

```text
agent_rdp --session operator mouse click 500 300 --json
agent_rdp --session operator mouse right-click 500 300 --json
agent_rdp --session operator mouse double-click 500 300 --json
agent_rdp --session operator mouse move 100 200 --json
agent_rdp --session operator mouse drag 100 100 500 500 --json
```

# Keyboard

## Type Unicode Text

```text
agent_rdp --session operator keyboard type 'Grüße 你好' --json
```

## Press Keys and Combinations

```text
agent_rdp --session operator keyboard press enter --json
agent_rdp --session operator keyboard press escape --json
agent_rdp --session operator keyboard press 'ctrl+c' --json
agent_rdp --session operator keyboard press 'ctrl+shift+esc' --json
agent_rdp --session operator keyboard press 'win+r' --json
agent_rdp --session operator keyboard press 'alt+f4' --json
```

## Preserve Text as One Argument

Text containing shell metacharacters must reach `agent-rdp` as one argument. When PowerShell launches WSL, PowerShell quoting and Bash quoting both apply. Verify the resulting target text before proceeding.

# Scrolling

## Use a Positional Amount

The scroll amount is positional in 0.6.5.

```text
agent_rdp --session operator scroll up 5 --at 600 400 --json
agent_rdp --session operator scroll down 5 --at 600 400 --json
agent_rdp --session operator scroll left 5 --at 600 400 --json
agent_rdp --session operator scroll right 5 --at 600 400 --json
```

# OCR

## Exact Text, Pattern, and Full Screen

```text
agent_rdp --session operator locate 'ProductName' --json
agent_rdp --session operator locate 'Windows*' --pattern --json
agent_rdp --session operator locate --all --json
```

Use returned bounding boxes and center coordinates. Recheck the screen immediately before clicking.

# Clipboard

## Set, Paste, and Get

```text
agent_rdp --session operator clipboard set 'CLIPBOARD-PASS' --json
agent_rdp --session operator keyboard press 'ctrl+v' --json
agent_rdp --session operator clipboard get --json
```

Clipboard set, paste, and get passed after the remote clipboard initialized. Bound a previously blocking retrieval with the native shell timeout mechanism, disconnect the exact named session, reconnect, wait five seconds, and initialize the remote clipboard before retrying.

# Drive Mapping

## Map During Connection

### Windows 11 PowerShell 7

```powershell
New-Item -ItemType Directory -Force -Path $SharePath | Out-Null
$env:AGENT_RDP_PASSWORD = $Password
agent_rdp --session operator connect --host 192.0.2.10 --username Work --width 1280 --height 800 --drive "${SharePath}:OperatorShare" --json
```

### Ubuntu Bash

```bash
mkdir -p "${SHARE_PATH}"
export AGENT_RDP_PASSWORD="${PASSWORD}"
agent_rdp --session operator connect --host 192.0.2.10 --username Work --width 1280 --height 800 --drive "${SHARE_PATH}:OperatorShare" --json
```

## List Mapped Drives

```text
agent_rdp --session operator drive list --json
```

The tested Windows UNC path was `\\tsclient\OperatorShare`. A remote file created below that path appeared in the mapped controller directory.

# Web Viewer

## Start Streaming

Place `--stream-port` before the `connect` subcommand.

```powershell
$env:AGENT_RDP_PASSWORD = $Password
agent_rdp --session operator --stream-port 19224 connect --host 192.0.2.10 --username Work --width 1280 --height 800 --json
agent_rdp --session operator view --port 19224 --json
```

```bash
export AGENT_RDP_PASSWORD="${PASSWORD}"
agent_rdp --session operator --stream-port 19224 connect --host 192.0.2.10 --username Work --width 1280 --height 800 --json
agent_rdp --session operator view --port 19224 --json
```

The tested viewer returned HTTP 200 at `http://127.0.0.1:19224/` and reported `http://localhost:19224`.

# WebSocket Protocol

## Endpoint and Frames

Connect to `ws://127.0.0.1:19224` after starting the stream port. The stream sent status messages and repeated frame messages containing base64 encoded JPEG data.

```json
{"type":"status"}
```

```json
{"type":"frame","data":"BASE64_JPEG_DATA"}
```

## Mouse Input

```json
{"type":"input_mouse","x":500,"y":300,"button":"left","pressed":true}
```

```json
{"type":"input_mouse","x":500,"y":300,"button":"left","pressed":false}
```

## Keyboard Input

```json
{"type":"input_keyboard","text":"WS-PASS"}
```

```json
{"type":"input_keyboard","key":"Control","pressed":true}
```

```json
{"type":"input_keyboard","key":"v","pressed":true}
```

```json
{"type":"input_keyboard","key":"v","pressed":false}
```

```json
{"type":"input_keyboard","key":"Control","pressed":false}
```

## Clipboard Input

```json
{"type":"clipboard_set","text":"WSCLIP-PASS"}
```

```json
{"type":"clipboard_get"}
```

# Node API

## Resolve the Global Package Entry Point

### Windows 11 PowerShell 7

```powershell
$env:AGENT_RDP_NPM_ROOT = (& $Npm root -g).Trim()
$env:AGENT_RDP_MODULE_URL = 'file:///' + (($env:AGENT_RDP_NPM_ROOT + '\agent-rdp\dist\index.js') -replace '\\','/')
```

### Ubuntu Bash

```bash
export AGENT_RDP_NPM_ROOT="$("${NPM}" root -g)"
export AGENT_RDP_MODULE_URL="file://${AGENT_RDP_NPM_ROOT}/agent-rdp/dist/index.js"
```

Resolve the npm global root at runtime so Node upgrades do not hardcode a versioned installation directory.

## Import and Attach to an Existing Session

```javascript
const { RdpSession } = await import(process.env.AGENT_RDP_MODULE_URL);
const session = new RdpSession({ session: "operator" });
const info = await session.getInfo();
const shot = await session.screenshot({ format: "png" });
const drives = await session.drives.list();
const clipboard = await session.clipboard.get();
const matches = await session.locate("ProductName");
await session.keyboard.press("win+r");
await session.keyboard.type("API-PASS");
await session.mouse.move(500, 300);
await session.scroll.up(3);
await session.scroll.down(3);
const streamUrl = session.getStreamUrl();
await session.close();
```

The screenshot result contained PNG data encoded as base64 with width and height. The tested dimensions were 1280 by 800.

## Create and Disconnect a Session

```javascript
const { RdpSession } = await import(process.env.AGENT_RDP_MODULE_URL);
const session = new RdpSession({ session: "api-operator" });
await session.connect({
  host: "192.0.2.10",
  username: "Work",
  password: process.env.AGENT_RDP_PASSWORD,
  width: 1280,
  height: 800,
});
const info = await session.getInfo();
const shot = await session.screenshot({ format: "png" });
const drives = await session.drives.list();
const streamUrl = session.getStreamUrl();
await session.disconnect();
```

After API disconnect, inspect the CLI session list. If the disconnected named session remains, clean it with the tested CLI disconnect command.

# Failure Recovery

## Inspect Before Cleanup

```text
agent_rdp --session operator session info --json
agent_rdp session list --json
```

## Clean Only the Exact Session

```text
agent_rdp --session operator disconnect --json
agent_rdp session list --json
```

Do not terminate unrelated processes. Identify the daemon associated with the exact named session before any process termination.

# Completion Gate

## Windows 11 PowerShell 7

```powershell
agent_rdp --session operator screenshot --output $FinalPng --format png --json
if ((Get-Item -LiteralPath $FinalPng).Length -le 0) { throw 'Final screenshot is empty' }
agent_rdp --session operator disconnect --json
agent_rdp session list --json
```

## Ubuntu Bash

```bash
agent_rdp --session operator screenshot --output "${FINAL_PNG}" --format png --json
test -s "${FINAL_PNG}"
agent_rdp --session operator disconnect --json
agent_rdp session list --json
```

Completion requires a current final PNG, successful machine readable command results, and an empty or expected session list after disconnect.
