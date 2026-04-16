import { Command } from "commander";
import { resolveClient } from "../core/client-helper.js";
import { Formatter } from "../utils/formatter.js";
import { withErrorHandler } from "../utils/errors.js";
import { logCliEvent } from "../utils/logger.js";
import {
  buildFramework,
  collectFramework,
  openFrameworkJar,
  resolveFrameworkJarPath,
  runFrameworkPipeline,
  summarizeFrameworkArtifact,
  summarizeFrameworkJarPath,
} from "../android/framework.js";

function addFrameworkCommonOptions(cmd: Command): Command {
  return cmd
    .option("--source-dir <dir>", "Framework source directory")
    .option("--out-dir <dir>", "Framework output directory")
    .option("--adb-path <path>", "ADB executable path")
    .option("--serial <serial>", "ADB device serial")
    .option("--clean-source", "Remove source/ after the command finishes successfully");
}

export function makeArdCommand(): Command {
  const cmd = new Command("ard");
  cmd.description("Android Specific Analysis");

  cmd
    .option("-s, --session <name>", "Target session by name")
    .option("-P, --port <port>", "Server port");

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

  const framework = cmd.command("framework").description("Collect and process Android framework files");

  addFrameworkCommonOptions(
    framework
      .command("collect")
      .description("Collect framework files from the connected Android device")
  )
    .action(withErrorHandler(async (opts) => {
      const fmt = new Formatter();
      const { oem, layout, result } = await collectFramework(opts);
      const artifact = summarizeFrameworkArtifact(layout, oem);
      logCliEvent({
        command: "ard",
        action: "framework_collect",
        ...artifact,
        sourceDir: layout.sourceDir,
        outDir: layout.outDir,
        scanned: result.scanned,
        pulled: result.pulled,
        failed: result.failed,
      });
      fmt.output({ artifact, layout, collection: result });
    }));

  addFrameworkCommonOptions(
    framework
      .command("process <oem>")
      .description("Process a local framework source directory for the specified oem and pack framework_<brand>_<vendor>.jar")
  )
    .action(withErrorHandler(async (oem: string, opts) => {
      const fmt = new Formatter();
      const result = await buildFramework({ ...opts, oem });
      const artifact = summarizeFrameworkArtifact(result.layout, oem);
      logCliEvent({
        command: "ard",
        action: "framework_process",
        ...artifact,
        sourceDir: result.layout.sourceDir,
        outDir: result.layout.outDir,
        processed: result.process.processed,
        failed: result.process.failed,
        packedFiles: result.pack.fileCount,
        cleanSource: opts.cleanSource ?? false,
      });
      fmt.output({ artifact, ...result });
    }));

  addFrameworkCommonOptions(
    framework
      .command("run")
      .description("Collect from the connected device, process, pack, and optionally open framework_<brand>_<vendor>.jar")
      .option("--no-open", "Do not open the generated framework jar in a DECX session")
      .option("-n, --name <name>", "Session name when opening the generated framework jar")
      .option("-P, --port <port>", "Server port when opening the generated framework jar")
  )
    .action(withErrorHandler(async (opts) => {
      const fmt = new Formatter();
      const result = await runFrameworkPipeline({ ...opts, noOpen: opts.open === false });
      const artifact = summarizeFrameworkJarPath(result.pack.jarPath);
      logCliEvent({
        command: "ard",
        action: "framework_run",
        ...(artifact ?? { jarPath: result.pack.jarPath }),
        sourceDir: result.layout.sourceDir,
        outDir: result.layout.outDir,
        scanned: result.collection?.scanned,
        pulled: result.collection?.pulled,
        processed: result.process.processed,
        failed: result.process.failed,
        packedFiles: result.pack.fileCount,
        opened: result.open !== undefined,
        cleanSource: opts.cleanSource ?? false,
      });
      fmt.output({ artifact, ...result });
    }));

  addFrameworkCommonOptions(
    framework
      .command("open [jar]")
      .description("Open the generated framework jar or a provided JAR in a DECX session")
      .option("-n, --name <name>", "Session name")
      .option("-P, --port <port>", "Server port")
  )
    .action(withErrorHandler(async (jar: string | undefined, opts) => {
      const fmt = new Formatter();
      const resolvedJar = await resolveFrameworkJarPath(jar, opts);
      const open = await openFrameworkJar(resolvedJar, opts);
      const artifact = summarizeFrameworkJarPath(resolvedJar);
      logCliEvent({
        command: "ard",
        action: "framework_open",
        ...(artifact ?? { jarPath: resolvedJar }),
      });
      fmt.output({ artifact, jar: resolvedJar, open });
    }));

  return cmd;
}
