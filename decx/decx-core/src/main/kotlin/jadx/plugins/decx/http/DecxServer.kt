package jadx.plugins.decx.http

import io.javalin.Javalin
import io.javalin.http.Context
import io.javalin.http.bodyAsClass
import jadx.api.JadxDecompiler
import jadx.plugins.decx.DecxConstants
import jadx.plugins.decx.api.DecxApi
import jadx.plugins.decx.api.DecxRoutes
import jadx.plugins.decx.api.DecxApiImpl
import jadx.plugins.decx.utils.CacheUtils
import jadx.plugins.decx.utils.LogUtils
import jadx.plugins.decx.utils.PluginUtils
import jadx.plugins.decx.model.DecxError
import jadx.plugins.decx.utils.AnalysisResultUtils

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
        private const val RESTART_DELAY_MS = 2000L
        private const val HEALTH_KIND = "health"

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
            ctx.status(DecxError.HEALTH_CHECK_FAILED.status).json(
                AnalysisResultUtils.error(
                    kind = HEALTH_KIND,
                    code = DecxError.HEALTH_CHECK_FAILED.code,
                    message = DecxError.HEALTH_CHECK_FAILED.format(e.message ?: "unknown")
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
            DecxRoutes.all.forEach { route ->
                post(route.path) { ctx -> handleRoute(ctx, route.path) }
            }
        }
    }

    private fun handleRoute(ctx: Context, path: String) {
        try {
            val payload = readPayload(ctx)
            val page = payload["page"] as? Int ?: 1

            val handler = routeHandler ?: throw IllegalStateException("RouteHandler not initialized")
            val response = handler.handle(path, payload, page)

            ctx.json(response)
        } catch (e: Exception) {
            handleRouteError(ctx, e, path)
        }
    }

    private fun handleRouteError(ctx: Context, e: Exception, path: String) {
        val decxError = when (e) {
            is IllegalArgumentException -> {
                if (e.message?.startsWith("Unknown endpoint") == true) DecxError.UNKNOWN_ENDPOINT else DecxError.INVALID_PARAMETER
            }
            is NoSuchMethodException -> DecxError.METHOD_NOT_FOUND
            is IllegalStateException -> DecxError.SERVICE_ERROR
            else -> DecxError.SERVER_INTERNAL_ERROR
        }
        val detail = e.message ?: path
        val message = decxError.format(detail)

        ctx.status(decxError.status).json(
            AnalysisResultUtils.error(
                kind = routeHandler?.pathToKind(path) ?: "unknown",
                code = decxError.code,
                message = message
            )
        )
        LogUtils.error(decxError, e, detail)
    }

    private fun readPayload(ctx: Context): Map<String, Any> {
        if (ctx.body().isBlank()) {
            return emptyMap()
        }
        return ctx.bodyAsClass()
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
