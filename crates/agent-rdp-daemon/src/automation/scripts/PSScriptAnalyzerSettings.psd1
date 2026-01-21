@{
    # Rules to exclude from analysis
    # These rules are suppressed because they don't apply to this automation agent context
    ExcludeRules = @(
        # Empty catch blocks are intentional when checking for optional UI Automation patterns
        # that may not be supported by all elements (e.g., ValuePattern, TogglePattern)
        'PSAvoidUsingEmptyCatchBlock',

        # Functions like Get-ElementStates and Get-SupportedPatterns return collections,
        # so plural nouns are semantically correct
        'PSUseSingularNouns',

        # This automation agent doesn't need ShouldProcess support (-WhatIf, -Confirm)
        # as it's designed for programmatic use, not interactive shell sessions
        'PSUseShouldProcessForStateChangingFunctions',

        # Write-Log is our custom logging function, not overwriting any built-in cmdlet
        # PSScriptAnalyzer may flag this based on commonly installed modules
        'PSAvoidOverwritingBuiltInCmdlets'
    )
}
