/**
 * agent-rdp Node.js API
 *
 * Programmatic interface for controlling Windows Remote Desktop sessions.
 *
 * @example
 * ```typescript
 * import { RdpSession } from 'agent-rdp';
 *
 * const rdp = new RdpSession({ session: 'default' });
 *
 * await rdp.connect({
 *   host: '192.168.1.100',
 *   username: 'Administrator',
 *   password: 'secret',
 * });
 *
 * const { base64 } = await rdp.screenshot();
 * await rdp.mouse.click({ x: 100, y: 200 });
 * await rdp.keyboard.type({ text: 'Hello World' });
 * await rdp.disconnect();
 * ```
 */

import { IpcClient } from './client.js';
import { DaemonManager } from './daemon.js';
import { AutomationController } from './automation.js';
import {
  ConnectOptions,
  ConnectResult,
  ScreenshotOptions,
  ScreenshotResult,
  SessionInfo,
  MappedDrive,
  MouseClickOptions,
  MouseDragOptions,
  ScrollOptions,
  KeyboardTypeOptions,
  KeyboardPressOptions,
  ClipboardSetOptions,
  OcrMatch,
  Request,
  Response,
  RdpError,
} from './types.js';

// Re-export types
export * from './types.js';
export { AutomationController } from './automation.js';

export interface RdpSessionOptions {
  /** Session name (default: 'default') */
  session?: string;
  /** Request timeout in milliseconds (default: 30000) */
  timeout?: number;
  /** WebSocket streaming port (0 = disabled). Connect to ws://localhost:<port> for frames. */
  streamPort?: number;
}

/**
 * Mouse controller for RDP sessions.
 */
export class MouseController {
  constructor(private rdp: RdpSession) {}

  /** Move cursor to position. */
  async move(options: MouseClickOptions): Promise<void> {
    await this.rdp._send({ type: 'mouse', action: 'move', x: options.x, y: options.y });
  }

  /** Left click at position. */
  async click(options: MouseClickOptions): Promise<void> {
    await this.rdp._send({ type: 'mouse', action: 'click', x: options.x, y: options.y });
  }

  /** Right click at position. */
  async rightClick(options: MouseClickOptions): Promise<void> {
    await this.rdp._send({ type: 'mouse', action: 'right_click', x: options.x, y: options.y });
  }

  /** Double click at position. */
  async doubleClick(options: MouseClickOptions): Promise<void> {
    await this.rdp._send({ type: 'mouse', action: 'double_click', x: options.x, y: options.y });
  }

  /** Drag from one position to another. */
  async drag(options: MouseDragOptions): Promise<void> {
    await this.rdp._send({
      type: 'mouse',
      action: 'drag',
      from_x: options.from.x,
      from_y: options.from.y,
      to_x: options.to.x,
      to_y: options.to.y,
    });
  }
}

/**
 * Keyboard controller for RDP sessions.
 */
export class KeyboardController {
  constructor(private rdp: RdpSession) {}

  /** Type a text string (Unicode). */
  async type(options: KeyboardTypeOptions): Promise<void> {
    await this.rdp._send({ type: 'keyboard', action: 'type', text: options.text });
  }

  /** Press a key combination (e.g., 'ctrl+c', 'alt+tab') or single key (e.g., 'enter'). */
  async press(options: KeyboardPressOptions): Promise<void> {
    await this.rdp._send({ type: 'keyboard', action: 'press', keys: options.keys });
  }
}

/**
 * Scroll controller for RDP sessions.
 */
export class ScrollController {
  constructor(private rdp: RdpSession) {}

  /** Scroll up. */
  async up(options: ScrollOptions = {}): Promise<void> {
    await this.rdp._send({ type: 'scroll', direction: 'up', amount: options.amount ?? 3, x: options.x, y: options.y });
  }

