/**
 * jiap-server.jar finder and installer.
 */

import * as path from "path";
import * as os from "os";
import { existsSync, mkdirSync, createWriteStream } from "fs";
const JIAP_SERVER_HOME: string | undefined = process.env.JIAP_SERVER_HOME;
import { Formatter } from "../utils/formatter.js";

const INSTALL_DIR = path.join(os.homedir(), ".jiap", "bin");
const INSTALL_PATH = path.join(INSTALL_DIR, "jiap-server.jar");

/**
 * Find jiap-server.jar from known locations.
 * Priority: JIAP_SERVER_HOME env > ~/.jiap/bin/jiap-server.jar
 */
export function findJiapServerJar(): string | null {
  if (JIAP_SERVER_HOME) {
    if (JIAP_SERVER_HOME.endsWith(".jar") && existsSync(JIAP_SERVER_HOME)) return JIAP_SERVER_HOME;
    const fromDir = path.join(JIAP_SERVER_HOME, "jiap-server.jar");
    if (existsSync(fromDir)) return fromDir;
  }

  if (existsSync(INSTALL_PATH)) return INSTALL_PATH;

  return null;
}

/**
 * Download and install the latest jiap-server.jar from GitHub releases.
 */
export async function installJiapServer(fmt: Formatter, prerelease: boolean = false): Promise<[boolean, string]> {
  try {
    if (prerelease) {
      fmt.info("Fetching latest prerelease info from GitHub...");
    } else {
      fmt.info("Fetching latest release info from GitHub...");
    }

    const endpoint = prerelease
      ? "https://api.github.com/repos/jygzyc/jiap/releases?per_page=10"
      : "https://api.github.com/repos/jygzyc/jiap/releases/latest";

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

    const asset = release.assets.find((a) => a.name.includes("jiap-server"));

    if (!asset) {
      return [false, `No jiap-server asset found in release ${release.tag_name}`];
    }

    fmt.info(`Downloading ${asset.name} (v${release.tag_name})...`);
    mkdirSync(INSTALL_DIR, { recursive: true });

    const downloadRes = await fetch(asset.browser_download_url, { redirect: "follow" });
    if (!downloadRes.ok || !downloadRes.body) {
      return [false, `Download failed: HTTP ${downloadRes.status}`];
    }

    const tmpPath = `${INSTALL_PATH}.tmp`;
    const fileStream = createWriteStream(tmpPath);

    for await (const chunk of downloadRes.body as AsyncIterable<Buffer>) {
      fileStream.write(chunk);
    }
    fileStream.end();

    // Rename tmp to final
    const { renameSync, unlinkSync } = await import("fs");
    try {
      renameSync(tmpPath, INSTALL_PATH);
    } catch {
      unlinkSync(tmpPath);
      return [false, "Failed to save downloaded file"];
    }

    return [true, `Installed jiap-server v${release.tag_name} to ${INSTALL_PATH}`];
  } catch (err) {
    return [false, `Installation failed: ${err}`];
  }
}
