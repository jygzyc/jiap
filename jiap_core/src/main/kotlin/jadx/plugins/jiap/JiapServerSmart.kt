package jadx.plugins.jiap

import io.javalin.Javalin
import io.javalin.http.Context
import io.javalin.http.bodyAsClass
import jadx.api.plugins.JadxPluginContext
import jadx.gui.ui.MainWindow

import org.slf4j.LoggerFactory
import java.util.concurrent.ScheduledExecutorService
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger
import kotlin.collections.mapOf
import java.lang.reflect.Method
import java.lang.reflect.Parameter

import jadx.plugins.jiap.service.CommonService
import jadx.plugins.jiap.service.AndroidFrameworkService
import jadx.plugins.jiap.service.AndroidAppService
import jadx.plugins.jiap.model.JiapResult
import jadx.plugins.jiap.utils.ErrorMessages
import jadx.plugins.jiap.utils.JiapConstants
import jadx.plugins.jiap.utils.PreferencesManager

/**
 * 智能参数映射的JiapServer实现
 *
 * 特点：
 * - 自动映射JSON参数到方法参数
 * - 支持类型自动转换
 * - 极简的路由配置
 */
class JiapServerSmart(
    private val pluginContext: JadxPluginContext,
    private val scheduler: ScheduledExecutorService
) {

    companion object {
        private val logger = LoggerFactory.getLogger(JiapServerSmart::class.java)
    }

    // 状态管理
    private val started = AtomicBoolean(false)
    private lateinit var app: Javalin
    private val currentPort = AtomicInteger(PreferencesManager.getPort())

    // 服务实例
    private val commonService: CommonService = CommonService(pluginContext)
    private val androidFrameworkService: AndroidFrameworkService = AndroidFrameworkService(pluginContext)
    private val androidAppService: AndroidAppService = AndroidAppService(pluginContext)

    /**
     * 路由目标定义
     */
    data class RouteTarget(
        val service: Any,
        val methodName: String,
        val requiredParams: Set<String> = emptySet()
    )

    /**
     * 路由映射表 - 路径到服务方法的映射
     */
    private val routeMappings = mapOf(
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
            requiredParams = setOf("method")
        ),
        "/api/jiap/get_class_info" to RouteTarget(
            service = commonService,
            methodName = "handleGetClassInfo",
            requiredParams = setOf("class")
        ),
        "/api/jiap/get_method_xref" to RouteTarget(
            service = commonService,
            methodName = "handleGetMethodXref",
            requiredParams = setOf("method")
        ),
        "/api/jiap/get_class_xref" to RouteTarget(
            service = commonService,
            methodName = "handleGetClassXref",
            requiredParams = setOf("class")
        ),
        "/api/jiap/get_implement" to RouteTarget(
            service = commonService,
            methodName = "handleGetImplementOfInterface",
            requiredParams = setOf("interface")
        ),
        "/api/jiap/get_sub_classes" to RouteTarget(
            service = commonService,
            methodName = "handleGetSubclasses",
            requiredParams = setOf("class")
        ),
        "/api/jiap/get_class_source" to RouteTarget(
            service = commonService,
            methodName = "handleGetClassSource",
            requiredParams = setOf("class", "smali")
        ),
        "/api/jiap/get_method_source" to RouteTarget(
            service = commonService,
            methodName = "handleGetMethodSource",
            requiredParams = setOf("method", "smali")
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
            requiredParams = setOf("interface")
        )
    )

    /**
     * 启动服务器
     */
    fun start(port: Int = JiapConstants.DEFAULT_PORT) {
        if (started.get()) {
            logger.warn("JIAP: Server already running on port ${currentPort.get()}")
            return
        }

        if (!isJadxDecompilerAvailable()) {
            logger.error("JIAP: ${ErrorMessages.JADX_NOT_AVAILABLE}")
            throw IllegalStateException(ErrorMessages.JADX_NOT_AVAILABLE)
        }

        try {
            logger.info("JIAP: Starting server on port $port")
            app = Javalin.create().start(port)
            currentPort.set(port)
            PreferencesManager.setPort(port)
            configureRoutes()
            started.set(true)
            logger.info("JIAP: Server started successfully on port $port")

            setupShutdownHook()
        } catch (e: Exception) {
            started.set(false)
            logger.error("JIAP: ${ErrorMessages.SERVER_START_FAILED}", e)
            throw RuntimeException(ErrorMessages.SERVER_START_FAILED, e)
        }
    }

    /**
     * 停止服务器
     */
    fun stop() {
        if (!started.getAndSet(false)) {
            logger.debug("JIAP: Server already stopped")
            return
        }

        try {
            if (this::app.isInitialized) {
                app.stop()
                Thread.sleep(500)
            }
            logger.info("JIAP: Server stopped successfully")
        } catch (e: Exception) {
            logger.error("JIAP: ${ErrorMessages.SERVER_STOP_FAILED}", e)
        }
    }

    /**
     * 重启服务器
     */
    fun restart() {
        if (!started.get()) {
            logger.info("JIAP: Server not running, starting...")
            start(currentPort.get())
            return
        }

        Thread({
            try {
                logger.info("JIAP: Restarting server...")
                stop()
                Thread.sleep(1000)
                start(currentPort.get())
            } catch (e: Exception) {
                logger.error("JIAP: ${ErrorMessages.SERVER_RESTART_FAILED}", e)
                started.set(false)
            }
        }, "JiapServer-Restart").apply {
            isDaemon = true
        }.start()
    }

    fun getCurrentPort(): Int = currentPort.get()
    fun isRunning(): Boolean = started.get()

    /**
     * 配置路由
     */
    private fun configureRoutes() {
        // Health check
        app.get("/health") { ctx ->
            handleHealthCheck(ctx)
        }

        // 自动注册所有API路由
        routeMappings.keys.forEach { path ->
            app.post(path) { ctx ->
                handleRoute(ctx, path)
            }
        }

        logger.info("JIAP: Registered ${routeMappings.size} API routes")
    }

    /**
     * 智能路由处理器
     *
     * 这个方法是核心，它：
     * 1. 根据路径找到对应的Service方法
     * 2. 从JSON中提取参数
     * 3. 自动转换参数类型
     * 4. 调用方法并返回结果
     */
    private fun handleRoute(ctx: Context, path: String) {
        try {
            val operation = path.substringAfterLast("/")
            logger.info("JIAP: $operation")

            // 获取路由目标
            val routeTarget = routeMappings[path]
                ?: throw IllegalArgumentException("Unknown endpoint: $path")

            // 获取请求体
            val payload = ctx.bodyAsClass<Map<String, Any>>()

            // 验证必需参数
            validateRequiredParams(payload, routeTarget.requiredParams, ctx) ?: return

            // 动态调用方法
            val result = invokeServiceMethod(routeTarget, payload)

            handleServiceResult(result, ctx)
        } catch (e: Exception) {
            logger.error("JIAP: Error handling route $path", e)
            handleRouteError(ctx, e, path)
        }
    }

    /**
     * 验证必需参数
     */
    private fun validateRequiredParams(
        payload: Map<String, Any>,
        requiredParams: Set<String>,
        ctx: Context
    ): Boolean? {
        for (param in requiredParams) {
            if (payload[param] == null || payload[param].toString().isBlank()) {
                ctx.status(400).json(mapOf(
                    "error" to "${ErrorMessages.MISSING_REQUIRED_PARAMETER}: $param"
                ))
                return false
            }
        }
        return true
    }

    /**
     * 动态调用Service方法
     *
     * 这是魔法发生的地方：
     * - 通过反射获取方法
     - 根据参数类型自动转换JSON值
     - 调用方法并返回结果
     */
    private fun invokeServiceMethod(
        routeTarget: RouteTarget,
        payload: Map<String, Any>
    ): JiapResult {
        // 获取Service类
        val serviceClass = routeTarget.service::class.java

        // 查找方法
        val method = serviceClass.methods.find {
            it.name == routeTarget.methodName
        } ?: throw NoSuchMethodException(
            "Method ${routeTarget.methodName} not found in ${serviceClass.simpleName}"
        )

        // 准备参数
        val args = method.parameters.map { param ->
            convertParameterValue(payload[param.name ?: param.name, param.type])
        }.toTypedArray()

        // 调用方法
        @Suppress("UNCHECKED_CAST")
        return method.invoke(routeTarget.service, *args) as JiapResult
    }

    /**
     * 智能参数值转换
     *
     * 根据目标类型自动转换JSON值：
     * - Boolean: "true"/"1"/"yes" -> true, 其他 -> false
     * - String: 保持字符串
     * - Int: "123" -> 123
     */
    private fun convertParameterValue(
        value: Any?,
        targetType: Class<*>
    ): Any {
        if (value == null) {
            // 根据类型提供默认值
            return when (targetType) {
                Boolean::class.java, java.lang.Boolean.TYPE -> false
                String::class.java -> ""
                Int::class.java, java.lang.Integer.TYPE -> 0
                else -> throw IllegalArgumentException(
                    "Cannot convert null to type ${targetType.simpleName}"
                )
            }
        }

        return when (targetType) {
            Boolean::class.java, java.lang.Boolean.TYPE -> {
                when (value) {
                    is Boolean -> value
                    is String -> {
                        val lower = value.lowercase()
                        when (lower) {
                            "true", "1", "yes", "on" -> true
                            else -> false
                        }
                    }
                    is Number -> value.toInt() != 0
                    else -> false
                }
            }

            String::class.java -> value.toString()

            Int::class.java, java.lang.Integer.TYPE -> {
                when (value) {
                    is Number -> value.toInt()
                    is String -> {
                        try {
                            if (value.startsWith("0x")) {
                                value.substring(2).toInt(16)
                            } else {
                                value.toInt()
                            }
                        } catch (e: NumberFormatException) {
                            throw IllegalArgumentException(
                                "Cannot convert '$value' to integer"
                            )
                        }
                    }
                    else -> 0
                }
            }

            else -> value
        }
    }

    /**
     * 错误处理
     */
    private fun handleRouteError(ctx: Context, e: Exception, path: String) {
        val message = when (e) {
            is IllegalArgumentException -> e.message ?: "Invalid request"
            is NoSuchMethodException -> e.message ?: "Method not found"
            else -> "${ErrorMessages.INTERNAL_SERVER_ERROR}: ${e.message}"
        }

        ctx.status(500).json(mapOf(
            "error" to message,
            "path" to path
        ))
    }

    /**
     * 处理健康检查
     */
    fun handleHealthCheck(ctx: Context) {
        try {
            val running = started.get()
            val status = if (running) "Running" else "Stopped"
            val url = if (running) "http://127.0.0.1:${currentPort.get()}/" else "N/A"

            val result = mapOf(
                "status" to status,
                "url" to url,
                "port" to currentPort.get(),
                "timestamp" to System.currentTimeMillis(),
                "routes" to routeMappings.keys.sorted()
            )

            ctx.status(200).json(result)
        } catch (e: Exception) {
            logger.error("JIAP: ${ErrorMessages.HEALTH_CHECK_FAILED}", e)
            ctx.status(500).json(mapOf(
                "error" to ErrorMessages.INTERNAL_SERVER_ERROR,
                "message" to e.message
            ))
        }
    }

    /**
     * 处理服务调用结果
     */
    private fun handleServiceResult(result: JiapResult, ctx: Context) {
        if (result.success) {
            logger.debug("JIAP: Service call completed successfully")
            ctx.json(result.data)
        } else {
            logger.error("JIAP: ${ErrorMessages.SERVICE_CALL_FAILED}")
            ctx.status(500).json(result.data)
        }
    }

    /**
     * 设置关闭钩子
     */
    private fun setupShutdownHook() {
        Runtime.getRuntime().addShutdownHook(Thread({
            try {
                logger.info("JIAP: Shutdown hook triggered")
                stop()
            } catch (e: Exception) {
                logger.error("JIAP: Error during shutdown", e)
            }
        }, "JiapServer-ShutdownHook"))
    }

    /**
     * 检查JADX是否可用
     */
    private fun isJadxDecompilerAvailable(): Boolean {
        return try {
            val guiContext = pluginContext.guiContext
            if (guiContext != null) {
                val mainFrame = guiContext.mainFrame
                if (mainFrame is MainWindow) {
                    val wrapper = mainFrame.wrapper
                    val classes = wrapper.includedClassesWithInners
                    if (classes == null) {
                        logger.debug("JIAP: No classes available")
                        return false
                    }
                } else {
                    logger.debug("JIAP: Main frame is not MainWindow instance")
                }
            }

            val decompiler = pluginContext.decompiler
            if (decompiler == null) {
                logger.debug("JIAP: Decompiler not loaded")
                return false
            }

            val classCount = decompiler.classesWithInners.size
            if (classCount > 0) {
                logger.debug("JIAP: Found $classCount classes, JADX ready")
                true
            } else {
                logger.debug("JIAP: No classes loaded")
                false
            }
        } catch (e: Exception) {
            logger.debug("JIAP: JADX availability check failed: ${e.message}")
            false
        }
    }

    /**
     * 延迟初始化
     */
    fun delayedInitialization() {
        logger.info("JIAP: Starting delayed initialization")

        val periodicTask = scheduler.scheduleAtFixedRate({
            try {
                if (started.get()) {
                    logger.info("JIAP: Server started, stopping initialization checker")
                    scheduler.shutdown()
                    return@scheduleAtFixedRate
                }

                if (isJadxDecompilerAvailable()) {
                    logger.info("JIAP: JADX decompiler ready, starting server...")
                    restart()
                    scheduler.shutdown()
                } else {
                    logger.debug("JIAP: Waiting for JADX decompiler...")
                }
            } catch (e: Exception) {
                logger.error("JIAP: Error during initialization check", e)
            }
        }, 2, 1, TimeUnit.SECONDS)

        scheduler.schedule({
            if (!started.get()) {
                logger.warn("JIAP: ${ErrorMessages.JADX_INIT_TIMEOUT}")
            }
            periodicTask.cancel(false)
        }, 30, TimeUnit.SECONDS)
    }
}

/**
 * 使用示例：
 *
 * 1. 无参数接口：
 * POST /api/jiap/get_all_classes
 * {}
 *
 * 2. 单参数接口：
 * POST /api/jiap/search_method
 * {
 *     "method": "onCreate"
 * }
 *
 * 3. 多参数接口：
 * POST /api/jiap/get_class_source
 * {
 *     "class": "com.example.MainActivity",
 *     "smali": "true"  // 会自动转换为Boolean
 * }
 *
 * 4. 类型自动转换：
 * - "true"/"1"/"yes" -> true
 * - "false"/"0"/"no" -> false
 * - "123"/"0x7B" -> 123
 *
 * 5. 错误响应：
 * {
 *     "error": "Missing required parameter: method",
 *     "path": "/api/jiap/search_method"
 * }
 */