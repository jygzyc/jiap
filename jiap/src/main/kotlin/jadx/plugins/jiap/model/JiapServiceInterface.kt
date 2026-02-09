package jadx.plugins.jiap.model

import jadx.api.plugins.JadxPluginContext
import jadx.api.JadxDecompiler

interface JiapServiceInterface {
    val pluginContext: JadxPluginContext
    val decompiler: JadxDecompiler get() = pluginContext.decompiler

    val gui: Boolean
        get() = isGui()

    fun isGui(): Boolean {
        return pluginContext.guiContext != null
    }
}