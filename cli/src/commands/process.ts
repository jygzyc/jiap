/**
 * Process management commands.
 */

import { Command } from "commander";
import { spawn } from "child_process";
import * as path from "path";
import * as os from "os";
import { createWriteStream, existsSync, mkdirSync } from "fs";
import { createHash } from "crypto";
import { JIAPClient } from "../core/client.js";
import { Formatter } from "../utils/formatter.js";
import { Manager, hashFile } from "../core/config.js";
import { FileError, ProcessError, DecompileError, JiapError, ServerError, withErrorHandler } from "../utils/errors.js";
import { findJadx } from "../jadx/finder.js";
import { installJadx, installJiapPlugin, checkAllComponents, checkJiapServer } from "../jadx/installer.js";
import { decompile } from "../jadx/decompiler.js";
import { isSessionAlive } from "../core/session.js";

export function makeProcessCommand(): Command {
  const cmd = new Command("process");
  cmd.description("Manage JADX+JIAP processes and installation");

  // check
  cmd
    .command("check")
    .description("Check JADX, JIAP plugin, and server status")
    .option("-P, --port <port>", "Server port", String)
    .option("--install", "Install missing components")
    .option("--json", "JSON output")
    .action(async (opts) => {
      const fmt = new Formatter(opts.json);
      const mgr = Manager.get();
      const port = opts.port ? parseInt(opts.port) : mgr.server.defaultPort;

      const results = await checkAllComponents(port);
      if (opts.json) {
        fmt.output(results);
        return;
      }

      for (const [name, info] of Object.entries(results)) {
        const status = info.ok ? "OK  " : "FAIL";
        console.log(`  [${status}] ${name.padEnd(10)} ${info.info}`);
      }

      if (opts.install) {
        if (!results.jadx.ok) {
          fmt.info("Installing JADX...");
          const [ok, msg] = await installJadx();
          ok ? fmt.success(msg) : fmt.error(msg);
        }
        const jadxPath = findJadx();
        if (jadxPath && !results.plugin.ok) {
          fmt.info("Installing JIAP plugin...");
          const [ok, msg] = await installJiapPlugin(jadxPath);
          ok ? fmt.success(msg) : fmt.error(msg);
        }
      }
    });

  // open
  cmd
    .command("open <file>")
    .description("Open and analyze a file (APK, DEX, JAR, etc.)")
    .option("-P, --port <port>", "Server port")
    .option("--gui", "Launch with GUI")
    .option("-o, --output <dir>", "Output directory (headless decompilation)")
    .option("--threads <n>", "Number of decompilation threads", String)
    .option("--no-res", "Skip decoding resources")
    .option("--force", "Force start even if a session already exists")
    .option("-n, --name <name>", "Session name (default: APK filename without extension)")
    .option("--json", "JSON output")
    .action(withErrorHandler(async (filePath: string, opts) => {
      const fmt = new Formatter(opts.json);
      const mgr = Manager.get();
      const port = opts.port ? parseInt(opts.port) : mgr.server.defaultPort;

      const jadxPath = findJadx();
      if (!jadxPath) {
        throw new FileError("JADX not found. Run 'jiap process check --install' to install.");
      }

      // Resolve input: local file or URL
      const resolvedFile = await resolveFileInput(filePath);
      if (!existsSync(resolvedFile)) {
        throw new FileError(`File not found: ${resolvedFile}`, resolvedFile);
      }

      // Pure headless decompilation mode (no server)
      if (opts.output || opts.noRes) {
        const [ok, msg] = await decompile({
          input_file: resolvedFile,
          output_dir: opts.output,
          no_res: opts.noRes,
          threads: opts.threads ? parseInt(opts.threads) : 4,
        });
        if (!ok) throw new DecompileError(msg, resolvedFile);
        fmt.success(msg);
        if (opts.json) fmt.output({ input: resolvedFile, output: opts.output });
        return;
      }

      // --- Server mode: check for existing session ---
      const fileName = opts.name || path.basename(resolvedFile, path.extname(resolvedFile));
      const fileHash = await hashFile(resolvedFile);
      const existingSession = mgr.getSession(fileName);

      if (existingSession && !opts.force) {
        if (existingSession.hash === fileHash && isSessionAlive(existingSession)) {
          // Reuse existing session (same name + same hash + alive)
          fmt.info(`Session already active (name: ${existingSession.name}, PID: ${existingSession.pid}, port: ${existingSession.port})`);
          if (opts.json) {
            fmt.output({ name: existingSession.name, hash: existingSession.hash, pid: existingSession.pid, port: existingSession.port, file: resolvedFile, reused: true });
          }
          return;
        }
        if (existingSession.hash !== fileHash) {
          throw new ProcessError(
            `Session '${fileName}' already exists for a different APK (hash: ${existingSession.hash}). ` +
            `Use --force to overwrite, or --name to choose a different session name.`
          );
        }
        // Same name + same hash but dead — clean up before spawning new one
        mgr.removeSession(fileName);
      }

      // --- Check hash dedup: prevent same APK under different names ---
      if (!opts.force) {
        for (const s of mgr.listAliveSessions()) {
          if (s.hash === fileHash && s.name !== fileName) {
            throw new ProcessError(
              `Already open as session '${s.name}'. Use --force to open again.`
            );
          }
        }
      }

      // --- Check port availability ---
      const [portInUse] = await checkJiapServer(port, 1);
      if (portInUse) {
        // Port is occupied — see if it's one of our sessions
        const aliveSessions = mgr.listAliveSessions();
        const portSession = aliveSessions.find(s => s.port === port);
        if (portSession) {
          throw new ProcessError(
            `Port ${port} is already in use by session '${portSession.name}' (${portSession.path}). ` +
            `Use --port to specify a different port, or close that session first.`,
            port
          );
        }
        throw new ProcessError(
          `Port ${port} is already in use by an unknown process. ` +
          `Use --port to specify a different port.`,
          port
        );
      }

      // --- Spawn JADX ---
      const jadxArgs = [resolvedFile];
      if (!opts.gui) jadxArgs.unshift("--cmd");
      jadxArgs.push(`-Pjadx.plugins.install=github:jygzyc:jiap`);
      jadxArgs.push(`-Djiap.port=${port}`);
      jadxArgs.push(`-Djiap.gui=${String(opts.gui ?? false)}`);

      let proc;
      try {
        proc = spawn(jadxPath, jadxArgs, { detached: true, stdio: "ignore" });
      } catch (err) {
        throw new ProcessError(`Failed to start JADX: ${err}`);
      }

      if (!proc.pid) {
        throw new ProcessError("Failed to get PID from spawned process");
      }

      proc.unref();

      const session = await mgr.createSession(fileName, resolvedFile, proc.pid, port);
      fmt.success(`Started (name: ${session.name}, PID: ${proc.pid}, Port: ${port})`);

      // Wait for server — both GUI and headless
      const timeout = opts.gui ? 10 : 30;
      const ready = await waitForServer(port, timeout);
      if (ready) {
        fmt.success(`Server ready at http://127.0.0.1:${port}`);
      } else if (!opts.gui) {
        // Headless mode: server MUST start — error if it didn't
        throw new ServerError(`Server did not start within ${timeout}s on port ${port}`, port);
      } else {
        // GUI mode: just warn — GUI may take longer or user might not need the server
        fmt.warning(`Server not responding after ${timeout}s (GUI mode — may still be starting)`);
      }

      if (opts.json) {
        fmt.output({ name: session.name, hash: session.hash, pid: proc.pid, port, file: resolvedFile, reused: false });
      }
    }, (_filePath, opts) => new Formatter(Boolean(opts.json))));

  // close
  cmd
    .command("close [name]")
    .description("Stop JADX process by session name")
    .option("-a, --all", "Kill all processes")
    .option("--json", "JSON output")
    .action(withErrorHandler(async (name: string | undefined, opts) => {
      const fmt = new Formatter(opts.json);
      const mgr = Manager.get();

      // Cleanup stale sessions on every close invocation
      const cleaned = mgr.cleanupDead();
      if (cleaned > 0 && !opts.json) {
        fmt.info(`Cleaned ${cleaned} stale session(s)`);
      }

      if (opts.all) {
        const sessions = mgr.listAliveSessions();
        const killed: string[] = [], dead: string[] = [];
        for (const s of sessions) {
          const alive = await killProcessGroup(s.pid);
          mgr.removeSession(s.name);
          (alive ? killed : dead).push(s.name);
        }
        if (killed.length) fmt.success(`Killed ${killed.length} process(es)`);
        if (dead.length) fmt.info(`Removed ${dead.length} dead session(s)`);
        if (opts.json) fmt.output({ killed, dead });
        return;
      }

      if (!name) {
        const alive = mgr.listAliveSessions();
        if (alive.length === 1) {
          name = alive[0].name;
        } else {
          throw new ProcessError(
            alive.length === 0
              ? "No running sessions"
              : "Specify session name or use --all"
          );
        }
      }

      const session = mgr.getSession(name);
      if (!session) {
        throw new ProcessError(`Session not found: ${name}`);
      }

      const alive = await killProcessGroup(session.pid);
      mgr.removeSession(name);
      alive ? fmt.success(`Killed ${name}`) : fmt.info(`Removed dead session ${name}`);
    }, (_name, opts) => new Formatter(Boolean(opts.json))));

  // list
  cmd
    .command("list")
    .description("List running processes")
    .option("--json", "JSON output")
    .action((opts) => {
      const fmt = new Formatter(opts.json);
      const mgr = Manager.get();

      // Cleanup stale sessions
      const cleaned = mgr.cleanupDead();
      if (cleaned > 0 && !opts.json) {
        fmt.info(`Cleaned ${cleaned} stale session(s)`);
      }

      const sessions = mgr.listAliveSessions();
      if (opts.json) {
        fmt.output(sessions);
        return;
      }

      if (sessions.length === 0) {
        fmt.info("No running sessions");
        return;
      }

      // Table format
      const nameW = Math.max(4, ...sessions.map(s => s.name.length));
      const portW = 4;
      const pidW = 3;
      const header = `${"NAME".padEnd(nameW)}  ${"PORT".padEnd(portW)}  ${"PID".padEnd(pidW)}  PATH`;
      console.log(header);
      for (const s of sessions) {
        console.log(`${s.name.padEnd(nameW)}  ${String(s.port).padEnd(portW)}  ${String(s.pid).padEnd(pidW)}  ${s.path}`);
      }
    });

  // status
  cmd
    .command("status [name]")
    .description("Check server status")
    .option("-P, --port <port>", "Server port", String)
    .option("--json", "JSON output")
    .action(withErrorHandler(async (name: string | undefined, opts) => {
      const fmt = new Formatter(opts.json);
      const mgr = Manager.get();
      let port: number;

      if (name) {
        const session = mgr.getSession(name);
        if (!session) throw new ProcessError(`Session not found: ${name}`);
        port = session.port;
      } else if (opts.port) {
        port = parseInt(opts.port);
      } else {
        port = mgr.server.defaultPort;
      }

      const client = new JIAPClient("127.0.0.1", port);
      try {
        await client.healthCheck();
        fmt.success(`Server running on port ${port}`);
      } catch (err) {
        throw new JiapError(String(err), "SERVER_ERROR", { port });
      }
    }, (_name, opts) => new Formatter(Boolean(opts.json))));



  return cmd;
}

