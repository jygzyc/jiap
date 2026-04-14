/**
 * CLI command structure tests.
 *
 * Tests the command tree structure rather than help text output.
 * This decouples tests from description wording and commander internals.
 */

import { Command } from "commander";
import { makeProcessCommand } from "../src/commands/process.js";
import { makeCodeCommand } from "../src/commands/code.js";
import { makeArdCommand } from "../src/commands/ard.js";

function createProgram(): Command {
  const program = new Command();
  program.name("jiap").version("2.0.0");
  program.addCommand(makeProcessCommand());
  program.addCommand(makeCodeCommand());
  program.addCommand(makeArdCommand());
  return program;
}

function findCommand(root: Command, path: string[]): Command | undefined {
  let cmd: Command = root;
  for (const part of path) {
    const sub = cmd.commands.find(c => c.name() === part);
    if (!sub) return undefined;
    cmd = sub;
  }
  return cmd;
}

function getSubcommandNames(parent: Command): string[] {
  return parent.commands.map(c => c.name());
}

function getOptionFlags(cmd: Command): string[] {
  return cmd.options.map(o => o.flags);
}

// ============================================================================
// Root
// ============================================================================

describe("root", () => {
  it("registers 3 top-level commands", () => {
    const program = createProgram();
    expect(getSubcommandNames(program)).toEqual(["process", "code", "ard"]);
  });
});

// ============================================================================
// jiap process
// ============================================================================

describe("process", () => {
  let cmd: Command;

  beforeEach(() => {
    cmd = findCommand(createProgram(), ["process"])!;
  });

  it("registers 6 subcommands (check, open, close, list, status, install)", () => {
    expect(getSubcommandNames(cmd)).toEqual([
      "check", "open", "close", "list", "status", "install",
    ]);
  });

  it("open has <file> argument and -P/--port option", () => {
    const open = findCommand(cmd, ["open"])!;
    expect(open.registeredArguments.length).toBeGreaterThanOrEqual(1);
    const flags = getOptionFlags(open);
    expect(flags.some(f => f.includes("--port"))).toBe(true);
  });

  it("open has --force option", () => {
    const open = findCommand(cmd, ["open"])!;
    const flags = getOptionFlags(open);
    expect(flags.some(f => f.includes("--force"))).toBe(true);
  });

  it("close has optional [name] argument", () => {
    const close = findCommand(cmd, ["close"])!;
    expect(close.registeredArguments.length).toBeGreaterThanOrEqual(1);
  });
});

// ============================================================================
// jiap code
// ============================================================================

describe("code", () => {
  let cmd: Command;

  beforeEach(() => {
    cmd = findCommand(createProgram(), ["code"])!;
  });

  it("registers 12 subcommands", () => {
    expect(getSubcommandNames(cmd)).toEqual([
      "all-classes", "class-info", "class-source", "method-source",
      "search-class", "search-method", "xref-method", "xref-class",
      "xref-field", "implement", "subclass", "get-aidl",
    ]);
  });

  it("class-info has <class> argument", () => {
    const info = findCommand(cmd, ["class-info"])!;
    expect(info.registeredArguments.length).toBeGreaterThanOrEqual(1);
  });

  it("class-source has <class> argument", () => {
    const src = findCommand(cmd, ["class-source"])!;
    expect(src.registeredArguments.length).toBeGreaterThanOrEqual(1);
  });

  it("method-source has <signature> argument", () => {
    const ms = findCommand(cmd, ["method-source"])!;
    expect(ms.registeredArguments.length).toBeGreaterThanOrEqual(1);
  });

  it("implement has <interface> argument", () => {
    const impl = findCommand(cmd, ["implement"])!;
    expect(impl.registeredArguments.length).toBeGreaterThanOrEqual(1);
  });

  it("subclass has <class> argument", () => {
    const sub = findCommand(cmd, ["subclass"])!;
    expect(sub.registeredArguments.length).toBeGreaterThanOrEqual(1);
  });
});

// ============================================================================
// jiap ard
// ============================================================================

describe("ard", () => {
  let cmd: Command;

  beforeEach(() => {
    cmd = findCommand(createProgram(), ["ard"])!;
  });

  it("registers 10 subcommands", () => {
    expect(getSubcommandNames(cmd)).toEqual([
      "app-manifest", "main-activity", "app-application",
      "exported-components", "app-deeplinks", "app-receivers",
      "system-service-impl",
      "all-resources", "resource-file", "strings",
    ]);
  });

  it("system-service-impl has <interface> argument", () => {
    const ssi = findCommand(cmd, ["system-service-impl"])!;
    expect(ssi.registeredArguments.length).toBeGreaterThanOrEqual(1);
  });

  it("resource-file has <res> argument", () => {
    const rf = findCommand(cmd, ["resource-file"])!;
    expect(rf.registeredArguments.length).toBeGreaterThanOrEqual(1);
  });
});