  /** Scroll down. */
  async down(options: ScrollOptions = {}): Promise<void> {
    await this.rdp._send({ type: 'scroll', direction: 'down', amount: options.amount ?? 3, x: options.x, y: options.y });
  }

  /** Scroll left. */
  async left(options: ScrollOptions = {}): Promise<void> {
    await this.rdp._send({ type: 'scroll', direction: 'left', amount: options.amount ?? 3, x: options.x, y: options.y });
  }

  /** Scroll right. */
  async right(options: ScrollOptions = {}): Promise<void> {
    await this.rdp._send({ type: 'scroll', direction: 'right', amount: options.amount ?? 3, x: options.x, y: options.y });
  }
}

/**
 * Clipboard controller for RDP sessions.
 */
export class ClipboardController {
  constructor(private rdp: RdpSession) {}

  /** Get clipboard text. */
  async get(): Promise<string> {
    const response = await this.rdp._send({ type: 'clipboard', action: 'get' });
    const data = response.data as { type: 'clipboard'; text: string };
    return data.text;
  }

  /** Set clipboard text. */
  async set(options: ClipboardSetOptions): Promise<void> {
    await this.rdp._send({ type: 'clipboard', action: 'set', text: options.text });
  }
}

/**
 * Drive controller for RDP sessions.
 */
export class DriveController {
  constructor(private rdp: RdpSession) {}

  /** List mapped drives. */
  async list(): Promise<MappedDrive[]> {
    const response = await this.rdp._send({ type: 'drive', action: 'list' });
    const data = response.data as { type: 'drive_list'; drives: MappedDrive[] };
    return data.drives;
  }
}

/**
 * Main RDP session class.
 */
export class RdpSession {
  /** Mouse controller. */
  readonly mouse: MouseController;
  /** Keyboard controller. */
  readonly keyboard: KeyboardController;
  /** Scroll controller. */
  readonly scroll: ScrollController;
  /** Clipboard controller. */
  readonly clipboard: ClipboardController;
  /** Drive controller. */
  readonly drives: DriveController;
  /** Automation controller for Windows UI Automation. */
  readonly automation: AutomationController;

  private session: string;
  private timeout: number;
  private streamPort: number;
  private daemon: DaemonManager;
  private client: IpcClient | null = null;

  constructor(options: RdpSessionOptions = {}) {
    this.session = options.session ?? 'default';
    this.timeout = options.timeout ?? 30000;
    this.streamPort = options.streamPort ?? 0;
    this.daemon = new DaemonManager(this.session, this.streamPort);

    this.mouse = new MouseController(this);
    this.keyboard = new KeyboardController(this);
    this.scroll = new ScrollController(this);
    this.clipboard = new ClipboardController(this);
    this.drives = new DriveController(this);
    this.automation = new AutomationController(this);
  }

  /**
   * Connect to an RDP server.
   *
   * @param options Connection options
   * @param options.host Server hostname or IP
   * @param options.port Server port (default: 3389)
   * @param options.username Username for authentication
   * @param options.password Password for authentication
   * @param options.domain Optional domain
   * @param options.width Desktop width (default: 1280)
   * @param options.height Desktop height (default: 800)
   * @param options.drives Drives to map
   * @param options.enableWinAutomation Enable Windows UI Automation
   */
  async connect(options: ConnectOptions): Promise<ConnectResult> {
    // Ensure daemon is running and connect
    this.client = await this.daemon.ensureRunning();

    const request: Request = {
      type: 'connect',
      host: options.host,
      port: options.port ?? 3389,
      username: options.username,
      password: options.password,
      domain: options.domain,
      width: options.width ?? 1280,
      height: options.height ?? 800,
      drives: options.drives ?? [],
      enable_win_automation: options.enableWinAutomation,
    };

    const response = await this._send(request);
    const data = response.data as { type: 'connected'; host: string; width: number; height: number };

    return {
      host: data.host,
      width: data.width,
      height: data.height,
    };
  }

