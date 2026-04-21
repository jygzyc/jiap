import { Command } from "commander";
import { resolveClient } from "../core/client-helper.js";
import type { ClassFilterOptions, ClassGrepOptions, GlobalSearchOptions } from "../core/client.js";

function collectOption(value: string, previous: string[]): string[] {
  previous.push(value);
  return previous;
}

function addPackageFilterOptions(cmd: Command): Command {
  return cmd
    .option("--first <n>", "Search only the first N candidates after package filtering")
    .option("--include-package <name>", "Only search classes and methods in this package", collectOption, [])
    .option("--exclude-package <name>", "Exclude classes and methods in this package", collectOption, [])
    .option("--no-regex", "Treat filter values as literal text");
}

function addGlobalSearchOptions(cmd: Command): Command {
  return addPackageFilterOptions(cmd)
    .requiredOption("--max-results <n>", "Maximum returned search results")
    .option("--case-sensitive", "Use case-sensitive matching");
}

function addClassGrepOptions(cmd: Command): Command {
  return cmd
    .requiredOption("--max-results <n>", "Maximum returned grep results")
    .option("--case-sensitive", "Use case-sensitive matching")
    .option("--no-regex", "Treat the keyword as literal text");
}

function parseClassFilterOptions(opts: Record<string, unknown>): ClassFilterOptions {
  return {
    filter: {
      ...(opts.first ? { first: parseInt(String(opts.first), 10) } : {}),
      includes: Array.isArray(opts.includePackage) ? opts.includePackage.map(String) : [],
      excludes: Array.isArray(opts.excludePackage) ? opts.excludePackage.map(String) : [],
      ...(opts.regex === false ? { regex: false } : {}),
    },
  };
}

function parseGlobalSearchOptions(opts: Record<string, unknown>): GlobalSearchOptions {
  return {
    search: {
      ...(opts.first ? { first: parseInt(String(opts.first), 10) } : {}),
      maxResults: parseInt(String(opts.maxResults), 10),
      includes: Array.isArray(opts.includePackage) ? opts.includePackage.map(String) : [],
      excludes: Array.isArray(opts.excludePackage) ? opts.excludePackage.map(String) : [],
      caseSensitive: opts.caseSensitive === true,
      regex: opts.regex !== false,
    },
  };
}

function parseClassGrepOptions(opts: Record<string, unknown>): ClassGrepOptions {
  return {
    grep: {
      maxResults: parseInt(String(opts.maxResults), 10),
      caseSensitive: opts.caseSensitive === true,
      regex: opts.regex !== false,
    },
  };
}

