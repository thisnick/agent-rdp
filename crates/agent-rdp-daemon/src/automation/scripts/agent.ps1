#Requires -Version 5.1
# agent-rdp Windows UI Automation Agent
# Communicates via file-based IPC over mapped drive

param(
    [string]$BasePath = "\\TSCLIENT\agent-automation"
)

# ============ SETUP ============

# Load UI Automation assemblies
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Windows.Forms

# Global state
$script:RefMap = @{}  # ref number -> AutomationElement mapping
$script:SnapshotId = $null
$script:Version = "1.0.0"

# ============ HELPER TYPES ============

# Add mouse input helper via P/Invoke
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

public class MouseInput {
    [DllImport("user32.dll")]
    public static extern void mouse_event(int dwFlags, int dx, int dy, int dwData, int dwExtraInfo);

    public const int MOUSEEVENTF_LEFTDOWN = 0x0002;
    public const int MOUSEEVENTF_LEFTUP = 0x0004;
    public const int MOUSEEVENTF_RIGHTDOWN = 0x0008;
    public const int MOUSEEVENTF_RIGHTUP = 0x0010;
    public const int MOUSEEVENTF_MIDDLEDOWN = 0x0020;
    public const int MOUSEEVENTF_MIDDLEUP = 0x0040;

    public static void LeftClick() {
        mouse_event(MOUSEEVENTF_LEFTDOWN, 0, 0, 0, 0);
        mouse_event(MOUSEEVENTF_LEFTUP, 0, 0, 0, 0);
    }

    public static void RightClick() {
        mouse_event(MOUSEEVENTF_RIGHTDOWN, 0, 0, 0, 0);
        mouse_event(MOUSEEVENTF_RIGHTUP, 0, 0, 0, 0);
    }

    public static void MiddleClick() {
        mouse_event(MOUSEEVENTF_MIDDLEDOWN, 0, 0, 0, 0);
        mouse_event(MOUSEEVENTF_MIDDLEUP, 0, 0, 0, 0);
    }

    public static void DoubleClick() {
        LeftClick();
        System.Threading.Thread.Sleep(50);
        LeftClick();
    }
}
"@

# ============ HANDSHAKE ============

function Write-Handshake {
    $handshake = @{
        version = $script:Version
        agent_pid = $PID
        started_at = (Get-Date -Format "o")
        capabilities = @(
            "snapshot", "click", "double_click", "right_click",
            "focus", "get", "select", "fill", "clear", "check",
            "scroll", "window", "run", "wait_for", "status"
        )
        ready = $true
    }

    $json = $handshake | ConvertTo-Json -Depth 10
    $tmpPath = "$BasePath\handshake.json.tmp"
    $finalPath = "$BasePath\handshake.json"

    $json | Set-Content $tmpPath -Encoding UTF8
    Move-Item $tmpPath $finalPath -Force
}

# ============ ELEMENT STATES ============

function Get-ElementStates {
    param([System.Windows.Automation.AutomationElement]$Element)

    $states = @()
    $current = $Element.Current

    if ($current.IsEnabled) { $states += "enabled" }
    if ($current.IsKeyboardFocusable) { $states += "focusable" }
    if ($current.HasKeyboardFocus) { $states += "focused" }
    if ($current.IsOffscreen) { $states += "offscreen" }

    # Check IsReadOnly for elements that support it
    try {
        $valuePattern = $Element.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
        if ($valuePattern -and $valuePattern.Current.IsReadOnly) {
            $states += "readonly"
        } else {
            $states += "editable"
        }
    } catch {}

    # Check for multi-line text
    try {
        $textPattern = $Element.GetCurrentPattern([System.Windows.Automation.TextPattern]::Pattern)
        if ($textPattern) { $states += "multiline" }
    } catch {}

    return $states
}

function Get-SupportedPatterns {
    param([System.Windows.Automation.AutomationElement]$Element)

    $patterns = @()
    $supportedPatterns = $Element.GetSupportedPatterns()

    foreach ($pattern in $supportedPatterns) {
        $name = $pattern.ProgrammaticName -replace "PatternIdentifiers.Pattern$", ""
        $patterns += $name.ToLower()
    }

    return $patterns
}

# ============ SNAPSHOT ============

