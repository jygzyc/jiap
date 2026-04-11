package jadx.plugins.jiap.mcp

import java.io.BufferedReader
import java.io.File
import java.io.InputStreamReader
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.TimeUnit
import java.nio.file.Files
import java.nio.file.StandardCopyOption

import jadx.plugins.jiap.utils.LogUtils
import jadx.plugins.jiap.utils.PluginUtils

class SidecarProcessManager(private var serverPort: Int) {

    private var process: Process? = null
    private val _isRunning = AtomicBoolean(false)
    private val lastStatus = AtomicBoolean(false)

    companion object {
        private const val PORT_CHECK_TIMEOUT_MS = 500
        private const val PORT_RELEASE_WAIT_MS = 2000L
    }

    fun updatePort(newPort: Int) {
        serverPort = newPort
    }

    fun isRunning(): Boolean {
        val running = process?.isAlive == true
        if (running != lastStatus.get()) {
            lastStatus.set(running)
            _isRunning.set(running)
        }
        return running
    }

    fun start(): Boolean {
        if (isRunning()) {
            LogUtils.info("[MCP] Sidecar is already running")
            return true
        }

        try {
            val executor = findExecutor() ?: run {
                LogUtils.warn("[MCP] Python executable not found")
                return false
            }

            val scriptPath = findScriptPath() ?: run {
                LogUtils.warn("[MCP] Sidecar script not found: jiap_mcp_server.py")
                return false
            }

            if (executor != "uv" && !checkDependencies(executor, scriptPath)) {
                LogUtils.warn("[MCP] Missing Python dependencies (requests, fastmcp)")
                return false
            }

            val mcpPort = serverPort + 1
            val jiapUrl = PluginUtils.buildServerUrl(port = serverPort)

            if (isPortInUse(mcpPort)) {
                LogUtils.info("[MCP] Port $mcpPort is in use, attempting to release...")
                killProcessOnPort(mcpPort)
                Thread.sleep(PORT_RELEASE_WAIT_MS)

                if (isPortInUse(mcpPort)) {
                    LogUtils.warn("[MCP] Port $mcpPort still occupied after cleanup attempt")
                    return false
                }
            }

            LogUtils.info("[MCP] Server starting...")

            val normalizedScriptPath = scriptPath.replace("\\", "/")

            val command = mutableListOf<String>()
            when (executor) {
                "uv" -> command.addAll(listOf("uv", "run", normalizedScriptPath, "--url", jiapUrl))
                else -> command.addAll(listOf(executor, normalizedScriptPath, "--url", jiapUrl))
            }

            val pb = ProcessBuilder(command)
            pb.directory(File(normalizedScriptPath).parentFile)

            val isWindows = System.getProperty("os.name").lowercase().contains("win")
            if (!isWindows) {
                pb.redirectErrorStream(true)
            }

            pb.environment()["JIAP_URL"] = jiapUrl
            pb.environment()["PYTHONIOENCODING"] = "utf-8"
            pb.environment()["LANG"] = "en_US.UTF-8"

            val p = pb.start()
            process = p
            _isRunning.set(true)
            lastStatus.set(true)

            if (isWindows) {
                Thread({
                    try {
                        val buffer = ByteArray(4096)
                        while (p.errorStream.read(buffer) != -1) {}
                    } catch (_: Exception) {}
                }, "MCP-stderr-consumer").apply { isDaemon = true }.start()
            }
            Thread({
                try {
                    val buffer = ByteArray(4096)
                    while (p.inputStream.read(buffer) != -1) {}
                } catch (_: Exception) {}
            }, "MCP-stdout-consumer").apply { isDaemon = true }.start()

            Thread.sleep(1000)
            if (!p.isAlive) {
                val exitCode = p.exitValue()
                LogUtils.warn("[MCP] Exited immediately code: $exitCode")
                return false
            }

            LogUtils.info("[MCP] Server started (Port: ${serverPort + 1})")
            return true
        } catch (e: Exception) {
            LogUtils.warn("[MCP] Start failed: ${e.message}")
            return false
        }
    }

    fun stop() {
        if (!isRunning()) return

        process?.let { p ->
            LogUtils.info("[MCP] Stopping server...")
            try {
                p.destroy()
                val terminated = p.waitFor(3, TimeUnit.SECONDS)
                if (!terminated || p.isAlive) {
                    LogUtils.debug("[MCP] Did not terminate gracefully, forcing...")
                    p.destroyForcibly()
                    p.waitFor(2, TimeUnit.SECONDS)
                }
                if (System.getProperty("os.name").lowercase().contains("win")) {
                    Thread.sleep(500)
                }
            } catch (e: Exception) {
                LogUtils.warn("[MCP] Stop failed: ${e.message}")
                p.destroyForcibly()
            }
            process = null
        }
        _isRunning.set(false)
        lastStatus.set(false)
        LogUtils.info("[MCP] Server stopped")
    }

