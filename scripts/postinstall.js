#!/usr/bin/env node

/**
 * Postinstall script for agent-rdp
 *
 * Downloads the platform-specific native binary from GitHub releases.
 */

const { existsSync, mkdirSync, chmodSync, createWriteStream, unlinkSync, readFileSync } = require('fs');
const { dirname, join } = require('path');
const { platform, arch } = require('os');
const https = require('https');

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
    default:
      console.log(`Unsupported platform: ${p}`);
      process.exit(0);
  }

  let architecture;
  switch (a) {
    case 'x64': architecture = 'x64'; break;
    case 'arm64': architecture = 'arm64'; break;
    default:
      console.log(`Unsupported architecture: ${a}`);
      process.exit(0);
  }

  return { os, arch: architecture };
}

const { os, arch: architecture } = getPlatformKey();
const ext = os === 'win32' ? '.exe' : '';
const binaryName = `agent-rdp-${os}-${architecture}${ext}`;
const binaryPath = join(binDir, binaryName);

// Package info
const packageJson = JSON.parse(readFileSync(join(projectRoot, 'package.json'), 'utf8'));
const version = packageJson.version;

// GitHub release URL
const GITHUB_REPO = 'anthropics/agent-rdp';
const DOWNLOAD_URL = `https://github.com/${GITHUB_REPO}/releases/download/v${version}/${binaryName}`;

function downloadFile(url, dest) {
  return new Promise((resolve, reject) => {
    const file = createWriteStream(dest);

    const request = (url) => {
      https.get(url, (response) => {
        // Handle redirects
        if (response.statusCode === 301 || response.statusCode === 302) {
          request(response.headers.location);
          return;
        }

        if (response.statusCode !== 200) {
          file.close();
          unlinkSync(dest);
          reject(new Error(`HTTP ${response.statusCode}`));
          return;
        }

        response.pipe(file);
        file.on('finish', () => {
          file.close();
          resolve();
        });
      }).on('error', (err) => {
        file.close();
        if (existsSync(dest)) unlinkSync(dest);
        reject(err);
      });
    };

    request(url);
  });
}

async function main() {
  // Check if binary already exists
  if (existsSync(binaryPath)) {
    console.log(`agent-rdp: Binary already exists for ${os}-${architecture}`);
    return;
  }

  // Ensure bin directory exists
  if (!existsSync(binDir)) {
    mkdirSync(binDir, { recursive: true });
  }

  console.log(`agent-rdp: Downloading binary for ${os}-${architecture}...`);

  try {
    await downloadFile(DOWNLOAD_URL, binaryPath);

    // Make executable on Unix
    if (os !== 'win32') {
      chmodSync(binaryPath, 0o755);
    }

    console.log(`agent-rdp: Downloaded successfully`);
  } catch (err) {
    console.log(`agent-rdp: Could not download binary (${err.message})`);
    console.log('');
    console.log('To build from source:');
    console.log('  1. Install Rust: https://rustup.rs');
    console.log('  2. Run: npm run build:native');
  }
}

main().catch(console.error);