function Get-AccessibilityTree {
    param(
        [System.Windows.Automation.AutomationElement]$Element,
        [int]$MaxDepth = 10,
        [int]$CurrentDepth = 0,
        [ref]$RefCounter,
        [bool]$IncludeRefs = $true
    )

    if ($CurrentDepth -gt $MaxDepth) { return $null }

    $RefCounter.Value++
    $ref = $RefCounter.Value

    if ($IncludeRefs) {
        $script:RefMap[$ref] = $Element
    }

    $rect = $Element.Current.BoundingRectangle
    $node = [ordered]@{}

    if ($IncludeRefs) {
        $node["ref"] = $ref
    }

    $node["role"] = $Element.Current.ControlType.ProgrammaticName -replace "ControlType\.", ""

    if (-not [string]::IsNullOrEmpty($Element.Current.Name)) {
        $node["name"] = $Element.Current.Name
    }

    if (-not [string]::IsNullOrEmpty($Element.Current.AutomationId)) {
        $node["automation_id"] = $Element.Current.AutomationId
    }

    if (-not [string]::IsNullOrEmpty($Element.Current.ClassName)) {
        $node["class_name"] = $Element.Current.ClassName
    }

    if (-not [double]::IsInfinity($rect.X)) {
        $node["bounds"] = @{
            x = [int]$rect.X
            y = [int]$rect.Y
            width = [int]$rect.Width
            height = [int]$rect.Height
        }
    }

    $states = @(Get-ElementStates $Element)
    if ($states.Count -gt 0) {
        $node["states"] = $states
    }

    # Add value for editable elements
    try {
        $valuePattern = $Element.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
        if ($valuePattern) {
            $node["value"] = $valuePattern.Current.Value
        }
    } catch {}

    $patterns = @(Get-SupportedPatterns $Element)
    if ($patterns.Count -gt 0) {
        $node["patterns"] = $patterns
    }

    # Recurse into children
    $children = @()
    try {
        $childElements = $Element.FindAll(
            [System.Windows.Automation.TreeScope]::Children,
            [System.Windows.Automation.Condition]::TrueCondition
        )

        foreach ($child in $childElements) {
            $childNode = Get-AccessibilityTree -Element $child -MaxDepth $MaxDepth `
                -CurrentDepth ($CurrentDepth + 1) -RefCounter $RefCounter -IncludeRefs $IncludeRefs
            if ($childNode) {
                $children += $childNode
            }
        }
    } catch {}

    if ($children.Count -gt 0) {
        $node["children"] = $children
    } else {
        $node["children"] = @()
    }

    return $node
}

function Invoke-Snapshot {
    param($Params)

    $script:RefMap = @{}
    $refCounter = [ref]0
    $script:SnapshotId = [guid]::NewGuid().ToString().Substring(0, 8)

    $includeRefs = if ($null -ne $Params.include_refs) { $Params.include_refs } else { $true }
    $maxDepth = if ($null -ne $Params.max_depth) { [int]$Params.max_depth } else { 10 }

    $root = if ($Params.scope -eq "window" -and $Params.window) {
        Find-Element -Selector $Params.window
    } else {
        [System.Windows.Automation.AutomationElement]::RootElement
    }

    if (-not $root) {
        throw "Could not find root element for snapshot"
    }

    $tree = Get-AccessibilityTree -Element $root -MaxDepth $maxDepth `
        -RefCounter $refCounter -IncludeRefs $includeRefs

    return @{
        snapshot_id = $script:SnapshotId
        ref_count = $refCounter.Value
        root = $tree
    }
}

# ============ SELECTORS ============

function Find-Element {
    param([string]$Selector)

    if ([string]::IsNullOrEmpty($Selector)) {
        throw "Selector cannot be empty"
    }

    $root = [System.Windows.Automation.AutomationElement]::RootElement

    # @ref - reference from snapshot
    if ($Selector -match '^@(\d+)$') {
        $ref = [int]$Matches[1]
        if ($script:RefMap.ContainsKey($ref)) {
            return $script:RefMap[$ref]
        }
        throw "Stale ref: @$ref not found in current snapshot"
    }

    # #automationId
    if ($Selector -match '^#(.+)$') {
        $condition = New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::AutomationIdProperty, $Matches[1])
        return $root.FindFirst(
            [System.Windows.Automation.TreeScope]::Descendants, $condition)
    }

    # .className
    if ($Selector -match '^\.(.+)$') {
        $condition = New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::ClassNameProperty, $Matches[1])
        return $root.FindFirst(
            [System.Windows.Automation.TreeScope]::Descendants, $condition)
    }

    # ~pattern (wildcard name match)
    if ($Selector -match '^~(.+)$') {
        $pattern = $Matches[1]
        $allElements = $root.FindAll(
            [System.Windows.Automation.TreeScope]::Descendants,
            [System.Windows.Automation.Condition]::TrueCondition)

        foreach ($elem in $allElements) {
            if ($elem.Current.Name -like $pattern) {
                return $elem
            }
        }
        return $null
    }

    # role:type[name] - e.g., role:button[OK]
    if ($Selector -match '^role:(\w+)\[(.+)\]$') {
        $role = $Matches[1]
        $name = $Matches[2]

        $controlType = [System.Windows.Automation.ControlType]::$role
        if (-not $controlType) {
            throw "Unknown control type: $role"
        }

        $typeCondition = New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::ControlTypeProperty, $controlType)
        $nameCondition = New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::NameProperty, $name)
        $condition = New-Object System.Windows.Automation.AndCondition($typeCondition, $nameCondition)

        return $root.FindFirst(
            [System.Windows.Automation.TreeScope]::Descendants, $condition)
    }

    # Default: exact name match
    $condition = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::NameProperty, $Selector)
    return $root.FindFirst(
        [System.Windows.Automation.TreeScope]::Descendants, $condition)
}

