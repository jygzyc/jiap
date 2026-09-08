import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "fs";
import * as path from "path";
import { AdbClient } from "./adb.js";
import { collectFrameworkFiles, normalizeOem } from "./framework-collector.js";
import { defaultFrameworkRoot, ensureDirectory, resolveAdbOnlyTools, resolveFrameworkTools } from "./framework-tools.js";
import { cleanFrameworkOutputs, processFrameworkFiles } from "./framework-processor.js";
import { packFrameworkJar } from "./framework-packer.js";
import { openAnalysisTarget } from "../core/launcher.js";
import { DecxError, FileError } from "../utils/errors.js";
import type {
  FrameworkArtifactRecord,
  FrameworkArtifactSummary,
  FrameworkBuildResult,
  FrameworkCollectionResult,
  FrameworkCommandOptions,
  FrameworkOem,
  FrameworkPackResult,
  FrameworkPathLayout,
  FrameworkProcessResult,
  FrameworkRunResult,
  FrameworkToolPaths,
} from "./types.js";

function frameworkRootForOem(oem: FrameworkOem, outDir?: string): string {
  if (outDir) return path.resolve(outDir);
  return path.join(defaultFrameworkRoot(), oem);
}

function sanitizeArtifactSegment(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, "_")
    .replace(/_+/g, "_")
    .replace(/^_+|_+$/g, "") || "unknown";
}

function frameworkJarPathFor(outDir: string, oem: string, vendor: string): string {
  return path.join(outDir, `framework_${sanitizeArtifactSegment(oem)}_${sanitizeArtifactSegment(vendor)}.jar`);
}

function legacyFrameworkMetadataPath(outDir: string): string {
  return path.join(outDir, ".meta.json");
}

function removeLegacyFrameworkMetadata(outDir: string): void {
  rmSync(legacyFrameworkMetadataPath(outDir), { force: true });
}

function readFrameworkArtifact(artifactPath: string): FrameworkArtifactRecord | null {
  if (!existsSync(artifactPath)) return null;
  try {
    const parsed = JSON.parse(readFileSync(artifactPath, "utf-8")) as Partial<FrameworkArtifactRecord>;
    if (
      typeof parsed.name !== "string" ||
      typeof parsed.oem !== "string" ||
      typeof parsed.vendor !== "string" ||
      typeof parsed.rootDir !== "string" ||
      typeof parsed.jarPath !== "string" ||
      typeof parsed.updatedAt !== "number"
    ) {
      return null;
    }
    const normalizedOem = sanitizeArtifactSegment(parsed.oem);
    const normalizedVendor = sanitizeArtifactSegment(parsed.vendor);
    return {
      name: `framework_${normalizedOem}_${normalizedVendor}`,
      oem: normalizedOem,
      vendor: normalizedVendor,
      rootDir: parsed.rootDir,
      jarPath: parsed.jarPath,
      updatedAt: parsed.updatedAt,
    };
  } catch {
    return null;
  }
}

function writeFrameworkArtifact(artifactPath: string, artifact: FrameworkArtifactRecord): void {
  writeFileSync(artifactPath, JSON.stringify(artifact, null, 2) + "\n", "utf-8");
}

function buildFrameworkArtifactRecord(layout: FrameworkPathLayout, oem: string, vendor: string): FrameworkArtifactRecord {
  const normalizedOem = sanitizeArtifactSegment(oem);
  const normalizedVendor = sanitizeArtifactSegment(vendor);
  return {
    name: `framework_${normalizedOem}_${normalizedVendor}`,
    oem: normalizedOem,
    vendor: normalizedVendor,
    rootDir: layout.rootDir,
    jarPath: frameworkJarPathFor(layout.outDir, normalizedOem, normalizedVendor),
    updatedAt: Date.now(),
  };
}

export function summarizeFrameworkArtifact(layout: FrameworkPathLayout, oem: string): FrameworkArtifactSummary {
  const artifact = readFrameworkArtifact(layout.artifactPath);
  const session = buildFrameworkArtifactRecord(
    layout,
    artifact?.oem ?? oem,
    artifact?.vendor ?? "unknown",
  );
  return {
    session: session.name,
    oem: session.oem,
    vendor: session.vendor,
    jarPath: session.jarPath,
  };
}

export function summarizeFrameworkJarPath(jarPath: string): FrameworkArtifactSummary | null {
  const match = path.basename(jarPath).match(/^framework_([^_]+)_(.+)\.jar$/);
  if (!match) return null;
  const oem = match[1];
  const vendor = match[2];
  return {
    session: `framework_${oem}_${vendor}`,
    oem,
    vendor,
    jarPath,
  };
}

