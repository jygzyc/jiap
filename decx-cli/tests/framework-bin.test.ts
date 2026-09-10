/**
 * Tests for packaged native-tool extraction into DECX_HOME/bin.
 *
 * The archive content hash gates re-extraction via a marker file: matching
 * marker + existing target skips extraction, mismatch (CLI upgrade) removes
 * the archive's previous top-level dirs and re-extracts.
 */
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "fs";
import * as path from "path";
import { spawnSync } from "child_process";
import { extractPackagedBinArchiveTo } from "../src/android/framework-tools.js";
import { resetTestDir } from "./test-paths.js";

function tarBin(): string {
  return process.platform === "win32" && existsSync("C:\\Windows\\System32\\tar.exe")
    ? "C:\\Windows\\System32\\tar.exe"
    : "tar";
}

/** Build a real tar.gz fixture containing `dirs` from workDir. */
function makeArchive(archivePath: string, workDir: string, dirs: string[]): void {
  mkdirSync(workDir, { recursive: true });
  const result = spawnSync(tarBin(), ["-czf", archivePath, ...dirs], {
    cwd: workDir,
    encoding: "utf-8",
  });
  expect(result.status).toBe(0);
}

describe("packaged bin archive extraction", () => {
  it("extracts tools into the target bin dir and writes the hash marker", () => {
    const rootDir = resetTestDir("tmp", "framework-bin-extract");
    const workDir = path.join(rootDir, "staging");
    const binDir = path.join(rootDir, "home", "bin");
    mkdirSync(path.join(workDir, "linux", "x86_64"), { recursive: true });
    writeFileSync(path.join(workDir, "linux", "x86_64", "extract.erofs"), "tool-v1");
    const archive = path.join(rootDir, "bin.tar.gz");
    makeArchive(archive, workDir, ["linux"]);

    const target = extractPackagedBinArchiveTo(archive, binDir, path.join("linux", "x86_64", "extract.erofs"));
    expect(target).toBe(path.join(binDir, "linux", "x86_64", "extract.erofs"));
    expect(existsSync(target)).toBe(true);
    expect(readFileSync(target, "utf-8")).toBe("tool-v1");
    expect(existsSync(path.join(binDir, ".native-tools.sha256"))).toBe(true);

    rmSync(rootDir, { recursive: true, force: true });
  });

  it("skips re-extraction while the marker matches", () => {
    const rootDir = resetTestDir("tmp", "framework-bin-skip");
    const workDir = path.join(rootDir, "staging");
    const binDir = path.join(rootDir, "home", "bin");
    mkdirSync(path.join(workDir, "linux", "x86_64"), { recursive: true });
    writeFileSync(path.join(workDir, "linux", "x86_64", "extract.erofs"), "tool");
    writeFileSync(path.join(workDir, "linux", "x86_64", "fsck.erofs"), "sibling");
    const archive = path.join(rootDir, "bin.tar.gz");
    makeArchive(archive, workDir, ["linux"]);

    extractPackagedBinArchiveTo(archive, binDir, path.join("linux", "x86_64", "extract.erofs"));
    const sibling = path.join(binDir, "linux", "x86_64", "fsck.erofs");
    rmSync(sibling);
    extractPackagedBinArchiveTo(archive, binDir, path.join("linux", "x86_64", "extract.erofs"));
    // Sibling would have been restored by a re-extraction; still missing proves the skip.
    expect(existsSync(sibling)).toBe(false);

    rmSync(rootDir, { recursive: true, force: true });
  });

  it("cleans stale dirs and re-extracts when the archive hash changes", () => {
    const rootDir = resetTestDir("tmp", "framework-bin-upgrade");
    const workDir = path.join(rootDir, "staging");
    const binDir = path.join(rootDir, "home", "bin");
    const archive = path.join(rootDir, "bin.tar.gz");

    mkdirSync(path.join(workDir, "linux", "x86_64"), { recursive: true });
    writeFileSync(path.join(workDir, "linux", "x86_64", "extract.erofs"), "tool-v1");
    makeArchive(archive, workDir, ["linux"]);
    extractPackagedBinArchiveTo(archive, binDir, path.join("linux", "x86_64", "extract.erofs"));
    expect(readFileSync(path.join(binDir, "linux", "x86_64", "extract.erofs"), "utf-8")).toBe("tool-v1");

    // Simulate an upgrade: new archive content plus a stale leftover file.
    rmSync(workDir, { recursive: true, force: true });
    mkdirSync(path.join(workDir, "linux", "x86_64"), { recursive: true });
    writeFileSync(path.join(workDir, "linux", "x86_64", "extract.erofs"), "tool-v2");
    rmSync(archive);
    makeArchive(archive, workDir, ["linux"]);
    writeFileSync(path.join(binDir, "linux", "x86_64", "stale.bin"), "stale");

    extractPackagedBinArchiveTo(archive, binDir, path.join("linux", "x86_64", "extract.erofs"));
    expect(readFileSync(path.join(binDir, "linux", "x86_64", "extract.erofs"), "utf-8")).toBe("tool-v2");
    expect(existsSync(path.join(binDir, "linux", "x86_64", "stale.bin"))).toBe(false);

    rmSync(rootDir, { recursive: true, force: true });
  });

  it("leaves unrelated files in the bin dir (e.g. decx-server.jar) untouched", () => {
    const rootDir = resetTestDir("tmp", "framework-bin-coexist");
    const workDir = path.join(rootDir, "staging");
    const binDir = path.join(rootDir, "home", "bin");
    mkdirSync(path.join(workDir, "linux", "x86_64"), { recursive: true });
    writeFileSync(path.join(workDir, "linux", "x86_64", "extract.erofs"), "tool");
    mkdirSync(binDir, { recursive: true });
    writeFileSync(path.join(binDir, "decx-server.jar"), "server");
    const archive = path.join(rootDir, "bin.tar.gz");
    makeArchive(archive, workDir, ["linux"]);

    extractPackagedBinArchiveTo(archive, binDir, path.join("linux", "x86_64", "extract.erofs"));
    expect(existsSync(path.join(binDir, "decx-server.jar"))).toBe(true);

    rmSync(rootDir, { recursive: true, force: true });
  });
});
