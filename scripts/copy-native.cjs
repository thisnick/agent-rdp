#!/usr/bin/env node

/**
 * Copy native binary to the appropriate platform package for local development.
 */

const { copyFileSync, existsSync, mkdirSync, chmodSync } = require('fs');
const { join } = require('path');
const { platform, arch } = require('os');

const projectRoot = join(__dirname, '..');

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
const packageName = `${os}-${architecture}`;

// Source: cargo build output
const sourceBinary = join(projectRoot, 'target', 'release', `agent-rdp${ext}`);

// Destination: platform package bin directory
const destDir = join(projectRoot, 'packages', packageName, 'bin');
const destBinary = join(destDir, `agent-rdp${ext}`);

if (!existsSync(sourceBinary)) {
  console.error(`Error: Binary not found at ${sourceBinary}`);
  console.error('Run "cargo build --release" first.');
  process.exit(1);
}

// Ensure bin directory exists
if (!existsSync(destDir)) {
  mkdirSync(destDir, { recursive: true });
}

copyFileSync(sourceBinary, destBinary);

// Make executable on Unix
if (os !== 'win32') {
  chmodSync(destBinary, 0o755);
}

console.log(`Copied binary to packages/${packageName}/bin/agent-rdp${ext}`);
