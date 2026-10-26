// Mirrors the Rust types the app's commands return (src-tauri/src/inventory.rs and
// the aieye-model and aieye-store crates). Keep the two in step.

export type Evidence = "verified" | "declared" | "inferred";

export type Fact =
  | { kind: "tool_present"; tool: string; path: string }
  | { kind: "tool_running"; tool: string; pid: number; exe: string }
  | {
      kind: "configured_folder";
      tool: string;
      file: string;
      path: string;
      trust: string | null;
      boundary: boolean;
    }
  | {
      kind: "mcp_server";
      tool: string;
      file: string;
      scope: string | null;
      name: string;
      command: string | null;
      origin: string | null;
      env_names: string[];
    };

/** One fact with the fields every record carries. */
export interface FactRecord {
  device_id: string;
  org_id: string | null;
  source: string;
  evidence: Evidence;
  observed_at: string;
  fact: Fact;
}

export interface SnapshotInfo {
  id: number;
  device_id: string;
  taken_at: string;
  records: number;
  aieye_version: string;
}

export interface Totals {
  tools: number;
  running: number;
  folders: number;
  servers: number;
}

export type Status = "running" | "installed" | "config_only";

export interface ToolView {
  id: string;
  name: string;
  vendor: string;
  status: Status;
  verified: string[];
  present: FactRecord[];
  running: FactRecord[];
  folders: FactRecord[];
  servers: FactRecord[];
}

export interface ChangeLine {
  severity: "alert" | "info";
  tool: string;
  what: string;
  detail: string;
}

export interface ChangesView {
  from: SnapshotInfo;
  alerts: number;
  lines: ChangeLine[];
}

export interface ScanView {
  snapshot: SnapshotInfo;
  totals: Totals;
  tools: ToolView[];
  not_found: string[];
  changes: ChangesView | null;
}
