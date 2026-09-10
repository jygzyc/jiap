import { InvalidArgumentError } from "commander";
import type { Command } from "commander";
import type { SourceOutputLanguage } from "../core/client.js";

export function collectOption(value: string, previous: string[]): string[] {
  previous.push(value);
  return previous;
}

/** Parse a repeatable option value into a string array (commander passes [] by default). */
export function parseStringList(value: unknown): string[] {
  return Array.isArray(value) ? value.map(String) : [];
}

/** Parse an optional integer option value. */
export function parseOptionalInt(value: unknown): number | undefined {
  return value ? parseInt(String(value), 10) : undefined;
}

/** Parse the shared --page option (default 1). */
export function parsePage(opts: Record<string, unknown>): number {
  return parseOptionalInt(opts.page) ?? 1;
}

export function addPackageFilterOptions(cmd: Command): Command {
  return cmd
    .option("--limit <n>", "Maximum number of returned items")
    .option("--include-package <pattern>", "Include only class/package names matching this package pattern; repeatable", collectOption, [])
    .option("--exclude-package <pattern>", "Exclude class/package names matching this package pattern; repeatable", collectOption, [])
    .option("--no-regex", "Treat package filter patterns as literal text instead of regular expressions");
}

/** Parse a positive seconds value for the shared --timeout option. */
export function parseTimeoutSeconds(value: string): number {
  const n = Number(value);
  if (!Number.isFinite(n) || n <= 0) {
    // InvalidArgumentError: commander renders it as a clean one-line CLI error.
    throw new InvalidArgumentError(`expected a positive number of seconds, got '${value}'`);
  }
  return n;
}

/** Validate the --language option of the source commands (server default: java). */
export function parseSourceLanguage(value: string): SourceOutputLanguage {
  const normalized = value.toLowerCase();
  if (normalized === "java" || normalized === "kotlin" || normalized === "auto") {
    return normalized;
  }
  if (normalized === "kt") return "kotlin";
  throw new InvalidArgumentError(`expected java|kotlin|auto, got '${value}'`);
}

export function addLanguageOption(cmd: Command): Command {
  return cmd.option(
    "--language <lang>",
    "Output language: java (default), kotlin, or auto (per-class inference; Kotlin rendering requires the native engine)",
    parseSourceLanguage,
  );
}

/** Connection options shared by the code/android command groups (read via optsWithGlobals). */
export function addClientConnectionOptions(cmd: Command): Command {
  return cmd
    .option("-s, --session <name>", "Select a named DECX session; required when multiple sessions are running")
    .option("--port <port>", "Connect to a DECX HTTP server on this port")
    .option("--timeout <seconds>", "HTTP request timeout in seconds (default 30, or DECX_TIMEOUT); raise for cold hierarchy builds / full-archive sweeps on large apps", parseTimeoutSeconds);
}

export function parseClassFilterOptions(opts: Record<string, unknown>): {
  filter: {
    limit?: number;
    includes: string[];
    excludes: string[];
    regex?: boolean;
  };
} {
  const limit = parseOptionalInt(opts.limit);
  return {
    filter: {
      ...(limit !== undefined ? { limit } : {}),
      includes: parseStringList(opts.includePackage),
      excludes: parseStringList(opts.excludePackage),
      ...(opts.regex === false ? { regex: false } : {}),
    },
  };
}
