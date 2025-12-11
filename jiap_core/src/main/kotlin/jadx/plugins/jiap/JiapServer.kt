package jadx.plugins.jiap

import io.javalin.Javalin
import io.javalin.http.Context
import io.javalin.http.bodyAsClass
import jadx.api.plugins.JadxPluginContext
import jadx.gui.ui.MainWindow

import java.util.concurrent.ScheduledExecutorService
import java.util.concurrent.TimeUnit
import kotlin.collections.mapOf

import jadx.plugins.jiap.service.CommonService
import jadx.plugins.jiap.service.AndroidFrameworkService
import jadx.plugins.jiap.service.AndroidAppService
import jadx.plugins.jiap.model.JiapResult
import jadx.plugins.jiap.utils.JiapConstants
import jadx.plugins.jiap.utils.JiapConstants.ErrorCode
import jadx.plugins.jiap.utils.JiapConstants.Messages
import jadx.plugins.jiap.utils.PreferencesManager
import jadx.plugins.jiap.utils.PluginUtils
import jadx.plugins.jiap.utils.LogUtils
import jadx.plugins.jiap.utils.CacheUtils

class JiapServer(
    private val pluginContext: JadxPluginContext,
    private val scheduler: ScheduledExecutorService
) {
    
    companion object {
        private const val SHUTDOWN_TIMEOUT_MS = 500L
        private const val RESTART_DELAY_MS = 1000L
        private const val INIT_CHECK_INTERVAL_SECONDS = 1L
        private const val INITIAL_DELAY_SECONDS = 2L
        private const val MAX_INIT_WAIT_SECONDS = 30L
    }

    val currentPort: Int get() = PreferencesManager.getPort()

    val isRunning: Boolean get() = started


    private var app: Javalin? = null

    @Volatile
    private var started = false

    @Volatile
    private var initializationScheduled = false

    @Volatile
    private var shutdownHook: Thread? = null

    private val commonService: CommonService = CommonService(pluginContext)
    private val androidFrameworkService: AndroidFrameworkService = AndroidFrameworkService(pluginContext)
    private val androidAppService: AndroidAppService = AndroidAppService(pluginContext)

    private val routeMap: Map<String, RouteTarget>
        get() = mapOf(
            // Common Service
            "/api/jiap/get_all_classes" to RouteTarget(
                service = commonService,
                methodName = "handleGetAllClasses"
            ),
            "/api/jiap/selected_text" to RouteTarget(
                service = commonService,
                methodName = "handleGetSelectedText"
            ),
            "/api/jiap/search_method" to RouteTarget(
                service = commonService,
                methodName = "handleSearchMethod",
                params = setOf("method")
            ),
            "/api/jiap/get_class_info" to RouteTarget(
                service = commonService,
                methodName = "handleGetClassInfo",
                params = setOf("class")
            ),
            "/api/jiap/get_method_xref" to RouteTarget(
                service = commonService,
                methodName = "handleGetMethodXref",
                params = setOf("method")
            ),
            "/api/jiap/get_class_xref" to RouteTarget(
                service = commonService,
                methodName = "handleGetClassXref",
                params = setOf("class")
            ),
            "/api/jiap/get_implement" to RouteTarget(
                service = commonService,
                methodName = "handleGetImplementOfInterface",
                params = setOf("interface")
            ),
            "/api/jiap/get_sub_classes" to RouteTarget(
                service = commonService,
                methodName = "handleGetSubclasses",
                params = setOf("class")
            ),
            "/api/jiap/get_class_source" to RouteTarget(
                service = commonService,
                methodName = "handleGetClassSource",
                params = setOf("class", "smali")
            ),
            "/api/jiap/get_method_source" to RouteTarget(
                service = commonService,
                methodName = "handleGetMethodSource",
                params = setOf("method", "smali")
            ),

            // Android App Service
            "/api/jiap/get_app_manifest" to RouteTarget(
                service = androidAppService,
                methodName = "handleGetAppManifest"
            ),
            "/api/jiap/get_main_activity" to RouteTarget(
                service = androidAppService,
                methodName = "handleGetMainActivity"
            ),

            // Android Framework Service
            "/api/jiap/get_system_service_impl" to RouteTarget(
                service = androidFrameworkService,
                methodName = "handleGetSystemServiceImpl",
                params = setOf("interface")
            )
        )

    @Synchronized
    fun start(port: Int = JiapConstants.DEFAULT_PORT): Boolean {
        if (started) {
            LogUtils.warn(Messages.SERVER_RUNNING)
            return true
        }
        if (!isJadxDecompilerAvailable()) {
            LogUtils.error(ErrorCode.JADX_NOT_AVAILABLE, Messages.JADX_NOT_AVAILABLE)
            return false
        }

        return try {
            app = Javalin.create().start(port)
            PreferencesManager.setPort(port)
            configureRoutes()
            started = true
            initializationScheduled = false

            LogUtils.info(Messages.SERVER_STARTED)
            setupShutdownHook()
            true
        } catch (e: Exception) {
            started = false
            LogUtils.error(ErrorCode.SERVER_INTERNAL_ERROR, Messages.SERVER_START_FAILED, e)
            false
        }
    }

    @Synchronized
    fun stop(): Boolean {
        if (!started) {
            return true
        }

        started = false

        return try {
            app?.stop()
            app = null
            // Use a shorter timeout and make it interruptible
            val startTime = System.currentTimeMillis()
            while (System.currentTimeMillis() - startTime < SHUTDOWN_TIMEOUT_MS) {
                Thread.sleep(50) // Sleep in smaller chunks
            }
            LogUtils.info(Messages.SERVER_STOPPED)
            true
        } catch (e: Exception) {
            LogUtils.error(ErrorCode.SERVER_INTERNAL_ERROR, Messages.SERVER_STOP_FAILED, e)
            false
        } finally {
            removeShutdownHook()
        }
    }

    fun restart(): Boolean {
        if (!started) {
            LogUtils.info(Messages.SERVER_STARTING)
            return start(PreferencesManager.getPort())
        }

        Thread({
            try {
                LogUtils.info(Messages.SERVER_RESTARTING)
                stop()
                Thread.sleep(RESTART_DELAY_MS)
                start(PreferencesManager.getPort())
            } catch (e: Exception) {
                LogUtils.error(ErrorCode.SERVER_INTERNAL_ERROR, Messages.SERVER_RESTART_FAILED, e)
                started = false
            }
        }, "JiapServer-Restart").apply {
            isDaemon = true
        }.start()
        return true
    }

    @Synchronized
    fun delayedInitialization() {
        if (initializationScheduled) {
            return
        }
        initializationScheduled = true
        scheduler.scheduleAtFixedRate({
            try {
                when {
                    started -> scheduler.shutdown()
                    isJadxDecompilerAvailable() -> {
                        restart()
                        scheduler.shutdown()
                    }
                }
            } catch (e: Exception) {
                LogUtils.error(ErrorCode.SERVER_INTERNAL_ERROR, Messages.SERVER_INIT_CHECK_FAILED, e)
            }
        }, INITIAL_DELAY_SECONDS, INIT_CHECK_INTERVAL_SECONDS, TimeUnit.SECONDS)

        scheduler.schedule({
            if (!started) {
                LogUtils.warn(ErrorCode.JADX_NOT_AVAILABLE, Messages.JADX_INIT_FAILED, 30)
            }
        }, MAX_INIT_WAIT_SECONDS, TimeUnit.SECONDS)
    }

    private fun setupShutdownHook() {
        shutdownHook = Thread({
            try {
                stop()
            } catch (e: Exception) {
                LogUtils.error(ErrorCode.SERVER_INTERNAL_ERROR, Messages.SERVER_STOP_FAILED, e)
            }
        }, "JiapServer-ShutdownHook")
        Runtime.getRuntime().addShutdownHook(shutdownHook)
    }

    private fun removeShutdownHook() {
        shutdownHook?.let { hook ->
            try {
                Runtime.getRuntime().removeShutdownHook(hook)
            } catch (e: IllegalStateException) {
                // Hook is already running
                LogUtils.debug("Shutdown hook is already running, cannot remove")
            }
            shutdownHook = null
        }
    }

    private fun isJadxDecompilerAvailable(): Boolean {
        return try {
            val guiContext = pluginContext.guiContext
            if (guiContext != null) {
                val mainFrame = guiContext.mainFrame
                if (mainFrame is MainWindow) {
                    val wrapper = mainFrame.wrapper
                    wrapper.includedClassesWithInners ?: return false
                }
            }
            val decompiler = pluginContext.decompiler ?: return false
            val classCount = decompiler.classesWithInners.size
            classCount > 0
        } catch (e: Exception) {
            LogUtils.error(ErrorCode.JADX_NOT_AVAILABLE, Messages.JADX_NOT_AVAILABLE, e)  // 应该使用 JiapConstants.Messages
            false
        }
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
            val routeTarget = routeMap[path] ?: run {
                val error = Messages.UNKNOWN_ENDPOINT.format(path)
                LogUtils.error(ErrorCode.UNKNOWN_ENDPOINT, error)
                throw IllegalArgumentException(error)
            }
            val payload = ctx.bodyAsClass<Map<String, Any>>()
            if (!validateRequiredParams(payload, routeTarget.params, ctx)) {
                return
            }

            val page = (payload["page"] as? Int) ?: 1

            // Check cache first
            val cachedResponse = CacheUtils.getCachedResponse(path, payload, page)
            if (cachedResponse != null) {
                ctx.json(cachedResponse)
                return
            }

            // Cache miss - execute service method
            val result = invokeServiceMethod(routeTarget, payload)

            if (result.success) {
                // Cache the successful response
                CacheUtils.cacheResponse(path, payload, page, result.data)
                ctx.json(result.data)
            } else {
                ctx.status(500).json(result.data)
            }
        } catch (e: Exception) {
            handleRouteError(ctx, e, path)
        }
    }

    private fun validateRequiredParams(
        payload: Map<String, Any>,
        requiredParams: Set<String>,
        ctx: Context
    ): Boolean {
        for (param in requiredParams) {
            val value = payload[param]
            if (value == null || value.toString().isBlank()) {
                ctx.status(400).json(mapOf(
                    "error" to ErrorCode.MISSING_PARAMETER,
                    "message" to Messages.MISSING_PARAMETER.format(param)
                ))
                return false
            }
            // 验证参数类型是否为支持的类型
            if (value !is String && value !is Int && value !is Boolean) {
                ctx.status(400).json(mapOf(
                    "error" to ErrorCode.INVALID_PARAMETER,
                    "message" to Messages.INVALID_PARAMETER.format("$param: ${value::class.simpleName}")
                ))
                return false
            }
        }
        return true
    }

    private fun invokeServiceMethod(
        routeTarget: RouteTarget,
        payload: Map<String, Any>
    ): JiapResult {
        val method = routeTarget.getMethod()
        val params = method.parameters
        LogUtils.info("Service call %s".format(routeTarget.methodName))

        if (params.isEmpty()) {
            @Suppress("UNCHECKED_CAST")
            return method.invoke(routeTarget.service) as JiapResult
        }
        val args = routeTarget.params.map { paramName ->
            // Get value from payload
            val value = payload[paramName]

            // Find matching parameter type
            // We assume params are in the same order as defined in routeTarget.params
            val paramType = params[routeTarget.params.indexOf(paramName)].type

            // Convert to proper type
            PluginUtils.convertValue(value, paramType)
        }.toTypedArray()

        @Suppress("UNCHECKED_CAST")
        return method.invoke(routeTarget.service, *args) as JiapResult
    }

    private fun handleRouteError(ctx: Context, e: Exception, path: String) {
        val (errorCode, message) = when (e) {
            is IllegalArgumentException -> ErrorCode.UNKNOWN_ENDPOINT to Messages.UNKNOWN_ENDPOINT.format(path)
            is NoSuchMethodException -> ErrorCode.SERVICE_CALL_FAILED to Messages.METHOD_NOT_FOUND.format(path)
            else -> ErrorCode.SERVER_INTERNAL_ERROR to "${Messages.SERVICE_CALL_FAILED}: ${e.message}"
        }

        ctx.status(500).json(mapOf(
            "error" to errorCode,
            "message" to message
        ))
        LogUtils.error(errorCode, message, e)
    }

    fun handleHealthCheck(ctx: Context) {
        try {
            val port = PreferencesManager.getPort()
            val url = PluginUtils.buildServerUrl(running = started, port = port)
            ctx.json(mapOf(
                "status" to if (started) "running" else "stopped",
                "url" to url,
                "port" to port,
                "timestamp" to System.currentTimeMillis()
            ))
        } catch (e: Exception) {
            LogUtils.error(ErrorCode.HEALTH_CHECK_FAILED, Messages.HEALTH_CHECK_FAILED, e)
            ctx.status(500).json(mapOf(
                "error" to ErrorCode.HEALTH_CHECK_FAILED,
                "message" to e.message
            ))
        }
    }
}

data class RouteTarget(
    val service: Any,
    val methodName: String,
    val params: Set<String> = emptySet()
) {
    // Cached method reference, lazily initialized
    @Volatile
    private var cachedMethod: java.lang.reflect.Method? = null

    // Get method reference with caching
    fun getMethod(): java.lang.reflect.Method {
        // Double-checked locking pattern
        if (cachedMethod == null) {
            synchronized(this) {
                if (cachedMethod == null) {
                    val serviceClass = service::class.java
                    val method = serviceClass.methods.find {
                        it.name == methodName
                    } ?: throw NoSuchMethodException(
                        "Method $methodName not found in ${serviceClass.simpleName}"
                    )
                    cachedMethod = method
                }
            }
        }
        return cachedMethod!!
    }
}
