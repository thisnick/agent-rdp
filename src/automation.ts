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
  /** Filter to interactive elements only (buttons, inputs, focusable). */
  interactive?: boolean;
  /** Compact mode - remove empty structural elements. */
  compact?: boolean;
  /** Maximum tree depth (default: 10). */
  depth?: number;
  /** Scope to a specific element (window, panel, etc.) via selector. */
  selector?: string;
}

export interface GetOptions {
  /** Property to retrieve: 'name', 'value', 'states', 'bounds', or 'all'. */
  property?: 'name' | 'value' | 'states' | 'bounds' | 'all';
}

export interface SelectOptions {
  /** Item name to select within container (optional). */
  item?: string;
}

export interface ToggleOptions {
  /** Target state: true=on, false=off. Omit to just toggle. */
  state?: boolean;
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
  /** Process timeout in milliseconds when waiting (default: 10000). */
  processTimeout?: number;
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
 * const snapshot = await rdp.automation.snapshot();
 * console.log(`Found ${snapshot.ref_count} elements`);
 *
 * // Interactive elements only
 * const interactive = await rdp.automation.snapshot({ interactive: true });
 *
 * // Compact output with depth limit
 * const compact = await rdp.automation.snapshot({ interactive: true, compact: true, depth: 5 });
 *
 * // Invoke a button (use @eN format from snapshot)
 * await rdp.automation.invoke('@e5');
 *
 * // Select a list item
 * await rdp.automation.select('@e10');
 *
 * // Toggle a checkbox
 * await rdp.automation.toggle('@e7', { state: true });
 *
 * // Fill text in an element
 * await rdp.automation.fill('#SearchBox', 'Hello World');
 *
 * // Open context menu
 * await rdp.automation.contextMenu('@e5');
 *
 * // Wait for an element to appear
 * await rdp.automation.waitFor('#SaveDialog', { state: 'visible' });
 * ```
 */
export class AutomationController {
  constructor(private rdp: RdpSession) {}

  /**
   * Take a snapshot of the accessibility tree.
   *
   * Refs are always included (use @eN format to reference elements).
   */
  async snapshot(options: SnapshotOptions = {}): Promise<AutomationSnapshot> {
    const request: AutomateRequest = {
      type: 'automate',
      action: 'snapshot',
      interactive_only: options.interactive ?? false,
      compact: options.compact ?? false,
      max_depth: options.depth ?? 10,
      selector: options.selector,
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
   * Invoke an element (InvokePattern) - for buttons, links, menu items.
   */
  async invoke(selector: string): Promise<void> {
    await this.rdp._send({
      type: 'automate',
      action: 'invoke',
      selector,
    });
  }

  /**
   * Select an element or item within container (SelectionItemPattern).
   * For list items, radio buttons, etc.
   */
  async select(selector: string, options: SelectOptions = {}): Promise<void> {
    await this.rdp._send({
      type: 'automate',
      action: 'select',
      selector,
      item: options.item,
    });
  }

  /**
   * Toggle an element (TogglePattern) - for checkboxes.
   */
  async toggle(selector: string, options: ToggleOptions = {}): Promise<void> {
    await this.rdp._send({
      type: 'automate',
      action: 'toggle',
      selector,
      state: options.state,
    });
  }

  /**
   * Expand an element (ExpandCollapsePattern) - for menus, tree items, combo boxes.
   */
  async expand(selector: string): Promise<void> {
    await this.rdp._send({
      type: 'automate',
      action: 'expand',
      selector,
    });
  }

  /**
   * Collapse an element (ExpandCollapsePattern).
   */
  async collapse(selector: string): Promise<void> {
    await this.rdp._send({
      type: 'automate',
      action: 'collapse',
      selector,
    });
  }

  /**
   * Open context menu for an element (Focus + Shift+F10).
   */
  async contextMenu(selector: string): Promise<void> {
    await this.rdp._send({
      type: 'automate',
      action: 'context_menu',
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
      timeout_ms: options.processTimeout ?? 10000,
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
