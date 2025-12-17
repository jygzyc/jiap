package jadx.plugins.jiap

import jadx.api.plugins.JadxPlugin
import jadx.api.plugins.JadxPluginContext
import jadx.api.plugins.JadxPluginInfo
import jadx.api.plugins.JadxPluginInfoBuilder
import jadx.plugins.jiap.ui.JiapUIManager
import jadx.plugins.jiap.utils.LogUtils
import jadx.plugins.jiap.utils.JiapConstants
import jadx.plugins.jiap.utils.PreferencesManager
import jadx.plugins.jiap.utils.CacheUtils

import java.util.concurrent.Executors
import java.util.concurrent.ScheduledExecutorService
import java.util.concurrent.TimeUnit

class JiapPlugin : JadxPlugin {

    companion object {
        const val PLUGIN_NAME = "JIAP"
        const val PLUGIN_ID = "jadx-jiap-plugin"
    }

    private lateinit var scheduler: ScheduledExecutorService
    lateinit var server: JiapServer

    override fun init(ctx: JadxPluginContext) {
        // Initialize preferences if decompiler is available
        ctx.decompiler?.let { decompiler ->
            PreferencesManager.initializeJadxArgs(decompiler)
        }

        try {
            // Create scheduler for delayed initialization
            scheduler = Executors.newSingleThreadScheduledExecutor { r ->
                Thread(r, "JiapPlugin-Scheduler").apply {
                    isDaemon = true
                }
            }

            server = JiapServer(ctx, scheduler)
            server.delayedInitialization()

            ctx.guiContext?.also { guiContext ->
                val uiManager = JiapUIManager(ctx, server)
                uiManager.initializeGuiComponents(guiContext)
            }

        } catch (e: Exception) {
            LogUtils.error(JiapConstants.ErrorCode.SERVER_INTERNAL_ERROR, "Failed to initialize plugin", e)
            cleanupOnError()
            throw e
        }
    }

    override fun getPluginInfo(): JadxPluginInfo? {
        return JadxPluginInfoBuilder.pluginId(PLUGIN_ID)
            .name(PLUGIN_NAME)
            .description("JIAP plugin for jadx")
            .homepage("https://github.com/jygzyc/jiap")
            .requiredJadxVersion("1.5.2, r2472")
            .build()
    }

    override fun unload() {
        try {
            LogUtils.info("Cleaning up JIAP plugin resources...")

            // Clear cache
            CacheUtils.clearCache()

            if (::server.isInitialized) {
                if (server.isRunning) {
                    server.stop()
                }
            }
            if (::scheduler.isInitialized) {
                scheduler.shutdown()
                if (!scheduler.awaitTermination(3, TimeUnit.SECONDS)) {
                    LogUtils.warn("Scheduler did not terminate gracefully within 3 seconds, forcing shutdown")
                    scheduler.shutdownNow()
                }
            }

        } catch (e: Exception) {
            LogUtils.error(JiapConstants.ErrorCode.SERVER_INTERNAL_ERROR, "Error during plugin unload", e)
        }
    }

    private fun cleanupOnError() {
        try {
            if (::scheduler.isInitialized) {
                scheduler.shutdownNow()
            }
        } catch (e: Exception) {
            LogUtils.warn("Failed to cleanup scheduler during error recovery", e)
        }
    }
}
