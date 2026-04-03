# Name-Based Session Management Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace PID-based session identification with filename-based naming. Users reference sessions by APK filename instead of unreliable PIDs.

**Architecture:** Session files move from `~/.jiap/sessions/<hash>.json` to `~/.jiap/sessions/<name>.json`. The `name` field (APK filename without extension) becomes the primary key. Hash is retained for content dedup. The `--pid` flag is replaced by `--session` / `-s`.

**Tech Stack:** TypeScript, Node.js, Commander.js

---

## File Map

| File | Action | Responsibility |
|------|--------|---------------|
| `cli/src/core/types.ts` | Modify | Add `name` to `Session` interface |
| `cli/src/core/session.ts` | Modify | Switch to name-based storage, remove `getSessionByPid` |
| `cli/src/core/config.ts` | Modify | Update `Manager` session delegates |
| `cli/src/core/client-helper.ts` | Modify | Replace `--pid` with `--session`, add autoSelect |
| `cli/src/commands/process.ts` | Modify | Add `--name`, name collision logic, update close/list/status |
| `cli/src/commands/code.ts` | Modify | Replace `--pid` with `--session` |
| `cli/src/commands/ard.ts` | Modify | Replace `--pid` with `--session` |
| `cli/tests/session.test.ts` | Modify | Update to name-based tests |
| `cli/tests/config.test.ts` | Modify | Remove `getSessionByPid` test |
| `cli/tests/client.test.ts` | No change | No `--pid` references |

---

### Task 1: Update Session type

**Files:**
- Modify: `cli/src/core/types.ts:3-9`

- [ ] **Step 1: Add `name` field to Session interface**

```typescript
export interface Session {
  name: string;     // NEW — APK filename without extension
  hash: string;
  pid: number;
  port: number;
  path: string;
  startedAt: number;
}
```

- [ ] **Step 2: Run typecheck**

Run: `cd cli && npx tsc --noEmit`
Expected: Errors in files that construct `Session` without `name` — these will be fixed in subsequent tasks.

- [ ] **Step 3: Commit**

```bash
git add cli/src/core/types.ts
git commit -m "feat(session): add name field to Session type"
```

---

### Task 2: Rewrite session.ts for name-based storage

**Files:**
- Modify: `cli/src/core/session.ts`

- [ ] **Step 1: Update session.ts**

Replace the entire file content with:

