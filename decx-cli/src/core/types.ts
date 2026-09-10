// Shared types for DECX CLI

/** Which engine backs a session's server process. */
export type SessionEngine = "jvm" | "native";

export interface Session {
  name: string;
  hash: string;
  pid: number;
  port: number;
  path: string;
  startedAt: number;
  scripts?: string[];
  /** Server engine; missing = "jvm" (records created before the native engine). */
  engine?: SessionEngine;
}

export interface Config {
  serverJar: { version: string };
  server: { defaultPort: number };
}
