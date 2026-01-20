/**
 * TypeScript interfaces for agent-rdp, mirroring agent-rdp-protocol.
 */

// --- Connection Types ---

export interface ConnectOptions {
  host: string;
  port?: number;
  username: string;
  password: string;
  domain?: string;
  width?: number;
  height?: number;
  drives?: DriveMapping[];
}

export interface DriveMapping {
  path: string;
  name: string;
}

export interface ConnectResult {
  host: string;
  width: number;
  height: number;
}

// --- Screenshot Types ---

export interface ScreenshotOptions {
  format?: 'png' | 'jpeg';
}

export interface ScreenshotResult {
  base64: string;
  width: number;
  height: number;
  format: string;
}

// --- Session Types ---

export type ConnectionState = 'disconnected' | 'connecting' | 'connected' | 'failed';

export interface SessionInfo {
  name: string;
  state: ConnectionState;
  host?: string;
  width?: number;
  height?: number;
  pid: number;
  uptime_secs: number;
}

export interface MappedDrive {
  name: string;
  path: string;
}

// --- API Parameter Types ---

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

// --- Request Types (for IPC) ---

export interface ConnectRequest {
  type: 'connect';
  host: string;
  port: number;
  username: string;
  password: string;
  domain?: string;
  width: number;
  height: number;
  drives: DriveMapping[];
}

export interface ScreenshotRequest {
  type: 'screenshot';
  format: 'png' | 'jpeg';
}

export interface MouseMoveRequest {
  type: 'mouse';
  action: 'move';
  x: number;
  y: number;
}

export interface MouseClickRequest {
  type: 'mouse';
  action: 'click';
  x: number;
  y: number;
}

export interface MouseRightClickRequest {
  type: 'mouse';
  action: 'right_click';
  x: number;
  y: number;
}

export interface MouseDoubleClickRequest {
  type: 'mouse';
  action: 'double_click';
  x: number;
  y: number;
}

export interface MouseDragRequest {
  type: 'mouse';
  action: 'drag';
  from_x: number;
  from_y: number;
  to_x: number;
  to_y: number;
}

export type MouseRequest =
  | MouseMoveRequest
  | MouseClickRequest
  | MouseRightClickRequest
  | MouseDoubleClickRequest
  | MouseDragRequest;

export interface KeyboardTypeRequest {
  type: 'keyboard';
  action: 'type';
  text: string;
}

export interface KeyboardPressRequest {
  type: 'keyboard';
  action: 'press';
  keys: string;
}

export type KeyboardRequest = KeyboardTypeRequest | KeyboardPressRequest;

export interface ScrollRequest {
  type: 'scroll';
  direction: 'up' | 'down' | 'left' | 'right';
  amount: number;
  x?: number;
  y?: number;
}

export interface ClipboardGetRequest {
  type: 'clipboard';
  action: 'get';
}

export interface ClipboardSetRequest {
  type: 'clipboard';
  action: 'set';
  text: string;
}

export type ClipboardRequest = ClipboardGetRequest | ClipboardSetRequest;

export interface DriveListRequest {
  type: 'drive';
  action: 'list';
}

export interface DisconnectRequest {
  type: 'disconnect';
}

export interface SessionInfoRequest {
  type: 'session_info';
}

export interface PingRequest {
  type: 'ping';
}

export type Request =
  | ConnectRequest
  | ScreenshotRequest
  | MouseRequest
  | KeyboardRequest
  | ScrollRequest
  | ClipboardRequest
  | DriveListRequest
  | DisconnectRequest
  | SessionInfoRequest
  | PingRequest;

// --- Response Types ---

export type ErrorCode =
  | 'not_connected'
  | 'already_connected'
  | 'connection_failed'
  | 'authentication_failed'
  | 'timeout'
  | 'invalid_request'
  | 'not_supported'
  | 'internal_error'
  | 'session_not_found'
  | 'ipc_error'
  | 'daemon_not_running'
  | 'clipboard_error'
  | 'drive_error';

export interface ErrorInfo {
  code: ErrorCode;
  message: string;
}

export interface ResponseOk {
  type: 'ok';
}

export interface ResponseConnected {
  type: 'connected';
  host: string;
  width: number;
  height: number;
}

export interface ResponseScreenshot {
  type: 'screenshot';
  width: number;
  height: number;
  format: string;
  base64: string;
}

export interface ResponseClipboard {
  type: 'clipboard';
  text: string;
}

export interface ResponseSessionInfo {
  type: 'session_info';
  name: string;
  state: ConnectionState;
  host?: string;
  width?: number;
  height?: number;
  pid: number;
  uptime_secs: number;
}

export interface ResponseDriveList {
  type: 'drive_list';
  drives: MappedDrive[];
}

export interface ResponsePong {
  type: 'pong';
}

export type ResponseData =
  | ResponseOk
  | ResponseConnected
  | ResponseScreenshot
  | ResponseClipboard
  | ResponseSessionInfo
  | ResponseDriveList
  | ResponsePong;

export interface Response {
  success: boolean;
  data?: ResponseData;
  error?: ErrorInfo;
}

// --- Error Class ---

export class RdpError extends Error {
  constructor(
    public code: ErrorCode,
    message: string,
  ) {
    super(message);
    this.name = 'RdpError';
  }
}
