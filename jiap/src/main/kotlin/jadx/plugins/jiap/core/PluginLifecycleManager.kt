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
                
                val server = JiapServer(ctx)
                server.start(PreferencesManager.getPort())
                onReady(server, server.sidecarManager)
                
                warmupDecompilation(decompiler)
            }
        } catch (e: Exception) {
            LogUtils.error(JiapError.SERVER_INTERNAL_ERROR, "Failed to initialize components", e)
            throw e
        }
    }
    
    private fun warmupDecompilation(decompiler: jadx.api.JadxDecompiler) {
        Thread({
            try {
                val classes = decompiler.classesWithInners ?: emptyList()
                if (classes.isEmpty()) return@Thread

                val sdkPackagePrefixes = listOf(
                    "android.support.",
                    "androidx.",
                    "java.",
                    "javax.",
                    "kotlin.",
                    "kotlinx."
                )

                val appClasses = classes.filter { clazz ->
                    val fullName = clazz.fullName
                    sdkPackagePrefixes.none { prefix -> fullName.startsWith(prefix) }
                }

                val targetCount = 15000
                val classesToDecompile: List<jadx.api.JavaClass>

                if (appClasses.size >= targetCount) {
                    classesToDecompile = appClasses.shuffled().take(targetCount)
                } else {
                    classesToDecompile = classes
                }
                LogUtils.debug("Warmup classes count: ${classesToDecompile.size}")

                val startTime = System.currentTimeMillis()

                classesToDecompile.forEach { clazz ->
                    try {
                        clazz.decompile()
                    } catch (_: Exception) {}
                }

                val elapsed = System.currentTimeMillis() - startTime
                LogUtils.info("Warmup completed in ${elapsed}ms, decompiler engine ready")
            } catch (e: Exception) {
                LogUtils.debug("Warmup failed: ${e.message}")
            }
        }, "Jiap-Warmup").apply { isDaemon = true }.start()
    }
}
