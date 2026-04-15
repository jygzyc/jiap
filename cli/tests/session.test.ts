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
      // Clean all sessions (including non-test ones) to ensure isolation
      if (existsSync(SESSIONS_DIR)) {
        for (const f of readdirSync(SESSIONS_DIR)) {
          if (f.endsWith(".json")) {
            try { rmSync(path.join(SESSIONS_DIR, f)); } catch { /* ignore */ }
          }
        }
      }
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