export function resolveFrameworkLayout(options: FrameworkCommandOptions, requireOem: boolean = false): FrameworkPathLayout {
  const oem = options.oem ? normalizeOem(options.oem) : undefined;
  if (requireOem && !oem) {
    throw new Error("OEM is required");
  }

  const rootDir = frameworkRootForOem(oem ?? "google", options.outDir);
  const sourceDir = ensureDirectory(options.sourceDir ?? path.join(rootDir, "source"));
  const outDir = ensureDirectory(rootDir);
  const outTmpDir = ensureDirectory(path.join(outDir, "out_tmp"));
  const apexTmpDir = ensureDirectory(path.join(outDir, "apex_tmp"));
  const artifactPath = path.join(outDir, ".artifact.json");
  const artifact = readFrameworkArtifact(artifactPath);
  const brand = sanitizeArtifactSegment(oem ?? artifact?.oem ?? "google");
  const vendor = sanitizeArtifactSegment(artifact?.vendor ?? "unknown");

  return {
    rootDir: outDir,
    sourceDir,
    outDir,
    outTmpDir,
    apexTmpDir,
    artifactPath,
    jarPath: frameworkJarPathFor(outDir, brand, vendor),
  };
}

export async function collectFramework(
  options: FrameworkCommandOptions,
): Promise<{ oem: FrameworkOem; layout: FrameworkPathLayout; result: FrameworkCollectionResult }> {
  // Collection only pulls ready-made files (now from the /apex mount instead of
  // .apex images), so it never needs the WSL-backed image-extraction tools.
  const tools = resolveAdbOnlyTools(options.adbPath);
  const adb = new AdbClient({ adbPath: tools.adb, serial: options.serial });
  adb.ensureAvailable();
  adb.ensureDeviceConnected();
  const oem = adb.detectFrameworkOem();
  const layout = resolveFrameworkLayout({ ...options, oem }, true);
  const vendor = sanitizeArtifactSegment(adb.getProp("ro.product.model") || "unknown");
  const artifact = buildFrameworkArtifactRecord(layout, oem, vendor);
  writeFrameworkArtifact(layout.artifactPath, artifact);
  removeLegacyFrameworkMetadata(layout.outDir);
  const refreshedLayout = resolveFrameworkLayout({ ...options, oem }, true);
  const result = await collectFrameworkFiles(adb, oem, refreshedLayout.sourceDir);
  return { oem, layout: refreshedLayout, result };
}

export async function processFramework(
  options: FrameworkCommandOptions & { oem?: string },
  detectVendor: (
    opts: Pick<FrameworkCommandOptions, "adbPath" | "serial">,
  ) => Promise<string> = detectConnectedFrameworkVendor,
): Promise<{ layout: FrameworkPathLayout; result: FrameworkProcessResult }> {
  // Vendor (device model) feeds the framework_<oem>_<vendor>.jar name. When no
  // artifact recorded it yet, ask the device: a single connected device is
  // auto-selected; several devices without --serial fail fast with
  // ADB_DEVICE_AMBIGUOUS; no device at all keeps "unknown" (offline-safe).
  const preLayout = resolveFrameworkLayout(options);
  const preArtifact = readFrameworkArtifact(preLayout.artifactPath);
  if (!preArtifact || preArtifact.vendor === "unknown") {
    let vendor: string | null = null;
    try {
      vendor = await detectVendor(options);
    } catch (error) {
      if (error instanceof DecxError && error.code === "ADB_DEVICE_AMBIGUOUS") throw error;
      // No adb / no device / read failure: keep the offline default.
    }
    if (vendor && vendor !== "unknown") {
      writeFrameworkArtifact(
        preLayout.artifactPath,
        buildFrameworkArtifactRecord(preLayout, preArtifact?.oem ?? options.oem ?? "google", vendor),
      );
    }
  }
  const layout = resolveFrameworkLayout(options);
  // Image-extraction tools are resolved lazily: ext4 payload images are
  // parsed natively by the processor, so the WSL-backed toolchain
  // (debugfs / extract.erofs) is only touched when a payload actually needs
  // it — EROFS images or ext4 features the native reader does not support.
  // A source pulled from /apex (plain jars/dex) never resolves tools at all.
  let cachedTools: FrameworkToolPaths | null = null;
  const resolveTools = async (): Promise<FrameworkToolPaths> => {
    cachedTools ??= await resolveFrameworkTools(options.adbPath);
    return cachedTools;
  };
  const result = await processFrameworkFiles(layout, resolveTools);
  return { layout, result };
}

export async function packFramework(
  options: FrameworkCommandOptions & { oem?: string },
): Promise<{ layout: FrameworkPathLayout; result: FrameworkPackResult }> {
  const layout = resolveFrameworkLayout(options);
  mkdirSync(layout.outDir, { recursive: true });
  const result = await packFrameworkJar(layout);
  removeLegacyFrameworkMetadata(layout.outDir);
  cleanFrameworkOutputs(layout, options.cleanSource ?? false);
  return { layout, result };
}

