//! HUD State and Telemetry Types

export type HudState =
  | "IDLE"
  | "WAKE_DETECTED"
  | "LISTENING"
  | "PROCESSING"
  | "PLANNING"
  | "EXECUTING"
  | "AWAITING_USER"
  | "SPEAKING"
  | "SUCCESS"
  | "ERROR";

export interface ExecutionTelemetry {
  taskId?: string;
  toolName?: string;
  applicationName?: string;
  processId?: number;
  durationMs?: number;
  statusMessage?: string;
  artifactType?: string;
  path?: string;
  filename?: string;
  width?: number;
  height?: number;
}

export interface HudEvent {
  type: string;
  payload: any;
  timestamp: number;
}
