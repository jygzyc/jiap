/**
 * decx-server.jar finder and installer.
 */

import * as path from "path";
import * as os from "os";
import { existsSync, mkdirSync, renameSync, unlinkSync } from "fs";
import { downloadWithProgress } from "../utils/progress.js";

const DECX_SERVER_HOME: string | undefined = process.env.DECX_SERVER_HOME;

/**
 * Compare two semver strings (e.g. "2.2.1" vs "2.3.0").
 * Returns >0 if a > b, <0 if a < b, 0 if equal.
 */
function compareSemver(a: string, b: string): number {
  const pa = a.split("-")[0].split(".").map(Number);
  const pb = b.split("-")[0].split(".").map(Number);
  for (let i = 0; i < 3; i++) {
    const va = pa[i] ?? 0;
    const vb = pb[i] ?? 0;
    if (va !== vb) return va - vb;
  }
  return 0;
}

const INSTALL_DIR = path.join(os.homedir(), ".decx", "bin");
const INSTALL_PATH = path.join(INSTALL_DIR, "decx-server.jar");

/**
 * Find decx-server.jar from known locations.
 * Priority: DECX_SERVER_HOME env > ~/.decx/bin/decx-server.jar
 */
export function findDecxServerJar(): string | null {
  if (DECX_SERVER_HOME) {
    if (DECX_SERVER_HOME.endsWith(".jar") && existsSync(DECX_SERVER_HOME)) return DECX_SERVER_HOME;
    const fromDir = path.join(DECX_SERVER_HOME, "decx-server.jar");
    if (existsSync(fromDir)) return fromDir;
  }

  if (existsSync(INSTALL_PATH)) return INSTALL_PATH;

  return null;
}

/**
 * Check if a newer server version is available.
 */
export async function checkForServerUpdate(
  currentVersion: string,
  prerelease: boolean = false
): Promise<{ available: boolean; latestVersion: string }> {
  const endpoint = prerelease
    ? "https://api.github.com/repos/jygzyc/decx/releases?per_page=10"
    : "https://api.github.com/repos/jygzyc/decx/releases/latest";

  const res = await fetch(endpoint, {
    headers: { "Accept": "application/vnd.github+json" },
  });
  if (!res.ok) return { available: false, latestVersion: currentVersion };

  let latestTag: string;
  if (prerelease) {
    const releases = await res.json() as Array<{ tag_name: string; prerelease: boolean }>;
    const pre = releases.find((r) => r.prerelease);
    if (!pre) return { available: false, latestVersion: currentVersion };
    latestTag = pre.tag_name;
  } else {
    const release = await res.json() as { tag_name: string };
    latestTag = release.tag_name;
  }

  const latest = latestTag.replace(/^v/, "");
  const current = currentVersion.replace(/^v/, "");
  const available = compareSemver(latest, current) > 0;
  return { available, latestVersion: latest };
}

/**
 * Download and install the latest decx-server.jar from GitHub releases.
 * Returns [success, message, version?].
 */
export async function installDecxServer(
  prerelease: boolean = false
): Promise<[boolean, string, string?]> {
  try {
    console.error(`  Fetching latest ${prerelease ? "prerelease" : "release"} info from GitHub...`);

    const endpoint = prerelease
      ? "https://api.github.com/repos/jygzyc/decx/releases?per_page=10"
      : "https://api.github.com/repos/jygzyc/decx/releases/latest";

    const res = await fetch(endpoint, {
      headers: { "Accept": "application/vnd.github+json" },
    });
    if (!res.ok) {
      return [false, `GitHub API error: HTTP ${res.status}`];
    }

    let release: { tag_name: string; assets: Array<{ name: string; browser_download_url: string }> };

    if (prerelease) {
      const releases = await res.json() as Array<{ tag_name: string; prerelease: boolean; assets: Array<{ name: string; browser_download_url: string }> }>;
      const pre = releases.find((r) => r.prerelease);
      if (!pre) {
        return [false, "No prerelease found"];
      }
      release = pre;
    } else {
      release = await res.json() as { tag_name: string; assets: Array<{ name: string; browser_download_url: string }> };
    }

    const asset = release.assets.find((a) => a.name.includes("decx-server"));

    if (!asset) {
      return [false, `No decx-server asset found in release ${release.tag_name}`];
    }

    mkdirSync(INSTALL_DIR, { recursive: true });

    const downloadRes = await fetch(asset.browser_download_url, { redirect: "follow" });
    if (!downloadRes.ok || !downloadRes.body) {
      return [false, `Download failed: HTTP ${downloadRes.status}`];
    }

    const tmpPath = `${INSTALL_PATH}.tmp`;
    const totalSize = Number(downloadRes.headers.get("content-length") || 0);
    await downloadWithProgress(downloadRes.body, tmpPath, totalSize, {
      label: asset.name,
    });

    try {
      renameSync(tmpPath, INSTALL_PATH);
    } catch {
      unlinkSync(tmpPath);
      return [false, "Failed to save downloaded file"];
    }

    return [true, `Installed decx-server v${release.tag_name} to ${INSTALL_PATH}`, release.tag_name];
  } catch (err) {
    return [false, `Installation failed: ${err}`];
  }
}
