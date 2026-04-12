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
import { Manager } from "../core/config.js";
import { hashFile } from "../utils/hash.js";
import { FileError, ProcessError, JiapError, ServerError, withErrorHandler } from "../utils/errors.js";
import { findJiapServerJar, installJiapServer } from "../server/installer.js";
import { isSessionAlive } from "../core/session.js";

export function makeProcessCommand(): Command {
  const cmd = new Command("process");
  cmd.description("Manage JIAP server processes and installation");

  // check
  cmd
    .command("check")
    .description("Check JIAP server status")
    .option("-P, --port <port>", "Server port", String)
    .option("--install", "Install missing components")
    .option("--json", "JSON output")
    .action(async (opts) => {
      const fmt = new Formatter(opts.json);
      const mgr = Manager.get();
      const port = opts.port ? parseInt(opts.port) : mgr.server.defaultPort;

      // Check jiap-server.jar
      const jarPath = findJiapServerJar();
      const jarOk = jarPath !== null;
      const jarInfo = jarOk ? jarPath! : "Not found. Use --install or 'jiap process install' to install.";

      // Check running server
      const [serverOk, serverInfo] = await checkServer(port);

      const results = {
        server: { ok: serverOk, info: serverInfo },
        jar: { ok: jarOk, info: jarInfo },
      };

      if (opts.json) {
        fmt.output(results);
        return;
      }

      for (const [name, info] of Object.entries(results)) {
        const status = info.ok ? "OK  " : "FAIL";
        console.log(`  [${status}] ${name.padEnd(10)} ${info.info}`);
      }

      if (opts.install && !jarOk) {
        const [ok, msg] = await installJiapServer(fmt);
        if (ok) { fmt.success(msg); } else { fmt.error(msg); }
      }
    });

  // open
  cmd
    .command("open <file>")
    .allowUnknownOption(true)
    .description("Open and analyze a file (APK, DEX, JAR, etc.). Standard jadx-cli options are passed through.")
    .option("-P, --port <port>", "Server port")
    .option("--force", "Force start even if a session already exists")
    .option("-n, --name <name>", "Session name (default: APK filename without extension)")
    .option("--json", "JSON output")
    .action(withErrorHandler(async (filePath: string, opts) => {
      const fmt = new Formatter(opts.json);
      const mgr = Manager.get();
      const port = opts.port ? parseInt(opts.port) : mgr.server.defaultPort;

      const jarPath = findJiapServerJar();
      if (!jarPath) {
        throw new FileError("jiap-server.jar not found. Run 'jiap process check --install' to install.");
      }

      // Resolve input: local file or URL
      const resolvedFile = await resolveFileInput(filePath);
      if (!existsSync(resolvedFile)) {
        throw new FileError(`File not found: ${resolvedFile}`, resolvedFile);
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
      const [portInUse] = await checkServer(port, 1);
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

      // --- Spawn jiap-server ---
      const jadxArgs = extractPassthroughArgs();
      const javaArgs = ["-jar", jarPath, resolvedFile, "--port", String(port), ...jadxArgs];

      let proc;
      try {
        proc = spawn("java", javaArgs, { detached: true, stdio: "ignore" });
      } catch (err) {
        throw new ProcessError(`Failed to start jiap-server: ${err}`);
      }

      if (!proc.pid) {
        throw new ProcessError("Failed to get PID from spawned process");
      }

      proc.unref();

      const session = await mgr.createSession(fileName, fileHash, resolvedFile, proc.pid, port);
      fmt.success(`Started (name: ${session.name}, PID: ${proc.pid}, Port: ${port})`);

      // Wait for server
      const timeout = 30;
      const ready = await waitForServer(port, timeout);
      if (ready) {
        fmt.success(`Server ready at http://127.0.0.1:${port}`);
      } else {
        throw new ServerError(`Server did not start within ${timeout}s on port ${port}`, port);
      }

      if (opts.json) {
        fmt.output({ name: session.name, hash: session.hash, pid: proc.pid, port, file: resolvedFile, reused: false });
      }
    }, (_filePath, opts) => new Formatter(Boolean(opts.json))));

  // close
  cmd
    .command("close [name]")
    .description("Stop JIAP server by session name")
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
      if (alive) { fmt.success(`Killed ${name}`); } else { fmt.info(`Removed dead session ${name}`); }
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

  // install
  cmd
    .command("install")
    .description("Install or update jiap-server.jar")
    .option("-p, --prerelease", "Install prerelease version")
    .option("--json", "JSON output")
    .action(withErrorHandler(async (opts) => {
      const fmt = new Formatter(opts.json);
      const [ok, msg] = await installJiapServer(fmt, opts.prerelease);
      if (ok) {
        fmt.success(msg);
      } else {
        throw new ServerError(msg);
      }
    }, (opts) => new Formatter(Boolean(opts.json))));

  return cmd;
}

/**
 * Check if JIAP server is reachable on the given port.
 */
async function checkServer(port: number, retries: number = 3): Promise<[boolean, string]> {
  for (let i = 0; i < retries; i++) {
    try {
      const res = await fetch(`http://127.0.0.1:${port}/health`, { signal: AbortSignal.timeout(2000) });
      if (res.ok) return [true, `Server running on port ${port}`];
    } catch { /* retry */ }
    if (i < retries - 1) {
      await new Promise(r => setTimeout(r, 500));
    }
  }
  return [false, `No server on port ${port}`];
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

/**
 * Extract non-JIAP args from process.argv for passthrough to jiad-server.
 * Filters out JIAP-specific flags and the file argument, keeping everything else
 * (standard jadx-cli options) for direct passthrough.
 */
function extractPassthroughArgs(): string[] {
  const cmdArgs = process.argv.slice(2); // skip node and script
  const openIdx = cmdArgs.indexOf("open");
  if (openIdx === -1) return [];

  const raw = cmdArgs.slice(openIdx + 1);
  const jiapFlagsWithValue = ["-P", "--port", "-n", "--name"];
  const jiapFlags = ["--force", "--json"];

  const result: string[] = [];
  let fileSkipped = false;
  let i = 0;

  while (i < raw.length) {
    const arg = raw[i];

    // Skip the first non-flag arg (file path)
    if (!fileSkipped && !arg.startsWith("-")) {
      fileSkipped = true;
      i++;
      continue;
    }

    // Skip JIAP flags with values (--flag value or --flag=value)
    const isJiapWithValue = jiapFlagsWithValue.some(
      (f) => arg === f || arg.startsWith(f + "=")
    );
    if (isJiapWithValue) {
      i += arg.includes("=") ? 1 : 2;
      continue;
    }

    // Skip JIAP boolean flags
    if (jiapFlags.includes(arg)) {
      i++;
      continue;
    }

    result.push(arg);
    i++;
  }

  return result;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}
