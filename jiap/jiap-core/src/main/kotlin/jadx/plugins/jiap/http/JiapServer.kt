package jadx.plugins.jiap.http

import io.javalin.Javalin
import io.javalin.http.Context
import io.javalin.http.bodyAsClass
import jadx.api.JadxDecompiler
import jadx.plugins.jiap.JiapConstants
import jadx.plugins.jiap.api.JiapApi
import jadx.plugins.jiap.api.JiapApiImpl
import jadx.plugins.jiap.utils.CacheUtils
import jadx.plugins.jiap.utils.LogUtils
import jadx.plugins.jiap.utils.PluginUtils
import jadx.plugins.jiap.model.JiapError

/**
 * HTTP server built on Javalin. Delegates all business logic to [JiapApi].
 *
 * Used by both standalone server and plugin mode.
 */
class JiapServer(
    private val api: JiapApi,
    private val port: Int = JiapConstants.DEFAULT_PORT
) {

    companion object {
        private const val SHUTDOWN_TIMEOUT_MS = 2000L
        private const val RESTART_DELAY_MS = 2000L

        val ALL_ROUTES = setOf(
            // Common Service
            "/api/jiap/get_all_classes",
            "/api/jiap/get_class_info",
            "/api/jiap/get_class_source",
            "/api/jiap/search_class_key",
            "/api/jiap/search_method",
            "/api/jiap/get_method_source",
            "/api/jiap/get_method_xref",
            "/api/jiap/get_field_xref",
            "/api/jiap/get_class_xref",
            "/api/jiap/get_implement",
            "/api/jiap/get_sub_classes",
            // Android App Service
            "/api/jiap/get_aidl",
            "/api/jiap/get_app_manifest",
            "/api/jiap/get_main_activity",
            "/api/jiap/get_application",
            "/api/jiap/get_exported_components",
            "/api/jiap/get_deep_links",
            "/api/jiap/get_dynamic_receivers",
            "/api/jiap/get_all_resources",
            "/api/jiap/get_resource_file",
            "/api/jiap/get_strings",
            // Android Framework Service
            "/api/jiap/get_system_service_impl",
        )

        /** Create a JiapServer directly from a decompiler instance. */
        fun create(decompiler: JadxDecompiler, port: Int = JiapConstants.DEFAULT_PORT): JiapServer {
            return JiapServer(JiapApiImpl(decompiler), port)
        }
    }

    val isRunning: Boolean get() = started

    private var app: Javalin? = null
    private var routeHandler: RouteHandler? = null

    @Volatile
    private var started = false

    @Volatile
    private var shutdownHook: Thread? = null

    @Synchronized
    fun start(overridePort: Int = port): Boolean {
        if (started) {
            LogUtils.warn("Server is running")
            return true
        }

        return try {
            app = Javalin.create { cfg ->
                cfg.showJavalinBanner = false
            }.start(overridePort)
            routeHandler = RouteHandler(api)
            configureRoutes()
            started = true

            LogUtils.info("Server started on port $overridePort")
            setupShutdownHook()
            true
        } catch (e: Exception) {
            started = false
            LogUtils.error(JiapError.SERVER_INTERNAL_ERROR, e, "Start failed")
            false
        }
    }

    @Synchronized
    fun stop(): Boolean {
        if (!started) return true

        started = false

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
            return start()
        }

        Thread({
            try {
                CacheUtils.reinitializeCache()
                LogUtils.info("Restarting server...")
                stop()
                Thread.sleep(RESTART_DELAY_MS)
                start()
            } catch (e: Exception) {
                LogUtils.error(JiapError.SERVER_INTERNAL_ERROR, e, "Restart failed")
                started = false
            }
        }, "JiapServer-Restart").apply {
            isDaemon = true
        }.start()
        return true
    }

    fun handleHealthCheck(ctx: Context) {
        try {
            ctx.json(
                mapOf(
                    "status" to if (started) "running" else "stopped",
                    "url" to PluginUtils.buildServerUrl(port = port, running = started),
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

    /**
     * Expose the underlying [JiapApi] for direct access without going through HTTP.
     */
    fun getApi(): JiapApi = api

    private fun configureRoutes() {
        app?.apply {
            get("/health") { ctx -> handleHealthCheck(ctx) }
            ALL_ROUTES.forEach { path ->
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
            // ignore
        } finally {
            shutdownHook = null
        }
    }
}
