/**
 * Lightweight append-only API call logger.
 *
 * Logs every JIAPClient.request() call to the current session's log file
 * at ~/.jiap/logs/<session>.log in JSONL format.
 * Always-on, never throws — logging failures must not break the CLI.
 */

import { appendFileSync, mkdirSync, existsSync } from "fs";
import * as path from "path";
import * as os from "os";

const LOG_DIR = path.join(os.homedir(), ".jiap", "logs");

let dirInitialized = false;

function ensureLogDir(): void {
  if (!dirInitialized) {
    if (!existsSync(LOG_DIR)) {
      mkdirSync(LOG_DIR, { recursive: true });
    }
    dirInitialized = true;
  }
}

export interface ApiLogEntry {
  ts: string;
  method: string;
  path: string;
  duration_ms: number;
  status: "ok" | "error";
  error?: string;
}

/**
 * Append an API call log entry to a session log file. Never throws.
 */
export function logApiCall(sessionName: string, entry: ApiLogEntry): void {
  try {
    ensureLogDir();
    const logFile = path.join(LOG_DIR, `${sessionName}.log`);
    appendFileSync(logFile, JSON.stringify(entry) + "\n", "utf-8");
  } catch {
    // Silent failure — logging must never break the CLI
  }
}
