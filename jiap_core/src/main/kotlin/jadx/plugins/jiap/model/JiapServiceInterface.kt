package jadx.plugins.jiap.model

import jadx.api.plugins.JadxPluginContext

interface JiapServiceInterface {
    val pluginContext: JadxPluginContext
    val decompiler get() = pluginContext.decompiler

    val gui: Boolean
        get() = isGui()

    fun isGui(): Boolean {
        return pluginContext.guiContext != null
    }
}