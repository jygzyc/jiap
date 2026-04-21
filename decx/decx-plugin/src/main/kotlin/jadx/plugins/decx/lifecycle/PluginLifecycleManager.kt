package jadx.plugins.decx.lifecycle

import jadx.api.plugins.JadxPluginContext
import jadx.plugins.decx.api.DecxApi
import jadx.plugins.decx.api.DecxApiImpl
import jadx.plugins.decx.http.DecxServer
import jadx.plugins.decx.service.UIService
import jadx.plugins.decx.model.DecxError
import jadx.plugins.decx.utils.LogUtils
import jadx.plugins.decx.utils.PreferencesManager
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit

class PluginLifecycleManager(
    private val ctx: JadxPluginContext,
    private val onReady: (DecxServer, DecxApi) -> Unit
) {
    private val scheduler = Executors.newSingleThreadScheduledExecutor { r ->
        Thread(r, "DecxPlugin-Scheduler").apply { isDaemon = true }
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
        } catch (_: Exception) {
            false
        }
    }

    private fun initializeComponents() {
        try {
            ctx.decompiler?.let { decompiler ->
                PreferencesManager.initialize(decompiler)

                val port = PreferencesManager.getPort()
                val uiService = UIService(ctx)
                val api: DecxApi = DecxApiImpl(decompiler, uiService = uiService)
                val server = DecxServer(api, port)
                server.start()
                onReady(server, api)

                warmupDecompilation(decompiler)
            }
        } catch (e: Exception) {
            LogUtils.error(DecxError.SERVER_INTERNAL_ERROR, "Failed to initialize components", e)
            throw e
        }
    }

    private fun warmupDecompilation(decompiler: jadx.api.JadxDecompiler) {
        Thread({
            try {
                val classes = decompiler.classesWithInners ?: emptyList()
                if (classes.isEmpty()) return@Thread

                val sdkPackagePrefixes = listOf(
                    "android.support.", "androidx.", "java.", "javax.", "kotlin.", "kotlinx."
                )

                val appClasses = classes.filter { clazz ->
                    sdkPackagePrefixes.none { prefix -> clazz.fullName.startsWith(prefix) }
                }

                val targetCount = 15000
                val classesToDecompile: List<jadx.api.JavaClass> = if (appClasses.size >= targetCount) {
                    appClasses.shuffled().take(targetCount)
                } else {
                    classes
                }
                LogUtils.debug("Warmup classes count: ${classesToDecompile.size}")

                val startTime = System.currentTimeMillis()
                classesToDecompile.forEach { clazz ->
                    try { clazz.decompile() } catch (_: Exception) {}
                }

                val elapsed = System.currentTimeMillis() - startTime
                LogUtils.info("Warmup completed in ${elapsed}ms, decompiler engine ready")
            } catch (e: Exception) {
                LogUtils.debug("Warmup failed: ${e.message}")
            }
        }, "Decx-Warmup").apply { isDaemon = true }.start()
    }
}
