/**
 * Self-management commands for jiap-cli.
 */

import { Command } from "commander";
import { spawnSync } from "child_process";
import { readFileSync } from "fs";
import * as path from "path";
import { fileURLToPath } from "url";
import { Formatter } from "../utils/formatter.js";
import { Manager } from "../core/config.js";
import { checkForServerUpdate, installJiapServer } from "../server/installer.js";
import { JiapError, ServerError, withErrorHandler } from "../utils/errors.js";

function getCliVersion(): string {
  if (process.env.npm_package_version) return process.env.npm_package_version;
  try {
    const pkgPath = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "package.json");
    const pkg = JSON.parse(readFileSync(pkgPath, "utf-8"));
    return pkg.version ?? "unknown";
  } catch {
    return "unknown";
  }
}

export function makeSelfCommand(): Command {
  const cmd = new Command("self");
  cmd.description("Self-management commands (update, install, etc.)");

  cmd
    .command("install")
    .description("Install or update jiap-server.jar")
    .option("-p, --prerelease", "Install prerelease version")
    .action(withErrorHandler(async (opts) => {
      const fmt = new Formatter();
      const [ok, msg, version] = await installJiapServer(opts.prerelease);
      if (ok) {
        fmt.output({ ok: true, version, path: msg });
      } else {
        throw new ServerError(msg);
      }
    }));

  cmd
    .command("update")
    .description("Update jiap-server and/or jiap-cli")
    .option("-p, --prerelease", "Install prerelease server version")
    .action(withErrorHandler(async (opts) => {
      const fmt = new Formatter();
      const mgr = Manager.get();
      const currentVersion = mgr.serverJar.version;

      // Update server
      console.error(`  Updating jiap-server (current: v${currentVersion})...`);

      const updateInfo = await checkForServerUpdate(currentVersion, opts.prerelease);
      if (updateInfo.available) {
        console.error(`  New version available: v${updateInfo.latestVersion}`);
        const [ok, msg, version] = await installJiapServer(opts.prerelease);
        if (ok && version) {
          mgr.updateServerVersion(version);
          console.error(`  ${msg}`);
        } else {
          throw new JiapError(msg, "UPDATE_ERROR");
        }
      } else {
        console.error("  Server already up to date");
      }

      // Update CLI
      console.error(`  Updating jiap-cli (current: v${getCliVersion()})...`);
      console.error("  Running: npm install -g jiap-cli@latest ...");

      const result = spawnSync("npm", ["install", "-g", "jiap-cli@latest"], {
        stdio: "inherit",
        timeout: 120_000,
      });

      if (result.status !== 0) {
        throw new JiapError("CLI update failed. Check npm output above.", "UPDATE_ERROR");
      }

      fmt.output({ ok: true, message: "Update complete. Restart your shell to use the new version." });
    }));

  return cmd;
}
