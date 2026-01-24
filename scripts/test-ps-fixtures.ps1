#!/usr/bin/env pwsh
#Requires -Version 7.0
<#
.SYNOPSIS
    Tests PowerShell agent compatibility with generated fixtures.

.DESCRIPTION
    This script validates that the PowerShell automation agent can parse
    request fixtures generated from Rust types. It ensures schema compatibility
    between Rust and PowerShell.

.NOTES
    Run after `cargo test -p agent-rdp-protocol` to generate fixtures.
#>

param(
    [switch]$Verbose
)

$ErrorActionPreference = "Stop"
$script:TestsPassed = 0
$script:TestsFailed = 0
$script:FixturesDir = Join-Path $PSScriptRoot ".." "fixtures"

function Write-TestResult {
    param(
        [string]$Name,
        [bool]$Passed,
        [string]$Message = ""
    )

    if ($Passed) {
        Write-Host "  [PASS] $Name" -ForegroundColor Green
        $script:TestsPassed++
    } else {
        Write-Host "  [FAIL] $Name" -ForegroundColor Red
        if ($Message) {
            Write-Host "         $Message" -ForegroundColor Yellow
        }
        $script:TestsFailed++
    }
}

function Test-FixturesParseable {
    param(
        [string]$FixtureDir,
        [string]$Category
    )

    Write-Host "`nTesting $Category fixtures..." -ForegroundColor Cyan

    $fixtures = Get-ChildItem -Path $FixtureDir -Filter "*.json" -ErrorAction SilentlyContinue

    if (-not $fixtures) {
        Write-Host "  [SKIP] No fixtures found in $FixtureDir" -ForegroundColor Yellow
        return
    }

    foreach ($fixture in $fixtures) {
        $testName = $fixture.BaseName
        try {
            $content = Get-Content -Path $fixture.FullName -Raw
            $json = $content | ConvertFrom-Json

            # Validate required fields based on category
            switch ($Category) {
                "automation" {
                    $hasId = $null -ne $json.id
                    $hasCommand = $null -ne $json.command
                    $hasParams = $null -ne $json.params

                    if (-not $hasId) {
                        throw "Missing 'id' field"
                    }
                    if (-not $hasCommand) {
                        throw "Missing 'command' field"
                    }
                    if (-not $hasParams) {
                        throw "Missing 'params' field"
                    }

                    # Validate command matches the op in params
                    if ($json.params.op -and $json.params.op -ne $json.command) {
                        # Some commands like 'window' have different op names
                        if ($json.command -ne "window" -or $json.params.op -ne "window") {
                            throw "Command '$($json.command)' does not match params.op '$($json.params.op)'"
                        }
                    }
                }
                "responses" {
                    $hasId = $null -ne $json.id
                    $hasTimestamp = $null -ne $json.timestamp
                    $hasSuccess = $null -ne $json.success

                    if (-not $hasId) {
                        throw "Missing 'id' field"
                    }
                    if (-not $hasTimestamp) {
                        throw "Missing 'timestamp' field"
                    }
                    if ($null -eq $json.success) {
                        throw "Missing 'success' field"
                    }

                    # If success is false, should have error
                    if (-not $json.success -and -not $json.error) {
                        throw "Failed response missing 'error' field"
                    }
                }
            }

            Write-TestResult -Name $testName -Passed $true

            if ($Verbose) {
                Write-Host "         Command: $($json.command)" -ForegroundColor DarkGray
            }
        }
        catch {
            Write-TestResult -Name $testName -Passed $false -Message $_.Exception.Message
        }
    }
}

function Test-CommandCoverage {
    Write-Host "`nChecking command coverage..." -ForegroundColor Cyan

    # Expected automation commands from Rust AutomateRequest enum
    $expectedCommands = @(
        "snapshot",
        "get",
        "focus",
        "click",
        "select",
        "toggle",
        "expand",
        "collapse",
        "context_menu",
        "fill",
        "clear",
        "scroll",
        "window",
        "run",
        "wait_for",
        "status"
    )

    $automationDir = Join-Path $FixturesDir "automation"
    $fixtures = Get-ChildItem -Path $automationDir -Filter "*.json" -ErrorAction SilentlyContinue

    if (-not $fixtures) {
        Write-Host "  [SKIP] No automation fixtures found" -ForegroundColor Yellow
        return
    }

    $coveredCommands = @{}
    foreach ($fixture in $fixtures) {
        $content = Get-Content -Path $fixture.FullName -Raw | ConvertFrom-Json
        $command = $content.command
        if ($command) {
            $coveredCommands[$command] = $true
        }
    }

    foreach ($cmd in $expectedCommands) {
        if ($coveredCommands.ContainsKey($cmd)) {
            Write-TestResult -Name "Command '$cmd' has fixture" -Passed $true
        } else {
            Write-TestResult -Name "Command '$cmd' has fixture" -Passed $false -Message "No fixture found for command"
        }
    }
}

# Main
Write-Host "PowerShell Fixture Validation" -ForegroundColor White
Write-Host "=============================" -ForegroundColor White

# Check fixtures directory exists
if (-not (Test-Path $FixturesDir)) {
    Write-Host "`nFixtures directory not found: $FixturesDir" -ForegroundColor Red
    Write-Host "Run 'cargo test -p agent-rdp-protocol' first to generate fixtures." -ForegroundColor Yellow
    exit 1
}

# Test automation request fixtures
$automationDir = Join-Path $FixturesDir "automation"
if (Test-Path $automationDir) {
    Test-FixturesParseable -FixtureDir $automationDir -Category "automation"
}

# Test response fixtures
$responsesDir = Join-Path $FixturesDir "responses"
if (Test-Path $responsesDir) {
    Test-FixturesParseable -FixtureDir $responsesDir -Category "responses"
}

# Test command coverage
Test-CommandCoverage

# Summary
Write-Host "`n=============================" -ForegroundColor White
Write-Host "Results: $script:TestsPassed passed, $script:TestsFailed failed" -ForegroundColor $(if ($script:TestsFailed -gt 0) { "Red" } else { "Green" })

if ($script:TestsFailed -gt 0) {
    exit 1
}
