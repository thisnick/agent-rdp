# actions.ps1 - All automation action functions using native UI Automation patterns

function Invoke-Invoke {
    param($Params)

    $element = Find-Element -Selector $Params.selector
    if (-not $element) { throw "Element not found: $($Params.selector)" }

    # Verify element still exists before interacting
    try {
        $null = $element.Current.ProcessId
    } catch {
        throw "Element no longer exists (window may have closed)"
    }

    # Use InvokePattern
    try {
        $invokePattern = $element.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
        if ($invokePattern) {
            $invokePattern.Invoke()
            return @{ invoked = $true; method = "InvokePattern" }
        }
    } catch {
        throw "Element does not support InvokePattern: $($_.Exception.Message)"
    }

    throw "Element does not support InvokePattern"
}

function Invoke-Expand {
    param($Params)

    $element = Find-Element -Selector $Params.selector
    if (-not $element) { throw "Element not found: $($Params.selector)" }

    try {
        $expandPattern = $element.GetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern)
        if ($expandPattern) {
            $expandPattern.Expand()
            return @{ expanded = $true; method = "ExpandCollapsePattern" }
        }
    } catch {
        throw "Element does not support ExpandCollapsePattern: $($_.Exception.Message)"
    }

    throw "Element does not support ExpandCollapsePattern"
}

function Invoke-Collapse {
    param($Params)

    $element = Find-Element -Selector $Params.selector
    if (-not $element) { throw "Element not found: $($Params.selector)" }

    try {
        $expandPattern = $element.GetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern)
        if ($expandPattern) {
            $expandPattern.Collapse()
            return @{ collapsed = $true; method = "ExpandCollapsePattern" }
        }
    } catch {
        throw "Element does not support ExpandCollapsePattern: $($_.Exception.Message)"
    }

    throw "Element does not support ExpandCollapsePattern"
}