export async function buildFramework(
  options: FrameworkCommandOptions & { oem?: string },
): Promise<FrameworkBuildResult> {
  const { layout, result: process } = await processFramework(options);
  mkdirSync(layout.outDir, { recursive: true });
  const pack = await packFrameworkJar(layout);
  const summary = summarizeFrameworkArtifact(layout, options.oem ?? "google");
  writeFrameworkArtifact(layout.artifactPath, {
    name: summary.session,
    oem: summary.oem,
    vendor: summary.vendor,
    rootDir: layout.rootDir,
    jarPath: frameworkJarPathFor(layout.outDir, summary.oem, summary.vendor),
    updatedAt: Date.now(),
  });
  removeLegacyFrameworkMetadata(layout.outDir);
  cleanFrameworkOutputs(layout, options.cleanSource ?? false);
  return { layout, process, pack };
}

export async function openFrameworkJar(
  jarPath: string,
  options: Pick<FrameworkCommandOptions, "name" | "port">,
): Promise<Record<string, unknown>> {
  const defaultName = options.name ?? summarizeFrameworkJarPath(jarPath)?.session;
  return openAnalysisTarget(jarPath, {
    name: defaultName,
    port: options.port,
    force: false,
    passthroughArgs: [],
  });
}

async function detectConnectedFrameworkOem(
  options: Pick<FrameworkCommandOptions, "adbPath" | "serial">,
): Promise<FrameworkOem> {
  const tools = resolveAdbOnlyTools(options.adbPath);
  const adb = new AdbClient({ adbPath: tools.adb, serial: options.serial });
  adb.ensureAvailable();
  adb.ensureDeviceConnected();
  return adb.detectFrameworkOem();
}

/** Device model for the artifact vendor segment (single-device auto-select). */
export async function detectConnectedFrameworkVendor(
  options: Pick<FrameworkCommandOptions, "adbPath" | "serial">,
): Promise<string> {
  const tools = resolveAdbOnlyTools(options.adbPath);
  const adb = new AdbClient({ adbPath: tools.adb, serial: options.serial });
  adb.ensureAvailable();
  adb.ensureDeviceConnected();
  return adb.getProp("ro.product.model") || "unknown";
}

export async function resolveFrameworkJarPath(
  explicitJar: string | undefined,
  options: Pick<FrameworkCommandOptions, "oem" | "outDir" | "sourceDir" | "adbPath" | "serial">,
  detectOem: (opts: Pick<FrameworkCommandOptions, "adbPath" | "serial">) => Promise<FrameworkOem> = detectConnectedFrameworkOem,
): Promise<string> {
  if (explicitJar) {
    return explicitJar;
  }

  const oem = options.oem ? normalizeOem(options.oem) : await detectOem(options);
  const layout = resolveFrameworkLayout({ ...options, oem }, true);
  if (!existsSync(layout.jarPath)) {
    const artifact = readFrameworkArtifact(layout.artifactPath);
    if (artifact?.jarPath && existsSync(artifact.jarPath)) {
      return artifact.jarPath;
    }
    throw new FileError(
      `No generated framework jar found for OEM '${oem}' at ${layout.jarPath}. ` +
      `Run 'decx android framework process ${oem}' or provide a jar path.`,
      layout.jarPath,
    );
  }
  return layout.jarPath;
}

export async function resolveProcessOem(
  options: FrameworkCommandOptions,
  detectOem: (opts: Pick<FrameworkCommandOptions, "adbPath" | "serial">) => Promise<FrameworkOem> = detectConnectedFrameworkOem,
): Promise<FrameworkOem> {
  // 1. Explicit positional/value wins (supports the pure-offline case).
  if (options.oem) {
    return normalizeOem(options.oem);
  }

  // 2. Reuse the OEM persisted by a prior `collect`/`run` at this output dir.
  const layout = resolveFrameworkLayout(options);
  const artifact = readFrameworkArtifact(layout.artifactPath);
  if (artifact?.oem) {
    try {
      return normalizeOem(artifact.oem);
    } catch {
      // Stored OEM is unrecognized; keep falling back.
    }
  }

  // 3. Last resort: auto-detect from a connected device.
  try {
    return await detectOem(options);
  } catch {
    // fall through to the explicit error below
  }

  throw new FileError(
    "Framework OEM could not be resolved. Pass it explicitly " +
    "(`decx android framework process <oem>`), point at a collected output with `--out-dir`, " +
    "or connect a device for auto-detection.",
  );
}

export async function runFrameworkPipeline(
  options: FrameworkCommandOptions,
): Promise<FrameworkRunResult> {
  const { oem, result: collection } = await collectFramework(options);
  const { layout, process, pack } = await buildFramework({ ...options, oem });
  const open = options.noOpen ? undefined : await openFrameworkJar(pack.jarPath, options);
  return { layout, collection, process, pack, open };
}