    private fun checkDependencies(executor: String, scriptPath: String): Boolean {
        try {
            val pb = ProcessBuilder(executor, "-c", "import requests; import fastmcp; from pydantic import Field")
            pb.directory(File(scriptPath).parentFile)
            val p = pb.start()
            return p.waitFor(10, TimeUnit.SECONDS) && p.exitValue() == 0
        } catch (e: Exception) {
            LogUtils.debug("[MCP] Dependency check failed: ${e.message}")
            return false
        }
    }

    private fun findExecutor(): String? {
        for (cmd in listOf("uv", "python3", "python")) {
            try {
                val p = ProcessBuilder(cmd, "--version").start()
                if (p.waitFor() == 0) return cmd
            } catch (_: Exception) { continue }
        }
        return null
    }

    private fun findScriptPath(): String? {
        val mcpPath = File(System.getProperty("user.home"), ".jiap/mcp")

        try {
            if (!mcpPath.exists()) {
                mcpPath.mkdirs()
            }

            val scriptFile = File(mcpPath, "jiap_mcp_server.py")
            val projectFile = File(mcpPath, "pyproject.toml")
            val requirementsFile = File(mcpPath, "requirements.txt")

            LogUtils.info("[MCP] Extracting scripts in $mcpPath...")
            extractResource("/mcp/jiap_mcp_server.py", scriptFile)
            extractResource("/mcp/pyproject.toml", projectFile)
            extractResource("/mcp/requirements.txt", requirementsFile)

            if (scriptFile.exists()) {
                return scriptFile.absolutePath
            }
        } catch (e: Exception) {
            LogUtils.debug("[MCP] Failed to handle script: ${e.message}")
        }
        return null
    }

    private fun extractResource(resourcePath: String, targetFile: File) {
        try {
            SidecarProcessManager::class.java.getResourceAsStream(resourcePath)?.use { input ->
                Files.copy(input, targetFile.toPath(), StandardCopyOption.REPLACE_EXISTING)
            }
        } catch (e: Exception) {
            LogUtils.debug("[MCP] Extract failed $resourcePath: ${e.message}")
        }
    }

    fun cleanupMcpFiles() {
        try {
            val mcpPath = File(System.getProperty("user.home"), ".jiap/mcp")
            if (mcpPath.exists() && mcpPath.isDirectory) {
                LogUtils.info("[MCP] Cleaning up .jiap/mcp directory...")
                mcpPath.deleteRecursively()
                LogUtils.info("[MCP] Files cleanup completed")
            }
        } catch (e: Exception) {
            LogUtils.warn("[MCP] Failed to cleanup files: ${e.message}")
        }
    }

    private fun isPortInUse(port: Int): Boolean {
        return try {
            java.net.Socket("localhost", port).use {
                it.soTimeout = PORT_CHECK_TIMEOUT_MS
                true
            }
        } catch (_: Exception) { false }
    }

    private fun killProcessOnPort(port: Int) {
        val isWindows = System.getProperty("os.name").lowercase().contains("win")

        try {
            if (isWindows) {
                val pb = ProcessBuilder("cmd", "/c", "netstat -ano | findstr :$port | findstr LISTENING")
                pb.redirectErrorStream(true)
                val p = pb.start()
                val reader = BufferedReader(InputStreamReader(p.inputStream))
                val pids = mutableSetOf<String>()
                var line: String?
                while (reader.readLine().also { line = it } != null) {
                    line?.let {
                        val parts = it.trim().split(Regex("\\s+"))
                        if (parts.isNotEmpty()) {
                            val pid = parts.last()
                            if (pid.matches(Regex("\\d+"))) pids.add(pid)
                        }
                    }
                }
                p.waitFor()
                pids.forEach { pid ->
                    try {
                        ProcessBuilder("taskkill", "/F", "/PID", pid).start().waitFor(3, TimeUnit.SECONDS)
                    } catch (_: Exception) {}
                }
            } else {
                val pb = ProcessBuilder("sh", "-c", "lsof -t -i:$port 2>/dev/null")
                pb.redirectErrorStream(true)
                val p = pb.start()
                val reader = BufferedReader(InputStreamReader(p.inputStream))
                val pids = mutableSetOf<String>()
                var line: String?
                while (reader.readLine().also { line = it } != null) {
                    line?.trim()?.let { if (it.matches(Regex("\\d+"))) pids.add(it) }
                }
                p.waitFor()
                pids.forEach { pid ->
                    try {
                        ProcessBuilder("kill", "-9", pid).start().waitFor(2, TimeUnit.SECONDS)
                    } catch (_: Exception) {}
                }
            }
        } catch (e: Exception) {
            LogUtils.debug("[MCP] Error killing process on port $port: ${e.message}")
        }
    }
}
