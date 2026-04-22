package jadx.plugins.decx

import jadx.api.plugins.JadxPlugin
import jadx.api.plugins.JadxPluginContext
import jadx.api.plugins.JadxPluginInfo
import jadx.api.plugins.JadxPluginInfoBuilder
import jadx.plugins.decx.api.DecxApi
import jadx.plugins.decx.http.DecxServer
import jadx.plugins.decx.lifecycle.PluginLifecycleManager
import jadx.plugins.decx.model.DecxError
import jadx.plugins.decx.ui.DecxUIManager
import jadx.plugins.decx.mcp.SidecarProcessManager
import jadx.plugins.decx.utils.CacheUtils
import jadx.plugins.decx.utils.LogUtils
import jadx.plugins.decx.utils.PreferencesManager

class DecxPlugin : JadxPlugin {

    companion object {
        const val PLUGIN_NAME = "Decx"
        const val PLUGIN_ID = "jadx-decx-plugin"
    }

    private var server: DecxServer? = null
    private var sidecarManager: SidecarProcessManager? = null

    override fun init(ctx: JadxPluginContext) {
        try {
            ctx.decompiler?.let { decompiler ->
                PreferencesManager.initialize(decompiler)
            }

            PluginLifecycleManager(ctx) { srv, api ->
                this.server = srv
                val mcp = SidecarProcessManager(PreferencesManager.getPort())
                this.sidecarManager = mcp

                ctx.guiContext?.let { guiContext ->
                    DecxUIManager(ctx, srv, api, mcp).initializeGuiComponents(guiContext)
                }
            }.start()

        } catch (e: Exception) {
            LogUtils.error(DecxError.SERVER_INTERNAL_ERROR, "Failed to initialize plugin", e)
            cleanupOnError()
            throw e
        }
    }

    override fun getPluginInfo(): JadxPluginInfo? {
        return JadxPluginInfoBuilder.pluginId(PLUGIN_ID)
            .name(PLUGIN_NAME)
            .description("Decompiler + X - Bridges JADX with AI assistants via CLI and MCP, Powerful support with skills")
            .homepage("https://github.com/jygzyc/decx")
            .requiredJadxVersion("1.5.2, r2472")
            .build()
    }

    override fun unload() {
        try {
            LogUtils.info("Cleaning up Decx plugin resources...")
            sidecarManager?.stop()
            sidecarManager?.cleanupMcpFiles()
            CacheUtils.clearCache()
            server?.stop()
            PreferencesManager.clearCache()
        } catch (e: Exception) {
            LogUtils.error(DecxError.SERVER_INTERNAL_ERROR, "Error during plugin unload", e)
        }
    }

    private fun cleanupOnError() {
        try {
            sidecarManager?.stop()
        } finally {
            sidecarManager?.cleanupMcpFiles()
            server?.stop()
        }
    }
}
