package jadx.plugins.jiap

import jadx.api.plugins.JadxPlugin
import jadx.api.plugins.JadxPluginContext
import jadx.api.plugins.JadxPluginInfo
import jadx.api.plugins.JadxPluginInfoBuilder
import jadx.plugins.jiap.ui.JiapUIManager
import jadx.plugins.jiap.utils.LogUtils
import jadx.plugins.jiap.utils.PreferencesManager
import jadx.plugins.jiap.utils.CacheUtils
import jadx.plugins.jiap.core.SidecarProcessManager
import jadx.plugins.jiap.core.JiapServer
import jadx.plugins.jiap.core.PluginLifecycleManager
import jadx.plugins.jiap.JiapConstants
import jadx.plugins.jiap.model.JiapError

class JiapPlugin : JadxPlugin {

    companion object {
        const val PLUGIN_NAME = "JIAP"
        const val PLUGIN_ID = "jadx-jiap-plugin"
    }

    private var sidecarManager: SidecarProcessManager? = null
    private var server: JiapServer? = null

    override fun init(ctx: JadxPluginContext) {
        try {
            ctx.decompiler?.let { decompiler ->
                PreferencesManager.initializeJadxArgs(decompiler)
            }
            
            PluginLifecycleManager(ctx) { srv, sidecar ->
                this.server = srv
                
                ctx.guiContext?.let { guiContext ->
                    JiapUIManager(ctx, srv, sidecar).initializeGuiComponents(guiContext)
                }
            }.start()

        } catch (e: Exception) {
            LogUtils.error(JiapError.SERVER_INTERNAL_ERROR, "Failed to initialize plugin", e)
            cleanupOnError()
            throw e
        }
    }

    override fun getPluginInfo(): JadxPluginInfo? {
        return JadxPluginInfoBuilder.pluginId(PLUGIN_ID)
            .name(PLUGIN_NAME)
            .description("Java Intelligence Analysis Platform - Bridges JADX with AI assistants via MCP")
            .homepage("https://github.com/jygzyc/jiap")
            .requiredJadxVersion("1.5.2, r2472")
            .build()
    }

    override fun unload() {
        try {
            LogUtils.info("Cleaning up JIAP plugin resources...")

            CacheUtils.clearCache()
            server?.stop()

        } catch (e: Exception) {
            LogUtils.error(JiapError.SERVER_INTERNAL_ERROR, "Error during plugin unload", e)
        }
    }

    private fun cleanupOnError() {
        server?.stop()
    }
}
