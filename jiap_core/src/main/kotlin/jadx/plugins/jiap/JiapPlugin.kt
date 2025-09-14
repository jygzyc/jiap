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

class JiapPlugin : JadxPlugin {

    companion object {
        const val PLUGIN_NAME = "jiap"
        const val PLUGIN_ID = "jadx-jiap-plugin"

        private val logger = LoggerFactory.getLogger(JiapPlugin::class.java)
        lateinit var scheduler: ScheduledExecutorService
        lateinit var server: JiapServer
        lateinit var pluginContext: JadxPluginContext
    }

    override fun init(ctx: JadxPluginContext) {
        if (ctx.decompiler != null) {
            PreferencesManager.initializeJadxArgs(ctx.decompiler)
        }
        if (ctx.guiContext != null) {
            val uiManager = JiapUIManager(pluginContext)
            ctx.guiContext?.let { uiManager.initializeGuiComponents(it) }
        }
        try {
            server = JiapServer(ctx)
            scheduler = Executors.newSingleThreadScheduledExecutor { r ->
                val thread = Thread(r)
                thread.isDaemon = true
                thread.name = "JiapPlugin-Scheduler"
                thread
            }
            server.delayedInitialization()
        } catch (e: Exception){
            logger.error("Jiap Plugin: Failed to initialize", e)
        }
    }

    override fun getPluginInfo(): JadxPluginInfo? {
        return JadxPluginInfoBuilder.pluginId(PLUGIN_ID)
            .name("Java Intelligence Analysis Platform Plugin")
            .description("JIAP plugin for jadx")
            .homepage("https://github.com/jygzyc/jiap")
            .build()
    }
}