```typescript
/**
 * Session management for JIAP CLI.
 *
 * Sessions are stored in ~/.jiap/sessions/<name>.json
 * where name is the APK filename without extension.
 */

import {
  existsSync, mkdirSync, writeFileSync, readFileSync,
  readdirSync, unlinkSync, renameSync,
} from "fs";
import * as path from "path";
import * as os from "os";
import { randomBytes } from "crypto";
import { spawnSync } from "child_process";
import type { Session } from "./types.js";

const SESSIONS_DIR = path.join(os.homedir(), ".jiap", "sessions");
const SESSION_MAX_AGE_MS = 30 * 24 * 60 * 60 * 1000; // 30 days

function sessionFilePath(name: string): string {
  return path.join(SESSIONS_DIR, `${name}.json`);
}

/**
 * Verify that a PID still belongs to a JADX/JIAP process (not a reused PID).
 */
function isJadxProcess(pid: number): boolean {
  try {
    process.kill(pid, 0);
  } catch {
    return false;
  }

  try {
    if (process.platform === "win32") return true;
    const psCmd = process.platform === "darwin" ? "ps" : "ps";
    const result = spawnSync(psCmd, ["-p", String(pid), "-o", "comm="], {
      encoding: "utf-8",
      timeout: 3000,
    });
    if (result.status !== 0 || !result.stdout.trim()) return false;
    const comm = result.stdout.trim().toLowerCase();
    return comm.includes("java") || comm.includes("jadx") || comm.includes("node");
  } catch {
    return true;
  }
}

/**
 * Atomic write: write to temp file then rename (POSIX atomic).
 */
function atomicWriteJson(filePath: string, data: unknown): void {
  mkdirSync(path.dirname(filePath), { recursive: true });
  const tmpFile = `${filePath}.${randomBytes(4).toString("hex")}.tmp`;
  writeFileSync(tmpFile, JSON.stringify(data, null, 2), "utf-8");
  renameSync(tmpFile, filePath);
}

export function createSession(name: string, hash: string, apkPath: string, pid: number, port: number): Session {
  const session: Session = { name, hash, pid, port, path: apkPath, startedAt: Date.now() };
  atomicWriteJson(sessionFilePath(name), session);
  return session;
}

export function readSession(name: string): Session | null {
  const file = sessionFilePath(name);
  if (!existsSync(file)) return null;
  try {
    return JSON.parse(readFileSync(file, "utf-8")) as Session;
  } catch {
    return null;
  }
}

export function deleteSession(name: string): void {
  const file = sessionFilePath(name);
  if (existsSync(file)) unlinkSync(file);
}

export function listAllSessions(): Session[] {
  if (!existsSync(SESSIONS_DIR)) return [];
  const sessions: Session[] = [];
  for (const f of readdirSync(SESSIONS_DIR)) {
    if (!f.endsWith(".json")) continue;
    try {
      sessions.push(JSON.parse(readFileSync(path.join(SESSIONS_DIR, f), "utf-8")) as Session);
    } catch { /* skip invalid */ }
  }
  return sessions;
}

export function isSessionAlive(session: Session): boolean {
  return isJadxProcess(session.pid);
}

export function cleanupDead(): number {
  let n = 0;
  const now = Date.now();
  for (const s of listAllSessions()) {
    const expired = now - s.startedAt > SESSION_MAX_AGE_MS;
    if (expired || !isJadxProcess(s.pid)) {
      deleteSession(s.name);
      n++;
    }
  }
  return n;
}

/**
 * Resolve session name from options.
 * Returns null if no session can be determined.
 */
export function autoSelectSession(): Session | null {
  const alive = listAllSessions().filter(s => isSessionAlive(s));
  if (alive.length === 1) return alive[0];
  return null;
}
```

- [ ] **Step 2: Run typecheck**

Run: `cd cli && npx tsc --noEmit`
Expected: Errors in config.ts and process.ts (callers using old API) — fixed in next tasks.

- [ ] **Step 3: Commit**

```bash
git add cli/src/core/session.ts
git commit -m "feat(session): rewrite to name-based storage"
```

---

### Task 3: Update Manager session delegates

**Files:**
- Modify: `cli/src/core/config.ts:82-100`

- [ ] **Step 1: Update Manager class session methods**

Replace lines 82-100 in `cli/src/core/config.ts`:

```typescript
  // --- Session delegates ---

  async createSession(name: string, apkPath: string, pid: number, port: number) {
    return session.createSession(name, await hashFile(apkPath), apkPath, pid, port);
  }

  getSession(name: string) { return session.readSession(name); }

  removeSession(name: string) { session.deleteSession(name); }

  listSessions() { return session.listAllSessions(); }

  listAliveSessions() {
    return session.listAllSessions().filter(s => session.isSessionAlive(s));
  }

  cleanupDead() { return session.cleanupDead(); }

  autoSelectSession() { return session.autoSelectSession(); }
```

- [ ] **Step 2: Run typecheck**

Run: `cd cli && npx tsc --noEmit`
Expected: Errors in process.ts and client-helper.ts — fixed in next tasks.

- [ ] **Step 3: Commit**

```bash
git add cli/src/core/config.ts
git commit -m "feat(session): update Manager delegates for name-based sessions"
```

---

### Task 4: Rewrite resolveClient in client-helper.ts

**Files:**
- Modify: `cli/src/core/client-helper.ts`

- [ ] **Step 1: Rewrite client-helper.ts**

Replace entire file:

