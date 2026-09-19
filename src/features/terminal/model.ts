export interface TerminalProfile {
  id: string;
  label: string;
  shellPath: string;
}

export interface TerminalOpenRequest {
  profileId: string;
  columns: number;
  rows: number;
}

export interface TerminalOpenResult {
  sessionId: string;
  profileId: string;
  shellLabel: string;
  cwd: string;
}

export interface TerminalActionResult {
  accepted: boolean;
}

export interface TerminalEvent {
  sessionId: string;
  kind: "data" | "exit" | "error";
  timestamp: string;
  data?: string;
  encoding?: "base64";
  byteLength?: number;
  exitCode?: number;
  detail?: string;
}

