# snapshot.ps1 - Accessibility tree snapshot and element state functions

# Interactive patterns - elements with these can be interacted with
$script:InteractivePatterns = @("invoke", "value", "toggle", "selectionitem", "expandcollapse", "rangevalue", "scroll")

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