```typescript
/**
 * Shared client resolution helper for commands.
 */

import { JIAPClient } from "./client.js";
import { Formatter } from "../utils/formatter.js";
import { Manager } from "./config.js";

export function resolveClient(
  opts: Record<string, unknown>
): { fmt: Formatter; client: JIAPClient } {
  const jsonMode = Boolean(opts.json);
  const fmt = new Formatter(jsonMode);
  const mgr = Manager.get();

  let port: number;
  if (opts.session && opts.port) {
    fmt.error("--session and --port are mutually exclusive");
    process.exit(1);
  } else if (opts.session) {
    const session = mgr.getSession(opts.session as string);
    if (!session) {
      fmt.error(`Session "${opts.session}" not found`);
      process.exit(1);
    }
    port = session.port;
  } else if (opts.port) {
    port = parseInt(opts.port as string);
  } else {
    // Auto-select: use single alive session if exactly one exists
    const session = mgr.autoSelectSession();
    if (session) {
      port = session.port;
    } else {
      port = mgr.server.defaultPort;
    }
  }

  const client = new JIAPClient("127.0.0.1", port);
  return { fmt, client };
}
```

- [ ] **Step 2: Run typecheck**

Run: `cd cli && npx tsc --noEmit`
Expected: Clean (no errors from this file).

- [ ] **Step 3: Commit**

```bash
git add cli/src/core/client-helper.ts
git commit -m "feat(cli): replace --pid with --session in resolveClient"
```

---

### Task 5: Update code.ts and ard.ts commands

**Files:**
- Modify: `cli/src/commands/code.ts:8-11`
- Modify: `cli/src/commands/ard.ts:10-13`

- [ ] **Step 1: Replace --pid with --session in code.ts**

In `cli/src/commands/code.ts`, replace:

```typescript
    .option("-p, --pid <pid>", "Target process by PID")
    .option("-P, --port <port>", "Server port")
```

With:

```typescript
    .option("-s, --session <name>", "Target session by name")
    .option("-P, --port <port>", "Server port")
```

- [ ] **Step 2: Replace --pid with --session in ard.ts**

In `cli/src/commands/ard.ts`, replace:

```typescript
    .option("-p, --pid <pid>", "Target process by PID")
    .option("-P, --port <port>", "Server port")
```

With:

```typescript
    .option("-s, --session <name>", "Target session by name")
    .option("-P, --port <port>", "Server port")
```

- [ ] **Step 3: Run typecheck**

Run: `cd cli && npx tsc --noEmit`
Expected: Errors only in process.ts — fixed in next task.

- [ ] **Step 4: Commit**

```bash
git add cli/src/commands/code.ts cli/src/commands/ard.ts
git commit -m "feat(cli): replace --pid with --session in code/ard commands"
```

---

### Task 6: Update process.ts commands

**Files:**
- Modify: `cli/src/commands/process.ts`

- [ ] **Step 1: Add --name option to open command**

In the `open` command section, after line 71 (`.option("--force", ...)`), add:

```typescript
    .option("--name <name>", "Custom session name (default: APK filename)")
```

- [ ] **Step 2: Rewrite open action session logic**

Replace lines 103-118 in `cli/src/commands/process.ts` (the `--- Server mode` block):

```typescript
      // --- Server mode: check for existing session ---
      const fileHash = await hashFile(resolvedFile);
      const fileName = opts.name || path.basename(resolvedFile, path.extname(resolvedFile));

      // Check name collision
      const existingByName = mgr.getSession(fileName);
      if (existingByName && !opts.force) {
        if (existingByName.hash === fileHash && isSessionAlive(existingByName)) {
          fmt.info(`Session "${fileName}" already running (port: ${existingByName.port})`);
          if (opts.json) {
            fmt.output({ name: fileName, pid: existingByName.pid, port: existingByName.port, file: resolvedFile, reused: true });
          }
          return;
        }
        if (existingByName.hash === fileHash && !isSessionAlive(existingByName)) {
          mgr.removeSession(fileName);
        } else {
          throw new ProcessError(
            `Session "${fileName}" already exists for a different APK (${existingByName.path}). Use --name to specify a different name.`
          );
        }
      }

      // Check hash dedup: prevent same APK under different names
      if (!opts.force) {
        for (const s of mgr.listAliveSessions()) {
          if (s.hash === fileHash && s.name !== fileName) {
            throw new ProcessError(
              `Already open as session "${s.name}". Use --force to open again.`
            );
          }
        }
      }
```

- [ ] **Step 3: Update spawn + createSession call**

Replace line 160:

