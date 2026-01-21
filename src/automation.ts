/**
 * Automation controller for Windows UI Automation.
 */

import type { RdpSession } from './index.js';
import type {
  AutomateRequest,
  AutomationSnapshot,
  AutomationElementValue,
  AutomationWindowInfo,
  AutomationStatus,
  AutomationRunResult,
} from './types.js';

export interface SnapshotOptions {
  /** Include reference numbers for elements. */
  refs?: boolean;
  /** Scope: 'desktop' or 'window'. */
  scope?: 'desktop' | 'window';
  /** Window selector (required when scope='window'). */
  window?: string;
  /** Maximum tree depth. */
  maxDepth?: number;
}

export interface GetOptions {
  /** Property to retrieve: 'name', 'value', 'states', 'bounds', or 'all'. */
  property?: 'name' | 'value' | 'states' | 'bounds' | 'all';
}

export interface ClickOptions {
  /** Mouse button: 'left', 'right', or 'middle'. */
  button?: 'left' | 'right' | 'middle';
  /** Double-click instead of single click. */
  double?: boolean;
}

export interface ScrollOptions {
  /** Scroll direction. */
  direction?: 'up' | 'down' | 'left' | 'right';
  /** Scroll amount. */
  amount?: number;
  /** Child selector to scroll into view. */
  toChild?: string;
}

export interface RunOptions {
  /** Command arguments. */
  args?: string[];
  /** Wait for command to complete. */
  wait?: boolean;
  /** Run with hidden window. */
  hidden?: boolean;
}

export interface WaitForOptions {
  /** Timeout in milliseconds. */
  timeout?: number;
  /** State to wait for: 'visible', 'enabled', or 'gone'. */
  state?: 'visible' | 'enabled' | 'gone';
}

/**
 * Automation controller for Windows UI Automation.
 *
 * Provides methods for interacting with Windows applications via the
 * Windows UI Automation API through a PowerShell agent.
 *
 * @example
 * ```typescript
 * // Take a snapshot of the accessibility tree
 * const snapshot = await rdp.automation.snapshot({ refs: true });
 * console.log(`Found ${snapshot.ref_count} elements`);
 *
 * // Click an element by ref number
 * await rdp.automation.click('@5');
 *
 * // Fill text in an element
 * await rdp.automation.fill('#SearchBox', 'Hello World');
 *
 * // Wait for an element to appear
 * await rdp.automation.waitFor('#SaveDialog', { state: 'visible' });
 * ```
 */
export class AutomationController {
  constructor(private rdp: RdpSession) {}

  /**
   * Take a snapshot of the accessibility tree.
   */
  async snapshot(options: SnapshotOptions = {}): Promise<AutomationSnapshot> {
    const request: AutomateRequest = {
      type: 'automate',
      action: 'snapshot',
      include_refs: options.refs ?? true,
      scope: options.scope ?? 'desktop',
      window: options.window,
      max_depth: options.maxDepth ?? 10,
    };
    const response = await this.rdp._send(request);
    return response.data as unknown as AutomationSnapshot;
  }

  /**
   * Get element properties.
   */
  async get(selector: string, options: GetOptions = {}): Promise<AutomationElementValue> {
    const request: AutomateRequest = {
      type: 'automate',
      action: 'get',
      selector,
      property: options.property,
    };
    const response = await this.rdp._send(request);
    return response.data as unknown as AutomationElementValue;
  }

  /**
   * Set focus to an element.
   */
  async focus(selector: string): Promise<void> {
    await this.rdp._send({
      type: 'automate',
      action: 'focus',
      selector,
    });
  }

  /**
   * Click an element.
   */
  async click(selector: string, options: ClickOptions = {}): Promise<void> {
    await this.rdp._send({
      type: 'automate',
      action: 'click',
      selector,
      button: options.button ?? 'left',
      double: options.double ?? false,
    });
  }

