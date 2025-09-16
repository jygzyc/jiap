package jadx.plugins.jiap

import io.javalin.Javalin
import io.javalin.http.Context
import io.javalin.http.bodyAsClass
import jadx.api.plugins.JadxPluginContext
import jadx.gui.ui.MainWindow

import org.slf4j.LoggerFactory
import java.util.concurrent.ScheduledExecutorService
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import kotlin.collections.mapOf

import jadx.plugins.jiap.service.CommonService
import jadx.plugins.jiap.service.AndroidFrameworkService
import jadx.plugins.jiap.service.AndroidAppService
import jadx.plugins.jiap.model.JiapResult
import jadx.plugins.jiap.utils.ErrorMessages
import jadx.plugins.jiap.utils.JiapConstants
import jadx.plugins.jiap.utils.PreferencesManager

class JiapServer(
    private val pluginContext: JadxPluginContext,
    private val scheduler: ScheduledExecutorService
) {
    
    companion object {
        private val logger = LoggerFactory.getLogger(JiapServer::class.java)
    }

    private var started = false
    private lateinit var app: Javalin
    private var currentPort = AtomicInteger(PreferencesManager.getPort())

    private val commonService: CommonService = CommonService(pluginContext)
    private val androidFrameworkService: AndroidFrameworkService = AndroidFrameworkService(pluginContext)
    private val androidAppService: AndroidAppService = AndroidAppService(pluginContext)
    
    fun start(port: Int = JiapConstants.DEFAULT_PORT) {
        try {
            app = Javalin.create().start(port)
            currentPort.set(port)
            PreferencesManager.setPort(port)
            configureRoutes()
            started = true
            logger.info("JIAP: Server started on port $port")

            // Add shutdown hook to clean up port when JVM exits
            Runtime.getRuntime().addShutdownHook(Thread {
                try {
                    logger.info("JIAP: Shutdown hook triggered, stopping server...")
                    stop()
                } catch (e: Exception) {
                    logger.error("JIAP: Error during shutdown hook", e)
                }
            })
        } catch (e: Exception) {
            logger.error("JIAP: Failed to start JIAP server", e)
        }
    }

    fun stop() {
        try {
            if (this::app.isInitialized) {
                app.stop()
                // Wait for port to be completely released
                Thread.sleep(500)
            }
            started = false
            logger.info("JIAP: Server stopped and port released")
        } catch (e: Exception) {
            logger.error("JIAP: Error stopping JIAP server", e)
            started = false
        }
    }

    fun restart() {
        Thread {
            try{
                logger.info("JIAP: Restarting JIAP server...")
                stop()
                Thread.sleep(1000)
                start(currentPort.get())
            }catch(e: Exception){
                logger.error("JIAP: Error restarting server", e)
                // Ensure server is stopped even on exception
                try {
                    stop()
                } catch (stopException: Exception) {
                    logger.error("JIAP: Error stopping server during restart exception", stopException)
                }
                started = false
            }
        }.start()
    }

    fun getCurrentPort(): Int {
        return currentPort.get()
    }

    fun isRunning(): Boolean {
        return started
    }

    fun delayedInitialization() {
        val periodicTask = scheduler.scheduleAtFixedRate({
            try {
                if(started) {
                    scheduler.shutdown()
                    return@scheduleAtFixedRate
                }
                if(isJadxDecompilerAvailable()) {
                    logger.info("JIAP: Jadx Decompiler fully loaded")
                    restart()
                    scheduler.shutdown()
                } else {
                    logger.debug("JIAP: Waiting for JADX decompiler to be available...")
                }
            } catch (e: Exception) {
                logger.error("JIAP: Error during delayed initialization", e)
            }
        }, 2, 1, TimeUnit.SECONDS)
        
        scheduler.schedule({
            if (!started) {
                logger.warn("JIAP: Delayed initialization did not complete in time, forcing start")
                try {
                    restart()
                } catch (e: Exception) {
                    logger.error("JIAP: Failed to start server during forced initialization", e)
                }
            }
            periodicTask.cancel(false)
        }, 30, TimeUnit.SECONDS)
    }

    private fun isJadxDecompilerAvailable(): Boolean {
        try{
            if (pluginContext.guiContext != null) {
                val mainFrame = pluginContext.guiContext?.mainFrame
                if (mainFrame is MainWindow) {
                    val wrapper = mainFrame.wrapper
                    val classes = wrapper.includedClassesWithInners
                    if (classes == null){
                        logger.debug("JIAP: No classes to be available")
                        return false
                    }
                } else {
                    logger.debug("JIAP: Main frame is not MainWindow instance")
                }
            }
            val isDecompilerLoaded = pluginContext.decompiler != null
            if (!isDecompilerLoaded) {
                logger.debug("JIAP: Decompiler not loaded")
                return false
            }
            logger.debug("JIAP: Found ${pluginContext.decompiler.classesWithInners.size} classes, JADX appears to be loaded")
            return true
        }catch (e: Exception){
            logger.debug("JIAP: jadx decompiler not available: ${e.message}")
            return false
        }
    }
    
    private fun configureRoutes() {
        // Health check endpoint
        app.get("/health") { ctx ->
            logger.info("JIAP Plugin: Got Health Ping")
            handleHealthCheck(ctx)
        }

        // Basic JADX endpoints
        app.post("/api/jiap/get_all_classes") { ctx ->
            logger.info("JIAP Plugin: Get All Classes")
            val result = commonService.handleGetAllClasses()
            handleServiceResult(result, ctx)
        }

        app.post("/api/jiap/get_class_source") { ctx ->
            logger.info("JIAP Plugin: Get Class Source")
            val payload = ctx.bodyAsClass<Map<String, Any>>()
            val className = extractStringParam(payload, "class")
                ?: run {
                    ctx.status(400).json(mapOf("error" to ErrorMessages.MISSING_CLASS_PARAM))
                    return@post
                }
            val isSmali = extractBooleanParam(payload, "smali")
                ?: run {
                    ctx.status(400).json(mapOf("error" to ErrorMessages.INVALID_SMALI_PARAM))
                    return@post
                }
            val result = commonService.handleGetClassSource(className, isSmali)
            handleServiceResult(result, ctx)
        }

        app.post("/api/jiap/get_method_source") { ctx ->
            logger.info("JIAP Plugin: Get Method Source")
            val payload = ctx.bodyAsClass<Map<String, Any>>()
            val methodName = extractStringParam(payload, "method")
                ?: run {
                    ctx.status(400).json(mapOf("error" to ErrorMessages.MISSING_CLASS_PARAM))
                    return@post
                }
            val isSmali = extractBooleanParam(payload, "smali")
                ?: run {
                    ctx.status(400).json(mapOf("error" to ErrorMessages.INVALID_SMALI_PARAM))
                    return@post
                }
            val result = commonService.handleGetMethodSource(methodName, isSmali)
            handleServiceResult(result, ctx)
        }

        app.post("/api/jiap/list_methods")  { ctx ->
            logger.info("JIAP Plugin: List Methods of Class")
            val payload = ctx.bodyAsClass<Map<String, Any>>()
            val className = extractStringParam(payload, "class")
                ?: run {
                    ctx.status(400).json(mapOf("error" to ErrorMessages.MISSING_CLASS_PARAM))
                    return@post
                }
            val result = commonService.handleListMethodsOfClass(className)
            handleServiceResult(result, ctx)
        }

        app.post("/api/jiap/search_class") { ctx ->
            logger.info("JIAP Plugin: Search Class by Name")
            val payload = ctx.bodyAsClass<Map<String, Any>>()
            val className = extractStringParam(payload, "class")
                ?: run {
                    ctx.status(400).json(mapOf("error" to ErrorMessages.MISSING_CLASS_PARAM))
                    return@post
                }
            val result = commonService.handleSearchClassByName(className)
            handleServiceResult(result, ctx)
        }

        app.post("/api/jiap/get_method_xref") { ctx ->
            logger.info("JIAP Plugin: Get Method Xref")
            val payload = ctx.bodyAsClass<Map<String, Any>>()
            val methodName = extractStringParam(payload, "method")
                ?: run {
                    ctx.status(400).json(mapOf("error" to ErrorMessages.MISSING_METHOD_PARAM))
                    return@post
                }
            val result = commonService.handleGetMethodXref(methodName)
            handleServiceResult(result, ctx)
        }

        // Android App specific endpoints
        app.post("/api/jiap/get_app_manifest") { ctx ->
            logger.info("JIAP Plugin: Get App Manifest")
            val result = androidAppService.handleGetAppManifest()
            handleServiceResult(result, ctx)
        }

        app.post("/api/jiap/get_main_activity") { ctx ->
            logger.info("JIAP Plugin: Get Main Activity")
            val result = androidAppService.handleGetMainActivity()
            handleServiceResult(result, ctx)
        }

        // Android Framework specific endpoints
        app.post("/api/jiap/get_system_service_impl") {ctx ->
            logger.info("JIAP Plugin: Get System Service Implementation")
            val payload = ctx.bodyAsClass<Map<String, Any>>()
            val interfaceName = extractStringParam(payload, "interface")
                ?: run {
                    ctx.status(400).json(mapOf("error" to ErrorMessages.MISSING_INTERFACE_PARAM))
                    return@post
                }
            val result = androidFrameworkService.handleGetSystemServiceImpl(interfaceName)
            handleServiceResult(result, ctx)
        }
    }

    fun handleHealthCheck(ctx: Context) {
        try{
            val status = if(isRunning()) "Running" else "Stopped"
            val url = if(isRunning()) "http://127.0.0.1:" + currentPort.get() + "/" else "N/A"
            val result = hashMapOf(
                "status" to status,
                "url" to url
            )
            ctx.status(200).json(result)
        }catch (e: Exception){
            logger.error("Jiap Server error: {}", e.message, e)
            ctx.status(500).json(mapOf("error" to "Internal server error: ${e.message}"))
        }
    }
    
    private fun extractStringParam(
        payload: Map<String, Any>,
        key: String
    ): String? {
        return payload[key] as? String
    }

    private fun extractBooleanParam(
        payload: Map<String, Any>,
        key: String
    ): Boolean? {
        return payload[key] as? Boolean
    }

    private fun handleServiceResult(result: JiapResult, ctx: Context) {
        if (result.success) {
            logger.debug("Operation completed successfully")
            ctx.json(result.data)
        } else {
            logger.error("Operation failed")
            ctx.status(500).json(result.data)
        }
    }
}
