package jadx.plugins.jiap.utils

import jadx.api.JadxArgs
import jadx.api.JadxDecompiler

object PreferencesManager {

    const val JIAP_PORT: String = "jiap.port"
    lateinit var jadxArgs: JadxArgs

    fun initializeJadxArgs(decompiler: JadxDecompiler){
        jadxArgs = decompiler.args
    }

    fun getPreference(key: String, defaultValue: String): String{
        return jadxArgs.pluginOptions.getOrDefault(key, defaultValue)
    }

    fun setPreference(key: String, value: String){
        jadxArgs.pluginOptions[key] = value
    }

    fun setPort(port: Int){
        setPreference(JIAP_PORT, port.toString())
    }

    fun getPort(): Int{
        return getPreference(JIAP_PORT, JiapConstants.DEFAULT_PORT.toString()).toInt()
    }
}