package jadx.plugins.jiap.server

import jadx.api.JadxArgs
import jadx.api.JadxDecompiler
import jadx.plugins.jiap.JiapConstants
import jadx.plugins.jiap.http.JiapServer
import jadx.plugins.jiap.utils.PluginUtils
import java.io.File

/**
 * JIAP Server — Java Intelligence Analysis Platform.
 *
 * Usage:
 *   java -jar jiap-server.jar <file> [options]
 *
 * Options:
 *   -p, --port <port>           Server port (default: 25419)
 *   -t, --threads <n>           Decompilation threads (default: auto)
 *   --show-bad-code            Show inconsistent code
 *   --no-imports                Don't use import statements
 *   --no-inline-anonymous       Don't inline anonymous classes
 *   --no-res                   Skip resource decoding
 *   --no-debug-info             Don't decode debug info
 *   --deobf                     Activate deobfuscation
 *   --escape-unicode            Escape non-ASCII characters in strings
 *   --help                     Show this help
 */
object JiapServerApp {

    @JvmStatic
    fun main(args: Array<String>) {
        val parsed = parseArgs(args)
        if (parsed == null) return

        val inputFile = parsed.inputPath
        if (!inputFile.exists()) {
            System.err.println("Error: File not found: ${inputFile.absolutePath}")
            System.exit(1)
        }

        println("JIAP Server")
        println("==========")
        println("File:   ${inputFile.absolutePath}")
        println("Port:   ${parsed.port}")
        println()

        println("[*] Initializing decompiler...")
        val jadxArgs = JadxArgs()
        jadxArgs.inputFiles = listOf(inputFile)
        if (parsed.threads > 0) jadxArgs.threadsCount = parsed.threads
        if (parsed.showBadCode) jadxArgs.setShowInconsistentCode(true)
        if (parsed.noImports) jadxArgs.setUseImports(false)
        if (parsed.noInlineAnonymous) jadxArgs.setInlineAnonymousClasses(false)
        if (parsed.noRes) jadxArgs.setSkipResources(true)
        if (parsed.noDebugInfo) jadxArgs.setDebugInfo(false)
        if (parsed.deobf) jadxArgs.setDeobfuscationOn(true)
        if (parsed.escapeUnicode) jadxArgs.setEscapeUnicode(true)

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

        val server = JiapServer.create(decompiler, parsed.port)
        val started = server.start(parsed.port)
        if (!started) {
            System.err.println("Error: Failed to start server on port ${parsed.port}")
            System.exit(1)
            return
        }

        val serverUrl = PluginUtils.buildServerUrl(port = parsed.port, running = true)
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

    private data class ParsedArgs(
        val inputPath: File,
        val port: Int,
        val threads: Int,
        val showBadCode: Boolean,
        val noImports: Boolean,
        val noInlineAnonymous: Boolean,
        val noRes: Boolean,
        val noDebugInfo: Boolean,
        val deobf: Boolean,
        val escapeUnicode: Boolean,
    )

    private fun parseArgs(args: Array<String>): ParsedArgs? {
        var port = JiapConstants.DEFAULT_PORT
        var threads = 0
        var showBadCode = false
        var noImports = false
        var noInlineAnonymous = false
        var noRes = false
        var noDebugInfo = false
        var deobf = false
        var escapeUnicode = false
        var inputPath: String? = null
        var i = 0

        while (i < args.size) {
            when (args[i]) {
                "-p", "--port" -> {
                    i++
                    if (i >= args.size) {
                        System.err.println("Error: --port requires a value")
                        System.exit(1)
                        return null
                    }
                    port = args[i].toIntOrNull() ?: run {
                        System.err.println("Error: Invalid port: ${args[i]}")
                        System.exit(1)
                        return null
                    }
                }
                "-t", "--threads" -> {
                    i++
                    if (i >= args.size) {
                        System.err.println("Error: --threads requires a value")
                        System.exit(1)
                        return null
                    }
                    threads = args[i].toIntOrNull() ?: run {
                        System.err.println("Error: Invalid threads: ${args[i]}")
                        System.exit(1)
                        return null
                    }
                }
                "--show-bad-code" -> showBadCode = true
                "--no-imports" -> noImports = true
                "--no-inline-anonymous" -> noInlineAnonymous = true
                "--no-res" -> noRes = true
                "--no-debug-info" -> noDebugInfo = true
                "--deobf" -> deobf = true
                "--escape-unicode" -> escapeUnicode = true
                "--help", "-h" -> {
                    printHelp()
                    return null
                }
                else -> {
                    if (inputPath != null) {
                        System.err.println("Error: Unexpected argument: ${args[i]}")
                        System.err.println("Only one file can be specified.")
                        System.exit(1)
                        return null
                    }
                    inputPath = args[i]
                }
            }
            i++
        }

        if (inputPath == null) {
            System.err.println("Error: No file specified.")
            System.err.println("Usage: java -jar jiap-server.jar <file> [options]")
            System.err.println("Run with --help for more information.")
            System.exit(1)
            return null
        }

        return ParsedArgs(File(inputPath), port, threads, showBadCode, noImports, noInlineAnonymous, noRes, noDebugInfo, deobf, escapeUnicode)
    }

    private fun printHelp() {
        println("""
JIAP Server — Java Intelligence Analysis Platform

Usage:
  java -jar jiap-server.jar <file> [options]

Arguments:
  <file>                   Path to APK, DEX, JAR, AAR, or class file

Options:
  -p, --port <port>           Server port (default: ${JiapConstants.DEFAULT_PORT})
  -t, --threads <n>           Decompilation threads (default: auto)
  --show-bad-code            Show inconsistent code
  --no-imports                Don't use import statements
  --no-inline-anonymous       Don't inline anonymous classes
  --no-res                   Skip resource decoding
  --no-debug-info             Don't decode debug info
  --deobf                     Activate deobfuscation
  --escape-unicode            Escape non-ASCII characters in strings
  -h, --help                  Show this help

Examples:
  java -jar jiap-server.jar app.apk
  java -jar jiap-server.jar classes.dex --port 9000
  java -jar jiap-server.jar library.jar -t 8 --no-res --show-bad-code
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
        println("JIAP version: ${getVersion()}")
        println("JADX core:   (bundled)")
        println()
        println("License: GNU General Public License v3.0")
        println("Source:      https://github.com/jygzyc/jiap")
        println()
        print("CLI tool connects to this server using: jiapcli -P <port>\n")
        println()
    }

    private fun getVersion(): String {
        return System.getenv("JIAP_VERSION") ?: "dev"
    }
}
