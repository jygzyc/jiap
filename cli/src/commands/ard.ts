import { Command } from "commander";
import { Formatter } from "../utils/formatter.js";
import { resolveClient } from "../core/client-helper.js";
import { withErrorHandler } from "../utils/errors.js";

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
    .command("receivers")
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

  // ── Framework collection ──────────────────────────────────────────────────
  // TODO: temporarily disabled — re-enable when ready

  // cmd
  //   .command("framework-collect")
  //   .description("Collect framework files from device (pulls files)")
  //   .option("--oem <oem>", "OEM (vivo, oppo, xiaomi, honor, google)", String, "google")
  //   .option("-o, --output <dir>", "Output directory", String)
  //   .option("--device <id>", "ADB device ID", String)
  //   .option("--json", "JSON output")
  //   .action(withErrorHandler(async (opts) => {
  //     if (process.platform === "win32") {
  //       throw new Error("framework-collect is not supported on Windows");
  //     }
  //     const fmt = new Formatter(opts.json ?? false);
  //     const outputDir = expandPath(opts.output ?? "~/.jiap/framework");
  //     const collector = opts.device
  //       ? new FrameworkCollector(opts.device)
  //       : new FrameworkCollector();
  //
  //     if (!collector.checkDevice()) {
  //       throw new AdbOperationError("No device connected");
  //     }
  //
  //     const results = collector.collect(undefined, outputDir, opts.oem);
  //     const successCount = results.filter(r => r.success).length;
  //     const failCount = results.length - successCount;
  //
  //     fmt.success(`Collected ${successCount} files → ${outputDir}`);
  //     if (failCount > 0) {
  //       fmt.warning(`Failed: ${failCount}`);
  //       for (const r of results.filter(r => !r.success).slice(0, 10)) {
  //         fmt.info(`  ${r.taskId}: ${r.error}`);
  //       }
  //     }
  //     if (opts.json) {
  //       fmt.output({ success: successCount, failed: failCount, output: outputDir });
  //     }
  //   }, (opts) => new Formatter(Boolean(opts.json))));

  // ── Framework processing ─────────────────────────────────────────────────

  // cmd
  //   .command("framework-process")
  //   .description("Process collected framework files and pack into out.jar")
  //   .option("--source <dir>", "Source directory with collected files", String)
  //   .option("-o, --output <dir>", "Output directory", String)
  //   .option("--oem <oem>", "OEM (vivo, oppo, xiaomi, honor, google)", String, "google")
  //   .option("--clean", "Remove temp directories after processing")
  //   .option("--json", "JSON output")
  //   .action(withErrorHandler(async (opts) => {
  //     if (process.platform === "win32") {
  //       throw new Error("framework-process is not supported on Windows (requires extract.erofs / debugfs)");
  //     }
  //     const sourceDir = expandPath(opts.source ?? "~/.jiap/framework");
  //     const outDir = expandPath(opts.output ?? "~/.jiap/out");
  //     const fmt = new Formatter(opts.json ?? false);
  //
  //     if (!existsSync(sourceDir)) {
  //       throw new AdbOperationError(`Source directory does not exist: ${sourceDir}`);
  //     }
  //
  //     fmt.info(`Processing ${sourceDir} ...`);
  //
  //     const processor = new FrameworkProcessor({
  //       sourceDir,
  //       outDir,
  //       oem: opts.oem,
  //       clean: opts.clean ?? false,
  //     });
  //
  //     const { success, failed, errors, jarPath } = await processor.run();
  //
  //     fmt.success(`Done: ${success} succeeded, ${failed} failed`);
  //     if (jarPath) {
  //       fmt.info(`Output: ${jarPath}`);
  //     }
  //     if (errors.length > 0) {
  //       fmt.warning(`Errors (${errors.length}):`);
  //       for (const err of errors.slice(0, 10)) {
  //         fmt.info(`  ${err}`);
  //       }
  //     }
  //   }, (opts) => new Formatter(Boolean(opts.json))));

  return cmd;
}
