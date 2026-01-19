#!/usr/bin/env node

/**
 * Copy native binary to bin directory for local development.
 */

const { copyFileSync, existsSync, mkdirSync, chmodSync } = require('fs');
const { join } = require('path');
const { platform, arch } = require('os');

const projectRoot = join(__dirname, '..');
const binDir = join(projectRoot, 'bin');

// Platform detection
function getPlatformKey() {
  const p = platform();
  const a = arch();

  let os;
  switch (p) {
    case 'darwin': os = 'darwin'; break;
    case 'linux': os = 'linux'; break;
    case 'win32': os = 'win32'; break;
    default: throw new Error(`Unsupported platform: ${p}`);
  }

  let architecture;
  switch (a) {
    case 'x64': architecture = 'x64'; break;
    case 'arm64': architecture = 'arm64'; break;
    default: throw new Error(`Unsupported architecture: ${a}`);
  }

  return { os, arch: architecture };
}

const { os, arch: architecture } = getPlatformKey();
const ext = os === 'win32' ? '.exe' : '';
const binaryName = `agent-rdp-${os}-${architecture}${ext}`;

// Source: cargo build output
const sourceBinary = join(projectRoot, 'target', 'release', `agent-rdp${ext}`);
const destBinary = join(binDir, binaryName);

if (!existsSync(sourceBinary)) {
  console.error(`Error: Binary not found at ${sourceBinary}`);
  console.error('Run "cargo build --release" first.');
  process.exit(1);
}

// Ensure bin directory exists
if (!existsSync(binDir)) {
  mkdirSync(binDir, { recursive: true });
}

copyFileSync(sourceBinary, destBinary);

// Make executable on Unix
if (os !== 'win32') {
  chmodSync(destBinary, 0o755);
}

console.log(`Copied ${binaryName} to bin/`);
