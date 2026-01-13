package jadx.plugins.jiap.utils

import jadx.api.JadxArgs
import jadx.api.JadxDecompiler
import jadx.plugins.jiap.JiapConstants

object PreferencesManager {

    const val JIAP_PORT: String = "jiap.port"
    const val JIAP_MCP_PATH: String = "jiap.mcp_path"
    lateinit var jadxArgs: JadxArgs

    fun initializeJadxArgs(decompiler: JadxDecompiler){
        jadxArgs = decompiler.args
    }

    fun getPreference(key: String, defaultValue: String): String{
        if (!::jadxArgs.isInitialized) {
            throw Exception("PreferencesManager not initialized - JADX args not set")
        }
        return synchronized(jadxArgs.pluginOptions) {
            jadxArgs.pluginOptions.getOrDefault(key, defaultValue)
        }
    }

    fun setPreference(key: String, value: String){
        if (!::jadxArgs.isInitialized) {
            throw Exception("PreferencesManager not initialized - JADX args not set")
        }
        synchronized(jadxArgs.pluginOptions) {
            jadxArgs.pluginOptions[key] = value
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
}
