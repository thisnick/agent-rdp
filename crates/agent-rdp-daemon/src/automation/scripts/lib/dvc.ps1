# Dynamic Virtual Channel (DVC) communication module for agent-rdp
# Uses WTS API + File Handle approach for DVC communication with the Rust daemon
#
# For DVC, Microsoft recommends using WTSVirtualChannelQuery to get a file handle
# and then use ReadFile/WriteFile instead of WTSVirtualChannelRead/Write.
# See: https://learn.microsoft.com/en-us/windows/win32/termserv/dvc-server-component-example

# Add WTS API and Kernel32 P/Invoke definitions
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

public class WtsApi {
    public const int WTS_CURRENT_SESSION = -1;
    public const int WTS_CHANNEL_OPTION_DYNAMIC = 0x00000001;
    public const int WTS_CHANNEL_OPTION_DYNAMIC_PRI_LOW = 0x00000000;
    public const int WTS_CHANNEL_OPTION_DYNAMIC_PRI_MED = 0x00000002;
    public const int WTS_CHANNEL_OPTION_DYNAMIC_PRI_HIGH = 0x00000004;
    public const int WTS_CHANNEL_OPTION_DYNAMIC_PRI_REAL = 0x00000006;

    // WTSVirtualFileHandle query type
    public const int WTSVirtualFileHandle = 1;

    [DllImport("Wtsapi32.dll", SetLastError = true, CharSet = CharSet.Ansi)]
    public static extern IntPtr WTSVirtualChannelOpenEx(
        int SessionId,
        [MarshalAs(UnmanagedType.LPStr)] string pVirtualName,
        int flags);

    [DllImport("Wtsapi32.dll", SetLastError = true)]
    public static extern bool WTSVirtualChannelQuery(
        IntPtr hChannelHandle,
        int WTSVirtualClass,
        out IntPtr ppBuffer,
        out int pBytesReturned);

    [DllImport("Wtsapi32.dll", SetLastError = true)]
    public static extern void WTSFreeMemory(IntPtr pMemory);

    [DllImport("Wtsapi32.dll", SetLastError = true)]
    public static extern bool WTSVirtualChannelClose(IntPtr hChannel);
}

public class Kernel32 {
    public const uint DUPLICATE_SAME_ACCESS = 0x00000002;

    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool DuplicateHandle(
        IntPtr hSourceProcessHandle,
        IntPtr hSourceHandle,
        IntPtr hTargetProcessHandle,
        out IntPtr lpTargetHandle,
        uint dwDesiredAccess,
        bool bInheritHandle,
        uint dwOptions);

    [DllImport("kernel32.dll")]
    public static extern IntPtr GetCurrentProcess();

    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool CloseHandle(IntPtr hObject);

    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool ReadFile(
        IntPtr hFile,
        byte[] lpBuffer,
        int nNumberOfBytesToRead,
        out int lpNumberOfBytesRead,
        IntPtr lpOverlapped);

    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool WriteFile(
        IntPtr hFile,
        byte[] lpBuffer,
        int nNumberOfBytesToWrite,
        out int lpNumberOfBytesWritten,
        IntPtr lpOverlapped);
}
"@ -ErrorAction SilentlyContinue

# Channel name must match Rust side
$script:DvcChannelName = "AgentRdp::Automation"

# DVC channel state
$script:DvcWtsHandle = [IntPtr]::Zero
$script:DvcFileHandle = [IntPtr]::Zero

# Open DVC channel and get file handle for I/O
function Open-DvcChannel {
    param(
        [int]$Priority = 0  # 0=Low, 2=Med, 4=High, 6=Real
    )

    $flags = [WtsApi]::WTS_CHANNEL_OPTION_DYNAMIC -bor $Priority

    # Step 1: Open the dynamic channel
    $wtsHandle = [WtsApi]::WTSVirtualChannelOpenEx(
        [WtsApi]::WTS_CURRENT_SESSION,
        $script:DvcChannelName,
        $flags
    )

    if ($wtsHandle -eq [IntPtr]::Zero) {
        $errorCode = [System.Runtime.InteropServices.Marshal]::GetLastWin32Error()
        throw "Failed to open DVC channel '$($script:DvcChannelName)': Win32 error $errorCode"
    }

    # Step 2: Query for the file handle
    $fileHandlePtr = [IntPtr]::Zero
    $bytesReturned = 0

    $success = [WtsApi]::WTSVirtualChannelQuery(
        $wtsHandle,
        [WtsApi]::WTSVirtualFileHandle,
        [ref]$fileHandlePtr,
        [ref]$bytesReturned
    )

    if (-not $success) {
        $errorCode = [System.Runtime.InteropServices.Marshal]::GetLastWin32Error()
        [WtsApi]::WTSVirtualChannelClose($wtsHandle)
        throw "WTSVirtualChannelQuery failed: Win32 error $errorCode"
    }

    if ($bytesReturned -ne [IntPtr]::Size) {
        [WtsApi]::WTSFreeMemory($fileHandlePtr)
        [WtsApi]::WTSVirtualChannelClose($wtsHandle)
        throw "WTSVirtualChannelQuery returned unexpected size: $bytesReturned"
    }

    # Step 3: Read the file handle from the returned pointer
    $wtsFileHandle = [System.Runtime.InteropServices.Marshal]::ReadIntPtr($fileHandlePtr)
    [WtsApi]::WTSFreeMemory($fileHandlePtr)

    # Step 4: Duplicate the handle so we can use it after closing WTS handle
    $duplicatedHandle = [IntPtr]::Zero
    $currentProcess = [Kernel32]::GetCurrentProcess()

    $success = [Kernel32]::DuplicateHandle(
        $currentProcess,
        $wtsFileHandle,
        $currentProcess,
        [ref]$duplicatedHandle,
        0,
        $false,
        [Kernel32]::DUPLICATE_SAME_ACCESS
    )

    if (-not $success) {
        $errorCode = [System.Runtime.InteropServices.Marshal]::GetLastWin32Error()
        [WtsApi]::WTSVirtualChannelClose($wtsHandle)
        throw "DuplicateHandle failed: Win32 error $errorCode"
    }

    # Store handles for later use
    $script:DvcWtsHandle = $wtsHandle
    $script:DvcFileHandle = $duplicatedHandle

    return $duplicatedHandle
}

