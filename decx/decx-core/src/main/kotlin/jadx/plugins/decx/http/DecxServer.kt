package jadx.plugins.decx.http

import io.javalin.Javalin
import io.javalin.http.Context
import io.javalin.http.bodyAsClass
import jadx.api.JadxDecompiler
import jadx.plugins.decx.DecxConstants
import jadx.plugins.decx.api.DecxApi
import jadx.plugins.decx.api.DecxApiImpl
import jadx.plugins.decx.utils.CacheUtils
import jadx.plugins.decx.utils.LogUtils
import jadx.plugins.decx.utils.PluginUtils
import jadx.plugins.decx.model.DecxError

/**
 * HTTP server built on Javalin. Delegates all business logic to [DecxApi].
 *
 * Used by both standalone server and plugin mode.
 */
class DecxServer(
    private val api: DecxApi,
    private val port: Int = DecxConstants.DEFAULT_PORT
) {

    companion object {
        private const val SHUTDOWN_TIMEOUT_MS = 2000L
        private const val RESTART_DELAY_MS = 2000L

        val ALL_ROUTES = setOf(
            // Common Service
            "/api/decx/get_all_classes",
            "/api/decx/get_class_info",
            "/api/decx/get_class_source",
            "/api/decx/search_class_key",
            "/api/decx/search_method",
            "/api/decx/get_method_source",
            "/api/decx/get_method_xref",
            "/api/decx/get_field_xref",
            "/api/decx/get_class_xref",
            "/api/decx/get_implement",
            "/api/decx/get_sub_classes",
            // Android App Service
            "/api/decx/get_aidl",
            "/api/decx/get_app_manifest",
            "/api/decx/get_main_activity",
            "/api/decx/get_application",
            "/api/decx/get_exported_components",
            "/api/decx/get_deep_links",
            "/api/decx/get_dynamic_receivers",
            "/api/decx/get_all_resources",
            "/api/decx/get_resource_file",
            "/api/decx/get_strings",
            // Android Framework Service
            "/api/decx/get_system_service_impl",
        )

        /** Create a DecxServer directly from a decompiler instance. */
        fun create(decompiler: JadxDecompiler, port: Int = DecxConstants.DEFAULT_PORT): DecxServer {
            return DecxServer(DecxApiImpl(decompiler), port)
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
            LogUtils.error(DecxError.SERVER_INTERNAL_ERROR, e, "Start failed")
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
            LogUtils.error(DecxError.SERVER_INTERNAL_ERROR, e, "Stop failed")
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
                LogUtils.error(DecxError.SERVER_INTERNAL_ERROR, e, "Restart failed")
                started = false
            }
        }, "DecxServer-Restart").apply {
            isDaemon = true
        }.start()
        return true
    }

    fun handleHealthCheck(ctx: Context) {
        try {
            ctx.json(
                mapOf(
                    "status" to if (started) "running" else "stopped",
                    "version" to DecxConstants.getVersion(),
                    "url" to PluginUtils.buildServerUrl(port = port, running = started),
                    "port" to port,
                    "timestamp" to System.currentTimeMillis()
                )
            )
        } catch (e: Exception) {
            LogUtils.error(DecxError.HEALTH_CHECK_FAILED, e)
            ctx.status(500).json(
                mapOf(
                    "error" to DecxError.HEALTH_CHECK_FAILED.code,
                    "message" to e.message
                )
            )
        }
    }

    /**
     * Expose the underlying [DecxApi] for direct access without going through HTTP.
     */
    fun getApi(): DecxApi = api

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
        val (decxError, message) = when (e) {
            is IllegalArgumentException -> DecxError.INVALID_PARAMETER to (e.message ?: "Invalid parameters: $path")
            is NoSuchMethodException -> DecxError.METHOD_NOT_FOUND to (e.message ?: "Method missing: $path")
            is IllegalStateException -> DecxError.SERVICE_ERROR to (e.message ?: "State error: $path")
            else -> DecxError.SERVER_INTERNAL_ERROR to "Internal error: ${e.message ?: path}"
        }

        ctx.status(500).json(
            mapOf(
                "error" to decxError.code,
                "message" to message
            )
        )
        LogUtils.error(decxError, e, message)
    }

    private fun setupShutdownHook() {
        shutdownHook = Thread({
            try {
                stop()
            } catch (e: Exception) {
                LogUtils.error(DecxError.SERVER_INTERNAL_ERROR, e, "Stop failed")
            }
        }, "DecxServer-ShutdownHook")
        Runtime.getRuntime().addShutdownHook(shutdownHook)
    }

    private fun removeShutdownHook() {
        val hook = shutdownHook ?: return
        try {
            if (Thread.currentThread().name != "DecxServer-ShutdownHook") {
                Runtime.getRuntime().removeShutdownHook(hook)
            }
        } catch (e: Exception) {
            // ignore
        } finally {
            shutdownHook = null
        }
    }
}