# ============ ACTIONS ============

function Invoke-Click {
    param($Params)

    $element = Find-Element -Selector $Params.selector
    if (-not $element) { throw "Element not found: $($Params.selector)" }

    $button = if ($Params.button) { $Params.button } else { "left" }
    $double = if ($Params.double) { $Params.double } else { $false }

    # Try InvokePattern first (for buttons)
    if ($button -eq "left" -and -not $double) {
        try {
            $invokePattern = $element.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
            if ($invokePattern) {
                $invokePattern.Invoke()
                return @{ clicked = $true; method = "invoke" }
            }
        } catch {}
    }

    # Fall back to click at center of bounds
    $rect = $element.Current.BoundingRectangle
    $x = [int]($rect.X + $rect.Width / 2)
    $y = [int]($rect.Y + $rect.Height / 2)

    [System.Windows.Forms.Cursor]::Position = New-Object System.Drawing.Point($x, $y)
    Start-Sleep -Milliseconds 50

    switch ($button) {
        "left" {
            if ($double) {
                [MouseInput]::DoubleClick()
            } else {
                [MouseInput]::LeftClick()
            }
        }
        "right" { [MouseInput]::RightClick() }
        "middle" { [MouseInput]::MiddleClick() }
    }

    return @{ clicked = $true; method = "mouse"; x = $x; y = $y; button = $button; double = $double }
}

function Invoke-DoubleClick {
    param($Params)
    $Params.double = $true
    return Invoke-Click -Params $Params
}

function Invoke-RightClick {
    param($Params)
    $Params.button = "right"
    return Invoke-Click -Params $Params
}

function Invoke-Focus {
    param($Params)

    $element = Find-Element -Selector $Params.selector
    if (-not $element) { throw "Element not found: $($Params.selector)" }

    $element.SetFocus()

    return @{ focused = $true }
}

function Invoke-Get {
    param($Params)

    $element = Find-Element -Selector $Params.selector
    if (-not $element) { throw "Element not found: $($Params.selector)" }

    $property = if ($Params.property) { $Params.property } else { "all" }
    $result = @{}

    if ($property -eq "all" -or $property -eq "name") {
        $result["name"] = $element.Current.Name
    }

    if ($property -eq "all" -or $property -eq "value") {
        try {
            $valuePattern = $element.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
            if ($valuePattern) {
                $result["value"] = $valuePattern.Current.Value
            }
        } catch {
            $result["value"] = $null
        }
    }

    if ($property -eq "all" -or $property -eq "states") {
        $result["states"] = @(Get-ElementStates $element)
    }

    if ($property -eq "all" -or $property -eq "bounds") {
        $rect = $element.Current.BoundingRectangle
        $result["bounds"] = @{
            x = [int]$rect.X
            y = [int]$rect.Y
            width = [int]$rect.Width
            height = [int]$rect.Height
        }
    }

    return $result
}

