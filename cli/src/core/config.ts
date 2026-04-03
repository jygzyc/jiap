/**
 * Configuration management for JIAP CLI.
 */

import { existsSync, mkdirSync, writeFileSync, readFileSync } from "fs";
import * as path from "path";
import * as os from "os";
import type { Config } from "./types.js";
import { hashFile } from "../utils/hash.js";
import * as session from "./session.js";

const HOME = os.homedir();
const CONFIG_DIR = path.join(HOME, ".jiap");
const CONFIG_FILE = path.join(CONFIG_DIR, "config.json");

export { hashFile } from "../utils/hash.js";
export * from "./session.js";

export function expandPath(p: string): string {
  if (p.startsWith("~/") || p === "~") return path.join(HOME, p.slice(1));
  return p;
}

function defaultConfig(): Config {
  return {
    jadx: { path: null, version: "1.5.3", installDir: "~/.jiap/jadx" },
    server: { defaultPort: 25419, timeout: 30 },
    output: { defaultDir: "~/.jiap/output", decompileDir: "~/.jiap/decompiled" },
  };
}

function readConfig(): Config {
  if (!existsSync(CONFIG_FILE)) return defaultConfig();
  try {
    const data = JSON.parse(readFileSync(CONFIG_FILE, "utf-8"));
    return { ...defaultConfig(), ...data };
  } catch {
    return defaultConfig();
  }
}

function writeConfig(config: Config): void {
  mkdirSync(CONFIG_DIR, { recursive: true });
  writeFileSync(CONFIG_FILE, JSON.stringify(config, null, 2), "utf-8");
}

export class Manager {
  private static _instance: Manager | null = null;
  private config: Config;

  private constructor() {
    this.config = readConfig();
  }

  static get(): Manager {
    if (!Manager._instance) Manager._instance = new Manager();
    return Manager._instance;
  }

  static reset(): void {
    Manager._instance = null;
  }

  get jadx() { return this.config.jadx; }
  get server() { return this.config.server; }
  get output() { return this.config.output; }

  getJadxPath(): string | null {
    // Only check config override — full search logic is in finder.ts
    if (this.config.jadx.path) {
      const p = expandPath(this.config.jadx.path);
      if (existsSync(p)) return p;
    }
    return null;
  }

  setJadxPath(p: string): void {
    this.config.jadx.path = p;
    writeConfig(this.config);
  }

  // --- Session delegates ---

  async createSession(name: string, apkPath: string, pid: number, port: number) {
    const hash = await hashFile(apkPath);
    return session.createSession(name, hash, apkPath, pid, port);
  }

  getSession(name: string) { return session.readSession(name); }

  removeSession(name: string) { session.deleteSession(name); }

  autoSelectSession() { return session.autoSelectSession(); }

  listSessions() { return session.listAllSessions(); }

  listAliveSessions() {
    return session.listAllSessions().filter(s => session.isSessionAlive(s));
  }

  cleanupDead() { return session.cleanupDead(); }
}
