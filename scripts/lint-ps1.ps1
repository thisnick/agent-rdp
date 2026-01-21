#!/usr/bin/env pwsh
# Lint PowerShell scripts using PSScriptAnalyzer

$ErrorActionPreference = "Stop"

# Install PSScriptAnalyzer if not present
if (-not (Get-Module -ListAvailable -Name PSScriptAnalyzer)) {
    Write-Host "Installing PSScriptAnalyzer..."
    Install-Module -Name PSScriptAnalyzer -Force -Scope CurrentUser
}

# Run analysis on automation scripts
$scriptsPath = Join-Path $PSScriptRoot ".." "crates" "agent-rdp-daemon" "src" "automation" "scripts"
$settingsPath = Join-Path $scriptsPath "PSScriptAnalyzerSettings.psd1"

Write-Host "Analyzing PowerShell scripts in: $scriptsPath"
Write-Host "Using settings: $settingsPath"

$results = Invoke-ScriptAnalyzer -Path $scriptsPath -Recurse -Settings $settingsPath

if ($results.Count -gt 0) {
    $results | Format-Table -Property Severity, RuleName, ScriptName, Line, Message -AutoSize
    Write-Error "PSScriptAnalyzer found $($results.Count) issues"
    exit 1
}

Write-Host "No issues found in PowerShell scripts" -ForegroundColor Green
exit 0