function Invoke-Fill {
    param($Params)

    $element = Find-Element -Selector $Params.selector
    if (-not $element) { throw "Element not found: $($Params.selector)" }

    $element.SetFocus()
    Start-Sleep -Milliseconds 50

    # Try ValuePattern first
    try {
        $valuePattern = $element.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
        if ($valuePattern) {
            $valuePattern.SetValue($Params.text)
            return @{ filled = $true; text = $Params.text; method = "value_pattern" }
        }
    } catch {}

    # Fallback: select all and type
    [System.Windows.Forms.SendKeys]::SendWait("^a")
    Start-Sleep -Milliseconds 50

    # Escape special characters for SendKeys
    $escaped = $Params.text -replace '([+^%~(){}])', '{$1}'
    [System.Windows.Forms.SendKeys]::SendWait($escaped)

    return @{ filled = $true; text = $Params.text; method = "sendkeys" }
}

function Invoke-Clear {
    param($Params)

    $element = Find-Element -Selector $Params.selector
    if (-not $element) { throw "Element not found: $($Params.selector)" }

    $element.SetFocus()
    Start-Sleep -Milliseconds 50

    # Try ValuePattern first
    try {
        $valuePattern = $element.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
        if ($valuePattern) {
            $valuePattern.SetValue("")
            return @{ cleared = $true; method = "value_pattern" }
        }
    } catch {}

    # Fallback: select all and delete
    [System.Windows.Forms.SendKeys]::SendWait("^a{DEL}")

    return @{ cleared = $true; method = "sendkeys" }
}

function Invoke-Select {
    param($Params)

    $element = Find-Element -Selector $Params.selector
    if (-not $element) { throw "Element not found: $($Params.selector)" }

    # Try SelectionItemPattern
    try {
        # Find the item to select within the parent element
        $itemCondition = New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::NameProperty, $Params.item)
        $item = $element.FindFirst(
            [System.Windows.Automation.TreeScope]::Descendants, $itemCondition)

        if ($item) {
            $selectPattern = $item.GetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern)
            if ($selectPattern) {
                $selectPattern.Select()
                return @{ selected = $true; item = $Params.item; method = "selection_pattern" }
            }
        }
    } catch {}

    # Try ExpandCollapsePattern for combo boxes
    try {
        $expandPattern = $element.GetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern)
        if ($expandPattern) {
            $expandPattern.Expand()
            Start-Sleep -Milliseconds 200

            # Now find and click the item
            $itemCondition = New-Object System.Windows.Automation.PropertyCondition(
                [System.Windows.Automation.AutomationElement]::NameProperty, $Params.item)
            $item = $element.FindFirst(
                [System.Windows.Automation.TreeScope]::Descendants, $itemCondition)

            if ($item) {
                $invokePattern = $item.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
                if ($invokePattern) {
                    $invokePattern.Invoke()
                    return @{ selected = $true; item = $Params.item; method = "expand_invoke" }
                }
            }
        }
    } catch {}

    throw "Could not select item: $($Params.item)"
}

function Invoke-Check {
    param($Params)

    $element = Find-Element -Selector $Params.selector
    if (-not $element) { throw "Element not found: $($Params.selector)" }

    $uncheck = if ($Params.uncheck) { $Params.uncheck } else { $false }

    try {
        $togglePattern = $element.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern)
        if ($togglePattern) {
            $currentState = $togglePattern.Current.ToggleState
            $targetState = if ($uncheck) {
                [System.Windows.Automation.ToggleState]::Off
            } else {
                [System.Windows.Automation.ToggleState]::On
            }

            if ($currentState -ne $targetState) {
                $togglePattern.Toggle()
            }

            return @{
                checked = -not $uncheck
                previous_state = $currentState.ToString()
                new_state = $targetState.ToString()
            }
        }
    } catch {}

    # Fallback: click to toggle
    $element.SetFocus()
    [MouseInput]::LeftClick()

    return @{ checked = -not $uncheck; method = "click" }
}

