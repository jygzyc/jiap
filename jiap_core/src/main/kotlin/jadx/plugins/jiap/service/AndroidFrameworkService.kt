package jadx.plugins.jiap.service

import org.slf4j.LoggerFactory
import jadx.api.JadxDecompiler
import jadx.api.plugins.JadxPluginContext
import jadx.api.JavaClass

import jadx.plugins.jiap.model.JiapServiceInterface
import jadx.plugins.jiap.model.JiapResult

class AndroidFrameworkService(override val pluginContext: JadxPluginContext) : JiapServiceInterface{
    
    companion object {
        private val logger = LoggerFactory.getLogger(AndroidFrameworkService::class.java)
    }

    val decompiler: JadxDecompiler = pluginContext.decompiler
    
    private val serviceStubIndex: Map<String, JavaClass> by lazy {
        buildServiceStubIndex()
    }

    private fun buildServiceStubIndex(): Map<String, JavaClass> {
        return decompiler.classesWithInners
            .filter { it.smali != null }
            .mapNotNull { clazz ->
                val smali = clazz.smali
                val stubMatch = ".super L(.*)\$Stub;".toRegex().find(smali)
                val interfaceName = stubMatch?.groupValues?.get(1)?.replace("/", ".")
                if (interfaceName != null) interfaceName to clazz else null
            }
            .toMap()
    }

    fun handleGetSystemServiceImpl(interfaceName: String): JiapResult {
        try {
            val serviceClazz = serviceStubIndex[interfaceName]
                ?: return JiapResult(success = false, data = hashMapOf("error" to "getSystemService: $interfaceName not found"))
            serviceClazz.decompile()
            val result = hashMapOf<String, Any>(
                "type" to "code",
                "name" to serviceClazz.fullName,
                "code" to serviceClazz.code
            )
            return JiapResult(success = true, data = result)
        } catch (e: Exception) {
            logger.error("JIAP Error: get System Service Implementation", e)
            return JiapResult(success = false, data = hashMapOf("error" to "getSystemServiceImpl: ${e.message}"))
        }
    }
}