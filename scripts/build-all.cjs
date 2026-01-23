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
  { target: 'aarch64-apple-darwin', package: 'darwin-arm64', useCross: false },
  { target: 'x86_64-apple-darwin', package: 'darwin-x64', useCross: false },
  // Linux
  { target: 'x86_64-unknown-linux-gnu', package: 'linux-x64', useCross: true },
  { target: 'aarch64-unknown-linux-gnu', package: 'linux-arm64', useCross: true },
  // Windows
  { target: 'x86_64-pc-windows-gnu', package: 'win32-x64', useCross: true },
];

const PROJECT_ROOT = path.join(__dirname, '..');

function run(cmd) {
  console.log(`> ${cmd}`);
  execSync(cmd, { stdio: 'inherit' });
}

function build(target, useCross) {
  const builder = useCross ? 'cross' : 'cargo';
  run(`${builder} build --release --target ${target}`);
}

function copyBinary(target, packageName) {
  const isWindows = target.includes('windows');
  const ext = isWindows ? '.exe' : '';
  const src = path.join(PROJECT_ROOT, 'target', target, 'release', `agent-rdp${ext}`);
  const destDir = path.join(PROJECT_ROOT, 'packages', packageName, 'bin');
  const dest = path.join(destDir, `agent-rdp${ext}`);

  if (fs.existsSync(src)) {
    if (!fs.existsSync(destDir)) {
      fs.mkdirSync(destDir, { recursive: true });
    }
    fs.copyFileSync(src, dest);
    if (!isWindows) {
      fs.chmodSync(dest, 0o755);
    }
    console.log(`Copied: packages/${packageName}/bin/agent-rdp${ext}`);
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

  for (const { target, package: packageName, useCross } of targets) {
    console.log(`\n=== Building ${target} ===\n`);
    try {
      build(target, useCross);
      copyBinary(target, packageName);
    } catch (err) {
      console.error(`Failed to build ${target}: ${err.message}`);
      process.exit(1);
    }
  }

  console.log('\nDone!');
}

main();
