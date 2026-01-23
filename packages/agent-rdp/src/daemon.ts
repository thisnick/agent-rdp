/**
 * Daemon process management for agent-rdp.
 */

import * as fs from 'node:fs';
import * as path from 'node:path';
import { createRequire } from 'node:module';
import { spawn } from 'node:child_process';
import { IpcClient, getSessionDir, getSocketPath } from './client.js';
import { RdpError } from './types.js';

const MAX_STARTUP_WAIT_MS = 10000;
const STARTUP_POLL_INTERVAL_MS = 100;

// Create require function for resolving platform package paths in ESM
const require = createRequire(import.meta.url);

/**
 * Get the platform package name for the current OS/arch.
 */
function getPlatformPackage(): string {
  const platform = process.platform; // 'darwin', 'linux', 'win32'
  const arch = process.arch; // 'arm64', 'x64'
  return `@agent-rdp/${platform}-${arch}`;
}

/**
 * Find the agent-rdp binary.
 */
function findBinary(): string {
  const platformPackage = getPlatformPackage();
  const ext = process.platform === 'win32' ? '.exe' : '';

  try {
    // Resolve the platform package's package.json, then find the binary
    const packageJsonPath = require.resolve(`${platformPackage}/package.json`);
    const packageDir = path.dirname(packageJsonPath);
    const binaryPath = path.join(packageDir, 'bin', `agent-rdp${ext}`);

    if (!fs.existsSync(binaryPath)) {
      throw new RdpError(
        'internal_error',
        `Binary not found at ${binaryPath}. Platform package ${platformPackage} may not be installed correctly.`
      );
    }

    return binaryPath;
  } catch (err) {
    if (err instanceof RdpError) {
      throw err;
    }
    throw new RdpError(
      'not_supported',
      `Platform package ${platformPackage} is not installed. ` +
        `Make sure you have the correct optional dependency installed for your platform.`
    );
  }
}

/**
 * Manages the daemon lifecycle for a session.
 */
export class DaemonManager {
  private sessionDir: string;
  private pidFile: string;

  constructor(
    private session: string,
    private streamPort: number = 0,
  ) {
    this.sessionDir = getSessionDir(session);
    this.pidFile = path.join(this.sessionDir, 'pid');
  }

  /**
   * Check if the daemon is running.
   */
  isRunning(): boolean {
    if (!fs.existsSync(this.pidFile)) {
      return false;
    }

    try {
      const pid = parseInt(fs.readFileSync(this.pidFile, 'utf8').trim(), 10);
      // Check if process exists
      process.kill(pid, 0);
      return true;
    } catch {
      return false;
    }
  }

  /**
   * Ensure the daemon is running, spawning it if necessary.
   * Returns an IpcClient connected to the daemon.
   */
  async ensureRunning(): Promise<IpcClient> {
    if (!this.isRunning()) {
      await this.spawn();
    }

    const client = new IpcClient(this.session);
    await client.connect();
    return client;
  }

  /**
   * Spawn the daemon process.
   */
  private async spawn(): Promise<void> {
    const binary = findBinary();

    // Ensure session directory exists
    fs.mkdirSync(this.sessionDir, { recursive: true });

    // Build daemon arguments
    const args = ['--session', this.session];
    if (this.streamPort > 0) {
      args.push('--stream-port', this.streamPort.toString());
    }
    args.push('session', 'daemon');

    // Spawn daemon in background
    const child = spawn(binary, args, {
      detached: true,
      stdio: 'ignore',
    });

    child.unref();

    // Wait for daemon to be ready (socket file exists or TCP port responds)
    const socketPath = getSocketPath(this.session);
    const startTime = Date.now();

    while (Date.now() - startTime < MAX_STARTUP_WAIT_MS) {
      if (typeof socketPath === 'number') {
        // Windows: try TCP connection
        try {
          const client = new IpcClient(this.session);
          await client.connect();
          await client.close();
          return;
        } catch {
          // Not ready yet
        }
      } else {
        // Unix: check if socket file exists
        if (fs.existsSync(socketPath)) {
          // Give it a moment to be ready
          await sleep(50);
          return;
        }
      }

      await sleep(STARTUP_POLL_INTERVAL_MS);
    }

    throw new RdpError('daemon_not_running', 'Daemon failed to start within timeout');
  }

  /**
   * Get the socket path for this session.
   */
  getSocketPath(): string | number {
    return getSocketPath(this.session);
  }
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
