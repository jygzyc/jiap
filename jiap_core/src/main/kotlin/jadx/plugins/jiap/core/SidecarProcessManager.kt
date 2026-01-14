package jadx.plugins.jiap.core

import jadx.api.plugins.JadxPluginContext
import java.io.BufferedReader
import java.io.File
import java.io.InputStreamReader
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.TimeUnit
import java.nio.file.Files
import java.nio.file.StandardCopyOption

import jadx.plugins.jiap.model.JiapError
import jadx.plugins.jiap.utils.LogUtils
import jadx.plugins.jiap.utils.PreferencesManager
import jadx.plugins.jiap.utils.PluginUtils

class SidecarProcessManager(private val pluginContext: JadxPluginContext) {
    private var process: Process? = null
    private val _isRunning = AtomicBoolean(false)
    private val lastStatus = AtomicBoolean(false)

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

            LogUtils.info("Sidecar starting...")
            
            val command = mutableListOf<String>()
            when (executor) {
                "uv" -> command.addAll(listOf("uv", "run", scriptPath, "--jiap-url", jiapUrl, "--mcp-port", mcpPort.toString()))
                else -> command.addAll(listOf(executor, scriptPath, "--jiap-url", jiapUrl, "--mcp-port", mcpPort.toString()))
            }

            val pb = ProcessBuilder(command)
            pb.directory(File(scriptPath).parentFile)
            
            val isWindows = System.getProperty("os.name").lowercase().contains("win")
            if (!isWindows) {
                pb.redirectErrorStream(true)
            }
            
            pb.environment()["JIAP_URL"] = jiapUrl
            
            val p = pb.start()
            process = p
            _isRunning.set(true)
            lastStatus.set(true)

            // 统一日志处理器
            fun startLogger(inputStream: java.io.InputStream, prefix: String) {
                Thread({
                    try {
                        val reader = BufferedReader(InputStreamReader(inputStream))
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

            startLogger(p.inputStream, "[MCP Sidecar STDOUT]")
            if (isWindows) {
                startLogger(p.errorStream, "[MCP Sidecar STDERR]")
            }

            Thread.sleep(1000)
            if (!p.isAlive) {
                val exitCode = p.exitValue()
                LogUtils.error(JiapError.SIDECAR_START_FAILED, "Exited immediately code: $exitCode")
                return false
            }

            LogUtils.info("Sidecar started (Port: $mcpPort)")
            return true
        } catch (e: Exception) {
            LogUtils.error(JiapError.SIDECAR_START_FAILED, e.message ?: "Unknown error")
            return false
        }
    }

    fun stop() {
        if (!isRunning()) return

        process?.let { p ->
            LogUtils.info("Stopping sidecar...")
            try {

                p.destroy()
                val terminated = p.waitFor(3, TimeUnit.SECONDS)

                if (!terminated || p.isAlive) {
                    LogUtils.debug("Sidecar did not terminate gracefully, forcing...")
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
        LogUtils.info("Sidecar stopped")
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
            val userHome = System.getProperty("user.home")
            val jiapDir = File(userHome, ".jiap/mcp")
            if (!jiapDir.exists()) {
                jiapDir.mkdirs()
            }

            val scriptFile = File(jiapDir, "jiap_mcp_server.py")
            val projectFile = File(jiapDir, "pyproject.toml")
            val requirementsFile = File(jiapDir, "requirements.txt")

            LogUtils.info("Extracting/Updating MCP scripts in $jiapDir...")
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
}
