import { Command } from "commander";
import { resolveClient } from "../core/client-helper.js";

export function makeCodeCommand(): Command {
  const cmd = new Command("code");
  cmd.description("Common Code Analysis");

  cmd
    .option("-s, --session <name>", "Target session by name")
    .option("-P, --port <port>", "Server port")
    .option("--json", "JSON output");

  cmd
    .command("all-classes")
    .description("Get all classes")
    .option("--page <n>", "Page number", String)
    .action(async (opts) => {
      const { fmt, client } = resolveClient(opts);
      const page = opts.page ? parseInt(opts.page) : 1;
      fmt.output(await client.getAllClasses(page));
    });

  cmd
    .command("class-info <class>")
    .description("Get class information")
    .option("--page <n>", "Page number", String)
    .action(async (className: string, opts) => {
      const { fmt, client } = resolveClient(opts);
      const page = opts.page ? parseInt(opts.page) : 1;
      fmt.output(await client.getClassInfo(className, page));
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
    .command("search-class <keyword>")
    .description("Search in class content")
    .option("--page <n>", "Page number", String)
    .action(async (keyword: string, opts) => {
      const { fmt, client } = resolveClient(opts);
      const page = opts.page ? parseInt(opts.page) : 1;
      fmt.output(await client.searchClassKey(keyword, page));
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
    .command("selected-text")
    .description("Get currently selected text from JADX GUI editor")
    .option("--page <n>", "Page number", String)
    .action(async (opts) => {
      const { fmt, client } = resolveClient(opts);
      const page = opts.page ? parseInt(opts.page) : 1;
      fmt.output(await client.getSelectedText(page));
    });

  cmd
    .command("selected-class")
    .description("Get currently selected class from JADX GUI editor")
    .option("--page <n>", "Page number", String)
    .action(async (opts) => {
      const { fmt, client } = resolveClient(opts);
      const page = opts.page ? parseInt(opts.page) : 1;
      fmt.output(await client.getSelectedClass(page));
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
