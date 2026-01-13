package jadx.plugins.jiap.core

import jadx.api.plugins.JadxPluginContext
import jadx.plugins.jiap.JiapConstants
import jadx.plugins.jiap.model.JiapError
import jadx.plugins.jiap.utils.LogUtils
import jadx.plugins.jiap.utils.PreferencesManager
import java.util.concurrent.Executors
import java.util.concurrent.ScheduledExecutorService
import java.util.concurrent.TimeUnit

class PluginLifecycleManager(
    private val ctx: JadxPluginContext,
    private val onReady: (JiapServer, SidecarProcessManager) -> Unit
) {
    private val scheduler: ScheduledExecutorService = Executors.newSingleThreadScheduledExecutor { r ->
        Thread(r, "JiapPlugin-Scheduler").apply {
            isDaemon = true
        }
    }

    fun start() {
        scheduler.scheduleAtFixedRate({
            try {
                if (isDecompilerReady()) {
                    LogUtils.info("JADX decompiler ready, starting services...")
                    scheduler.shutdown()
                    
                    initializeComponents()
                }
            } catch (e: Exception) {
                LogUtils.debug("Waiting for decompiler: ${e.message}")
            }
        }, 1, 1, TimeUnit.SECONDS)
    }

    private fun isDecompilerReady(): Boolean {
        return try {
            val decompiler = ctx.decompiler ?: return false
            decompiler.classesWithInners.isNotEmpty()
        } catch (e: Exception) {
            false
        }
    }

    private fun initializeComponents() {
        try {
            ctx.decompiler?.let { decompiler ->
                PreferencesManager.initializeJadxArgs(decompiler)
            }
            
            val server = JiapServer(ctx)
            
            server.start(PreferencesManager.getPort())
            
            onReady(server, server.sidecarManager)
            
        } catch (e: Exception) {
            LogUtils.error(JiapError.SERVER_INTERNAL_ERROR, "Failed to initialize components", e)
            throw e
        }
    }
}
