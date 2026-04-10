package jadx.plugins.jiap.utils

import com.google.gson.GsonBuilder
import jadx.api.JadxDecompiler
import jadx.api.impl.InMemoryCodeCache
import jadx.plugins.jiap.JiapConstants
import jadx.plugins.jiap.model.JiapError
import java.io.File
import java.util.concurrent.locks.ReentrantReadWriteLock
import kotlin.concurrent.read
import kotlin.concurrent.write

object PreferencesManager {

    private const val CONFIG_DIR_NAME = ".jiap"
    private const val CONFIG_FILE_NAME = "config.json"

    private val configLock = ReentrantReadWriteLock()

    private val configDir: File = File(System.getProperty("user.home"), CONFIG_DIR_NAME)
    private val configFile: File = File(configDir, CONFIG_FILE_NAME)
    private val gson = GsonBuilder().setPrettyPrinting().create()

    private data class JiapConfig(
        var port: Int = JiapConstants.DEFAULT_PORT,
        var cache: String = JiapConstants.DEFAULT_CACHE_MODE
    )

    @Volatile
    private var config: JiapConfig = JiapConfig()

    @Volatile
    private var configInitialized = false

    // ========== Initialization ==========

    /**
     * Initialize with a JADX decompiler (plugin mode).
     * Sets up code cache based on config.
     */
    fun initialize(decompiler: JadxDecompiler) {
        ensureConfigLoaded()
        setupCodeCache(decompiler)
    }

    /**
     * Initialize in standalone mode (no plugin context).
     * Only loads config file.
     */
    fun initializeStandalone() {
        ensureConfigLoaded()
    }

    private fun setupCodeCache(decompiler: JadxDecompiler) {
        val cache = configLock.read { config.cache }
        val cacheDir = getCacheDir()

        try {
            when (cache) {
                "memory" -> {
                    decompiler.args.setCodeCache(InMemoryCodeCache())
                }
                "disk" -> {
                    try {
                        val diskCacheClass = Class.forName("jadx.gui.cache.code.disk.DiskCodeCache")
                        val codeStringCacheClass = Class.forName("jadx.gui.cache.code.CodeStringCache")
                        val diskCache = diskCacheClass.getConstructor(java.nio.file.Path::class.java)
                            .newInstance(cacheDir.toPath())
                        val codeStringCache = codeStringCacheClass.getConstructor(diskCacheClass)
                            .newInstance(diskCache)
                        @Suppress("UNCHECKED_CAST")
                        decompiler.args.setCodeCache(codeStringCache as jadx.api.ICodeCache)
                    } catch (_: ClassNotFoundException) {
                        LogUtils.debug("Disk code cache not available (jadx-gui not loaded), using memory cache")
                        decompiler.args.setCodeCache(InMemoryCodeCache())
                    }
                }
            }
        } catch (e: Exception) {
            LogUtils.debug("Failed to setup code cache: ${e.message}")
        }
    }

    // ========== Port ==========

    fun setPort(port: Int) {
        configLock.write { config.port = port }
        saveConfig()
    }

    fun getPort(): Int = configLock.read { config.port }

    // ========== Cache ==========

    fun getCache(): String = configLock.read { config.cache }

    private fun getCacheDir(): File {
        return File(System.getProperty("user.home"), ".jiap/cache/").apply { mkdirs() }
    }

    fun clearCache() {
        val cacheDir = getCacheDir()
        if (cacheDir.exists()) {
            cacheDir.deleteRecursively()
            LogUtils.info("Code cache cleared: $cacheDir")
        }
    }

    // ========== Config File ==========

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
            if (!configDir.exists()) configDir.mkdirs()

            if (configFile.exists()) {
                val json = configFile.readText()
                config = gson.fromJson(json, JiapConfig::class.java) ?: JiapConfig()
                LogUtils.debug("Loaded config from $configFile")
            } else {
                config = JiapConfig()
                saveConfig()
                LogUtils.debug("Created default config at $configFile")
            }
        } catch (e: Exception) {
            LogUtils.error(JiapError.SERVICE_ERROR, "Failed to load config: ${e.message}")
            config = JiapConfig()
        }
    }

    private fun saveConfig() {
        try {
            if (!configDir.exists()) configDir.mkdirs()
            configFile.writeText(gson.toJson(config))
            LogUtils.debug("Saved config to $configFile")
        } catch (e: Exception) {
            LogUtils.error(JiapError.SERVICE_ERROR, "Failed to save config: ${e.message}")
        }
    }
}
