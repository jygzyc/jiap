/**
 * Shared client resolution helper for commands.
 */

import { JIAPClient } from "./client.js";
import { Formatter } from "../utils/formatter.js";
import { Manager } from "./config.js";

export function resolveClient(
  opts: Record<string, unknown>
): { fmt: Formatter; client: JIAPClient } {
  const jsonMode = opts.json !== undefined ? Boolean(opts.json) : true;
  const fmt = new Formatter(jsonMode);
  const mgr = Manager.get();

  if (opts.session && opts.port) {
    fmt.error("Cannot specify both --session and --port");
    process.exit(1);
  }

  let port: number;
  if (opts.port) {
    port = parseInt(opts.port as string);
  } else if (opts.session) {
    const s = mgr.getSession(opts.session as string);
    if (!s) {
      fmt.error(`Session not found: ${opts.session}`);
      process.exit(1);
    }
    port = s.port;
  } else {
    // Auto-select: if exactly one alive session, use it; otherwise default port
    const auto = mgr.autoSelectSession();
    if (auto) {
      port = auto.port;
    } else {
      port = mgr.server.defaultPort;
    }
  }

  const client = new JIAPClient("127.0.0.1", port);
  return { fmt, client };
}
