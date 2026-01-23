#!/usr/bin/env node

/**
 * CLI entry point for agent-rdp.
 * Resolves and executes the platform-specific binary.
 */

import { spawnSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { existsSync } from 'node:fs';

const require = createRequire(import.meta.url);

const platform = process.platform; // darwin, linux, win32
const arch = process.arch;         // arm64, x64
const ext = platform === 'win32' ? '.exe' : '';
const platformPackage = `@agent-rdp/${platform}-${arch}`;

let binaryPath;

try {
  const packageJsonPath = require.resolve(`${platformPackage}/package.json`);
  binaryPath = join(dirname(packageJsonPath), 'bin', `agent-rdp${ext}`);
} catch {
  console.error(`Error: Platform package ${platformPackage} is not installed.`);
  console.error(`This platform (${platform}-${arch}) may not be supported.`);
  process.exit(1);
}

if (!existsSync(binaryPath)) {
  console.error(`Error: Binary not found at ${binaryPath}`);
  console.error(`The platform package ${platformPackage} may not be installed correctly.`);
  process.exit(1);
}

const result = spawnSync(binaryPath, process.argv.slice(2), {
  stdio: 'inherit',
});

process.exit(result.status ?? 1);
