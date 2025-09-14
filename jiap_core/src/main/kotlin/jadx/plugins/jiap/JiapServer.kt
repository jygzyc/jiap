package jadx.plugins.jiap

import io.javalin.Javalin
import io.javalin.http.Context
import io.javalin.http.bodyAsClass
import jadx.api.plugins.JadxPluginContext
import jadx.gui.ui.MainWindow

import org.slf4j.LoggerFactory
import java.util.concurrent.ScheduledExecutorService
import java.util.concurrent.TimeUnit
import kotlin.collections.mapOf

import jadx.plugins.jiap.service.CommonService
import jadx.plugins.jiap.service.AndroidFrameworkService
import jadx.plugins.jiap.service.AndroidAppService
import jadx.plugins.jiap.model.JiapResult
import jadx.plugins.jiap.utils.ErrorMessages
import jadx.plugins.jiap.utils.JiapConstants

class JiapServer(
    private val pluginContext: JadxPluginContext
) {
    
    companion object {
        private val logger = LoggerFactory.getLogger(JiapServer::class.java)
    }

    private var started = false
    private lateinit var app: Javalin

    private val scheduler: ScheduledExecutorService = JiapPlugin.scheduler
    private val commonService: CommonService = CommonService(pluginContext)
    private val androidFrameworkService: AndroidFrameworkService = AndroidFrameworkService(pluginContext)
    private val androidAppService: AndroidAppService = AndroidAppService(pluginContext)
    
    fun start(port: Int = JiapConstants.DEFAULT_PORT) {
        try {
            app = Javalin.create().start(port)
            configureRoutes()
            logger.info("JIAP server started on port $port")
        } catch (e: Exception) {
            logger.error("Failed to start JIAP server", e)
        }
    }

    fun stop() {
        try {
            if (this::app.isInitialized) {
                app.stop()
                logger.info("JIAP server stopped")
            }
        } catch (e: Exception) {
            logger.error("Error stopping JIAP server", e)
        }
    }

    fun delayedInitialization() {
        scheduler.scheduleAtFixedRate({
            try {
                if(started) {
                    scheduler.shutdown()
                    return@scheduleAtFixedRate
                }
                if(isJadxDecompilerAvailable()) {
                    start()
                    started = true
                    scheduler.shutdown()
                } else {
                    logger.info("JIAP: Waiting for JADX decompiler to be available...")
                }
            } catch (e: Exception) {
                logger.error("JIAP: Error during delayed initialization", e)
            }
        }, 2, 1, TimeUnit.SECONDS)
        scheduler.schedule({
            if (!started) {
                logger.warn("JIAP: Delayed initialization did not complete in time, forcing start")
                try {
                    start()
                    started = true
                } catch (e: Exception) {
                    logger.error("JIAP: Failed to start server during forced initialization", e)
                }
            }
        }, 30, TimeUnit.SECONDS)
    }

    private fun isJadxDecompilerAvailable(): Boolean {
        try{
            if (pluginContext.guiContext != null) {
                val mainWindow = pluginContext.guiContext?.mainFrame as MainWindow
                val wrapper = mainWindow.wrapper
                val classes = wrapper.includedClassesWithInners
                if (classes == null){
                    logger.debug("JIAP: No classes to be available")
                    return false
                }
            }
            val isDecompilerLoaded = pluginContext.decompiler != null
            if (!isDecompilerLoaded) {
                logger.debug("JIAP: Decompiler not loaded")
                return false
            }
            return true
        }catch (e: Exception){
            logger.error("Jiap Plugin: jadx decompiler not available", e)
            return false
        }
    }
    
    private fun configureRoutes() {
        // Health check endpoint
        app.get("/health") { ctx ->
            handleHealthCheck(ctx)
        }

        // Basic JADX endpoints
        app.post("/api/jiap/get_all_classes") { ctx ->
            val result = commonService.handleGetAllClasses()
            handleServiceResult(result, ctx)
        }

        app.post("/api/jiap/get_class_source") { ctx ->
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

        // Android App specific endpoints
        app.post("/api/jiap/get_app_manifest") { ctx ->
            val result = androidAppService.handleGetAppManifest()
            handleServiceResult(result, ctx)
        }

        // Android Framework specific endpoints
        app.post("/api/jiap/get_system_service_impl") {ctx ->
            val payload = ctx.bodyAsClass<Map<String, Any>>()
            val interfaceName = extractStringParam(payload, "interface")
                ?: run {
                    ctx.status(400).json(mapOf("error" to ErrorMessages.MISSING_INTERFACE_PARAM))
                    return@post
                }
            val result = androidFrameworkService.handleGetSystemServiceImpl(interfaceName)
            handleServiceResult(result, ctx)
        }
        
        // app.post("/api/jadx/search_class_by_name") { ctx ->
        //     val payload = ctx.bodyAsClass<Map<String, Any>>()
        //     val fileId = extractStringParam(payload, "id")
        //         ?: run {
        //             ctx.status(400).json(mapOf("error" to ErrorMessages.MISSING_FILEID_PARAM))
        //             return@post
        //         }
        //     val classFullName = extractStringParam(payload, "class")
        //         ?: run {
        //             ctx.status(400).json(mapOf("error" to ErrorMessages.MISSING_CLASS_PARAM))
        //             return@post
        //         }
            
        //     logger.info("Request received: searchClassByName for class '{}'", classFullName)
        //     handleServiceResult(jadxCommonService.handleSearchClassByName(fileId, classFullName), ctx)
        // }
        
        // app.post("/api/jadx/get_class_source") { ctx ->
        //     val payload = ctx.bodyAsClass<Map<String, Any>>()
        //     val fileId = extractStringParam(payload, "id")
        //         ?: run {
        //             ctx.status(400).json(mapOf("error" to ErrorMessages.MISSING_FILEID_PARAM))
        //             return@post
        //         }
        //     val className = extractStringParam(payload, "class")
        //         ?: run {
        //             ctx.status(400).json(mapOf("error" to ErrorMessages.MISSING_CLASS_PARAM))
        //             return@post
        //         }
        //     val isSmali = extractBooleanParam(payload, "smali")
        //         ?: run {
        //             ctx.status(400).json(mapOf("error" to ErrorMessages.INVALID_SMALI_PARAM))
        //             return@post
        //         }
            
        //     logger.info("Request received: getClassSource for class '{}', smali={}", className, isSmali)
        //     handleServiceResult(jadxCommonService.handleGetClassSource(fileId, className, isSmali), ctx)
        // }
        
        // app.post("/api/jadx/get_method_xref") { ctx ->
        //     val payload = ctx.bodyAsClass<Map<String, Any>>()
        //     val fileId = extractStringParam(payload, "id")
        //         ?: run {
        //             ctx.status(400).json(mapOf("error" to ErrorMessages.MISSING_FILEID_PARAM))
        //             return@post
        //         }
        //     val methodInfo = extractStringParam(payload, "method")
        //         ?: run {
        //             ctx.status(400).json(mapOf("error" to ErrorMessages.MISSING_METHOD_PARAM))
        //             return@post
        //         }
            
        //     logger.info("Request received: getMethodXref for method '{}'", methodInfo)
        //     handleServiceResult(jadxCommonService.handleGetMethodXref(fileId, methodInfo), ctx)
        // }
        
        // app.post("/api/jadx/get_class_xref") { ctx ->
        //     val payload = ctx.bodyAsClass<Map<String, Any>>()
        //     val fileId = extractStringParam(payload, "id")
        //         ?: run {
        //             ctx.status(400).json(mapOf("error" to ErrorMessages.MISSING_FILEID_PARAM))
        //             return@post
        //         }
        //     val className = extractStringParam(payload, "class")
        //         ?: run {
        //             ctx.status(400).json(mapOf("error" to ErrorMessages.MISSING_CLASS_PARAM))
        //             return@post
        //         }
            
        //     logger.info("Request received: getClassXref for class '{}'", className)
        //     handleServiceResult(jadxCommonService.handleGetClassXref(fileId, className), ctx)
        // }
        
        // app.post("/api/jadx/get_field_xref") { ctx ->
        //     val payload = ctx.bodyAsClass<Map<String, Any>>()
        //     val fileId = extractStringParam(payload, "id")
        //         ?: run {
        //             ctx.status(400).json(mapOf("error" to ErrorMessages.MISSING_FILEID_PARAM))
        //             return@post
        //         }
        //     val fieldInfo = extractStringParam(payload, "field")
        //         ?: run {
        //             ctx.status(400).json(mapOf("error" to ErrorMessages.MISSING_FIELD_PARAM))
        //             return@post
        //         }
            
        //     logger.info("Request received: getFieldXref for field '{}'", fieldInfo)
        //     handleServiceResult(jadxCommonService.handleGetFieldXref(fileId, fieldInfo), ctx)
        // }
        
        // app.post("/api/jadx/get_interface_impl") { ctx ->
        //     val payload = ctx.bodyAsClass<Map<String, Any>>()
        //     val fileId = extractStringParam(payload, "id")
        //         ?: run {
        //             ctx.status(400).json(mapOf("error" to ErrorMessages.MISSING_FILEID_PARAM))
        //             return@post
        //         }
        //     val interfaceName = extractStringParam(payload, "interface")
        //         ?: run {
        //             ctx.status(400).json(mapOf("error" to ErrorMessages.MISSING_INTERFACE_PARAM))
        //             return@post
        //         }
            
        //     logger.info("Request received: getImplementOfInterface for interface '{}'", interfaceName)
        //     handleServiceResult(jadxCommonService.handleGetImplementOfInterface(fileId, interfaceName), ctx)
        // }
        
        // app.post("/api/jadx/get_subclasses") { ctx ->
        //     val payload = ctx.bodyAsClass<Map<String, Any>>()
        //     val fileId = extractStringParam(payload, "id")
        //         ?: run {
        //             ctx.status(400).json(mapOf("error" to ErrorMessages.MISSING_FILEID_PARAM))
        //             return@post
        //         }
        //     val className = extractStringParam(payload, "class")
        //         ?: run {
        //             ctx.status(400).json(mapOf("error" to ErrorMessages.MISSING_CLASS_PARAM))
        //             return@post
        //         }
            
        //     logger.info("Request received: getSubclasses for class '{}'", className)
        //     handleServiceResult(jadxCommonService.handleGetSubclasses(fileId, className), ctx)
        // }
        
        // app.post("/api/jadx/get_system_service_impl") { ctx ->
        //     val payload = ctx.bodyAsClass<Map<String, Any>>()
        //     val fileId = extractStringParam(payload, "id")
        //         ?: run {
        //             ctx.status(400).json(mapOf("error" to ErrorMessages.MISSING_FILEID_PARAM))
        //             return@post
        //         }
        //     val className = extractStringParam(payload, "class")
        //         ?: run {
        //             ctx.status(400).json(mapOf("error" to ErrorMessages.MISSING_CLASS_PARAM))
        //             return@post
        //         }
            
        //     logger.info("Request received: getSystemServiceImpl for class '{}'", className)
        //     handleServiceResult(androidFrameworkService.handleGetSystemServiceImpl(fileId, className), ctx)
        // }
        
        // app.post("/api/jadx/get_app_manifest") { ctx ->
        //     val payload = ctx.bodyAsClass<Map<String, Any>>()
        //     val fileId = extractStringParam(payload, "id")
        //         ?: run {
        //             ctx.status(400).json(mapOf("error" to ErrorMessages.MISSING_FILEID_PARAM))
        //             return@post
        //         }
            
        //     logger.info("Request received: getAppManifest")
        //     handleServiceResult(androidAppService.handleGetAppManifest(fileId), ctx)
        // }
        
        // // Exception handler
        // app.exception(Exception::class.java) { e, ctx ->
        //     logger.error("Unhandled exception: {}", e.message, e)
        //     ctx.status(500).json(mapOf("error" to "Internal server error"))
        // }
    }

    fun handleHealthCheck(ctx: Context) {
        try{
            ctx.status(200).json(mapOf("status" to "OK"))
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
