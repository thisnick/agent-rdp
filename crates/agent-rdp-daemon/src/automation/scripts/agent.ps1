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

# ============ HELPER TYPES ============

# Add mouse input and window enumeration helpers via P/Invoke
Add-Type -TypeDefinition @"
using System;
using System.Collections.Generic;
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

public class WindowEnum {
    public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

    [DllImport("user32.dll")]
    public static extern bool EnumWindows(EnumWindowsProc lpEnumFunc, IntPtr lParam);

    [DllImport("user32.dll")]
    public static extern bool IsWindowVisible(IntPtr hWnd);

    private static List<IntPtr> windowHandles;

    public static IntPtr[] GetAllWindows() {
        windowHandles = new List<IntPtr>();
        EnumWindows(EnumWindowCallback, IntPtr.Zero);
        return windowHandles.ToArray();
    }

    private static bool EnumWindowCallback(IntPtr hWnd, IntPtr lParam) {
        // Include all windows, even invisible ones (some popups may not be "visible")
        windowHandles.Add(hWnd);
        return true;
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
    $finalPath = "$BasePath\handshake.json"

    # Write directly to avoid Move-Item issues over RDPDR
    $json | Set-Content $finalPath -Encoding UTF8
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

# Interactive patterns - elements with these can be interacted with
$script:InteractivePatterns = @("invoke", "value", "toggle", "selectionitem", "expandcollapse", "rangevalue", "scroll")

function Test-IsInteractive {
    param([System.Windows.Automation.AutomationElement]$Element)

    # Check if keyboard focusable
    if ($Element.Current.IsKeyboardFocusable) { return $true }

    # Check for interactive patterns
    $patterns = @(Get-SupportedPatterns $Element)
    foreach ($p in $patterns) {
        if ($script:InteractivePatterns -contains $p) { return $true }
    }

    return $false
}

function Test-IsEmptyStructural {
    param($Node)

    # Empty if no name, no value, no interactive patterns, and no children
    $hasName = -not [string]::IsNullOrEmpty($Node["name"])
    $hasValue = -not [string]::IsNullOrEmpty($Node["value"])
    $hasChildren = $Node["children"] -and $Node["children"].Count -gt 0

    # Structural roles that can be pruned if empty
    $structuralRoles = @("Pane", "Group", "Custom", "Document", "ScrollBar", "Thumb")
    $isStructural = $structuralRoles -contains $Node["role"]

    return $isStructural -and -not $hasName -and -not $hasValue -and -not $hasChildren
}

function Get-AccessibilityTree {
    param(
        [System.Windows.Automation.AutomationElement]$Element,
        [int]$MaxDepth = 10,
        [int]$CurrentDepth = 0,
        [ref]$RefCounter,
        [bool]$InteractiveOnly = $false,
        [bool]$Compact = $false
    )

    if ($CurrentDepth -gt $MaxDepth) { return $null }

    # For interactive filter, check if this element or any descendant is interactive
    $isInteractive = Test-IsInteractive $Element

    $RefCounter.Value++
    $ref = $RefCounter.Value
    $script:RefMap[$ref] = $Element

    $rect = $Element.Current.BoundingRectangle
    $node = [ordered]@{}

    # Always include ref (with "e" prefix handled in output)
    $node["ref"] = $ref

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

    # Recurse into children using RawViewWalker to capture all elements
    $children = @()
    try {
        $walker = [System.Windows.Automation.TreeWalker]::RawViewWalker
        $childElements = @()
        $child = $walker.GetFirstChild($Element)
        while ($child) {
            $childElements += $child
            $child = $walker.GetNextSibling($child)
        }

        foreach ($child in $childElements) {
            $childNode = Get-AccessibilityTree -Element $child -MaxDepth $MaxDepth `
                -CurrentDepth ($CurrentDepth + 1) -RefCounter $RefCounter `
                -InteractiveOnly $InteractiveOnly -Compact $Compact
            if ($childNode) {
                $children += $childNode
            }
        }
    } catch {}

    $node["children"] = @($children)

    # Apply interactive filter: skip non-interactive elements with no interactive children
    if ($InteractiveOnly) {
        $hasInteractiveChildren = $children.Count -gt 0
        if (-not $isInteractive -and -not $hasInteractiveChildren) {
            return $null
        }
    }

    # Apply compact filter: remove empty structural elements
    if ($Compact -and (Test-IsEmptyStructural $node)) {
        return $null
    }

    return $node
}

function Invoke-Snapshot {
    param($Params)

    $script:RefMap = @{}
    $refCounter = [ref]0
    $script:SnapshotId = [guid]::NewGuid().ToString().Substring(0, 8)

    $maxDepth = if ($null -ne $Params.max_depth) { [int]$Params.max_depth } else { 10 }
    $interactiveOnly = if ($null -ne $Params.interactive_only) { $Params.interactive_only } else { $false }
    $compact = if ($null -ne $Params.compact) { $Params.compact } else { $false }
    $focused = if ($null -ne $Params.focused) { $Params.focused } else { $false }

    # Determine root element(s)
    if ($Params.selector) {
        # Use selector to find specific element
        $root = Find-Element -Selector $Params.selector
        if (-not $root) {
            throw "Could not find element for snapshot: $($Params.selector)"
        }
        $tree = Get-AccessibilityTree -Element $root -MaxDepth $maxDepth `
            -RefCounter $refCounter -InteractiveOnly $interactiveOnly -Compact $compact
    } elseif ($focused) {
        # Start from focused element
        $root = [System.Windows.Automation.AutomationElement]::FocusedElement
        if (-not $root) {
            throw "No focused element found"
        }
        $tree = Get-AccessibilityTree -Element $root -MaxDepth $maxDepth `
            -RefCounter $refCounter -InteractiveOnly $interactiveOnly -Compact $compact
    } else {
        # Use RootElement (desktop) and walk with RawViewWalker
        $root = [System.Windows.Automation.AutomationElement]::RootElement
        $tree = Get-AccessibilityTree -Element $root -MaxDepth $maxDepth `
            -RefCounter $refCounter -InteractiveOnly $interactiveOnly -Compact $compact
    }

    return @{
        snapshot_id = $script:SnapshotId
        ref_count = $refCounter.Value
        root = $tree
    }
}

# ============ SELECTORS ============

# Search for element across ALL windows (not just RootElement descendants)
function Find-ElementAcrossAllWindows {
    param(
        [System.Windows.Automation.Condition]$Condition
    )

    # First try RootElement descendants (fast path)
    $root = [System.Windows.Automation.AutomationElement]::RootElement
    $result = $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $Condition)
    if ($result) { return $result }

    # If not found, enumerate all windows and search each
    $windowHandles = [WindowEnum]::GetAllWindows()
    foreach ($hwnd in $windowHandles) {
        try {
            $windowElement = [System.Windows.Automation.AutomationElement]::FromHandle($hwnd)
            if ($windowElement) {
                $result = $windowElement.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $Condition)
                if ($result) { return $result }
            }
        } catch {
            # Skip inaccessible windows
        }
    }

    return $null
}

# Search with wildcard pattern across ALL windows
function Find-ElementByPatternAcrossAllWindows {
    param([string]$Pattern)

    # First try RootElement descendants
    $root = [System.Windows.Automation.AutomationElement]::RootElement
    $allElements = $root.FindAll(
        [System.Windows.Automation.TreeScope]::Descendants,
        [System.Windows.Automation.Condition]::TrueCondition)

    foreach ($elem in $allElements) {
        if ($elem.Current.Name -like $Pattern) {
            return $elem
        }
    }

    # If not found, enumerate all windows
    $windowHandles = [WindowEnum]::GetAllWindows()
    foreach ($hwnd in $windowHandles) {
        try {
            $windowElement = [System.Windows.Automation.AutomationElement]::FromHandle($hwnd)
            if ($windowElement) {
                $allElements = $windowElement.FindAll(
                    [System.Windows.Automation.TreeScope]::Descendants,
                    [System.Windows.Automation.Condition]::TrueCondition)
                foreach ($elem in $allElements) {
                    if ($elem.Current.Name -like $Pattern) {
                        return $elem
                    }
                }
            }
        } catch {
            # Skip inaccessible windows
        }
    }

    return $null
}

function Find-Element {
    param([string]$Selector)

    if ([string]::IsNullOrEmpty($Selector)) {
        throw "Selector cannot be empty"
    }

    # @ref or @eN - reference from snapshot (supports both @123 and @e123 formats)
    if ($Selector -match '^@e?(\d+)$') {
        $ref = [int]$Matches[1]

        # Check if any snapshot has been taken
        if ($null -eq $script:SnapshotId -or $script:RefMap.Count -eq 0) {
            throw "No snapshot taken. Run 'automate snapshot' first before using @ref selectors."
        }

        if (-not $script:RefMap.ContainsKey($ref)) {
            throw "Invalid ref: $Selector not found in snapshot (snapshot has $($script:RefMap.Count) elements, snapshot_id=$($script:SnapshotId))"
        }

        $element = $script:RefMap[$ref]

        # Validate element is still accessible (UI may have changed)
        try {
            # Accessing any property will throw if element is stale
            $null = $element.Current.ProcessId
        } catch {
            throw "Stale ref: $Selector - element no longer exists (UI has changed). Take a new snapshot."
        }

        return $element
    }

    # #automationId - search across all windows
    if ($Selector -match '^#(.+)$') {
        $condition = New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::AutomationIdProperty, $Matches[1])
        return Find-ElementAcrossAllWindows -Condition $condition
    }

    # .className - search across all windows
    if ($Selector -match '^\.(.+)$') {
        $condition = New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::ClassNameProperty, $Matches[1])
        return Find-ElementAcrossAllWindows -Condition $condition
    }

    # ~pattern (wildcard name match) - search across all windows
    if ($Selector -match '^~(.+)$') {
        return Find-ElementByPatternAcrossAllWindows -Pattern $Matches[1]
    }

    # role:type[name] - search across all windows
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

        return Find-ElementAcrossAllWindows -Condition $condition
    }

    # Default: exact name match - search across all windows
    $condition = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::NameProperty, $Selector)
    return Find-ElementAcrossAllWindows -Condition $condition
}

# ============ ACTIONS ============

function Invoke-Click {
    param($Params)

    $element = Find-Element -Selector $Params.selector
    if (-not $element) { throw "Element not found: $($Params.selector)" }

    # Safely get button and double parameters (handle missing properties)
    $button = "left"
    $double = $false
    if ($null -ne $Params.PSObject.Properties['button']) { $button = $Params.button }
    if ($null -ne $Params.PSObject.Properties['double']) { $double = $Params.double }

    # Verify element still exists before interacting
    try {
        $null = $element.Current.ProcessId
    } catch {
        throw "Element no longer exists (window may have closed)"
    }

    # Try InvokePattern first (for buttons)
    if ($button -eq "left" -and -not $double) {
        try {
            $invokePattern = $element.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
            if ($invokePattern) {
                $invokePattern.Invoke()
                return @{ clicked = $true; method = "invoke" }
            }
        } catch {
            # InvokePattern failed, fall through to mouse click
            Write-Log "InvokePattern failed: $($_.Exception.Message), falling back to mouse" "WARN"
        }
    }

    # Fall back to click at center of bounds
    try {
        $rect = $element.Current.BoundingRectangle
        if ($rect.IsEmpty -or $rect.Width -eq 0 -or $rect.Height -eq 0) {
            throw "Element has no valid bounds (may be offscreen or collapsed)"
        }
        $x = [int]($rect.X + $rect.Width / 2)
        $y = [int]($rect.Y + $rect.Height / 2)
    } catch {
        throw "Cannot get element bounds: $($_.Exception.Message)"
    }

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
    # Create new params with double=true (can't add properties to PSObject from JSON)
    $clickParams = @{
        selector = $Params.selector
        button = "left"
        double = $true
    }
    return Invoke-Click -Params $clickParams
}

function Invoke-RightClick {
    param($Params)
    # Create new params with button=right
    $clickParams = @{
        selector = $Params.selector
        button = "right"
        double = $false
    }
    return Invoke-Click -Params $clickParams
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

                # Write response directly (avoid Move-Item over RDPDR)
                $resPath = "$responseDir\res_$($request.id).json"

                Write-Log "Writing response to: $resPath"
                $responseJson = $response | ConvertTo-Json -Depth 20
                $responseJson | Set-Content $resPath -Encoding UTF8
                Write-Log "Response written successfully"

                # Clean up request file (delete-on-close is handled properly by RDPDR backend)
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
                        $resPath = "$responseDir\res_$($request.id).json"
                        $errorResponse | ConvertTo-Json -Depth 10 | Set-Content $resPath -Encoding UTF8
                        Write-Log "Wrote error response to: $resPath"
                    }
                } catch {
                    Write-Log "Failed to write error response: $($_.Exception.Message)" "ERROR"
                }

                # Clean up request file even on error
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
            Write-Log "Mapped drive gone, exiting after error"
            exit 0
        }

        # Wait before retrying
        Write-Log "Waiting 2 seconds before restart..."
        Start-Sleep -Seconds 2
    }
}