```typescript
      const session = await mgr.createSession(fileName, resolvedFile, proc.pid, port);
```

And update the success message (line 161):

```typescript
      fmt.success(`Started "${fileName}" (PID: ${proc.pid}, Port: ${port})`);
```

- [ ] **Step 4: Update JSON output in open**

Replace lines 176-178:

```typescript
      if (opts.json) {
        fmt.output({ name: fileName, pid: proc.pid, port, file: resolvedFile, reused: false });
      }
```

- [ ] **Step 5: Update close command to use name**

Replace lines 181-232 (close command):

```typescript
  // close
  cmd
    .command("close [name]")
    .description("Stop JADX process by session name")
    .option("-a, --all", "Kill all processes")
    .option("--json", "JSON output")
    .action(withErrorHandler(async (name: string | undefined, opts) => {
      const fmt = new Formatter(opts.json);
      const mgr = Manager.get();

      const cleaned = mgr.cleanupDead();
      if (cleaned > 0 && !opts.json) {
        fmt.info(`Cleaned ${cleaned} stale session(s)`);
      }

      if (opts.all) {
        const sessions = mgr.listAliveSessions();
        const killed: string[] = [], dead: string[] = [];
        for (const s of sessions) {
          const alive = await killProcessGroup(s.pid);
          mgr.removeSession(s.name);
          (alive ? killed : dead).push(s.name);
        }
        if (killed.length) fmt.success(`Killed ${killed.length} process(es)`);
        if (dead.length) fmt.info(`Removed ${dead.length} dead session(s)`);
        if (opts.json) fmt.output({ killed, dead });
        return;
      }

      if (!name) {
        const alive = mgr.listAliveSessions();
        if (alive.length === 1) {
          name = alive[0].name;
        } else {
          throw new ProcessError(
            alive.length === 0
              ? "No running sessions"
              : "Specify a session name or use --all"
          );
        }
      }

      const session = mgr.getSession(name);
      if (!session) {
        throw new ProcessError(`Session not found: ${name}`);
      }

      const alive = await killProcessGroup(session.pid);
      mgr.removeSession(name);
      alive ? fmt.success(`Killed "${name}"`) : fmt.info(`Removed dead session "${name}"`);
    }, (_name, opts) => new Formatter(Boolean(opts.json))));
```

- [ ] **Step 6: Update list command output**

Replace lines 234-254 (list command):

```typescript
  // list
  cmd
    .command("list")
    .description("List running processes")
    .option("--json", "JSON output")
    .action((opts) => {
      const fmt = new Formatter(opts.json);
      const mgr = Manager.get();

      const cleaned = mgr.cleanupDead();
      if (cleaned > 0 && !opts.json) {
        fmt.info(`Cleaned ${cleaned} stale session(s)`);
      }

      const sessions = mgr.listAliveSessions();
      if (sessions.length === 0) {
        fmt.info("No running sessions");
        if (opts.json) fmt.output([]);
        return;
      }

      if (!opts.json) {
        // Table header
        console.log(`  ${"NAME".padEnd(20)} ${"PORT".padEnd(8)} ${"PID".padEnd(8)} PATH`);
        for (const s of sessions) {
          console.log(`  ${s.name.padEnd(20)} ${String(s.port).padEnd(8)} ${String(s.pid).padEnd(8)} ${s.path}`);
        }
      } else {
        fmt.output(sessions);
      }
    });
```

- [ ] **Step 7: Update status command to use name**

Replace lines 256-284 (status command):

```typescript
  // status
  cmd
    .command("status [name]")
    .description("Check session status")
    .option("-P, --port <port>", "Server port", String)
    .option("--json", "JSON output")
    .action(withErrorHandler(async (name: string | undefined, opts) => {
      const fmt = new Formatter(opts.json);
      const mgr = Manager.get();
      let port: number;

      if (name) {
        const session = mgr.getSession(name);
        if (!session) throw new ProcessError(`Session not found: ${name}`);
        port = session.port;
      } else if (opts.port) {
        port = parseInt(opts.port);
      } else {
        port = mgr.server.defaultPort;
      }

      const client = new JIAPClient("127.0.0.1", port);
      try {
        await client.healthCheck();
        fmt.success(`Server running on port ${port}`);
      } catch (err) {
        throw new JiapError(String(err), "SERVER_ERROR", { port });
      }
    }, (_name, opts) => new Formatter(Boolean(opts.json))));
```

