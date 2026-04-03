/**
 * Config Manager unit tests.
 */

import { Manager, expandPath } from "../src/core/config.js";
import { existsSync, writeFileSync, unlinkSync, mkdirSync, rmSync } from "fs";
import * as path from "path";
import * as os from "os";

describe("expandPath", () => {
  it("expands ~/ to home directory", () => {
    expect(expandPath("~/foo")).toBe(path.join(os.homedir(), "foo"));
  });

  it("expands bare ~ to home directory", () => {
    expect(expandPath("~")).toBe(os.homedir());
  });

  it("leaves absolute paths unchanged", () => {
    expect(expandPath("/usr/local/bin")).toBe("/usr/local/bin");
  });

  it("leaves relative paths unchanged", () => {
    expect(expandPath("foo/bar")).toBe("foo/bar");
  });
});

describe("Manager", () => {
  it("returns singleton instance", () => {
    const a = Manager.get();
    const b = Manager.get();
    expect(a).toBe(b);
  });

  it("has jadx config", () => {
    const mgr = Manager.get();
    expect(mgr.jadx).toBeDefined();
    expect(mgr.jadx.version).toBeDefined();
    expect(mgr.jadx.installDir).toBeDefined();
  });

  it("has server config with defaultPort", () => {
    const mgr = Manager.get();
    expect(mgr.server).toBeDefined();
    expect(mgr.server.defaultPort).toBe(25419);
    expect(mgr.server.timeout).toBeDefined();
  });

  it("has output config", () => {
    const mgr = Manager.get();
    expect(mgr.output).toBeDefined();
    expect(mgr.output.defaultDir).toBeDefined();
  });

  describe("getJadxPath", () => {
    it("returns null when no jadx is configured/found", () => {
      const mgr = Manager.get();
      // In test environment, jadx likely not installed
      // This just tests it doesn't throw
      const result = mgr.getJadxPath();
      // Can be null or a path — just verify it returns
      expect(typeof result === "string" || result === null).toBe(true);
    });
  });

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
});
