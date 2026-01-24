/**
 * Automation controller for Windows UI Automation.
 */

import type { RdpSession } from './index.js';
import type {
  AutomationSnapshot,
  AutomationElementValue,
  AutomationWindowInfo,
  AutomationStatus,
  AutomationRunResult,
  AutomationClickResult,
} from './types.js';

export interface SnapshotOptions {
  /** Filter to interactive elements only (buttons, inputs, focusable). */
  interactive?: boolean;
  /** Compact mode - remove empty structural elements. */
  compact?: boolean;
  /** Maximum tree depth (default: 5). */
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
    const request = {
      type: 'automate' as const,
      op: 'snapshot' as const,
      interactive_only: options.interactive ?? false,
      compact: options.compact ?? false,
      max_depth: options.depth ?? 10,
      selector: options.selector,
      focused: false,
    };
    const response = await this.rdp._send(request);
    return response.data as unknown as AutomationSnapshot;
  }

  /**
   * Get element properties.
   */
  async get(selector: string, options: GetOptions = {}): Promise<AutomationElementValue> {
    const request = {
      type: 'automate' as const,
      op: 'get' as const,
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
      type: 'automate' as const,
      op: 'focus' as const,
      selector,
    });
  }

  /**
   * Click an element - for buttons, links, menu items.
   *
   * @param selector - Element selector
   * @param options - Optional settings (e.g., doubleClick for file list items)
   * @returns Result with click coordinates and method used
   */
  async click(selector: string, options: { doubleClick?: boolean } = {}): Promise<AutomationClickResult> {
    const response = await this.rdp._send({
      type: 'automate' as const,
      op: 'click' as const,
      selector,
      double_click: options.doubleClick ?? false,
    });
    return response.data as unknown as AutomationClickResult;
  }

  /**
   * Select an element or item within container (SelectionItemPattern).
   * For list items, radio buttons, etc.
   */
  async select(selector: string, options: SelectOptions = {}): Promise<void> {
    await this.rdp._send({
      type: 'automate' as const,
      op: 'select' as const,
      selector,
      item: options.item,
    });
  }

  /**
   * Toggle an element (TogglePattern) - for checkboxes.
   */
  async toggle(selector: string, options: ToggleOptions = {}): Promise<void> {
    await this.rdp._send({
      type: 'automate' as const,
      op: 'toggle' as const,
      selector,
      state: options.state,
    });
  }

  /**
   * Expand an element (ExpandCollapsePattern) - for menus, tree items, combo boxes.
   */
  async expand(selector: string): Promise<void> {
    await this.rdp._send({
      type: 'automate' as const,
      op: 'expand' as const,
      selector,
    });
  }

  /**
   * Collapse an element (ExpandCollapsePattern).
   */
  async collapse(selector: string): Promise<void> {
    await this.rdp._send({
      type: 'automate' as const,
      op: 'collapse' as const,
      selector,
    });
  }

  /**
   * Open context menu for an element (Focus + Shift+F10).
   */
  async contextMenu(selector: string): Promise<void> {
    await this.rdp._send({
      type: 'automate' as const,
      op: 'context_menu' as const,
      selector,
    });
  }

  /**
   * Clear and fill text in an element.
   */
  async fill(selector: string, text: string): Promise<void> {
    await this.rdp._send({
      type: 'automate' as const,
      op: 'fill' as const,
      selector,
      text,
    });
  }

  /**
   * Clear text from an element.
   */
  async clear(selector: string): Promise<void> {
    await this.rdp._send({
      type: 'automate' as const,
      op: 'clear' as const,
      selector,
    });
  }

  /**
   * Scroll an element.
   */
  async scroll(selector: string, options: ScrollOptions = {}): Promise<void> {
    await this.rdp._send({
      type: 'automate' as const,
      op: 'scroll' as const,
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
      type: 'automate' as const,
      op: 'window' as const,
      action: 'list' as const,
    });
    const data = response.data as unknown as { type: 'window_list'; windows: AutomationWindowInfo[] };
    return data.windows;
  }

  /**
   * Focus a window.
   */
  async focusWindow(selector?: string): Promise<void> {
    await this.rdp._send({
      type: 'automate' as const,
      op: 'window' as const,
      action: 'focus' as const,
      selector,
    });
  }

  /**
   * Maximize a window.
   */
  async maximizeWindow(selector?: string): Promise<void> {
    await this.rdp._send({
      type: 'automate' as const,
      op: 'window' as const,
      action: 'maximize' as const,
      selector,
    });
  }

  /**
   * Minimize a window.
   */
  async minimizeWindow(selector?: string): Promise<void> {
    await this.rdp._send({
      type: 'automate' as const,
      op: 'window' as const,
      action: 'minimize' as const,
      selector,
    });
  }

  /**
   * Restore a window.
   */
  async restoreWindow(selector?: string): Promise<void> {
    await this.rdp._send({
      type: 'automate' as const,
      op: 'window' as const,
      action: 'restore' as const,
      selector,
    });
  }

  /**
   * Close a window.
   */
  async closeWindow(selector?: string): Promise<void> {
    await this.rdp._send({
      type: 'automate' as const,
      op: 'window' as const,
      action: 'close' as const,
      selector,
    });
  }

  /**
   * Run a PowerShell command.
   */
  async run(command: string, options: RunOptions = {}): Promise<AutomationRunResult> {
    const response = await this.rdp._send({
      type: 'automate' as const,
      op: 'run' as const,
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
      type: 'automate' as const,
      op: 'wait_for' as const,
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
      type: 'automate' as const,
      op: 'status' as const,
    });
    return response.data as unknown as AutomationStatus;
  }
}
