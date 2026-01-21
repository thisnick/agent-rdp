# actions.ps1 - All automation action functions (click, fill, scroll, etc.)

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
    $commandArgs = if ($Params.args) { $Params.args -join " " } else { "" }
    $wait = if ($null -ne $Params.wait) { $Params.wait } else { $false }
    $hidden = if ($null -ne $Params.hidden) { $Params.hidden } else { $false }

    $startInfo = New-Object System.Diagnostics.ProcessStartInfo
    $startInfo.FileName = "powershell.exe"
    $startInfo.Arguments = "-NoProfile -Command `"$command $commandArgs`""
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
