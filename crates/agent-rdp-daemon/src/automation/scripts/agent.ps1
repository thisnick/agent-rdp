#Requires -Version 5.1
# agent-rdp Windows UI Automation Agent
# Communicates via Dynamic Virtual Channel (DVC) with the Rust daemon

# BasePath kept for reference/logging (RDPDR drive still mapped for future file transfer)
[Diagnostics.CodeAnalysis.SuppressMessageAttribute('PSReviewUnusedParameter', 'BasePath')]
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
$script:Version = "1.1.0"  # Version bump for DVC support
# Local log path on Windows machine (RDPDR not used for logging anymore)
$script:LocalLogPath = "$env:TEMP\agent-rdp-automation.log"
$script:DvcHandle = [IntPtr]::Zero

# ============ LOGGING ============

function Write-Log {
    param([string]$Message, [string]$Level = "INFO")
    $timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss.fff"
    $logEntry = "[$timestamp] [$Level] $Message"

    # Write to local log only (reliable, on Windows machine)
    try {
        Add-Content -Path $script:LocalLogPath -Value $logEntry -ErrorAction SilentlyContinue
    } catch {}
}

# ============ LOAD LIBRARY FILES ============

$scriptDir = $PSScriptRoot
. "$scriptDir\lib\types.ps1"
. "$scriptDir\lib\snapshot.ps1"
. "$scriptDir\lib\selectors.ps1"
. "$scriptDir\lib\actions.ps1"
. "$scriptDir\lib\dvc.ps1"

# ============ MAIN LOOP ============

function Start-Agent {
    Write-Log "Agent starting with DVC transport"
    Write-Log "Local log path: $script:LocalLogPath"
    Write-Log "BasePath (for reference): $BasePath"

    # Open DVC channel
    Write-Log "Opening DVC channel..."
    try {
        $script:DvcHandle = Open-DvcChannel
        Write-Log "DVC channel opened successfully"
    } catch {
        Write-Log "Failed to open DVC channel: $($_.Exception.Message)" "ERROR"
        throw
    }

    # Send handshake
    $capabilities = @(
        "snapshot", "click", "select", "toggle", "expand", "collapse",
        "context_menu", "focus", "get", "fill", "clear",
        "scroll", "window", "run", "wait_for", "status"
    )

    try {
        Send-DvcHandshake -Handle $script:DvcHandle -Version $script:Version -Capabilities $capabilities
        Write-Log "DVC handshake sent: version=$($script:Version)"
    } catch {
        Write-Log "Failed to send handshake: $($_.Exception.Message)" "ERROR"
        throw
    }

    Write-Log "Entering main DVC loop"

    $loopCount = 0

    while ($true) {
        $loopCount++

        # Log every 1000 loops to show we're alive
        if ($loopCount % 1000 -eq 0) {
            Write-Log "Loop #$loopCount - still running via DVC..."
        }

        try {
            # Read request from DVC (with short timeout for polling)
            # Rust sends requests proactively, we just need to read them
            $request = Read-DvcMessage -Handle $script:DvcHandle -TimeoutMs 100

            if ($null -eq $request) {
                # No message available, continue polling
                continue
            }

            # Validate message type
            if ($request.type -ne "request") {
                Write-Log "Ignoring non-request message: type=$($request.type)" "WARN"
                continue
            }

            Write-Log "Processing DVC request: id=$($request.id), command=$($request.command)"

            $responseData = $null
            $responseError = $null
            $success = $true

            try {
                $responseData = switch ($request.command) {
                    "snapshot"     { Invoke-Snapshot -Params $request.params }
                    "click"        { Invoke-Click -Params $request.params }
                    "select"       { Invoke-Select -Params $request.params }
                    "toggle"       { Invoke-Toggle -Params $request.params }
                    "expand"       { Invoke-Expand -Params $request.params }
                    "collapse"     { Invoke-Collapse -Params $request.params }
                    "context_menu" { Invoke-ContextMenu -Params $request.params }
                    "focus"        { Invoke-Focus -Params $request.params }
                    "get"          { Invoke-Get -Params $request.params }
                    "fill"         { Invoke-Fill -Params $request.params }
                    "clear"        { Invoke-Clear -Params $request.params }
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
                $success = $false
                $responseError = @{
                    code = "command_failed"
                    message = $_.Exception.Message
                }
            }

            # Send response via DVC
            try {
                Send-DvcResponse -Handle $script:DvcHandle -Id $request.id -Success $success -Data $responseData -ErrorInfo $responseError
                Write-Log "Response sent for request $($request.id)"
            } catch {
                Write-Log "Failed to send response: $($_.Exception.Message)" "ERROR"
                # If we can't send response, channel may be dead
                throw
            }

        } catch {
            $errorMsg = $_.Exception.Message
            Write-Log "DVC error: $errorMsg" "ERROR"

            # Check if it's a channel error (likely means daemon disconnected)
            if ($errorMsg -match "Win32 error" -or $errorMsg -match "channel") {
                Write-Log "DVC channel error, exiting agent" "WARN"
                break
            }

            # For other errors, try to continue
            Start-Sleep -Milliseconds 100
        }
    }
}

# ============ CLEANUP ============

function Stop-Agent {
    Write-Log "Stopping agent..."

    if ($script:DvcHandle -ne [IntPtr]::Zero) {
        try {
            Close-DvcChannel -Handle $script:DvcHandle
            Write-Log "DVC channel closed"
        } catch {
            Write-Log "Error closing DVC channel: $($_.Exception.Message)" "WARN"
        }
        $script:DvcHandle = [IntPtr]::Zero
    }
}

# ============ ENTRY POINT ============

# Handle clean shutdown
$exitHandler = {
    Stop-Agent
}
Register-EngineEvent -SourceIdentifier PowerShell.Exiting -Action $exitHandler | Out-Null

# Run with retry logic
$maxRetries = 3
$retryCount = 0

while ($retryCount -lt $maxRetries) {
    try {
        Write-Log "=== Agent process starting (PID: $PID, attempt: $($retryCount + 1)) ==="
        Start-Agent
        # If Start-Agent returns normally, exit cleanly
        Write-Log "Agent exiting normally"
        Stop-Agent
        exit 0
    } catch {
        $retryCount++
        Write-Log "FATAL ERROR (attempt $retryCount/$maxRetries): $($_.Exception.Message)" "ERROR"
        Write-Log $_.ScriptStackTrace "ERROR"

        Stop-Agent

        if ($retryCount -lt $maxRetries) {
            Write-Log "Waiting before retry..."
            Start-Sleep -Seconds 2
        }
    }
}

Write-Log "Max retries exceeded, agent exiting"
exit 1
