#Requires -Version 5.1
# agent-rdp Windows UI Automation Agent
# Communicates via file-based IPC over mapped drive

param(
    [string]$BasePath = "\\TSCLIENT\agent-automation"
)

# ============ SETUP ============

# Set window title for easy identification
$Host.UI.RawUI.WindowTitle = "agent-rdp automation"

# Load UI Automation assemblies
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Windows.Forms

# Global state
$script:RefMap = @{}  # ref number -> AutomationElement mapping
$script:SnapshotId = $null
$script:Version = "1.0.0"
$script:LogPath = "$BasePath\agent.log"

# ============ LOGGING ============

function Write-Log {
    param([string]$Message, [string]$Level = "INFO")
    $timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss.fff"
    $logEntry = "[$timestamp] [$Level] $Message"
    try {
        Add-Content -Path $script:LogPath -Value $logEntry -ErrorAction SilentlyContinue
    } catch {}
}

# ============ LOAD LIBRARY FILES ============

$scriptDir = $PSScriptRoot
. "$scriptDir\lib\types.ps1"
. "$scriptDir\lib\snapshot.ps1"
. "$scriptDir\lib\selectors.ps1"
. "$scriptDir\lib\actions.ps1"

# ============ HANDSHAKE ============

function Write-Handshake {
    $handshake = @{
        ready = $true
        version = $script:Version
        agent_pid = $PID
        started_at = (Get-Date -Format "o")
        capabilities = @(
            "snapshot", "click", "double_click", "right_click",
            "focus", "get", "select", "fill", "clear", "check",
            "scroll", "window", "run", "wait_for", "status"
        )
    }

    $handshakePath = "$BasePath\handshake.json"
    $handshake | ConvertTo-Json -Depth 5 | Set-Content $handshakePath -Encoding UTF8
}

# ============ MAIN LOOP ============