async function waitForServer(port: number, timeout: number = 30): Promise<boolean> {
  const start = Date.now();
  while (Date.now() - start < timeout * 1000) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}/health`);
      if (response.ok) return true;
    } catch { /* starting */ }
    await new Promise(r => setTimeout(r, 500));
  }
  return false;
}

/**
 * Kill an entire process group (handles detached processes with children).
 * On Unix, uses negative PID to signal the process group.
 */
async function killProcessGroup(pid: number): Promise<boolean> {
  const tryKill = (sig: string) => {
    try {
      // Negative PID = kill entire process group (for detached: true)
      if (process.platform !== "win32") {
        process.kill(-pid, sig as NodeJS.Signals);
      } else {
        process.kill(pid, sig as NodeJS.Signals);
      }
      return true;
    }
    catch { return false; }
  };

  if (!tryKill("SIGTERM")) return false;

  const t1 = Date.now();
  while (Date.now() - t1 < 500) {
    try { process.kill(pid, 0); }
    catch { return true; }
    await new Promise(r => setTimeout(r, 50));
  }

  if (!tryKill("SIGKILL")) return false;

  const t2 = Date.now();
  while (Date.now() - t2 < 1000) {
    try { process.kill(pid, 0); }
    catch { return true; }
    await new Promise(r => setTimeout(r, 50));
  }
  return false;
}

const URL_RE = /^https?:\/\//i;

/**
 * Resolve file input: if it's a URL, download to ~/.jiap/tmp/ and return local path.
 * If it's a local path, return it as-is.
 */
async function resolveFileInput(input: string): Promise<string> {
  // Local file
  if (!URL_RE.test(input)) {
    return path.resolve(input);
  }

  // URL — download to ~/.jiap/tmp/
  const tmpDir = path.join(os.homedir(), ".jiap", "tmp");
  mkdirSync(tmpDir, { recursive: true });

  // Derive filename from URL (prefix with URL hash to avoid collisions)
  const url = new URL(input);
  const urlHash = createHash("md5").update(input).digest("hex").slice(0, 8);
  let filename = path.basename(url.pathname);
  // Keep original extension if recognized
  if (!/\.(apk|dex|jar|class|aar)$/i.test(filename)) {
    filename = `${filename}_${urlHash}.bin`;
  } else {
    // Prefix with urlHash to prevent collision between different URLs with same filename
    filename = `${urlHash}_${filename}`;
  }
  const localPath = path.join(tmpDir, filename);

  // Skip if already downloaded
  if (existsSync(localPath)) {
    return localPath;
  }

  const fmt = new Formatter(false);
  fmt.info(`Downloading ${input} ...`);

  const response = await fetch(input, { redirect: "follow" });
  if (!response.ok) {
    throw new FileError(`Download failed: HTTP ${response.status}`, input);
  }

  const totalSize = Number(response.headers.get("content-length") || 0);
  const fileStream = createWriteStream(localPath);
  const body = response.body!;

  let downloaded = 0;
  for await (const chunk of body as AsyncIterable<Buffer>) {
    fileStream.write(chunk);
    downloaded += chunk.length;
    if (totalSize > 0) {
      const pct = Math.round((downloaded / totalSize) * 100);
      process.stderr.write(`\r  ${pct}% (${formatBytes(downloaded)} / ${formatBytes(totalSize)})`);
    }
  }

  fileStream.end();
  if (totalSize > 0) process.stderr.write("\n");
  fmt.success(`Saved to ${localPath} (${formatBytes(downloaded)})`);

  return localPath;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}
