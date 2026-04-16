import { existsSync, mkdirSync, readFileSync, writeFileSync } from "fs";
import * as path from "path";
import { AdbClient } from "./adb.js";
import { collectFrameworkFiles, normalizeOem } from "./framework-collector.js";
import { defaultFrameworkRoot, ensureDirectory, resolveFrameworkTools } from "./framework-tools.js";
import { cleanFrameworkOutputs, processFrameworkFiles } from "./framework-processor.js";
import { packFrameworkJar } from "./framework-packer.js";
import { openAnalysisTarget } from "../commands/process.js";
import { FileError } from "../utils/errors.js";
import type {
  FrameworkArtifactMetadata,
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

function readFrameworkMetadata(metadataPath: string): FrameworkArtifactMetadata | null {
  if (!existsSync(metadataPath)) return null;
  try {
    const parsed = JSON.parse(readFileSync(metadataPath, "utf-8")) as Partial<FrameworkArtifactMetadata>;
    if (typeof parsed.vendor !== "string") return null;
    return {
      vendor: sanitizeArtifactSegment(parsed.vendor),
    };
  } catch {
    return null;
  }
}

function writeFrameworkMetadata(metadataPath: string, metadata: FrameworkArtifactMetadata): void {
  writeFileSync(metadataPath, JSON.stringify(metadata, null, 2) + "\n", "utf-8");
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
    return {
      name: parsed.name,
      oem: parsed.oem,
      vendor: parsed.vendor,
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
    jarPath: layout.jarPath,
    updatedAt: Date.now(),
  };
}

function resolveArtifactIdentity(metadataPath: string): FrameworkArtifactMetadata {
  const metadata = readFrameworkMetadata(metadataPath);
  return {
    vendor: sanitizeArtifactSegment(metadata?.vendor ?? "unknown"),
  };
}

export function summarizeFrameworkArtifact(layout: FrameworkPathLayout, oem: string): FrameworkArtifactSummary {
  const identity = resolveArtifactIdentity(layout.metadataPath);
  const session = buildFrameworkArtifactRecord(layout, oem, identity.vendor);
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
  const metadataPath = path.join(outDir, ".meta.json");
  const artifactPath = path.join(outDir, ".artifact.json");
  const identity = resolveArtifactIdentity(metadataPath);
  const brand = sanitizeArtifactSegment(oem ?? "google");

  return {
    rootDir: outDir,
    sourceDir,
    outDir,
    outTmpDir,
    apexTmpDir,
    metadataPath,
    artifactPath,
    jarPath: path.join(outDir, `framework_${brand}_${identity.vendor}.jar`),
  };
}

export async function collectFramework(
  options: FrameworkCommandOptions,
): Promise<{ oem: FrameworkOem; layout: FrameworkPathLayout; result: FrameworkCollectionResult }> {
  const tools = resolveFrameworkTools(options.adbPath);
  const adb = new AdbClient({ adbPath: tools.adb, serial: options.serial });
  adb.ensureAvailable();
  adb.ensureDeviceConnected();
  const oem = adb.detectFrameworkOem();
  const baseLayout = resolveFrameworkLayout({ ...options, oem }, true);
  const vendorFromDevice = adb.getProp("ro.product.model");
  writeFrameworkMetadata(baseLayout.metadataPath, {
    vendor: sanitizeArtifactSegment(vendorFromDevice || "unknown"),
  });
  const layout = resolveFrameworkLayout({ ...options, oem }, true);
  const artifact = buildFrameworkArtifactRecord(layout, oem, sanitizeArtifactSegment(vendorFromDevice || "unknown"));
  writeFrameworkArtifact(layout.artifactPath, artifact);
  const result = await collectFrameworkFiles(adb, oem, layout.sourceDir);
  return { oem, layout, result };
}

export async function processFramework(
  options: FrameworkCommandOptions & { oem?: string },
): Promise<{ layout: FrameworkPathLayout; result: FrameworkProcessResult }> {
  const layout = resolveFrameworkLayout(options);
  const tools = resolveFrameworkTools(options.adbPath);
  const result = await processFrameworkFiles(layout, tools);
  return { layout, result };
}

export async function packFramework(
  options: FrameworkCommandOptions & { oem?: string },
): Promise<{ layout: FrameworkPathLayout; result: FrameworkPackResult }> {
  const layout = resolveFrameworkLayout(options);
  mkdirSync(layout.outDir, { recursive: true });
  const result = await packFrameworkJar(layout);
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
    jarPath: layout.jarPath,
    updatedAt: Date.now(),
  });
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
  const tools = resolveFrameworkTools(options.adbPath);
  const adb = new AdbClient({ adbPath: tools.adb, serial: options.serial });
  adb.ensureAvailable();
  adb.ensureDeviceConnected();
  return adb.detectFrameworkOem();
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
      `Run 'decx ard framework process ${oem}' or provide a jar path.`,
      layout.jarPath,
    );
  }
  return layout.jarPath;
}

export async function runFrameworkPipeline(
  options: FrameworkCommandOptions,
): Promise<FrameworkRunResult> {
  const { oem, result: collection } = await collectFramework(options);
  const { layout, process, pack } = await buildFramework({ ...options, oem });
  const open = options.noOpen ? undefined : await openFrameworkJar(pack.jarPath, options);
  return { layout, collection, process, pack, open };
}
