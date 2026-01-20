@echo off
:: agent-rdp CLI wrapper for Windows
setlocal

set "SCRIPT_DIR=%~dp0"

:: Detect architecture
if "%PROCESSOR_ARCHITECTURE%"=="ARM64" (
    set "ARCH=arm64"
) else (
    set "ARCH=x64"
)

set "BINARY=%SCRIPT_DIR%agent-rdp-win32-%ARCH%.exe"

if exist "%BINARY%" (
    "%BINARY%" %*
    exit /b %ERRORLEVEL%
)

echo Error: No binary found for win32-%ARCH% >&2
echo Expected: %BINARY% >&2
echo. >&2
echo To build locally: >&2
echo   1. Install Rust: https://rustup.rs >&2
echo   2. Run: npm run build:native >&2
exit /b 1
