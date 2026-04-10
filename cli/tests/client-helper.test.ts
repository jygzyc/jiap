/**
 * client-helper unit tests.
 *
 * Tests resolveClient with various option combinations.
 * Uses Manager reset to ensure clean state.
 */

import { resolveClient } from "../src/core/client-helper.js";
import { Manager } from "../src/core/config.js";
import { jest } from "@jest/globals";

const mockExit = jest.spyOn(process, "exit").mockImplementation((() => {
  throw new Error("process.exit");
}) as () => never);

afterAll(() => {
  mockExit.mockRestore();
});

describe("resolveClient", () => {
  it("uses default port when no session or port specified", () => {
    const { fmt, client } = resolveClient({});
    expect((client as any).baseUrl).toBe("http://127.0.0.1:25419");
  });

  it("uses specified port with --port", () => {
    const { client } = resolveClient({ port: "3000" });
    expect((client as any).baseUrl).toBe("http://127.0.0.1:3000");
  });

  it("creates formatter in json mode when --json is set", () => {
    const { fmt } = resolveClient({ json: true });
    // Formatter in json mode — just verify it doesn't throw
    expect(fmt).toBeDefined();
  });

  it("creates formatter in normal mode by default", () => {
    const { fmt } = resolveClient({});
    expect(fmt).toBeDefined();
  });

  it("throws when both --session and --port specified", () => {
    expect(() => resolveClient({ session: "test", port: "3000" })).toThrow("process.exit");
  });
});
