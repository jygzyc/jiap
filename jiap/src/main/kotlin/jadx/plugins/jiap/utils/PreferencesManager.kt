package jadx.plugins.jiap.utils

import com.google.gson.Gson
import com.google.gson.GsonBuilder
import jadx.api.JadxArgs
import jadx.api.JadxDecompiler
import jadx.api.impl.InMemoryCodeCache
import jadx.gui.cache.code.CodeStringCache
import jadx.gui.cache.code.disk.DiskCodeCache
import jadx.plugins.jiap.JiapConstants
import jadx.plugins.jiap.model.JiapError
import java.io.File
import java.util.concurrent.locks.ReentrantReadWriteLock
import kotlin.concurrent.read
import kotlin.concurrent.write

object PreferencesManager {

    private const val JIAP_PORT = "jiap.port"
    private const val JIAP_MCP_PATH = "jiap.mcp_path"
    private const val JIAP_CACHE = "jiap.cache"

    // Config File (persisted to ~/.jiap/config.json)
    private const val CONFIG_DIR_NAME = ".jiap"
    private const val CONFIG_FILE_NAME = "config.json"

    private val argsLock = ReentrantReadWriteLock()
    private val configLock = ReentrantReadWriteLock()

    @Volatile
    private var jadxArgs: JadxArgs? = null

    @Volatile
    private lateinit var decompiler: JadxDecompiler

    private val configDir: File = File(System.getProperty("user.home"), CONFIG_DIR_NAME)
    private val configFile: File = File(configDir, CONFIG_FILE_NAME)
    private val gson: Gson = GsonBuilder().setPrettyPrinting().create()

    private data class JiapConfig(
        var autoStartMcp: Boolean = false
    )

    @Volatile
    private var config: JiapConfig = JiapConfig()

    @Volatile
    private var configInitialized = false

    // ========== Initialization ==========

    fun initializeJadxArgs(decompiler: JadxDecompiler) {
        this.decompiler = decompiler
        argsLock.write {
            jadxArgs = decompiler.args
        }
        setupCodeCache()
    }

    private fun setupCodeCache() {
        val args = jadxArgs ?: return

        try {
            val cache = getCache()
            val cacheDir = getCacheDir()

            when (cache) {
                "memory" -> {
                    args.setCodeCache(InMemoryCodeCache())
                }
                "disk" -> {
                    val diskCache = DiskCodeCache(decompiler.root, cacheDir.toPath())
                    args.setCodeCache(CodeStringCache(diskCache))
                }
            }
        } catch (e: Exception) {
            LogUtils.debug("Failed to setup code cache: ${e.message}")
        }
    }

    // ========== JADX Args Preferences ==========

    private fun getPreference(key: String, defaultValue: String): String {
        val args = argsLock.read {
            jadxArgs ?: throw Exception("PreferencesManager not initialized - JADX args not set")
        }
        return synchronized(args.pluginOptions) {
            args.pluginOptions.getOrDefault(key, defaultValue)
        }
    }

    private fun setPreference(key: String, value: String) {
        val args = argsLock.read {
            jadxArgs ?: throw Exception("PreferencesManager not initialized - JADX args not set")
        }
        synchronized(args.pluginOptions) {
            args.pluginOptions[key] = value
        }
    }

    fun setPort(port: Int) {
        setPreference(JIAP_PORT, port.toString())
    }

    fun getPort(): Int {
        return try {
            getPreference(JIAP_PORT, JiapConstants.DEFAULT_PORT.toString()).toInt()
        } catch (e: NumberFormatException) {
            JiapConstants.DEFAULT_PORT
        }
    }

    fun setMcpPath(path: String) {
        setPreference(JIAP_MCP_PATH, path)
    }

    fun getMcpPath(): String {
        return try {
            getPreference(JIAP_MCP_PATH, JiapConstants.DEFAULT_MCP_SIDECAR_SCRIPT)
        } catch (e: Exception) {
            JiapConstants.DEFAULT_MCP_SIDECAR_SCRIPT
        }
    }

    fun getCache(): String {
        return try {
            getPreference(JIAP_CACHE, JiapConstants.DEFAULT_CACHE_MODE)
        } catch (e: Exception) {
            JiapConstants.DEFAULT_CACHE_MODE
        }
    }

    private fun getCacheDir(): File {
        val userHome = System.getProperty("user.home")
        return File(userHome, ".jiap/cache/").apply { mkdirs() }
    }

    fun clearCache() {
        val cacheDir = getCacheDir()
        if (cacheDir.exists()) {
            cacheDir.deleteRecursively()
            LogUtils.info("Code cache cleared: $cacheDir")
        }
    }


    private fun ensureConfigLoaded() {
        if (!configInitialized) {
            configLock.write {
                if (!configInitialized) {
                    loadConfig()
                    configInitialized = true
                }
            }
        }
    }

    private fun loadConfig() {
        try {
            if (!configDir.exists()) {
                configDir.mkdirs()
            }

            if (configFile.exists()) {
                val json = configFile.readText()
                config = gson.fromJson(json, JiapConfig::class.java) ?: JiapConfig()
                LogUtils.debug("Loaded config from $configFile: autoStartMcp=${config.autoStartMcp}")
            } else {
                config = JiapConfig()
                saveConfigInternal()
                LogUtils.debug("Created default config at $configFile")
            }
        } catch (e: Exception) {
            LogUtils.error(JiapError.SERVICE_ERROR, "Failed to load config: ${e.message}")
            config = JiapConfig()
        }
    }

    private fun saveConfigInternal() {
        try {
            if (!configDir.exists()) {
                configDir.mkdirs()
            }
            val json = gson.toJson(config)
            configFile.writeText(json)
            LogUtils.debug("Saved config to $configFile")
        } catch (e: Exception) {
            LogUtils.error(JiapError.SERVICE_ERROR, "Failed to save config: ${e.message}")
        }
    }

    fun getAutoStartMcp(): Boolean {
        ensureConfigLoaded()
        return configLock.read {
            config.autoStartMcp
        }
    }

    fun setAutoStartMcp(enabled: Boolean) {
        ensureConfigLoaded()
        configLock.write {
            config.autoStartMcp = enabled
        }
        saveConfigInternal()
    }
}