function Invoke-Scroll {
    param($Params)

    $element = Find-Element -Selector $Params.selector
    if (-not $element) { throw "Element not found: $($Params.selector)" }

    # If to_child is specified, scroll until that child is visible
    if ($Params.to_child) {
        $child = Find-Element -Selector $Params.to_child
        if ($child) {
            try {
                $scrollItemPattern = $child.GetCurrentPattern([System.Windows.Automation.ScrollItemPattern]::Pattern)
                if ($scrollItemPattern) {
                    $scrollItemPattern.ScrollIntoView()
                    return @{ scrolled = $true; to_child = $Params.to_child }
                }
            } catch {}
        }
    }

    # Try ScrollPattern
    try {
        $scrollPattern = $element.GetCurrentPattern([System.Windows.Automation.ScrollPattern]::Pattern)
        if ($scrollPattern) {
            $amount = if ($Params.amount) { [int]$Params.amount } else { 1 }
            $direction = if ($Params.direction) { $Params.direction } else { "down" }

            for ($i = 0; $i -lt $amount; $i++) {
                switch ($direction) {
                    "up" { $scrollPattern.ScrollVertical([System.Windows.Automation.ScrollAmount]::SmallDecrement) }
                    "down" { $scrollPattern.ScrollVertical([System.Windows.Automation.ScrollAmount]::SmallIncrement) }
                    "left" { $scrollPattern.ScrollHorizontal([System.Windows.Automation.ScrollAmount]::SmallDecrement) }
                    "right" { $scrollPattern.ScrollHorizontal([System.Windows.Automation.ScrollAmount]::SmallIncrement) }
                }
            }

            return @{ scrolled = $true; direction = $direction; amount = $amount }
        }
    } catch {}

    throw "Element does not support scrolling"
}

function Invoke-Window {
    param($Params)

    $action = $Params.action

    if ($action -eq "list") {
        $windows = @()
        $root = [System.Windows.Automation.AutomationElement]::RootElement

        $condition = New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
            [System.Windows.Automation.ControlType]::Window)

        $windowElements = $root.FindAll(
            [System.Windows.Automation.TreeScope]::Children, $condition)

        foreach ($win in $windowElements) {
            $rect = $win.Current.BoundingRectangle
            $windows += @{
                title = $win.Current.Name
                process_id = $win.Current.ProcessId
                bounds = @{
                    x = [int]$rect.X
                    y = [int]$rect.Y
                    width = [int]$rect.Width
                    height = [int]$rect.Height
                }
            }
        }

        return @{ windows = $windows }
    }

    # For other actions, find the window
    $window = $null
    if ($Params.selector) {
        $window = Find-Element -Selector $Params.selector
    } else {
        # Get foreground window
        Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public class Win32 {
    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();
}
"@
        $hwnd = [Win32]::GetForegroundWindow()
        $window = [System.Windows.Automation.AutomationElement]::FromHandle($hwnd)
    }

    if (-not $window) {
        throw "Window not found"
    }

    switch ($action) {
        "focus" {
            $window.SetFocus()
            return @{ action = "focus"; success = $true }
        }
        "maximize" {
            $windowPattern = $window.GetCurrentPattern([System.Windows.Automation.WindowPattern]::Pattern)
            if ($windowPattern) {
                $windowPattern.SetWindowVisualState([System.Windows.Automation.WindowVisualState]::Maximized)
                return @{ action = "maximize"; success = $true }
            }
        }
        "minimize" {
            $windowPattern = $window.GetCurrentPattern([System.Windows.Automation.WindowPattern]::Pattern)
            if ($windowPattern) {
                $windowPattern.SetWindowVisualState([System.Windows.Automation.WindowVisualState]::Minimized)
                return @{ action = "minimize"; success = $true }
            }
        }
        "restore" {
            $windowPattern = $window.GetCurrentPattern([System.Windows.Automation.WindowPattern]::Pattern)
            if ($windowPattern) {
                $windowPattern.SetWindowVisualState([System.Windows.Automation.WindowVisualState]::Normal)
                return @{ action = "restore"; success = $true }
            }
        }
        "close" {
            $windowPattern = $window.GetCurrentPattern([System.Windows.Automation.WindowPattern]::Pattern)
            if ($windowPattern) {
                $windowPattern.Close()
                return @{ action = "close"; success = $true }
            }
        }
    }

    throw "Window action failed: $action"
}

