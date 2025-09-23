package jadx.plugins.jiap.service

import org.slf4j.LoggerFactory
import jadx.api.JadxDecompiler
import jadx.api.plugins.JadxPluginContext

import jadx.plugins.jiap.model.JiapServiceInterface
import jadx.plugins.jiap.model.JiapResult

class AndroidFrameworkService(override val pluginContext: JadxPluginContext) : JiapServiceInterface{
    
    companion object {
        private val logger = LoggerFactory.getLogger(AndroidFrameworkService::class.java)
    }

    val decompiler: JadxDecompiler = pluginContext.decompiler
    
    fun handleGetSystemServiceImpl(interfaceName: String): JiapResult {
        try {
            val serviceClazz = decompiler.classesWithInners.firstOrNull {
                ".super ${interfaceName.replace(".", "/")}\$Stub" in it.smali
            } ?: return JiapResult(success = false, data = hashMapOf("error" to "getSystemService: $interfaceName not found"))
            val result = hashMapOf<String, Any>(
                "type" to "code",
                "name" to serviceClazz.fullName,
                "code" to serviceClazz.code
            )
            return JiapResult(success = true, data = result)
        }catch (e: Exception){
            logger.error("JIAP Error: get System Service Implementation", e)
            return JiapResult(success = false, data = hashMapOf("error" to "getSystemServiceImpl: ${e.message}"))
        }
    }
}