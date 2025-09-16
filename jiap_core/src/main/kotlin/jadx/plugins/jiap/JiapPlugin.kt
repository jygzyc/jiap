package jadx.plugins.jiap

import org.slf4j.LoggerFactory

import jadx.api.plugins.JadxPlugin
import jadx.api.plugins.JadxPluginContext
import jadx.api.plugins.JadxPluginInfo
import jadx.api.plugins.JadxPluginInfoBuilder
import jadx.plugins.jiap.ui.JiapUIManager
import jadx.plugins.jiap.utils.PreferencesManager

import java.util.concurrent.Executors
import java.util.concurrent.ScheduledExecutorService
import java.util.concurrent.TimeUnit

class JiapPlugin : JadxPlugin {

    companion object {
        const val PLUGIN_NAME = "jiap"
        const val PLUGIN_ID = "jadx-jiap-plugin"
        private val logger = LoggerFactory.getLogger(JiapPlugin::class.java)
    }

    private lateinit var scheduler: ScheduledExecutorService
    lateinit var server: JiapServer

    override fun init(ctx: JadxPluginContext) {
        if (ctx.decompiler != null) {
            PreferencesManager.initializeJadxArgs(ctx.decompiler)
        }
        try {
            scheduler = Executors.newSingleThreadScheduledExecutor { r ->
                val thread = Thread(r).apply {
                    isDaemon = true
                    name = "JiapPlugin-Scheduler"
                }
                thread
            }
            server = JiapServer(ctx, scheduler)
            server.delayedInitialization()
            
            if (ctx.guiContext != null) {
                val uiManager = JiapUIManager(ctx, server)
                ctx.guiContext?.let { uiManager.initializeGuiComponents(it) }
            }
        } catch (e: Exception){
            logger.error("Jiap Plugin: Failed to initialize", e)
        }
    }

    override fun getPluginInfo(): JadxPluginInfo? {
        return JadxPluginInfoBuilder.pluginId(PLUGIN_ID)
            .name("JIAP Plugin")
            .description("JIAP plugin for jadx")
            .homepage("https://github.com/jygzyc/jiap")
            .requiredJadxVersion("1.5.2, r2507")
            .build()
    }

    override fun unload() {
        try {
            logger.info("JIAP: Cleaning up plugin resources...")

            // Stop the server to release port
            if (this::server.isInitialized) {
                server.stop()
                logger.info("JIAP: Server stopped")
            }

            // Shutdown the scheduler thread pool
            if (this::scheduler.isInitialized) {
                scheduler.shutdown()
                if (!scheduler.awaitTermination(5, TimeUnit.SECONDS)) {
                    logger.warn("JIAP: Scheduler did not terminate gracefully, forcing shutdown")
                    scheduler.shutdownNow()
                }
                logger.info("JIAP: Scheduler shutdown completed")
            }

            logger.info("JIAP: Plugin cleanup completed")
        } catch (e: Exception) {
            logger.error("JIAP: Error during plugin cleanup", e)
        }
    }
}
