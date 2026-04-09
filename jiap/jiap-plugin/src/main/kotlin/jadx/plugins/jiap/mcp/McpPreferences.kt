package jadx.plugins.jiap.mcp

import java.io.File

/**
 * MCP-specific preferences. Only used in plugin mode.
 * Extends core PreferencesManager with MCP-only settings.
 */
object McpPreferences {

    private const val CONFIG_DIR_NAME = ".jiap"
    private const val CONFIG_FILE_NAME = "mcp.json"

    private val configDir = File(System.getProperty("user.home"), CONFIG_DIR_NAME)
    private val configFile = File(configDir, CONFIG_FILE_NAME)
    private val gson = com.google.gson.GsonBuilder().setPrettyPrinting().create()

    private data class McpConfig(
        var autoStart: Boolean = false
    )

    @Volatile
    private var config: McpConfig = McpConfig()

    fun getAutoStart(): Boolean {
        return config.autoStart
    }

    fun setAutoStart(enabled: Boolean) {
        config.autoStart = enabled
        saveConfig()
    }

    fun getMcpPath(): String {
        return File(configDir, "mcp").absolutePath
    }

    private fun ensureLoaded() {
        if (!configFile.exists()) {
            saveConfig()
            return
        }
        try {
            val json = configFile.readText()
            config = gson.fromJson(json, McpConfig::class.java) ?: McpConfig()
        } catch (_: Exception) {
            config = McpConfig()
        }
    }

    private fun saveConfig() {
        try {
            if (!configDir.exists()) configDir.mkdirs()
            configFile.writeText(gson.toJson(config))
        } catch (_: Exception) {
            // ignore
        }
    }

    init {
        ensureLoaded()
    }
}
