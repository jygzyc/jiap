import { existsSync, mkdirSync, readdirSync } from "fs";
import * as path from "path";
import { AdbClient } from "./adb.js";
import type { FrameworkCollectionResult, FrameworkOem } from "./types.js";

const DEFAULT_FRAMEWORK_DIRS = [
  "/system/framework",
  // Runtime APEX mount point: activated modules expose their payload already
  // extracted (javalib jars are ready-made files), so no image unpacking is
  // needed for anything collected here.
  "/apex",
  "/vendor/framework",
  "/system_ext/framework",
];

// OEM-specific primary collection directories; every OEM not listed here falls
// back to DEFAULT_FRAMEWORK_DIRS.
const OEM_DIRS: Partial<Record<FrameworkOem, string[]>> = {
  oppo: ["/system/framework", "/apex", "/system_ext/framework"],
  xiaomi: ["/system/framework", "/apex", "/system_ext/framework", "/vendor/framework"],
};

// Fallback tier: .apex/.capex images from these device directories are pulled
// (and later unpacked with debugfs/erofs) only for modules the /apex mount did
// not already provide.
const APEX_IMAGE_FALLBACK_DIRS = ["/system/apex"];

const PRIMARY_FILE_TYPES = [".apk", ".jar", ".dex"];
const APEX_IMAGE_FILE_TYPES = [".apex", ".capex"];

const SUPPORTED_FRAMEWORK_OEMS: readonly FrameworkOem[] = [
  "vivo",
  "oppo",
  "xiaomi",
  "honor",
  "google",
  "samsung",
];

function isFrameworkOem(value: string): value is FrameworkOem {
  return (SUPPORTED_FRAMEWORK_OEMS as readonly string[]).includes(value);
}

export function normalizeOem(value: string): FrameworkOem {
  const lowered = value.toLowerCase();
  if (!isFrameworkOem(lowered)) {
    throw new Error(`Unsupported OEM '${value}'. Supported: ${SUPPORTED_FRAMEWORK_OEMS.join(", ")}`);
  }
  return lowered;
}

function buildFindCommand(searchPaths: string[], fileTypes: string[]): string {
  const nameConditions = fileTypes.map((fileType) => `-name '*${fileType}'`).join(" -o ");
  return searchPaths.map((searchPath) => `find ${searchPath} -type f \\( ${nameConditions} \\)`).join(" ; ");
}

function filterScanOutput(output: string): string[] {
  return output
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.length > 0)
    .filter((line) => !line.includes("Permission denied") && !line.includes("No such file"));
}

export function getOemSearchPaths(oem: FrameworkOem): string[] {
  return OEM_DIRS[oem] ?? DEFAULT_FRAMEWORK_DIRS;
}

/** Module name of a remote path inside the /apex mount, with @version stripped. */
function apexMountModule(remotePath: string): string | null {
  const segments = remotePath.split("/").filter((segment) => segment.length > 0);
  const apexIndex = segments.indexOf("apex");
  if (apexIndex === -1 || segments.length < apexIndex + 2) return null;
  const moduleName = segments[apexIndex + 1].split("@")[0];
  return moduleName.length > 0 ? moduleName : null;
}

/** Module name of a fallback-tier image file (com.android.art.apex → com.android.art). */
function apexImageModule(remotePath: string): string | null {
  const extension = path.extname(remotePath).toLowerCase();
  if (extension !== ".apex" && extension !== ".capex") return null;
  return path.basename(remotePath, extension).split("@")[0] || null;
}

/** Modules already collected under <sourceDir>/apex/<module>[@version]/ with content. */
function collectedApexModules(sourceDir: string): Set<string> {
  const covered = new Set<string>();
  const apexDir = path.join(sourceDir, "apex");
  if (!existsSync(apexDir)) return covered;
  for (const entry of readdirSync(apexDir, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const moduleDir = path.join(apexDir, entry.name);
    // A directory left behind by a failed pull holds no content — do not count it.
    if (readdirSync(moduleDir).length === 0) continue;
    covered.add(entry.name.split("@")[0]);
  }
  return covered;
}

/**
 * Tiered framework collection:
 *
 * 1. Ready-made files — framework directories plus the runtime `/apex` mount,
 *    whose activated modules expose already-extracted jars.
 * 2. `.apex`/`.capex` images from `/system/apex`, pulled (and later unpacked
 *    with debugfs/erofs) only for modules tier 1 did not already cover.
 */
export async function collectFrameworkFiles(
  adb: AdbClient,
  oem: FrameworkOem,
  sourceDir: string,
): Promise<FrameworkCollectionResult> {
  const files: string[] = [];
  const failures: Array<{ path: string; error: string }> = [];
  const covered = collectedApexModules(sourceDir);

  const pullFile = (remotePath: string): void => {
    const localPath = path.join(sourceDir, remotePath.replace(/^\/+/, ""));
    mkdirSync(path.dirname(localPath), { recursive: true });
    try {
      adb.pull(remotePath, localPath);
      files.push(localPath);
      const mountModule = apexMountModule(remotePath);
      if (mountModule) covered.add(mountModule);
    } catch (error) {
      failures.push({
        path: remotePath,
        error: error instanceof Error ? error.message : String(error),
      });
    }
  };

  // Tier 1 — ready-made files: framework dirs plus the /apex mount content.
  const primaryFiles = filterScanOutput(
    adb.shell(buildFindCommand(getOemSearchPaths(oem), PRIMARY_FILE_TYPES)),
  );
  for (const remotePath of primaryFiles) {
    pullFile(remotePath);
  }

  // Tier 2 — .apex/.capex images, only for modules the mount did not cover.
  const imageFiles = filterScanOutput(
    adb.shell(buildFindCommand(APEX_IMAGE_FALLBACK_DIRS, APEX_IMAGE_FILE_TYPES)),
  );
  let skippedCoveredModules = 0;
  for (const remotePath of imageFiles) {
    const imageModule = apexImageModule(remotePath);
    if (imageModule && covered.has(imageModule)) {
      skippedCoveredModules += 1;
      continue;
    }
    pullFile(remotePath);
  }

  return {
    scanned: primaryFiles.length + imageFiles.length,
    pulled: files.length,
    failed: failures.length,
    files,
    failures,
    skippedCoveredModules,
  };
}