function Start-Agent {
    Write-Log "Agent starting, BasePath=$BasePath"
    Write-Log "Looking for requests in: $BasePath\requests"
    Write-Log "Writing responses to: $BasePath\responses"

    # Check if base path exists
    if (Test-Path $BasePath) {
        Write-Log "Base path exists: YES"
        $items = Get-ChildItem $BasePath -ErrorAction SilentlyContinue
        Write-Log "Base path contents: $($items.Name -join ', ')"
    } else {
        Write-Log "Base path exists: NO - this is a problem!" "ERROR"
    }

    Write-Handshake
    Write-Log "Handshake written"

    $requestDir = "$BasePath\requests"
    $responseDir = "$BasePath\responses"

    # Ensure directories exist
    if (-not (Test-Path $requestDir)) {
        New-Item -ItemType Directory -Path $requestDir -Force | Out-Null
        Write-Log "Created requests directory: $requestDir"
    } else {
        Write-Log "Requests directory already exists: $requestDir"
    }
    if (-not (Test-Path $responseDir)) {
        New-Item -ItemType Directory -Path $responseDir -Force | Out-Null
        Write-Log "Created responses directory: $responseDir"
    } else {
        Write-Log "Responses directory already exists: $responseDir"
    }

    Write-Log "Entering main loop - polling $requestDir for req_*.json files"

    $pollCount = 0
    while ($true) {
        $pollCount++

        # Log every 100 polls to show we're alive
        if ($pollCount % 100 -eq 0) {
            Write-Log "Poll #$pollCount - still running..."
            # Debug: show what's in the requests directory
            $allFiles = Get-ChildItem -Path $requestDir -ErrorAction SilentlyContinue
            Write-Log "  Request dir contents: $($allFiles.Name -join ', ')"
            $filtered = Get-ChildItem -Path $requestDir -Filter "req_*.json" -ErrorAction SilentlyContinue
            Write-Log "  Filtered (req_*.json): $($filtered.Name -join ', ')"
        }

        # Check if mapped drive is still available (cleanup detection)
        if (-not (Test-Path $BasePath)) {
            Write-Log "Mapped drive gone, exiting..." "WARN"
            exit 0
        }

        # Look for request files - use Where-Object since -Filter wildcards don't work on RDPDR drives
        $requests = Get-ChildItem -Path $requestDir -ErrorAction SilentlyContinue | Where-Object { $_.Name -like "req_*.json" }

        foreach ($reqFile in $requests) {
            Write-Log "Processing request: $($reqFile.Name)"

            try {
                $requestContent = Get-Content $reqFile.FullName -Raw
                Write-Log "Request content: $requestContent"
                $request = $requestContent | ConvertFrom-Json

                Write-Log "Parsed request: id=$($request.id), command=$($request.command)"

                $response = @{
                    id = $request.id
                    timestamp = (Get-Date -Format "o")
                    success = $true
                    data = $null
                    error = $null
                }

                try {
                    Write-Log "Executing command: $($request.command)"
                    $response.data = switch ($request.command) {
                        "snapshot"     { Invoke-Snapshot -Params $request.params }
                        "click"        { Invoke-Click -Params $request.params }
                        "double_click" { Invoke-DoubleClick -Params $request.params }
                        "right_click"  { Invoke-RightClick -Params $request.params }
                        "focus"        { Invoke-Focus -Params $request.params }
                        "get"          { Invoke-Get -Params $request.params }
                        "fill"         { Invoke-Fill -Params $request.params }
                        "clear"        { Invoke-Clear -Params $request.params }
                        "select"       { Invoke-Select -Params $request.params }
                        "check"        { Invoke-Check -Params $request.params }
                        "scroll"       { Invoke-Scroll -Params $request.params }
                        "window"       { Invoke-Window -Params $request.params }
                        "run"          { Invoke-Run -Params $request.params }
                        "wait_for"     { Invoke-WaitFor -Params $request.params }
                        "status"       { Get-AgentStatus }
                        default        { throw "Unknown command: $($request.command)" }
                    }
                    Write-Log "Command succeeded: $($request.command)"
                } catch {
                    Write-Log "Command failed: $($_.Exception.Message)" "ERROR"
                    $response.success = $false
                    $response.error = @{
                        code = "command_failed"
                        message = $_.Exception.Message
                    }
                }

                # Write response atomically: write to temp file, then rename
                # This prevents Rust from reading a partially-written file
                $resPath = "$responseDir\res_$($request.id).json"
                $tmpPath = "$responseDir\res_$($request.id).tmp"

                Write-Log "Writing response to: $resPath (via temp file)"
                $responseJson = $response | ConvertTo-Json -Depth 20
                $responseJson | Set-Content $tmpPath -Encoding UTF8

                # Atomic rename - Rust will see either no file or complete file
                Move-Item -Path $tmpPath -Destination $resPath -Force
                Write-Log "Response written successfully"

                # Consumer deletes: PowerShell consumes request, so delete it here
                Remove-Item $reqFile.FullName -Force -ErrorAction SilentlyContinue
                Write-Log "Request file cleaned up"

            } catch {
                Write-Log "Error processing request: $($_.Exception.Message)" "ERROR"
                Write-Log $_.ScriptStackTrace "ERROR"

                # Try to write an error response if we have a request ID
                try {
                    if ($request -and $request.id) {
                        $errorResponse = @{
                            id = $request.id
                            timestamp = (Get-Date -Format "o")
                            success = $false
                            data = $null
                            error = @{
                                code = "request_error"
                                message = $_.Exception.Message
                            }
                        }
                        # Atomic write for error response too
                        $resPath = "$responseDir\res_$($request.id).json"
                        $tmpPath = "$responseDir\res_$($request.id).tmp"
                        $errorResponse | ConvertTo-Json -Depth 10 | Set-Content $tmpPath -Encoding UTF8
                        Move-Item -Path $tmpPath -Destination $resPath -Force
                        Write-Log "Wrote error response to: $resPath"
                    }
                } catch {
                    Write-Log "Failed to write error response: $($_.Exception.Message)" "ERROR"
                }

                # Consumer deletes: clean up request file even on error
                try {
                    if ($reqFile) {
                        Remove-Item $reqFile.FullName -Force -ErrorAction SilentlyContinue
                    }
                } catch {}
            }
        }

        Start-Sleep -Milliseconds 50
    }
}

# ============ ENTRY POINT ============

# Never exit - keep trying to restart on errors
while ($true) {
    try {
        Write-Log "=== Agent process starting (PID: $PID) ==="
        Start-Agent
        # If Start-Agent returns normally (mapped drive gone), exit
        Write-Log "Agent exiting normally"
        exit 0
    } catch {
        Write-Log "FATAL ERROR (will retry): $($_.Exception.Message)" "ERROR"
        Write-Log $_.ScriptStackTrace "ERROR"
        try {
            $errorLog = "$BasePath\error.log"
            "$(Get-Date -Format 'o') FATAL: $($_.Exception.Message)`n$($_.ScriptStackTrace)" | Out-File $errorLog -Append
        } catch {}

        # Check if drive is still there - if not, exit
        if (-not (Test-Path $BasePath)) {
            Write-Log "Drive disconnected, exiting"
            exit 0
        }

        # Wait before retrying
        Start-Sleep -Seconds 2
    }
}
