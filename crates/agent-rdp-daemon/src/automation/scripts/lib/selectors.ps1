# selectors.ps1 - Element finding and selector parsing

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
