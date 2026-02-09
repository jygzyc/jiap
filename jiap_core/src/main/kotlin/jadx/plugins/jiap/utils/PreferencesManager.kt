package jadx.plugins.jiap.utils

import jadx.api.JadxArgs
import jadx.api.JadxDecompiler
import jadx.api.impl.InMemoryCodeCache
import jadx.gui.cache.code.CodeStringCache
import jadx.gui.cache.code.disk.DiskCodeCache
import java.io.File
import jadx.plugins.jiap.JiapConstants
import jadx.plugins.jiap.utils.LogUtils
import java.util.concurrent.locks.ReentrantReadWriteLock
import kotlin.concurrent.read
import kotlin.concurrent.write

object PreferencesManager {

    private const val JIAP_PORT: String = "jiap.port"
    private const val JIAP_MCP_PATH: String = "jiap.mcp_path"
    private const val JIAP_CACHE: String = "jiap.cache"
    
    private val lock = ReentrantReadWriteLock()
    
    @Volatile
    private var jadxArgs: JadxArgs? = null
    
    @Volatile
    private lateinit var decompiler: JadxDecompiler
    
    fun initializeJadxArgs(decompiler: JadxDecompiler){
        this.decompiler = decompiler
        lock.write {
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
    
    fun getPreference(key: String, defaultValue: String): String{
        val args = lock.read {
            jadxArgs ?: throw Exception("PreferencesManager not initialized - JADX args not set")
        }
        return synchronized(args.pluginOptions) {
            args.pluginOptions.getOrDefault(key, defaultValue)
        }
    }
    
    fun setPreference(key: String, value: String){
        val args = lock.read {
            jadxArgs ?: throw Exception("PreferencesManager not initialized - JADX args not set")
        }
        synchronized(args.pluginOptions) {
            args.pluginOptions[key] = value
        }
    }
    
    fun setPort(port: Int){
        setPreference(JIAP_PORT, port.toString())
    }
    
    fun getPort(): Int{
        return try {
            getPreference(JIAP_PORT, JiapConstants.DEFAULT_PORT.toString()).toInt()
        } catch (e: NumberFormatException) {
            JiapConstants.DEFAULT_PORT
        }
    }
    
    fun setMcpPath(path: String){
        setPreference(JIAP_MCP_PATH, path)
    }
    
    fun getMcpPath(): String{
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
}
