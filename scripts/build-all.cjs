#!/usr/bin/env node
/**
 * Build native binaries for all supported platforms using cross.
 *
 * Prerequisites:
 *   cargo install cross
 *   Docker must be running
 *
 * Usage:
 *   node scripts/build-all.cjs
 */

const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const TARGETS = [
  // macOS (native cargo, no cross needed)
  { target: 'aarch64-apple-darwin', output: 'agent-rdp-darwin-arm64', useCross: false },
  { target: 'x86_64-apple-darwin', output: 'agent-rdp-darwin-x64', useCross: false },
  // Linux
  { target: 'x86_64-unknown-linux-gnu', output: 'agent-rdp-linux-x64', useCross: true },
  { target: 'aarch64-unknown-linux-gnu', output: 'agent-rdp-linux-arm64', useCross: true },
  // Windows
  { target: 'x86_64-pc-windows-gnu', output: 'agent-rdp-win32-x64.exe', useCross: true },
];

const BIN_DIR = path.join(__dirname, '..', 'bin');

function run(cmd) {
  console.log(`> ${cmd}`);
  execSync(cmd, { stdio: 'inherit' });
}

function build(target, useCross) {
  const builder = useCross ? 'cross' : 'cargo';
  run(`${builder} build --release --target ${target}`);
}

function copyBinary(target, output) {
  const ext = target.includes('windows') ? '.exe' : '';
  const src = path.join(__dirname, '..', 'target', target, 'release', `agent-rdp${ext}`);
  const dest = path.join(BIN_DIR, output);

  if (fs.existsSync(src)) {
    fs.copyFileSync(src, dest);
    fs.chmodSync(dest, 0o755);
    console.log(`Copied: ${dest}`);
  } else {
    console.error(`Warning: ${src} not found`);
  }
}

// Check if cross is available for cross-compilation targets
function hasCross() {
  try {
    execSync('cross --version', { stdio: 'ignore' });
    return true;
  } catch {
    return false;
  }
}

// Filter targets based on current platform and available tools
function getAvailableTargets() {
  const platform = process.platform;
  const crossAvailable = hasCross();

  return TARGETS.filter(({ target, useCross }) => {
    // macOS targets only work on macOS
    if (target.includes('apple') && platform !== 'darwin') {
      console.log(`Skipping ${target} (requires macOS)`);
      return false;
    }

    // Cross targets need cross tool
    if (useCross && !crossAvailable) {
      console.log(`Skipping ${target} (cross not installed)`);
      return false;
    }

    return true;
  });
}

async function main() {
  const targets = getAvailableTargets();

  if (targets.length === 0) {
    console.error('No targets available to build.');
    console.error('Install cross for cross-compilation: cargo install cross');
    process.exit(1);
  }

  console.log(`Building ${targets.length} target(s)...\n`);

  for (const { target, output, useCross } of targets) {
    console.log(`\n=== Building ${target} ===\n`);
    try {
      build(target, useCross);
      copyBinary(target, output);
    } catch (err) {
      console.error(`Failed to build ${target}: ${err.message}`);
      process.exit(1);
    }
  }

  console.log('\nDone!');
}

main();
