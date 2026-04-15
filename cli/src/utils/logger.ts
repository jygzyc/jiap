/**
 * Lightweight append-only logger for JIAP CLI.
 *
 * Logs API calls and CLI events to session log files
 * at ~/.jiap/logs/<session>.log in JSONL format.
 * Always-on, never throws — logging failures must not break the CLI.
 */

import { appendFileSync, mkdirSync, existsSync } from "fs";
import * as path from "path";
import * as os from "os";

const LOG_DIR = path.join(os.homedir(), ".jiap", "logs");
const GENERAL_LOG = path.join(LOG_DIR, "cli.log");

let dirInitialized = false;

function ensureLogDir(): void {
  if (!dirInitialized) {
    if (!existsSync(LOG_DIR)) {
      mkdirSync(LOG_DIR, { recursive: true });
    }
    dirInitialized = true;
  }
}

/**
 * Resolve log file path for a session, falling back to general CLI log.
 */
function resolveLogFile(sessionName?: string): string {
  if (sessionName) {
    return path.join(LOG_DIR, `${sessionName}.log`);
  }
  return GENERAL_LOG;
}

// ============================================================================
// API call logging
// ============================================================================

export interface ApiLogEntry {
  ts: string;
  type: "api";
  method: string;
  path: string;
  duration_ms: number;
  status: "ok" | "error";
  error?: string;
}

/**
 * Append an API call log entry to a session log file. Never throws.
 */
export function logApiCall(sessionName: string, entry: Omit<ApiLogEntry, "ts" | "type">): void {
  try {
    ensureLogDir();
    const logFile = resolveLogFile(sessionName);
    appendFileSync(logFile, JSON.stringify({ ...entry, ts: new Date().toISOString(), type: "api" } as ApiLogEntry) + "\n", "utf-8");
  } catch {
    // Silent failure — logging must never break the CLI
  }
}

// ============================================================================
// CLI event logging
// ============================================================================

export interface CliEventEntry {
  ts: string;
  type: "cli";
  command: string;
  action: string;
  [key: string]: unknown;
}

/**
 * Append a CLI event log entry. Never throws.
 */
export function logCliEvent(entry: Omit<CliEventEntry, "ts" | "type">): void {
  try {
    ensureLogDir();
    appendFileSync(GENERAL_LOG, JSON.stringify({ ...entry, ts: new Date().toISOString(), type: "cli" } as CliEventEntry) + "\n", "utf-8");
  } catch {
    // Silent failure
  }
}

// ============================================================================
// Error logging
// ============================================================================

export interface ErrorLogEntry {
  ts: string;
  type: "error";
  code?: string;
  message: string;
  command?: string;
  [key: string]: unknown;
}

/**
 * Append an error log entry. Never throws.
 */
export function logError(entry: Omit<ErrorLogEntry, "ts" | "type">): void {
  try {
    ensureLogDir();
    appendFileSync(GENERAL_LOG, JSON.stringify({ ...entry, ts: new Date().toISOString(), type: "error" } as ErrorLogEntry) + "\n", "utf-8");
  } catch {
    // Silent failure
  }
}