# Close DVC channel
function Close-DvcChannel {
    if ($script:DvcFileHandle -ne [IntPtr]::Zero) {
        [Kernel32]::CloseHandle($script:DvcFileHandle) | Out-Null
        $script:DvcFileHandle = [IntPtr]::Zero
    }

    if ($script:DvcWtsHandle -ne [IntPtr]::Zero) {
        [WtsApi]::WTSVirtualChannelClose($script:DvcWtsHandle) | Out-Null
        $script:DvcWtsHandle = [IntPtr]::Zero
    }
}

# Maximum message size (1 MB should be plenty for JSON messages)
$script:MaxMessageSize = 1024 * 1024

# CHANNEL_PDU_HEADER size (8 bytes: 4 bytes length + 4 bytes flags)
# File handle reads on DVC include this header before the actual data
$script:ChannelPduHeaderSize = 8

# Read a JSON message from DVC using file handle
# Note: ReadFile on DVC file handle includes CHANNEL_PDU_HEADER (8 bytes) before data
# Returns $null on no data, throws on error
# Note: ReadFile is blocking - timeout not implemented (would require overlapped I/O)
function Read-DvcMessage {
    param(
        [IntPtr]$Handle
    )

    # Read into a buffer
    $buffer = New-Object byte[] $script:MaxMessageSize
    $bytesRead = 0

    # ReadFile is blocking, so we'll read synchronously
    $success = [Kernel32]::ReadFile($Handle, $buffer, $buffer.Length, [ref]$bytesRead, [IntPtr]::Zero)

    if (-not $success) {
        $errorCode = [System.Runtime.InteropServices.Marshal]::GetLastWin32Error()
        # Error 109 (ERROR_BROKEN_PIPE) means the channel was closed
        if ($errorCode -eq 109) {
            throw "DVC channel closed (ERROR_BROKEN_PIPE)"
        }
        throw "ReadFile failed: Win32 error $errorCode"
    }

    if ($bytesRead -eq 0) {
        return $null
    }

    # Skip CHANNEL_PDU_HEADER (8 bytes: 4-byte length + 4-byte flags)
    if ($bytesRead -le $script:ChannelPduHeaderSize) {
        throw "Message too short: $bytesRead bytes (need more than $($script:ChannelPduHeaderSize))"
    }

    $dataLength = $bytesRead - $script:ChannelPduHeaderSize

    # Parse JSON from after the header
    $json = [System.Text.Encoding]::UTF8.GetString($buffer, $script:ChannelPduHeaderSize, $dataLength)

    # Strip BOM if present
    if ($json.Length -gt 0 -and $json[0] -eq [char]0xFEFF) {
        $json = $json.Substring(1)
    }

    return $json | ConvertFrom-Json
}

# Write a JSON message to DVC using file handle
# DVC handles message framing - each WriteFile sends a complete message
function Write-DvcMessage {
    param(
        [IntPtr]$Handle,
        [hashtable]$Message
    )

    # Serialize to JSON (compressed, no whitespace padding)
    $json = $Message | ConvertTo-Json -Depth 20 -Compress
    $buffer = [System.Text.Encoding]::UTF8.GetBytes($json)

    # Write to channel using WriteFile (DVC handles framing)
    $bytesWritten = 0
    $success = [Kernel32]::WriteFile($Handle, $buffer, $buffer.Length, [ref]$bytesWritten, [IntPtr]::Zero)

    if (-not $success) {
        $errorCode = [System.Runtime.InteropServices.Marshal]::GetLastWin32Error()
        throw "WriteFile failed: Win32 error $errorCode"
    }

    if ($bytesWritten -ne $buffer.Length) {
        throw "Incomplete write: wrote $bytesWritten of $($buffer.Length) bytes"
    }
}

# Send handshake message
function Send-DvcHandshake {
    param(
        [IntPtr]$Handle,
        [string]$Version,
        [string[]]$Capabilities
    )

    $handshake = @{
        type = "handshake"
        version = $Version
        agent_pid = $PID
        capabilities = $Capabilities
    }

    Write-DvcMessage -Handle $Handle -Message $handshake
}

# Send response message
function Send-DvcResponse {
    param(
        [IntPtr]$Handle,
        [string]$Id,
        [bool]$Success,
        $Data = $null,
        $ErrorInfo = $null
    )

    $response = @{
        type = "response"
        id = $Id
        success = $Success
    }

    if ($null -ne $Data) {
        $response.data = $Data
    }

    if ($null -ne $ErrorInfo) {
        $response.error = $ErrorInfo
    }

    Write-DvcMessage -Handle $Handle -Message $response
}

# Send poll message (triggers Rust to send any queued requests)
function Send-DvcPoll {
    param([IntPtr]$Handle)

    $poll = @{
        type = "poll"
    }

    Write-DvcMessage -Handle $Handle -Message $poll
}
