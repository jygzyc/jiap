package jadx.plugins.jiap.core

import jadx.api.plugins.JadxPluginContext
import java.io.BufferedReader
import java.io.File
import java.io.InputStreamReader
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.TimeUnit
import java.nio.file.Files
import java.nio.file.StandardCopyOption
import java.nio.file.Path
import java.nio.file.Paths

import jadx.plugins.jiap.JiapConstants
import jadx.plugins.jiap.model.JiapError
import jadx.plugins.jiap.utils.LogUtils
import jadx.plugins.jiap.utils.PreferencesManager
import jadx.plugins.jiap.utils.PluginUtils

class SidecarProcessManager(private val pluginContext: JadxPluginContext) {
    private var process: Process? = null
    private val _isRunning = AtomicBoolean(false)
    private val lastStatus = AtomicBoolean(false)
    private val mcpPath: File
        get() = File(PreferencesManager.getMcpPath())
    
    companion object {
        private const val PORT_CHECK_TIMEOUT_MS = 500
        private const val PORT_RELEASE_WAIT_MS = 2000L
        private const val MAX_PORT_RETRY_ATTEMPTS = 3
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
            LogUtils.info("MCP Sidecar is already running")
            return true
        }

        try {
            val executor = findExecutor() ?: run {
                LogUtils.error(JiapError.PYTHON_NOT_FOUND)
                return false
            }

            val scriptPath = findScriptPath() ?: run {
                LogUtils.error(JiapError.SIDECAR_SCRIPT_NOT_FOUND, "jiap_mcp_server.py")
                return false
            }

            if (!checkDependencies(executor, scriptPath)) {
                LogUtils.error(JiapError.SERVICE_ERROR, "Missing Python dependencies (requests, fastmcp)")
                return false
            }

            val port = PreferencesManager.getPort()
            val mcpPort = port + 1
            val jiapUrl = PluginUtils.buildServerUrl(running=true, port=port)

            // Check and release port if occupied
            if (isPortInUse(mcpPort)) {
                LogUtils.info("Port $mcpPort is currently in use, attempting to release...")
                killProcessOnPort(mcpPort)
                Thread.sleep(PORT_RELEASE_WAIT_MS)
                
                if (isPortInUse(mcpPort)) {
                    LogUtils.error(JiapError.SIDECAR_START_FAILED, "Port $mcpPort is still occupied after cleanup attempt")
                    return false
                }
            }

            LogUtils.info("MCP Server starting...")

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

            // 统一日志处理器
            fun startLogger(inputStream: java.io.InputStream, prefix: String) {
                Thread({
                    try {
                        val reader = BufferedReader(InputStreamReader(inputStream, java.nio.charset.StandardCharsets.UTF_8))
                        var line: String?
                        while (reader.readLine().also { line = it } != null) {
                            LogUtils.info("$prefix $line")
                        }
                    } catch (e: Exception) {
                        if (isRunning()) {
                            LogUtils.debug("$prefix error: ${e.message}")
                        }
                    }
                }, prefix).apply { isDaemon = true }.start()
            }

            startLogger(p.inputStream, "[MCP STDOUT]")
            if (isWindows) {
                startLogger(p.errorStream, "[MCP STDERR]")
            }

            Thread.sleep(1000)
            if (!p.isAlive) {
                val exitCode = p.exitValue()
                LogUtils.error(JiapError.SIDECAR_START_FAILED, "Exited immediately code: $exitCode")
                return false
            }

            LogUtils.info("MCP Server started (Port: ${port + 1})")
            return true
        } catch (e: Exception) {
            LogUtils.error(JiapError.SIDECAR_START_FAILED, e.message ?: "Unknown error")
            return false
        }
    }

    fun stop() {
        if (!isRunning()) return

        process?.let { p ->
            LogUtils.info("Stopping MCP Server...")
            try {

                p.destroy()
                val terminated = p.waitFor(3, TimeUnit.SECONDS)

                if (!terminated || p.isAlive) {
                    LogUtils.debug("MCP Server did not terminate gracefully, forcing...")
                    p.destroyForcibly()
                    p.waitFor(2, TimeUnit.SECONDS)
                }

                if (System.getProperty("os.name").lowercase().contains("win")) {
                    Thread.sleep(500)
                }

            } catch (e: Exception) {
                LogUtils.error(JiapError.SIDECAR_STOP_FAILED, e.message ?: "Stop failed")
                p.destroyForcibly()
            }
            process = null
        }
        _isRunning.set(false)
        lastStatus.set(false)
        LogUtils.info("MCP Server stopped")
    }

    private fun checkDependencies(executor: String, scriptPath: String): Boolean {
        if (executor == "uv") return true
        
        try {
            LogUtils.debug("Checking dependencies for $executor...")
            val pb = ProcessBuilder(executor, "-c", "import requests; import fastmcp; from pydantic import Field")
            pb.directory(File(scriptPath).parentFile)
            val p = pb.start()
            if (p.waitFor(10, TimeUnit.SECONDS) && p.exitValue() == 0) {
                return true
            }
            return false
        } catch (e: Exception) {
            LogUtils.debug("Dependency check failed: ${e.message}")
            return false
        }
    }

    private fun findExecutor(): String? {
        val candidates = listOf("uv", "python3", "python")
        for (cmd in candidates) {
            try {
                val p = ProcessBuilder(cmd, "--version").start()
                if (p.waitFor() == 0) return cmd
            } catch (e: Exception) {
                continue
            }
        }
        return null
    }

    private fun findScriptPath(): String? {
        val configuredPath = PreferencesManager.getMcpPath()
        if (configuredPath.isNotBlank()) {
            val file = File(configuredPath)
            if (file.exists()) {
                if (file.isDirectory) {
                    val scriptInDir = File(file, "jiap_mcp_server.py")
                    if (scriptInDir.exists()) {
                        return scriptInDir.absolutePath
                    }
                } else if (file.isFile) {
                    return file.absolutePath
                }
            }
        }

        try {
            if (!mcpPath.exists()) {
                mcpPath.mkdirs()
            }

            val scriptFile = File(mcpPath, "jiap_mcp_server.py")
            val projectFile = File(mcpPath, "pyproject.toml")
            val requirementsFile = File(mcpPath, "requirements.txt")

            LogUtils.info("Extracting/Updating MCP scripts in $mcpPath...")
            extractResource("/mcp/jiap_mcp_server.py", scriptFile)
            extractResource("/mcp/pyproject.toml", projectFile)
            extractResource("/mcp/requirements.txt", requirementsFile)

            if (scriptFile.exists()) {
                return scriptFile.absolutePath
            }
        } catch (e: Exception) {
            LogUtils.debug("Failed to handle MCP script in home directory: ${e.message}")
        }
        return null
    }

    private fun extractResource(resourcePath: String, targetFile: File) {
        try {
            SidecarProcessManager::class.java.getResourceAsStream(resourcePath)?.use { input ->
                Files.copy(input, targetFile.toPath(), StandardCopyOption.REPLACE_EXISTING)
            }
        } catch (e: Exception) {
              LogUtils.error(JiapError.SIDECAR_SCRIPT_NOT_FOUND, "Extract failed $resourcePath: ${e.message}")
        }
    }

    fun cleanupMcpFiles() {
        try {

            if (mcpPath.exists() && mcpPath.isDirectory) {
                LogUtils.info("Cleaning up .jiap/mcp directory...")
                mcpPath.deleteRecursively()
                LogUtils.info("MCP files cleanup completed")
            }
        } catch (e: Exception) {
            LogUtils.error(JiapError.SERVICE_ERROR, "Failed to cleanup MCP files: ${e.message}")
        }
    }

    /**
     * Check if a port is currently in use by attempting to bind to it.
     * Uses socket connection test with short timeout.
     */
    private fun isPortInUse(port: Int): Boolean {
        return try {
            java.net.Socket("localhost", port).use { 
                it.soTimeout = PORT_CHECK_TIMEOUT_MS
                true 
            }
        } catch (e: Exception) {
            false
        }
    }

    /**
     * Attempt to kill any process occupying the specified port.
     * Uses OS-specific commands to find and terminate the process.
     */
    private fun killProcessOnPort(port: Int) {
        val isWindows = System.getProperty("os.name").lowercase().contains("win")
        
        try {
            if (isWindows) {
                val pb = ProcessBuilder("cmd", "/c", "netstat -ano | findstr :$port | findstr LISTENING")
                pb.redirectErrorStream(true)
                val p = pb.start()
                
                val reader = BufferedReader(InputStreamReader(p.inputStream))
                var line: String?
                val pids = mutableSetOf<String>()
                
                while (reader.readLine().also { line = it } != null) {
                    line?.let { 
                        val parts = it.trim().split(Regex("\\s+"))
                        if (parts.isNotEmpty()) {
                            val pid = parts.last()
                            if (pid.matches(Regex("\\d+"))) {
                                pids.add(pid)
                            }
                        }
                    }
                }
                p.waitFor()
                
                pids.forEach { pid ->
                    try {
                        LogUtils.info("Terminating process $pid on port $port")
                        val killPb = ProcessBuilder("taskkill", "/F", "/PID", pid)
                        killPb.start().waitFor(3, TimeUnit.SECONDS)
                    } catch (e: Exception) {
                        LogUtils.debug("Failed to kill process $pid: ${e.message}")
                    }
                }
            } else {
                val pb = ProcessBuilder("sh", "-c", "lsof -t -i:$port || netstat -tlnp 2>/dev/null | grep ':$port ' | awk '{print \\$7}' | cut -d'/' -f1")
                pb.redirectErrorStream(true)
                val p = pb.start()
                
                val reader = BufferedReader(InputStreamReader(p.inputStream))
                var line: String?
                val pids = mutableSetOf<String>()
                
                while (reader.readLine().also { line = it } != null) {
                    line?.trim()?.let { if (it.matches(Regex("\\d+"))) pids.add(it) }
                }
                p.waitFor()
                
                pids.forEach { pid ->
                    try {
                        LogUtils.info("Terminating process $pid on port $port")
                        val killPb = ProcessBuilder("kill", "-9", pid)
                        killPb.start().waitFor(2, TimeUnit.SECONDS)
                    } catch (e: Exception) {
                        LogUtils.debug("Failed to kill process $pid: ${e.message}")
                    }
                }
            }
        } catch (e: Exception) {
            LogUtils.debug("Error killing process on port $port: ${e.message}")
        }
    }
}
