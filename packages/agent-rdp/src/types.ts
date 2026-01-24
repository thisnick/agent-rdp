/**
 * TypeScript types for agent-rdp.
 *
 * IPC types are auto-generated from Rust (see generated/).
 * SDK convenience types are defined here.
 */

// --- Re-export generated IPC types ---
// These are auto-generated from agent-rdp-protocol via ts-rs.
// Run `cargo test -p agent-rdp-protocol --lib` to regenerate.

export type {
  // Request types
  Request,
  ConnectRequest,
  ScreenshotRequest,
  MouseRequest,
  KeyboardRequest,
  ScrollRequest,
  ClipboardRequest,
  DriveRequest,
  LocateRequest,
  AutomateRequest,

  // Response types
  Response,
  ResponseData,
  ErrorCode,
  ErrorInfo,
  SessionInfo,
  SessionSummary,
  MappedDrive,
  LocateResult,
  OcrMatch,

  // Supporting types
  DriveMapping,
  ImageFormat,
  MouseButton,
  ScrollDirection,
  ConnectionState,

  // Automation types
  AccessibilityElement,
  AccessibilitySnapshot,
  AutomationStatus,
  AutomationScrollDirection,
  AutomationHandshake,
  ClickResult,
  RunResult,
  ElementBounds,
  ElementValue,
  WindowInfo,
  WindowAction,
  WaitState,

  // File IPC types (daemon <-> PowerShell)
  FileIpcRequest,
  FileIpcResponse,
  FileIpcError,
} from './generated/index.js';

// --- SDK Convenience Types ---
// These are higher-level types for the SDK API, not IPC.

import type { DriveMapping, ErrorCode } from './generated/index.js';

/** Options for connecting to an RDP session. */
export interface ConnectOptions {
  host: string;
  port?: number;
  username: string;
  password: string;
  domain?: string;
  width?: number;
  height?: number;
  drives?: DriveMapping[];
  /** Enable Windows UI Automation. */
  enableWinAutomation?: boolean;
}

/** Result of a successful connection. */
export interface ConnectResult {
  host: string;
  width: number;
  height: number;
}

/** Options for taking a screenshot. */
export interface ScreenshotOptions {
  format?: 'png' | 'jpeg';
}

/** Result of a screenshot operation. */
export interface ScreenshotResult {
  base64: string;
  width: number;
  height: number;
  format: string;
}

/** A point representing x,y coordinates. */
export interface Point {
  x: number;
  y: number;
}

/** Options for mouse click operations. */
export interface MouseClickOptions {
  x: number;
  y: number;
}

/** Options for mouse drag operations. */
export interface MouseDragOptions {
  from: Point;
  to: Point;
}

/** Options for scroll operations. */
export interface ScrollOptions {
  /** Amount to scroll (default: 3). */
  amount?: number;
  /** X coordinate (optional). */
  x?: number;
  /** Y coordinate (optional). */
  y?: number;
}

/** Options for keyboard type operations. */
export interface KeyboardTypeOptions {
  /** Text to type. */
  text: string;
}

/** Options for keyboard press operations. */
export interface KeyboardPressOptions {
  /** Key combination (e.g., 'ctrl+c') or single key (e.g., 'enter'). */
  keys: string;
}

/** Options for clipboard set operations. */
export interface ClipboardSetOptions {
  /** Text to set. */
  text: string;
}

/** Options for locate (OCR) operations. */
export interface LocateOptions {
  /** Text to search for. Required unless all is true. */
  text?: string;
  /** If true, returns all text on screen. */
  all?: boolean;
  /** Use glob-style pattern matching (* and ?). */
  pattern?: boolean;
  /** Case-sensitive matching (default: false). */
  caseSensitive?: boolean;
}

// --- Automation convenience types (aliases for backwards compatibility) ---

/** Bounds for automation elements (alias for ElementBounds). */
export type { ElementBounds as AutomationElementBounds } from './generated/index.js';

/** Automation element (alias for AccessibilityElement). */
export type { AccessibilityElement as AutomationElement } from './generated/index.js';

/** Automation snapshot result. */
export type { AccessibilitySnapshot as AutomationSnapshot } from './generated/index.js';

/** Element value result (alias for ElementValue). */
export type { ElementValue as AutomationElementValue } from './generated/index.js';

/** Window info (alias for WindowInfo). */
export type { WindowInfo as AutomationWindowInfo } from './generated/index.js';

/** Run command result (alias for RunResult). */
export type { RunResult as AutomationRunResult } from './generated/index.js';

/** Click result (alias for ClickResult). */
export type { ClickResult as AutomationClickResult } from './generated/index.js';

// --- Error Class ---

/** Error class for RDP operations. */
export class RdpError extends Error {
  constructor(
    public code: ErrorCode,
    message: string,
  ) {
    super(message);
    this.name = 'RdpError';
  }
}