export function makeCodeCommand(): Command {
  const cmd = new Command("code");
  cmd.description("Common Code Analysis");

  cmd
    .option("-s, --session <name>", "Target session by name")
    .option("-P, --port <port>", "Server port");

  addPackageFilterOptions(cmd.command("all-classes"))
    .description("Get all classes")
    .option("--page <n>", "Page number", String)
    .action(async (opts) => {
      const { fmt, client } = resolveClient(opts);
      const page = opts.page ? parseInt(opts.page) : 1;
      fmt.output(await client.getAllClasses(parseClassFilterOptions(opts), page));
    });

  addGlobalSearchOptions(cmd.command("search-global <keyword>"))
    .description("Search classes, methods, and resources")
    .option("--page <n>", "Page number", String)
    .action(async (keyword: string, opts) => {
      const { fmt, client } = resolveClient(opts);
      const page = opts.page ? parseInt(opts.page) : 1;
      fmt.output(await client.searchGlobalKey(keyword, parseGlobalSearchOptions(opts), page));
    });

  cmd
    .command("class-context <class>")
    .description("Get class context with methods and fields")
    .option("--page <n>", "Page number", String)
    .action(async (className: string, opts) => {
      const { fmt, client } = resolveClient(opts);
      const page = opts.page ? parseInt(opts.page) : 1;
      fmt.output(await client.getClassContext(className, page));
    });

  cmd
    .command("class-source <class>")
    .description("Get class source code")
    .option("--smali", "Output in smali format")
    .option("--page <n>", "Page number", String)
    .action(async (className: string, opts) => {
      const { fmt, client } = resolveClient(opts);
      const page = opts.page ? parseInt(opts.page) : 1;
      fmt.output(await client.getClassSource(className, opts.smali ?? false, page));
    });

  cmd
    .command("method-source <signature>")
    .description("Get method source")
    .option("--smali", "Output in smali format")
    .option("--page <n>", "Page number", String)
    .action(async (sig: string, opts) => {
      const { fmt, client } = resolveClient(opts);
      const page = opts.page ? parseInt(opts.page) : 1;
      fmt.output(await client.getMethodSource(sig, opts.smali ?? false, page));
    });

  cmd
    .command("method-context <signature>")
    .description("Get method context with signature, callers, and callees")
    .option("--page <n>", "Page number", String)
    .action(async (sig: string, opts) => {
      const { fmt, client } = resolveClient(opts);
      const page = opts.page ? parseInt(opts.page) : 1;
      fmt.output(await client.getMethodContext(sig, page));
    });

  cmd
    .command("method-cfg <signature>")
    .description("Get method control flow graph")
    .option("--page <n>", "Page number", String)
    .action(async (sig: string, opts) => {
      const { fmt, client } = resolveClient(opts);
      const page = opts.page ? parseInt(opts.page) : 1;
      fmt.output(await client.getMethodCfg(sig, page));
    });

  addClassGrepOptions(cmd.command("search-class <class> <pattern>"))
    .description("Grep in one class and return matching lines with method signatures")
    .option("--page <n>", "Page number", String)
    .action(async (className: string, keyword: string, opts) => {
      const { fmt, client } = resolveClient(opts);
      const page = opts.page ? parseInt(opts.page) : 1;
      fmt.output(await client.searchClassKey(className, keyword, parseClassGrepOptions(opts), page));
    });

  cmd
    .command("search-method <name>")
    .description("Find methods by name")
    .option("--page <n>", "Page number", String)
    .action(async (name: string, opts) => {
      const { fmt, client } = resolveClient(opts);
      const page = opts.page ? parseInt(opts.page) : 1;
      fmt.output(await client.searchMethod(name, page));
    });

  cmd
    .command("xref-method <signature>")
    .description("Find method callers")
    .option("--page <n>", "Page number", String)
    .action(async (sig: string, opts) => {
      const { fmt, client } = resolveClient(opts);
      const page = opts.page ? parseInt(opts.page) : 1;
      fmt.output(await client.getMethodXref(sig, page));
    });

  cmd
    .command("xref-class <class>")
    .description("Find class usages")
    .option("--page <n>", "Page number", String)
    .action(async (className: string, opts) => {
      const { fmt, client } = resolveClient(opts);
      const page = opts.page ? parseInt(opts.page) : 1;
      fmt.output(await client.getClassXref(className, page));
    });

  cmd
    .command("xref-field <field>")
    .description("Find field usages")
    .option("--page <n>", "Page number", String)
    .action(async (fieldName: string, opts) => {
      const { fmt, client } = resolveClient(opts);
      const page = opts.page ? parseInt(opts.page) : 1;
      fmt.output(await client.getFieldXref(fieldName, page));
    });

  cmd
    .command("implement <interface>")
    .description("Find implementations")
    .option("--page <n>", "Page number", String)
    .action(async (iface: string, opts) => {
      const { fmt, client } = resolveClient(opts);
      const page = opts.page ? parseInt(opts.page) : 1;
      fmt.output(await client.getImplement(iface, page));
    });

  cmd
    .command("subclass <class>")
    .description("Find subclasses")
    .option("--page <n>", "Page number", String)
    .action(async (className: string, opts) => {
      const { fmt, client } = resolveClient(opts);
      const page = opts.page ? parseInt(opts.page) : 1;
      fmt.output(await client.getSubClasses(className, page));
    });

  return cmd;
}
