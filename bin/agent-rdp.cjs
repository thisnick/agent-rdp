#!/usr/bin/env node
// agent-rdp CLI wrapper
// Detects OS/arch and runs the appropriate native binary

const { spawn } = require('child_process');
const { existsSync } = require('fs');
const { join, dirname } = require('path');
const { platform, arch } = require('os');

const scriptDir = __dirname;

// Detect platform
let os;
switch (platform()) {
  case 'darwin': os = 'darwin'; break;
  case 'linux': os = 'linux'; break;
  case 'win32': os = 'win32'; break;
  default:
    console.error(`Error: Unsupported platform: ${platform()}`);
    process.exit(1);
}

// Detect architecture
let architecture;
switch (arch()) {
  case 'x64': architecture = 'x64'; break;
  case 'arm64': architecture = 'arm64'; break;
  default:
    console.error(`Error: Unsupported architecture: ${arch()}`);
    process.exit(1);
}

const ext = os === 'win32' ? '.exe' : '';
const binaryName = `agent-rdp-${os}-${architecture}${ext}`;
const binaryPath = join(scriptDir, binaryName);

if (!existsSync(binaryPath)) {
  console.error(`Error: No binary found for ${os}-${architecture}`);
  console.error(`Expected: ${binaryPath}`);
  console.error('');
  console.error('To build locally:');
  console.error('  1. Install Rust: https://rustup.rs');
  console.error('  2. Run: npm run build:native');
  process.exit(1);
}

// Spawn the binary with inherited stdio
const child = spawn(binaryPath, process.argv.slice(2), {
  stdio: 'inherit',
  windowsHide: false,
});

child.on('error', (err) => {
  console.error(`Error executing binary: ${err.message}`);
  process.exit(1);
});

child.on('close', (code) => {
  process.exit(code ?? 0);
});
