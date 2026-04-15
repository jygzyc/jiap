import { Command } from "commander";
import { resolveClient } from "../core/client-helper.js";

export function makeArdCommand(): Command {
  const cmd = new Command("ard");
  cmd.description("Android Specific Analysis");

  cmd
    .option("-s, --session <name>", "Target session by name")
    .option("-P, --port <port>", "Server port")
    .option("--json", "JSON output");

  // ── App analysis ──────────────────────────────────────────────────────────

  cmd
    .command("app-manifest")
    .description("Get Android App AndroidManifest.xml")
    .option("--page <n>", "Page number", String)
    .action(async (opts) => {
      const { fmt, client } = resolveClient(opts);
      const page = opts.page ? parseInt(opts.page) : 1;
      fmt.output(await client.getAppManifest(page));
    });

  cmd
    .command("main-activity")
    .description("Get main activity name")
    .option("--page <n>", "Page number", String)
    .action(async (opts) => {
      const { fmt, client } = resolveClient(opts);
      const page = opts.page ? parseInt(opts.page) : 1;
      fmt.output(await client.getMainActivity(page));
    });

  cmd
    .command("app-application")
    .description("Get Application class name")
    .option("--page <n>", "Page number", String)
    .action(async (opts) => {
      const { fmt, client } = resolveClient(opts);
      const page = opts.page ? parseInt(opts.page) : 1;
      fmt.output(await client.getApplication(page));
    });

  cmd
    .command("exported-components")
    .description("List exported components")
    .option("--page <n>", "Page number", String)
    .action(async (opts) => {
      const { fmt, client } = resolveClient(opts);
      const page = opts.page ? parseInt(opts.page) : 1;
      fmt.output(await client.getExportedComponents(page));
    });

  cmd
    .command("app-deeplinks")
    .description("List deep link schemes")
    .option("--page <n>", "Page number", String)
    .action(async (opts) => {
      const { fmt, client } = resolveClient(opts);
      const page = opts.page ? parseInt(opts.page) : 1;
      fmt.output(await client.getDeepLinks(page));
    });

  cmd
    .command("app-receivers")
    .description("List dynamic broadcast receivers")
    .option("--page <n>", "Page number", String)
    .action(async (opts) => {
      const { fmt, client } = resolveClient(opts);
      const page = opts.page ? parseInt(opts.page) : 1;
      const receivers = await client.getDynamicReceivers(page);
      fmt.output(receivers);
    });

  cmd
    .command("system-service-impl <interface>")
    .description("Find system service implementations")
    .option("--page <n>", "Page number", String)
    .action(async (iface: string, opts) => {
      const { fmt, client } = resolveClient(opts);
      const page = opts.page ? parseInt(opts.page) : 1;
      fmt.output(await client.getSystemServiceImpl(iface, page));
    });

  cmd
    .command("all-resources")
    .description("List all resource file names")
    .option("--page <n>", "Page number", String)
    .action(async (opts) => {
      const { fmt, client } = resolveClient(opts);
      const page = opts.page ? parseInt(opts.page) : 1;
      fmt.output(await client.getAllResources(page));
    });

  cmd
    .command("resource-file <res>")
    .description("Get resource file content by name")
    .option("--page <n>", "Page number", String)
    .action(async (res: string, opts) => {
      const { fmt, client } = resolveClient(opts);
      const page = opts.page ? parseInt(opts.page) : 1;
      fmt.output(await client.getResourceFile(res, page));
    });

  cmd
    .command("strings")
    .description("Get strings.xml content from app resources")
    .option("--page <n>", "Page number", String)
    .action(async (opts) => {
      const { fmt, client } = resolveClient(opts);
      const page = opts.page ? parseInt(opts.page) : 1;
      fmt.output(await client.getStrings(page));
    });

  cmd
    .command("get-aidl")
    .description("Get all AIDL interfaces")
    .option("--page <n>", "Page number", String)
    .action(async (opts) => {
      const { fmt, client } = resolveClient(opts);
      const page = opts.page ? parseInt(opts.page) : 1;
      fmt.output(await client.getAidlInterfaces(page));
    });

  return cmd;
}