- [ ] **Step 8: Update port conflict error message**

In the open command's port availability check, update line 128:

```typescript
            `Port ${port} is already in use by session "${portSession.name}" (${portSession.path}). ` +
```

- [ ] **Step 9: Run typecheck**

Run: `cd cli && npx tsc --noEmit`
Expected: Clean.

- [ ] **Step 10: Commit**

```bash
git add cli/src/commands/process.ts
git commit -m "feat(process): update open/close/list/status for name-based sessions"
```

---

### Task 7: Update session.test.ts

**Files:**
- Modify: `cli/tests/session.test.ts`

- [ ] **Step 1: Rewrite session.test.ts**

Replace entire file:

```typescript
/**
 * Tests for session management.
 */

import {
  createSession,
  readSession,
  deleteSession,
  listAllSessions,
  isSessionAlive,
  cleanupDead,
  autoSelectSession,
} from "../src/core/session.js";
import { existsSync, readdirSync, rmSync, readFileSync } from "fs";
import * as path from "path";
import * as os from "os";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const SESSIONS_DIR = path.join(os.homedir(), ".jiap", "sessions");

// Use unique test prefix to avoid colliding with real sessions
const TEST_PREFIX = "test_";

function testName(id: number): string {
  return `${TEST_PREFIX}${String(id).padStart(12, "0")}`;
}

function cleanupTestSessions(): void {
  if (!existsSync(SESSIONS_DIR)) return;
  for (const f of readdirSync(SESSIONS_DIR)) {
    if (f.startsWith(TEST_PREFIX) && f.endsWith(".json")) {
      try { rmSync(path.join(SESSIONS_DIR, f)); } catch { /* ignore */ }
    }
  }
}

describe("Session management", () => {
  beforeEach(() => {
    cleanupTestSessions();
  });

  afterAll(() => {
    cleanupTestSessions();
  });

  describe("createSession + readSession", () => {
    it("creates a session file and reads it back", () => {
      const session = createSession(testName(1), "aabbccdd", "/fake/path.apk", 12345, 25419);
      expect(session.name).toBe(testName(1));
      expect(session.hash).toBe("aabbccdd");
      expect(session.pid).toBe(12345);
      expect(session.port).toBe(25419);
      expect(session.path).toBe("/fake/path.apk");
      expect(session.startedAt).toBeGreaterThan(0);

      const read = readSession(testName(1));
      expect(read).not.toBeNull();
      expect(read!.name).toBe(testName(1));
      expect(read!.hash).toBe("aabbccdd");
      expect(read!.pid).toBe(12345);
    });

    it("returns null for non-existent session", () => {
      expect(readSession("nonexistent_name")).toBeNull();
    });
  });

  describe("deleteSession", () => {
    it("removes session file", () => {
      createSession(testName(2), "aaa", "/fake/path.apk", 11111, 25419);
      expect(readSession(testName(2))).not.toBeNull();

      deleteSession(testName(2));
      expect(readSession(testName(2))).toBeNull();
    });

    it("does not throw for non-existent session", () => {
      expect(() => deleteSession("nonexistent")).not.toThrow();
    });
  });

  describe("listAllSessions", () => {
    it("lists all sessions including test ones", () => {
      createSession(testName(10), "aaa", "/a.apk", 1001, 25419);
      createSession(testName(11), "bbb", "/b.apk", 1002, 25420);

      const sessions = listAllSessions();
      const testSessions = sessions.filter(s => s.name.startsWith(TEST_PREFIX));
      expect(testSessions.length).toBeGreaterThanOrEqual(2);
    });
  });

  describe("isSessionAlive", () => {
    it("returns false for non-existent PID", () => {
      const session = createSession(testName(30), "ccc", "/a.apk", 99999999, 25419);
      expect(isSessionAlive(session)).toBe(false);
    });
  });

  describe("cleanupDead", () => {
    it("removes sessions with dead PIDs", () => {
      createSession(testName(40), "ddd", "/a.apk", 99999999, 25419);
      createSession(testName(41), "eee", "/b.apk", 99999998, 25419);

      cleanupDead();
      expect(readSession(testName(40))).toBeNull();
      expect(readSession(testName(41))).toBeNull();
    });
  });

  describe("autoSelectSession", () => {
    it("returns null when no sessions exist", () => {
      expect(autoSelectSession()).toBeNull();
    });
  });

  describe("atomic writes", () => {
    it("session file is valid JSON after creation", () => {
      createSession(testName(50), "fff", "/a.apk", 55555, 25419);
      const raw = readFileSync(path.join(SESSIONS_DIR, `${testName(50)}.json`), "utf-8");
      const parsed = JSON.parse(raw);
      expect(parsed.name).toBe(testName(50));
    });

    it("no .tmp files left after creation", () => {
      createSession(testName(51), "ggg", "/a.apk", 55556, 25419);
      const files = readdirSync(SESSIONS_DIR);
      const tmpFiles = files.filter(f => f.includes(".tmp"));
      expect(tmpFiles.length).toBe(0);
    });
  });
});
```

