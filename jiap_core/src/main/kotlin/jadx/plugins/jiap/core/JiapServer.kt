package jadx.plugins.jiap.core

import io.javalin.Javalin
import io.javalin.http.Context
import io.javalin.http.bodyAsClass
import jadx.api.plugins.JadxPluginContext
import jadx.gui.ui.MainWindow

import java.util.concurrent.TimeUnit
import kotlin.collections.mapOf

import jadx.plugins.jiap.utils.PluginUtils
import jadx.plugins.jiap.utils.LogUtils
import jadx.plugins.jiap.utils.CacheUtils
import jadx.plugins.jiap.utils.PreferencesManager
import jadx.plugins.jiap.JiapConstants
import jadx.plugins.jiap.model.JiapResult
import jadx.plugins.jiap.model.JiapError
import jadx.plugins.jiap.JiapPlugin

class JiapServer(
    private val pluginContext: JadxPluginContext
) {

    companion object {
        private const val SHUTDOWN_TIMEOUT_MS = 2000L
        private const val RESTART_DELAY_MS = 2000L
    }

    val currentPort: Int get() = PreferencesManager.getPort()
    val isRunning: Boolean get() = started
    val sidecarManager = SidecarProcessManager(pluginContext)

    private var app: Javalin? = null
    private var routeHandler: RouteHandler? = null

    @Volatile
    private var started = false

    @Volatile
    private var shutdownHook: Thread? = null

    private val config: JiapConfig = JiapConfig(pluginContext)
    val routeMap: Map<String, RouteTarget> get() = config.routeMap

    @Synchronized
    fun start(port: Int = JiapConstants.DEFAULT_PORT): Boolean {
        if (started) {
            LogUtils.warn("Server is running")
            return true
        }

        return try {
            app = Javalin.create { config ->
                config.showJavalinBanner = false
            }.start(port)
            PreferencesManager.setPort(port)
            routeHandler = RouteHandler(config)
            configureRoutes()
            started = true

            LogUtils.info("Server started")
            setupShutdownHook()
            
            Thread({
                sidecarManager.start()
            }, "Jiap-Sidecar-Starter").apply { isDaemon = true }.start()
            
            true
        } catch (e: Exception) {
            started = false
            LogUtils.error(JiapError.SERVER_INTERNAL_ERROR, e, "Start failed")
            false
        }
    }

    @Synchronized
    fun stop(): Boolean {
        if (!started) {
            sidecarManager.stop() 
            return true
        }

        started = false
        sidecarManager.stop()

        return try {
            app?.stop()
            app = null
            val startTime = System.currentTimeMillis()
            while (System.currentTimeMillis() - startTime < SHUTDOWN_TIMEOUT_MS) {
                Thread.sleep(50)
            }
            LogUtils.info("Server stopped")
            true
        } catch (e: Exception) {
            LogUtils.error(JiapError.SERVER_INTERNAL_ERROR, e, "Stop failed")
            false
        } finally {
            removeShutdownHook()
        }
    }

    fun restart(): Boolean {
        if (!started) {
            LogUtils.info("Starting server...")
            return start(PreferencesManager.getPort())
        }

        Thread({
            try {
                CacheUtils.reinitializeCache()
                LogUtils.info("Restarting server...")
                stop()
                Thread.sleep(RESTART_DELAY_MS)
                start(PreferencesManager.getPort()) 
            } catch (e: Exception) {
                LogUtils.error(JiapError.SERVER_INTERNAL_ERROR, e, "Restart failed")
                started = false
            }
        }, "JiapServer-Restart").apply {
            isDaemon = true
        }.start()
        return true
    }

    private fun configureRoutes() {
        app?.apply {
            get("/health") { ctx -> handleHealthCheck(ctx) }
            routeMap.keys.forEach { path ->
                post(path) { ctx -> handleRoute(ctx, path) }
            }
        }
    }

    private fun handleRoute(ctx: Context, path: String) {
        try {
            val payload = ctx.bodyAsClass<Map<String, Any>>()
            val page = payload["page"] as? Int ?: 1
            
            val handler = routeHandler ?: throw IllegalStateException("RouteHandler not initialized")
            val response = handler.handle(path, payload, page)
            
            ctx.json(response)
        } catch (e: Exception) {
            handleRouteError(ctx, e, path)
        }
    }

    private fun handleRouteError(ctx: Context, e: Exception, path: String) {
        val (jiapError, message) = when (e) {
            is IllegalArgumentException -> JiapError.INVALID_PARAMETER to (e.message ?: "Invalid parameters: $path")
            is NoSuchMethodException -> JiapError.METHOD_NOT_FOUND to (e.message ?: "Method missing: $path")
            is IllegalStateException -> JiapError.SERVICE_ERROR to (e.message ?: "State error: $path")
            else -> JiapError.SERVER_INTERNAL_ERROR to "Internal error: ${e.message ?: path}"
        }

        ctx.status(500).json(
            mapOf(
                "error" to jiapError.code,
                "message" to message
            )
        )
        LogUtils.error(jiapError, e, message)
    }

    fun handleHealthCheck(ctx: Context) {
        try {
            val port = PreferencesManager.getPort()
            val url = PluginUtils.buildServerUrl(running = started, port = port)
            ctx.json(
                mapOf(
                    "status" to if (started) "running" else "stopped",
                    "url" to url,
                    "port" to port,
                    "timestamp" to System.currentTimeMillis()
                )
            )
        } catch (e: Exception) {
            LogUtils.error(JiapError.HEALTH_CHECK_FAILED, e)
            ctx.status(500).json(
                mapOf(
                    "error" to JiapError.HEALTH_CHECK_FAILED.code,
                    "message" to e.message
                )
            )
        }
    }

    private fun setupShutdownHook() {
        shutdownHook = Thread({
            try {
                stop()
            } catch (e: Exception) {
                LogUtils.error(JiapError.SERVER_INTERNAL_ERROR, e, "Stop failed")
            }
        }, "JiapServer-ShutdownHook")
        Runtime.getRuntime().addShutdownHook(shutdownHook)
    }

    private fun removeShutdownHook() {
        val hook = shutdownHook ?: return
        try {
            if (Thread.currentThread().name != "JiapServer-ShutdownHook") {
                Runtime.getRuntime().removeShutdownHook(hook)
            }
        } catch (e: Exception) {

        } finally {
            shutdownHook = null
        }
    }
}
