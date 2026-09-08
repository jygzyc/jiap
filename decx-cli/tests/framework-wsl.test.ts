/**
 * WSL tool-resolution tests for framework-tools.
 *
 * Simulates the WSL relay by mocking child_process.spawnSync: the module
 * resolves debugfs/erofs tools through `wsl.exe -e sh -c "command -v ..."`
 * and must exec the resolved ABSOLUTE path, because bare-name exec through
 * the relay fails for /usr/sbin binaries on some WSL builds
 * ("execvpe(debugfs) failed: No such file or directory").
 */
import { jest } from "@jest/globals";

const spawnSyncMock = jest.fn<(...args: unknown[]) => { status: number; stdout: string; stderr: string }>();

jest.unstable_mockModule("child_process", () => ({
  spawnSync: (...args: unknown[]) => spawnSyncMock(...args),
  execFileSync: jest.fn(),
}));

const { resolveAdbOnlyTools, resolveFrameworkTools } = await import("../src/android/framework-tools.js");

interface SpawnResult {
  status: number;
  stdout: string;
  stderr: string;
}

function ok(stdout = ""): SpawnResult {
  return { status: 0, stdout, stderr: "" };
}

function fail(stderr = ""): SpawnResult {
  return { status: 1, stdout: "", stderr };
}

/** Stub the spawn calls resolveFrameworkTools makes on win32. */
function stubSpawn(responses: {
  whereDebugfs?: SpawnResult;
  whereAdb?: SpawnResult;
  wslProbe?: SpawnResult;
  commandVDebugfs?: SpawnResult;
  commandVFsckErofs?: SpawnResult;
  commandVExtractErofs?: SpawnResult;
}): void {
  spawnSyncMock.mockImplementation(((cmd: string, args: string[]) => {
    const joined = [cmd, ...(args ?? [])].join(" ");
    if (joined === "where debugfs") return responses.whereDebugfs ?? fail();
    if (joined === "where adb") return responses.whereAdb ?? ok("C:\\platform-tools\\adb.exe\n");
    if (joined === "wsl.exe -e sh -c exit 0") return responses.wslProbe ?? ok();
    if (joined === "wsl.exe -e sh -c command -v debugfs") return responses.commandVDebugfs ?? fail();
    if (joined === "wsl.exe -e sh -c command -v fsck.erofs") return responses.commandVFsckErofs ?? fail();
    if (joined === "wsl.exe -e sh -c command -v extract.erofs") return responses.commandVExtractErofs ?? fail();
    return fail();
  }) as unknown as typeof spawnSyncMock);
}

describe("framework WSL tool resolution", () => {
  const originalPlatform = process.platform;

  beforeEach(() => {
    spawnSyncMock.mockReset();
    Object.defineProperty(process, "platform", { value: "win32" });
  });

  afterEach(() => {
    Object.defineProperty(process, "platform", { value: originalPlatform });
  });

  it("execs the absolute WSL path for debugfs, not the bare name", () => {
    stubSpawn({ commandVDebugfs: ok("/usr/sbin/debugfs\n") });
    const tools = resolveFrameworkTools(undefined, { wslAvailable: true });
    expect(tools.debugfs.argv).toEqual(["wsl.exe", "-e", "/usr/sbin/debugfs"]);
    expect(tools.debugfs.translatePaths).toBe(true);
  });

  it("keeps absolute paths for WSL-resolved erofs tools", () => {
    stubSpawn({
      commandVDebugfs: ok("/usr/sbin/debugfs\n"),
      commandVFsckErofs: ok("/usr/bin/fsck.erofs\n"),
    });
    const tools = resolveFrameworkTools(undefined, { wslAvailable: true });
    expect(tools.erofsExtractor.argv).toEqual(["wsl.exe", "-e", "/usr/bin/fsck.erofs"]);
    expect(tools.erofsExtractor.translatePaths).toBe(true);
  });

  it("falls back to extract.erofs when fsck.erofs is missing", () => {
    stubSpawn({
      commandVDebugfs: ok("/usr/sbin/debugfs\n"),
      commandVExtractErofs: ok("/usr/local/bin/extract.erofs\n"),
    });
    const tools = resolveFrameworkTools(undefined, { wslAvailable: true });
    expect(tools.erofsExtractor.argv).toEqual(["wsl.exe", "-e", "/usr/local/bin/extract.erofs"]);
  });

  it("rejects non-absolute command -v output instead of exec'ing a bare name", () => {
    stubSpawn({ commandVDebugfs: ok("debugfs\n") });
    expect(() => resolveFrameworkTools(undefined, { wslAvailable: true })).toThrow(/debugfs not found/);
  });

  it("throws e2fsprogs guidance when WSL has no debugfs", () => {
    stubSpawn({ commandVDebugfs: fail() });
    expect(() => resolveFrameworkTools(undefined, { wslAvailable: true })).toThrow(/apt install e2fsprogs/);
  });

  it("falls back to the packaged extract.erofs via WSL when WSL lacks erofs-utils", () => {
    stubSpawn({ commandVDebugfs: ok("/usr/sbin/debugfs\n") });
    const tools = resolveFrameworkTools(undefined, { wslAvailable: true });
    expect(tools.erofsExtractor.argv[0]).toBe("wsl.exe");
    const last = tools.erofsExtractor.argv[tools.erofsExtractor.argv.length - 1];
    expect(last).toMatch(/^\/mnt\/[a-z]\/.+extract\.erofs$/);
    expect(tools.erofsExtractor.translatePaths).toBe(true);
  });

  it("resolves adb-only tools without requiring WSL or image tools", () => {
    stubSpawn({ whereAdb: ok("C:\\platform-tools\\adb.exe\n") });
    const tools = resolveAdbOnlyTools(undefined);
    expect(tools.adb).toBe("adb");
    expect(tools.debugfs.argv).toEqual([]);
    expect(tools.erofsExtractor.argv).toEqual([]);
  });
});
