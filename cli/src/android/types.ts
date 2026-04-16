export type FrameworkOem = "vivo" | "oppo" | "xiaomi" | "honor" | "google";

export interface AdbCommandResult {
  stdout: string;
  stderr: string;
  status: number | null;
}

export interface AdbClientOptions {
  adbPath?: string;
  serial?: string;
}

export interface FrameworkToolPaths {
  adb: string;
  debugfs: string;
  erofsExtractor: string;
}

export interface FrameworkPathLayout {
  rootDir: string;
  sourceDir: string;
  outDir: string;
  outTmpDir: string;
  apexTmpDir: string;
  metadataPath: string;
  artifactPath: string;
  jarPath: string;
}

export interface FrameworkCommandOptions {
  oem?: string;
  sourceDir?: string;
  outDir?: string;
  adbPath?: string;
  serial?: string;
  cleanSource?: boolean;
  port?: string;
  name?: string;
  noOpen?: boolean;
}

export interface FrameworkCollectionResult {
  scanned: number;
  pulled: number;
  failed: number;
  files: string[];
  failures: Array<{ path: string; error: string }>;
}

export interface FrameworkProcessResult {
  processed: number;
  failed: number;
  outputs: string[];
  failures: Array<{ path: string; error: string }>;
}

export interface FrameworkPackResult {
  ok: boolean;
  jarPath: string;
  fileCount: number;
}

export interface FrameworkRunResult {
  layout: FrameworkPathLayout;
  collection?: FrameworkCollectionResult;
  process: FrameworkProcessResult;
  pack: FrameworkPackResult;
  open?: Record<string, unknown>;
}

export interface FrameworkBuildResult {
  layout: FrameworkPathLayout;
  process: FrameworkProcessResult;
  pack: FrameworkPackResult;
}

export interface FrameworkArtifactMetadata {
  vendor: string;
}

export interface FrameworkArtifactSummary {
  session: string;
  oem: string;
  vendor: string;
  jarPath: string;
}

export interface FrameworkArtifactRecord {
  name: string;
  oem: string;
  vendor: string;
  rootDir: string;
  jarPath: string;
  updatedAt: number;
}

export interface ToolCheckResult {
  ok: boolean;
  path: string | null;
  detail: string;
}

export interface FrameworkToolsCheck {
  adb: ToolCheckResult;
  debugfs: ToolCheckResult;
  erofsExtractor: ToolCheckResult;
}
