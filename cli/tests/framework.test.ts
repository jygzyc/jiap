import { existsSync, mkdirSync, rmSync, writeFileSync } from "fs";
import * as path from "path";
import { cleanFrameworkOutputs, cleanFrameworkTempDirs } from "../src/android/framework-processor.js";
import { normalizeOem } from "../src/android/framework-collector.js";
import { resolveFrameworkJarPath, resolveFrameworkLayout, summarizeFrameworkArtifact } from "../src/android/framework.js";
import { resolveFrameworkTools } from "../src/android/framework-tools.js";

describe("framework collector", () => {
  it("normalizes supported OEM names", () => {
    expect(normalizeOem("XIAOMI")).toBe("xiaomi");
    expect(normalizeOem("google")).toBe("google");
  });

  it("rejects unsupported OEM names", () => {
    expect(() => normalizeOem("samsung")).toThrow("Unsupported OEM");
  });
});

describe("framework layout", () => {
  it("derives source, temp, and jar paths from out-dir", () => {
    const layout = resolveFrameworkLayout({ outDir: "/tmp/decx-fw" });
    expect(layout.rootDir).toBe("/tmp/decx-fw");
    expect(layout.sourceDir).toBe(path.join("/tmp/decx-fw", "source"));
    expect(layout.outTmpDir).toBe(path.join("/tmp/decx-fw", "out_tmp"));
    expect(layout.apexTmpDir).toBe(path.join("/tmp/decx-fw", "apex_tmp"));
    expect(layout.metadataPath).toBe(path.join("/tmp/decx-fw", ".meta.json"));
    expect(layout.artifactPath).toBe(path.join("/tmp/decx-fw", ".artifact.json"));
    expect(layout.jarPath).toBe(path.join("/tmp/decx-fw", "framework_google_unknown.jar"));
  });

  it("uses oem as brand and defaults vendor to unknown without metadata", () => {
    const layout = resolveFrameworkLayout({
      oem: "xiaomi",
      outDir: "/tmp/decx-fw",
    });
    expect(layout.jarPath).toBe(path.join("/tmp/decx-fw", "framework_xiaomi_unknown.jar"));
  });

  it("uses oem as brand and reuses vendor from persisted metadata", () => {
    const outDir = "/tmp/decx-fw-meta";
    mkdirSync(outDir, { recursive: true });
    writeFileSync(
      path.join(outDir, ".meta.json"),
      JSON.stringify({ vendor: "K70 Ultra" }),
      "utf-8",
    );

    const layout = resolveFrameworkLayout({ oem: "xiaomi", outDir });
    expect(layout.jarPath).toBe(path.join(outDir, "framework_xiaomi_k70_ultra.jar"));

    rmSync(outDir, { recursive: true, force: true });
  });

  it("builds an artifact summary with oem, vendor, and jar path", () => {
    const outDir = "/tmp/decx-fw-summary";
    mkdirSync(outDir, { recursive: true });
    writeFileSync(
      path.join(outDir, ".meta.json"),
      JSON.stringify({ vendor: "K70 Ultra" }),
      "utf-8",
    );
    const layout = resolveFrameworkLayout({ oem: "xiaomi", outDir });

    expect(summarizeFrameworkArtifact(layout, "xiaomi")).toEqual({
      session: "framework_xiaomi_k70_ultra",
      oem: "xiaomi",
      vendor: "k70_ultra",
      jarPath: path.join(outDir, "framework_xiaomi_k70_ultra.jar"),
    });

    rmSync(outDir, { recursive: true, force: true });
  });

  it("resolves an explicit framework jar path without device detection", async () => {
    await expect(resolveFrameworkJarPath("/tmp/custom.jar", {})).resolves.toBe("/tmp/custom.jar");
  });

  it("resolves the generated framework jar using detected device oem", async () => {
    const outDir = "/tmp/decx-fw-open";
    mkdirSync(outDir, { recursive: true });
    writeFileSync(path.join(outDir, ".meta.json"), JSON.stringify({ vendor: "K70 Ultra" }), "utf-8");
    const expectedJar = path.join(outDir, "framework_xiaomi_k70_ultra.jar");
    writeFileSync(expectedJar, "", "utf-8");

    await expect(
      resolveFrameworkJarPath(undefined, { outDir }, async () => "xiaomi"),
    ).resolves.toBe(expectedJar);

    rmSync(outDir, { recursive: true, force: true });
  });

  it("falls back to the persisted artifact record when the computed jar path changed", async () => {
    const outDir = "/tmp/decx-fw-artifact-fallback";
    mkdirSync(outDir, { recursive: true });
    writeFileSync(path.join(outDir, ".meta.json"), JSON.stringify({ vendor: "unknown" }), "utf-8");
    writeFileSync(
      path.join(outDir, ".artifact.json"),
      JSON.stringify({
        name: "framework_xiaomi_k70_ultra",
        oem: "xiaomi",
        vendor: "k70_ultra",
        rootDir: outDir,
        jarPath: path.join(outDir, "framework_xiaomi_k70_ultra.jar"),
        updatedAt: Date.now(),
      }),
      "utf-8",
    );
    const expectedJar = path.join(outDir, "framework_xiaomi_k70_ultra.jar");
    writeFileSync(expectedJar, "", "utf-8");

    await expect(
      resolveFrameworkJarPath(undefined, { outDir }, async () => "xiaomi"),
    ).resolves.toBe(expectedJar);

    rmSync(outDir, { recursive: true, force: true });
  });

  it("fails with a clear error when no generated framework jar exists", async () => {
    const outDir = "/tmp/decx-fw-open-missing";
    mkdirSync(outDir, { recursive: true });

    await expect(
      resolveFrameworkJarPath(undefined, { outDir }, async () => "xiaomi"),
    ).rejects.toThrow("No generated framework jar found for OEM 'xiaomi'");

    rmSync(outDir, { recursive: true, force: true });
  });

  it("removes all tmp directories after packing cleanup", () => {
    const outDir = "/tmp/decx-fw-clean-pack";
    const layout = resolveFrameworkLayout({ outDir });
    mkdirSync(layout.outTmpDir, { recursive: true });
    mkdirSync(layout.apexTmpDir, { recursive: true });

    cleanFrameworkTempDirs(layout);

    expect(existsSync(layout.outTmpDir)).toBe(false);
    expect(existsSync(layout.apexTmpDir)).toBe(false);
    rmSync(outDir, { recursive: true, force: true });
  });

  it("optionally removes source during final cleanup", () => {
    const outDir = "/tmp/decx-fw-clean-source";
    const layout = resolveFrameworkLayout({ outDir });
    mkdirSync(layout.sourceDir, { recursive: true });
    mkdirSync(layout.outTmpDir, { recursive: true });
    mkdirSync(layout.apexTmpDir, { recursive: true });

    cleanFrameworkOutputs(layout, true);

    expect(existsSync(layout.sourceDir)).toBe(false);
    expect(existsSync(layout.outTmpDir)).toBe(false);
    expect(existsSync(layout.apexTmpDir)).toBe(false);
    rmSync(outDir, { recursive: true, force: true });
  });
});

describe("framework platform support", () => {
  const originalPlatform = process.platform;

  afterEach(() => {
    Object.defineProperty(process, "platform", { value: originalPlatform });
  });

  it("shows an explicit unsupported message on Windows", () => {
    Object.defineProperty(process, "platform", { value: "win32" });
    expect(() => resolveFrameworkTools()).toThrow("Windows is not supported");
  });
});