- [ ] **Step 2: Run tests**

Run: `cd cli && npm test -- tests/session.test.ts --verbose`
Expected: All pass.

- [ ] **Step 3: Commit**

```bash
git add cli/tests/session.test.ts
git commit -m "test: update session tests for name-based sessions"
```

---

### Task 8: Update config.test.ts

**Files:**
- Modify: `cli/tests/config.test.ts:66-97`

- [ ] **Step 1: Replace session delegation tests**

Replace lines 66-97 in `cli/tests/config.test.ts`:

```typescript
  describe("session delegation", () => {
    it("getSession returns null for unknown name", () => {
      const mgr = Manager.get();
      expect(mgr.getSession("nonexistent_test_session")).toBeNull();
    });

    it("listSessions returns an array", () => {
      const mgr = Manager.get();
      const sessions = mgr.listSessions();
      expect(Array.isArray(sessions)).toBe(true);
    });

    it("listAliveSessions returns an array", () => {
      const mgr = Manager.get();
      const alive = mgr.listAliveSessions();
      expect(Array.isArray(alive)).toBe(true);
    });

    it("cleanupDead returns a number", () => {
      const mgr = Manager.get();
      const count = mgr.cleanupDead();
      expect(typeof count).toBe("number");
    });
  });
```

- [ ] **Step 2: Run tests**

Run: `cd cli && npm test -- tests/config.test.ts --verbose`
Expected: All pass.

- [ ] **Step 3: Commit**

```bash
git add cli/tests/config.test.ts
git commit -m "test: update config tests for name-based sessions"
```

---

### Task 9: Update SKILL.md documentation

**Files:**
- Modify: `skill/jiapcli/SKILL.md:55-64`

- [ ] **Step 1: Update global options table**

Replace lines 55-64:

```markdown
### 全局选项

适用于 `code`/`ard`：

| 选项 | 说明 |
|------|------|
| `-s, --session <name>` | 指定目标 session 名称（APK 文件名） |
| `-P, --port <port>` | 指定服务器端口（默认 25419） |
| `--json` | JSON 格式输出 |
| `--page <n>` | 分页参数（默认 1） |
```

- [ ] **Step 2: Update process open description**

Add `--name` to the process table:

```markdown
| `jiap process open <apk> [--name <name>]` | 打开 APK 分析 |
```

- [ ] **Step 3: Commit**

```bash
git add skill/jiapcli/SKILL.md
git commit -m "docs: update SKILL.md for name-based session management"
```

---

### Task 10: Final verification

- [ ] **Step 1: Run typecheck**

Run: `cd cli && npx tsc --noEmit`
Expected: Clean, no errors.

- [ ] **Step 2: Run all tests**

Run: `cd cli && npm test`
Expected: All 8 test suites pass.

- [ ] **Step 3: Build**

Run: `cd cli && npm run build`
Expected: Clean build to `dist/`.

- [ ] **Step 4: Commit any remaining changes (if needed)**