function Invoke-Run {
    param($Params)

    $command = $Params.command
    $args = if ($Params.args) { $Params.args -join " " } else { "" }
    $wait = if ($null -ne $Params.wait) { $Params.wait } else { $false }
    $hidden = if ($null -ne $Params.hidden) { $Params.hidden } else { $false }

    $startInfo = New-Object System.Diagnostics.ProcessStartInfo
    $startInfo.FileName = "powershell.exe"
    $startInfo.Arguments = "-NoProfile -Command `"$command $args`""
    $startInfo.UseShellExecute = $false
    $startInfo.RedirectStandardOutput = $wait
    $startInfo.RedirectStandardError = $wait
    $startInfo.CreateNoWindow = $hidden

    $process = [System.Diagnostics.Process]::Start($startInfo)

    if ($wait) {
        $stdout = $process.StandardOutput.ReadToEnd()
        $stderr = $process.StandardError.ReadToEnd()
        $process.WaitForExit()

        return @{
            exit_code = $process.ExitCode
            stdout = $stdout
            stderr = $stderr
        }
    } else {
        return @{
            pid = $process.Id
        }
    }
}

function Invoke-WaitFor {
    param($Params)

    $selector = $Params.selector
    $timeout = if ($Params.timeout_ms) { [int]$Params.timeout_ms } else { 30000 }
    $state = if ($Params.state) { $Params.state } else { "visible" }

    $startTime = Get-Date
    $pollInterval = 100  # ms

    while ($true) {
        $elapsed = ((Get-Date) - $startTime).TotalMilliseconds
        if ($elapsed -gt $timeout) {
            throw "Timeout waiting for element: $selector (state: $state)"
        }

        $element = $null
        try {
            $element = Find-Element -Selector $selector
        } catch {}

        switch ($state) {
            "visible" {
                if ($element -and -not $element.Current.IsOffscreen) {
                    return @{ found = $true; state = $state; elapsed_ms = [int]$elapsed }
                }
            }
            "enabled" {
                if ($element -and $element.Current.IsEnabled) {
                    return @{ found = $true; state = $state; elapsed_ms = [int]$elapsed }
                }
            }
            "gone" {
                if (-not $element) {
                    return @{ found = $true; state = $state; elapsed_ms = [int]$elapsed }
                }
            }
        }

        Start-Sleep -Milliseconds $pollInterval
    }
}

function Get-AgentStatus {
    return @{
        agent_running = $true
        agent_pid = $PID
        version = $script:Version
        capabilities = @(
            "snapshot", "click", "double_click", "right_click",
            "focus", "get", "select", "fill", "clear", "check",
            "scroll", "window", "run", "wait_for", "status"
        )
    }
}

# ============ MAIN LOOP ============

function Start-Agent {
    Write-Handshake

    $requestDir = "$BasePath\requests"
    $responseDir = "$BasePath\responses"

    # Ensure directories exist
    if (-not (Test-Path $requestDir)) {
        New-Item -ItemType Directory -Path $requestDir -Force | Out-Null
    }
    if (-not (Test-Path $responseDir)) {
        New-Item -ItemType Directory -Path $responseDir -Force | Out-Null
    }

    while ($true) {
        # Check if mapped drive is still available (cleanup detection)
        if (-not (Test-Path $BasePath)) {
            Write-Host "Mapped drive gone, exiting..."
            exit 0
        }

        # Look for request files
        $requests = Get-ChildItem "$requestDir\req_*.json" -ErrorAction SilentlyContinue

        foreach ($reqFile in $requests) {
            $lockFile = $reqFile.FullName -replace '\.json$', '.processing'

            # Skip if already being processed
            if (Test-Path $lockFile) { continue }

            # Create lock file
            $null | Set-Content $lockFile

            try {
                $requestContent = Get-Content $reqFile.FullName -Raw
                $request = $requestContent | ConvertFrom-Json

                $response = @{
                    id = $request.id
                    timestamp = (Get-Date -Format "o")
                    success = $true
                    data = $null
                    error = $null
                }

                try {
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
                } catch {
                    $response.success = $false
                    $response.error = @{
                        code = "command_failed"
                        message = $_.Exception.Message
                    }
                }

                # Write response atomically
                $resPath = "$responseDir\res_$($request.id).json"
                $tmpPath = "$resPath.tmp"

                $response | ConvertTo-Json -Depth 20 | Set-Content $tmpPath -Encoding UTF8
                Move-Item $tmpPath $resPath -Force

            } finally {
                # Clean up request and lock files
                Remove-Item $reqFile.FullName -Force -ErrorAction SilentlyContinue
                Remove-Item $lockFile -Force -ErrorAction SilentlyContinue
            }
        }

        Start-Sleep -Milliseconds 50
    }
}

# ============ ENTRY POINT ============

try {
    Start-Agent
} catch {
    $errorLog = "$BasePath\error.log"
    $_ | Out-File $errorLog -Append
    exit 1
}