function Invoke-ContextMenu {
    param($Params)

    $element = Find-Element -Selector $Params.selector
    if (-not $element) { throw "Element not found: $($Params.selector)" }

    # Get the element's bounding rectangle and calculate center point
    $rect = $element.Current.BoundingRectangle
    if ($rect.IsEmpty) {
        throw "Element has no bounding rectangle (may be off-screen or invisible)"
    }

    $centerX = [int]($rect.X + $rect.Width / 2)
    $centerY = [int]($rect.Y + $rect.Height / 2)

    # Add Win32 mouse input type if not already defined
    if (-not ([System.Management.Automation.PSTypeName]'ContextMenuMouse').Type) {
        Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

public class ContextMenuMouse {
    [DllImport("user32.dll")]
    public static extern bool SetCursorPos(int x, int y);

    [DllImport("user32.dll")]
    public static extern void mouse_event(uint dwFlags, int dx, int dy, uint dwData, UIntPtr dwExtraInfo);

    public const uint MOUSEEVENTF_RIGHTDOWN = 0x0008;
    public const uint MOUSEEVENTF_RIGHTUP = 0x0010;
}
"@
    }

    # Move cursor to element center
    [ContextMenuMouse]::SetCursorPos($centerX, $centerY)
    Start-Sleep -Milliseconds 50

    # Perform right-click
    [ContextMenuMouse]::mouse_event([ContextMenuMouse]::MOUSEEVENTF_RIGHTDOWN, 0, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 50
    [ContextMenuMouse]::mouse_event([ContextMenuMouse]::MOUSEEVENTF_RIGHTUP, 0, 0, 0, [UIntPtr]::Zero)

    return @{
        context_menu_opened = $true
        method = "mouse_right_click"
        x = $centerX
        y = $centerY
    }
}

function Invoke-Toggle {
    param($Params)

    $element = Find-Element -Selector $Params.selector
    if (-not $element) { throw "Element not found: $($Params.selector)" }

    try {
        $togglePattern = $element.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern)
        if ($togglePattern) {
            $previousState = $togglePattern.Current.ToggleState

            # If a specific state is requested
            if ($null -ne $Params.state) {
                $targetState = if ($Params.state) {
                    [System.Windows.Automation.ToggleState]::On
                } else {
                    [System.Windows.Automation.ToggleState]::Off
                }

                # Toggle until we reach the target state (handles tri-state checkboxes)
                $maxAttempts = 3
                for ($i = 0; $i -lt $maxAttempts -and $togglePattern.Current.ToggleState -ne $targetState; $i++) {
                    $togglePattern.Toggle()
                    Start-Sleep -Milliseconds 50
                }
            } else {
                # Just toggle
                $togglePattern.Toggle()
            }

            return @{
                toggled = $true
                previous_state = $previousState.ToString()
                new_state = $togglePattern.Current.ToggleState.ToString()
                method = "TogglePattern"
            }
        }
    } catch {
        throw "Element does not support TogglePattern: $($_.Exception.Message)"
    }

    throw "Element does not support TogglePattern"
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

    # If no item name specified, select the element directly using SelectionItemPattern
    if (-not $Params.item) {
        try {
            $selectPattern = $element.GetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern)
            if ($selectPattern) {
                $selectPattern.Select()
                return @{ selected = $true; method = "SelectionItemPattern" }
            }
        } catch {
            throw "Element does not support SelectionItemPattern: $($_.Exception.Message)"
        }
        throw "Element does not support SelectionItemPattern"
    }

    # Item name specified - find and select within container
    # Try SelectionItemPattern first
    try {
        $itemCondition = New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::NameProperty, $Params.item)
        $item = $element.FindFirst(
            [System.Windows.Automation.TreeScope]::Descendants, $itemCondition)

        if ($item) {
            $selectPattern = $item.GetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern)
            if ($selectPattern) {
                $selectPattern.Select()
                return @{ selected = $true; item = $Params.item; method = "SelectionItemPattern" }
            }
        }
    } catch {}

    # Try ExpandCollapsePattern for combo boxes - expand first, then find item
    try {
        $expandPattern = $element.GetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern)
        if ($expandPattern) {
            $expandPattern.Expand()
            Start-Sleep -Milliseconds 200

            # Now find and select the item
            $itemCondition = New-Object System.Windows.Automation.PropertyCondition(
                [System.Windows.Automation.AutomationElement]::NameProperty, $Params.item)
            $item = $element.FindFirst(
                [System.Windows.Automation.TreeScope]::Descendants, $itemCondition)

            if ($item) {
                # Try SelectionItemPattern first
                try {
                    $selectPattern = $item.GetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern)
                    if ($selectPattern) {
                        $selectPattern.Select()
                        return @{ selected = $true; item = $Params.item; method = "ExpandCollapse+SelectionItemPattern" }
                    }
                } catch {}

                # Try InvokePattern
                try {
                    $invokePattern = $item.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
                    if ($invokePattern) {
                        $invokePattern.Invoke()
                        return @{ selected = $true; item = $Params.item; method = "ExpandCollapse+InvokePattern" }
                    }
                } catch {}
            }
        }
    } catch {}

    throw "Could not select item: $($Params.item)"
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
        # Use window-specific search for wildcard patterns
        if ($Params.selector -match '^~(.+)$') {
            $window = Find-WindowByPattern -Pattern $Matches[1]
        } else {
            $window = Find-Element -Selector $Params.selector
        }
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
    $commandArgs = if ($Params.args) { $Params.args -join " " } else { "" }
    $wait = if ($null -ne $Params.wait) { $Params.wait } else { $false }
    $hidden = if ($null -ne $Params.hidden) { $Params.hidden } else { $false }
    $timeoutMs = if ($Params.timeout_ms) { [int]$Params.timeout_ms } else { 10000 }

    $startInfo = New-Object System.Diagnostics.ProcessStartInfo
    $startInfo.FileName = "powershell.exe"
    $startInfo.Arguments = "-NoProfile -Command `"$command $commandArgs`""
    $startInfo.UseShellExecute = $false
    $startInfo.RedirectStandardOutput = $wait
    $startInfo.RedirectStandardError = $wait
    $startInfo.CreateNoWindow = $hidden

    $process = [System.Diagnostics.Process]::Start($startInfo)

    if ($wait) {
        # Use async reading to avoid deadlock when buffer fills
        $stdoutTask = $process.StandardOutput.ReadToEndAsync()
        $stderrTask = $process.StandardError.ReadToEndAsync()

        $exited = $process.WaitForExit($timeoutMs)

        if (-not $exited) {
            try { $process.Kill() } catch {}
            throw "Process timed out after $timeoutMs ms and was killed"
        }

        # Wait for async reads to complete (with short timeout since process exited)
        [void]$stdoutTask.Wait(5000)
        [void]$stderrTask.Wait(5000)

        return @{
            exit_code = $process.ExitCode
            stdout = $stdoutTask.Result
            stderr = $stderrTask.Result
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
            "snapshot", "invoke", "select", "toggle", "expand", "collapse",
            "context_menu", "focus", "get", "fill", "clear",
            "scroll", "window", "run", "wait_for", "status"
        )
    }
}
