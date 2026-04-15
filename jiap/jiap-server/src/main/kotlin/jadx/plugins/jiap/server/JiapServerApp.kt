package jadx.plugins.jiap.server

import jadx.api.JadxDecompiler
import jadx.cli.JadxCLIArgs
import jadx.plugins.jiap.JiapConstants
import jadx.plugins.jiap.http.JiapServer
import jadx.plugins.jiap.utils.PluginUtils

/**
 * JIAP Server — Java Intelligence Analysis Platform.
 *
 * Usage:
 *   java -jar jiap-server.jar <file> [options]
 *
 * JIAP Options:
 *   -p, --port <port>           Server port (default: 25419)
 *
 * All standard jadx-cli options are also supported.
 * Run with --help for details.
 */
object JiapServerApp {

	@JvmStatic
	fun main(args: Array<String>) {
		val (port, jadxRawArgs) = extractPortAndFilterArgs(args)

		if (jadxRawArgs.contains("--help") || jadxRawArgs.contains("-h")) {
			printHelp()
			return
		}
		if (jadxRawArgs.contains("--version")) {
			println("JIAP ${JiapConstants.getVersion()}")
			return
		}

		val jadxArgs = try {
			val cliArgs = JadxCLIArgs()
			if (!cliArgs.processArgs(jadxRawArgs)) return
			cliArgs.toJadxArgs()
		} catch (e: Exception) {
			System.err.println("Error: ${e.message}")
			System.exit(1)
			return
		}

		// Validate input file
		val inputFiles = jadxArgs.inputFiles
		if (inputFiles.isNullOrEmpty()) {
			System.err.println("Error: No file specified.")
			System.err.println("Usage: java -jar jiap-server.jar <file> [options]")
			System.exit(1)
			return
		}
		val inputFile = inputFiles.first()
		if (!inputFile.exists()) {
			System.err.println("Error: File not found: ${inputFile.absolutePath}")
			System.exit(1)
			return
		}

		println("JIAP Server")
		println("==========")
		println("File:   ${inputFile.absolutePath}")
		println("Port:   $port")
		println()

		println("[*] Initializing decompiler...")
		val decompiler: JadxDecompiler
		try {
			decompiler = JadxDecompiler(jadxArgs)
			decompiler.load()
		} catch (e: Exception) {
			System.err.println("Error: Failed to initialize decompiler: ${e.message}")
			System.exit(1)
			return
		}

		println("[*] Loading classes...")
		val classCount = decompiler.classesWithInners?.size ?: 0
		if (classCount == 0) {
			System.err.println("Error: No classes found in ${inputFile.name}")
			System.exit(1)
			return
		}
		println("[+] Loaded $classCount classes")

		println("[*] Warming up decompiler engine...")
		val warmupStart = System.currentTimeMillis()
		val sdkPrefixes = listOf("android.support.", "androidx.", "java.", "javax.", "kotlin.", "kotlinx.")
		val appClasses = decompiler.classesWithInners.filter { cls ->
			sdkPrefixes.none { cls.fullName.startsWith(it) }
		}
		val toWarmup = if (appClasses.size > 15000) appClasses.shuffled().take(15000) else appClasses
		toWarmup.forEach { cls ->
			try { cls.decompile() } catch (_: Exception) {}
		}
		val warmupElapsed = System.currentTimeMillis() - warmupStart
		println("[+] Warmup completed in ${warmupElapsed}ms")

		val server = JiapServer.create(decompiler, port)
		val started = server.start(port)
		if (!started) {
			System.err.println("Error: Failed to start server on port $port")
			System.exit(1)
			return
		}

		val serverUrl = PluginUtils.buildServerUrl(port = port, running = true)
		println()
		println("[+] JIAP Server running at $serverUrl")
		println("[+] API: POST ${serverUrl}/api/jiap/<endpoint>")
		println("[+] Health: GET ${serverUrl}/health")
		println()
		println("Press Ctrl+C to stop.")

		try {
			Thread.currentThread().join()
		} catch (_: InterruptedException) {
			println("\n[*] Shutting down...")
			server.stop()
		}
	}

	private fun extractPortAndFilterArgs(args: Array<String>): Pair<Int, Array<String>> {
		var port = JiapConstants.DEFAULT_PORT
		val result = mutableListOf<String>()
		var i = 0
		while (i < args.size) {
			when (args[i]) {
				"--port", "-p" -> {
					i++
					if (i >= args.size) {
						System.err.println("Error: --port requires a value")
						System.exit(1)
					}
					port = args[i].toIntOrNull() ?: run {
						System.err.println("Error: Invalid port: ${args[i]}")
						System.exit(1)
						port
					}
				}
				else -> result.add(args[i])
			}
			i++
		}
		return Pair(port, result.toTypedArray())
	}

	private fun printHelp() {
		println("""
JIAP Server — Java Intelligence Analysis Platform

Usage:
  java -jar jiap-server.jar <file> [options]

Arguments:
  <file>                   Path to APK, DEX, JAR, AAR, or class file

JIAP Options:
  -p, --port <port>           Server port (default: ${JiapConstants.DEFAULT_PORT})

JADX Options:
  All standard jadx-cli options are supported. Common ones:
  -j, --threads-count <n>     Processing threads count
  --show-bad-code             Show inconsistent code
  --no-imports                Disable use of import statements
  --no-inline-anonymous       Disable anonymous classes inline
  -r, --no-res                Do not decode resources
  --no-debug-info             Disable debug info parsing
  --deobf                     Activate deobfuscation
  --escape-unicode            Escape non-ASCII characters in strings
  --log-level <level>         Set log level (quiet, progress, error, warn, info, debug)

  For full list of options, see: https://github.com/skylot/jadx

Examples:
  java -jar jiap-server.jar app.apk
  java -jar jiap-server.jar classes.dex --port 9000
  java -jar jiap-server.jar library.jar -j 8 --no-res --show-bad-code
  java -jar jiap-server.jar app.apk --deobf --no-imports

API Endpoints:
  POST /api/jiap/get_all_classes        Get all classes
  POST /api/jiap/get_class_source       Get class source (params: cls, smali)
  POST /api/jiap/get_method_source      Get method source (params: mth, smali)
  POST /api/jiap/search_class_key       Search by keyword (params: key)
  POST /api/jiap/search_method          Search methods (params: mth)
  POST /api/jiap/get_method_xref        Method cross-references (params: mth)
  POST /api/jiap/get_app_manifest       Get AndroidManifest.xml
  POST /api/jiap/get_exported_components  Get exported components
  GET  /health                           Health check
        """.trimIndent())
		println()
		println("JIAP version: ${JiapConstants.getVersion()}")
		println("JADX core:   (bundled)")
		println()
		println("License: GNU General Public License v3.0")
		println("Source:      https://github.com/jygzyc/jiap")
		println()
		print("CLI tool connects to this server using: jiapcli -P <port>\n")
		println()
	}
}
