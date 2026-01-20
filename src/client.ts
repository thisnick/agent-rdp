/**
 * IPC client for communicating with the agent-rdp daemon.
 */

import * as net from 'node:net';
import * as crypto from 'node:crypto';
import { Request, Response, RdpError } from './types.js';

const DEFAULT_TIMEOUT = 30000;

/**
 * Get the socket path for a session.
 * On Windows, returns a TCP port number instead.
 */
export function getSocketPath(session: string): string | number {
  if (process.platform === 'win32') {
    // Windows: use TCP port derived from session name hash
    const hash = crypto.createHash('sha256').update(session).digest();
    const port = 49152 + (hash.readUInt16LE(0) % 16383); // Range: 49152-65535
    return port;
  } else {
    // Unix: use socket file
    return `/tmp/agent-rdp/${session}/socket`;
  }
}

/**
 * Get the temp directory for a session.
 */
export function getSessionDir(session: string): string {
  if (process.platform === 'win32') {
    const temp = process.env.TEMP || process.env.TMP || 'C:\\Windows\\Temp';
    return `${temp}\\agent-rdp\\${session}`;
  } else {
    return `/tmp/agent-rdp/${session}`;
  }
}

/**
 * IPC client for communicating with the daemon.
 */
export class IpcClient {
  private socket: net.Socket | null = null;
  private buffer = '';
  private pendingResolve: ((response: Response) => void) | null = null;
  private pendingReject: ((error: Error) => void) | null = null;

  constructor(private session: string) {}

  /**
   * Connect to the daemon socket.
   */
  async connect(): Promise<void> {
    const socketPath = getSocketPath(this.session);

    return new Promise((resolve, reject) => {
      const socket =
        typeof socketPath === 'number'
          ? net.createConnection({ port: socketPath, host: '127.0.0.1' })
          : net.createConnection({ path: socketPath });

      socket.on('connect', () => {
        this.socket = socket;
        resolve();
      });

      socket.on('error', (err) => {
        reject(new RdpError('ipc_error', `Failed to connect to daemon: ${err.message}`));
      });

      socket.on('data', (data) => {
        this.buffer += data.toString();
        this.processBuffer();
      });

      socket.on('close', () => {
        this.socket = null;
        if (this.pendingReject) {
          this.pendingReject(new RdpError('ipc_error', 'Connection closed'));
          this.pendingResolve = null;
          this.pendingReject = null;
        }
      });
    });
  }

  /**
   * Process buffered data looking for newline-delimited JSON.
   */
  private processBuffer(): void {
    const newlineIndex = this.buffer.indexOf('\n');
    if (newlineIndex === -1) return;

    const line = this.buffer.slice(0, newlineIndex);
    this.buffer = this.buffer.slice(newlineIndex + 1);

    if (this.pendingResolve) {
      try {
        const response = JSON.parse(line) as Response;
        this.pendingResolve(response);
      } catch (_err) {
        this.pendingReject?.(new RdpError('ipc_error', `Invalid JSON response: ${line}`));
      }
      this.pendingResolve = null;
      this.pendingReject = null;
    }

    // Process any remaining complete messages
    this.processBuffer();
  }

  /**
   * Send a request and wait for the response.
   */
  async send(request: Request, timeout = DEFAULT_TIMEOUT): Promise<Response> {
    if (!this.socket) {
      throw new RdpError('ipc_error', 'Not connected to daemon');
    }

    return new Promise((resolve, reject) => {
      this.pendingResolve = resolve;
      this.pendingReject = reject;

      const timeoutId = setTimeout(() => {
        this.pendingResolve = null;
        this.pendingReject = null;
        reject(new RdpError('timeout', 'Request timed out'));
      }, timeout);

      const originalResolve = resolve;
      this.pendingResolve = (response) => {
        clearTimeout(timeoutId);
        originalResolve(response);
      };

      const originalReject = reject;
      this.pendingReject = (error) => {
        clearTimeout(timeoutId);
        originalReject(error);
      };

      const json = JSON.stringify(request) + '\n';
      this.socket!.write(json);
    });
  }

  /**
   * Close the connection.
   */
  async close(): Promise<void> {
    if (this.socket) {
      this.socket.end();
      this.socket = null;
    }
  }

  /**
   * Check if connected.
   */
  isConnected(): boolean {
    return this.socket !== null;
  }
}