  /**
   * Double-click an element.
   */
  async doubleClick(selector: string): Promise<void> {
    await this.rdp._send({
      type: 'automate',
      action: 'double_click',
      selector,
    });
  }

  /**
   * Right-click an element.
   */
  async rightClick(selector: string): Promise<void> {
    await this.rdp._send({
      type: 'automate',
      action: 'right_click',
      selector,
    });
  }

  /**
   * Clear and fill text in an element.
   */
  async fill(selector: string, text: string): Promise<void> {
    await this.rdp._send({
      type: 'automate',
      action: 'fill',
      selector,
      text,
    });
  }

  /**
   * Clear text from an element.
   */
  async clear(selector: string): Promise<void> {
    await this.rdp._send({
      type: 'automate',
      action: 'clear',
      selector,
    });
  }

  /**
   * Select an item in a ComboBox or ListBox.
   */
  async select(selector: string, item: string): Promise<void> {
    await this.rdp._send({
      type: 'automate',
      action: 'select',
      selector,
      item,
    });
  }

  /**
   * Check or uncheck a CheckBox or RadioButton.
   */
  async check(selector: string, uncheck = false): Promise<void> {
    await this.rdp._send({
      type: 'automate',
      action: 'check',
      selector,
      uncheck,
    });
  }

  /**
   * Scroll an element.
   */
  async scroll(selector: string, options: ScrollOptions = {}): Promise<void> {
    await this.rdp._send({
      type: 'automate',
      action: 'scroll',
      selector,
      direction: options.direction,
      amount: options.amount,
      to_child: options.toChild,
    });
  }

  /**
   * List all windows.
   */
  async listWindows(): Promise<AutomationWindowInfo[]> {
    const response = await this.rdp._send({
      type: 'automate',
      action: 'window',
      window_action: 'list',
    });
    const data = response.data as unknown as { type: 'window_list'; windows: AutomationWindowInfo[] };
    return data.windows;
  }

  /**
   * Focus a window.
   */
  async focusWindow(selector?: string): Promise<void> {
    await this.rdp._send({
      type: 'automate',
      action: 'window',
      window_action: 'focus',
      selector,
    });
  }

  /**
   * Maximize a window.
   */
  async maximizeWindow(selector?: string): Promise<void> {
    await this.rdp._send({
      type: 'automate',
      action: 'window',
      window_action: 'maximize',
      selector,
    });
  }

  /**
   * Minimize a window.
   */
  async minimizeWindow(selector?: string): Promise<void> {
    await this.rdp._send({
      type: 'automate',
      action: 'window',
      window_action: 'minimize',
      selector,
    });
  }

  /**
   * Restore a window.
   */
  async restoreWindow(selector?: string): Promise<void> {
    await this.rdp._send({
      type: 'automate',
      action: 'window',
      window_action: 'restore',
      selector,
    });
  }

  /**
   * Close a window.
   */
  async closeWindow(selector?: string): Promise<void> {
    await this.rdp._send({
      type: 'automate',
      action: 'window',
      window_action: 'close',
      selector,
    });
  }

  /**
   * Run a PowerShell command.
   */
  async run(command: string, options: RunOptions = {}): Promise<AutomationRunResult> {
    const response = await this.rdp._send({
      type: 'automate',
      action: 'run',
      command,
      args: options.args ?? [],
      wait: options.wait ?? false,
      hidden: options.hidden ?? true,
    });
    return response.data as unknown as AutomationRunResult;
  }

  /**
   * Wait for an element to reach a state.
   */
  async waitFor(selector: string, options: WaitForOptions = {}): Promise<void> {
    await this.rdp._send({
      type: 'automate',
      action: 'wait_for',
      selector,
      timeout_ms: options.timeout ?? 30000,
      state: options.state ?? 'visible',
    });
  }

  /**
   * Get automation agent status.
   */
  async status(): Promise<AutomationStatus> {
    const response = await this.rdp._send({
      type: 'automate',
      action: 'status',
    });
    return response.data as unknown as AutomationStatus;
  }
}