  /**
   * Take a screenshot.
   */
  async screenshot(options: ScreenshotOptions = {}): Promise<ScreenshotResult> {
    const response = await this._send({
      type: 'screenshot',
      format: options.format ?? 'png',
    });

    const data = response.data as {
      type: 'screenshot';
      width: number;
      height: number;
      format: string;
      base64: string;
    };

    return {
      base64: data.base64,
      width: data.width,
      height: data.height,
      format: data.format,
    };
  }

  /**
   * Get session information.
   */
  async getInfo(): Promise<SessionInfo> {
    const response = await this._send({ type: 'session_info' });
    const data = response.data as {
      type: 'session_info';
      name: string;
      state: SessionInfo['state'];
      host?: string;
      width?: number;
      height?: number;
      pid: number;
      uptime_secs: number;
    };

    return {
      name: data.name,
      state: data.state,
      host: data.host,
      width: data.width,
      height: data.height,
      pid: data.pid,
      uptime_secs: data.uptime_secs,
    };
  }

  /**
   * Locate text on screen using OCR.
   * Searches within full lines of text and returns matching lines.
   *
   * @param text Text to search for (searches within full lines)
   * @param options Search options
   * @param options.pattern Use glob-style pattern matching (* and ?)
   * @param options.caseSensitive Case-sensitive matching (default: false)
   * @returns Array of matching text lines with coordinates
   *
   * @example
   * ```typescript
   * // Find lines containing text (e.g., "Non HDR - File Explorer")
   * const matches = await rdp.locate('Non HDR');
   * if (matches.length > 0) {
   *   await rdp.mouse.click({ x: matches[0].center_x, y: matches[0].center_y });
   * }
   *
   * // Pattern matching
   * const saveButtons = await rdp.locate('Save*', { pattern: true });
   * ```
   */
  async locate(
    text: string,
    options: { pattern?: boolean; caseSensitive?: boolean } = {},
  ): Promise<OcrMatch[]> {
    const response = await this._send({
      type: 'locate',
      text,
      pattern: options.pattern ?? false,
      ignore_case: !(options.caseSensitive ?? false),
      all: false,
    });

    const data = response.data as { matches: OcrMatch[] };
    return data.matches ?? [];
  }

  /**
   * Get all text lines on screen using OCR.
   *
   * @returns Array of all text lines with coordinates
   *
   * @example
   * ```typescript
   * const allLines = await rdp.locateAll();
   * for (const line of allLines) {
   *   console.log(`"${line.text}" at (${line.center_x}, ${line.center_y})`);
   * }
   * ```
   */
  async locateAll(): Promise<OcrMatch[]> {
    const response = await this._send({
      type: 'locate',
      text: '',
      all: true,
    });

    const data = response.data as { matches: OcrMatch[] };
    return data.matches ?? [];
  }

  /**
   * Disconnect from the RDP server.
   */
  async disconnect(): Promise<void> {
    await this._send({ type: 'disconnect' });
    await this.close();
  }

  /**
   * Close the IPC connection without disconnecting the RDP session.
   */
  async close(): Promise<void> {
    if (this.client) {
      await this.client.close();
      this.client = null;
    }
  }

  /**
   * Get the WebSocket streaming URL, if streaming is enabled.
   * Connect to this URL to receive JPEG frames.
   */
  getStreamUrl(): string | null {
    if (this.streamPort === 0) {
      return null;
    }
    return `ws://localhost:${this.streamPort}`;
  }

  /**
   * Internal: Send a request to the daemon.
   * @internal
   */
  async _send(request: Request): Promise<Response> {
    if (!this.client) {
      // Auto-connect to daemon if not connected
      this.client = await this.daemon.ensureRunning();
    }

    const response = await this.client.send(request, this.timeout);

    if (!response.success) {
      throw new RdpError(
        response.error?.code ?? 'internal_error',
        response.error?.message ?? 'Unknown error',
      );
    }

    return response;
  }
}
