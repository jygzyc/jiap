package jadx.plugins.jiap.service

import jadx.api.plugins.JadxPluginContext

import jadx.plugins.jiap.model.JiapServiceInterface
import jadx.plugins.jiap.model.JiapResult
import jadx.plugins.jiap.utils.LogUtils

class AndroidFrameworkService(override val pluginContext: JadxPluginContext) : JiapServiceInterface{
    
    fun handleGetSystemServiceImpl(interfaceName: String): JiapResult {
        try {
            val interfaceClazz = decompiler.searchJavaClassOrItsParentByOrigFullName(interfaceName)
                ?: return JiapResult(success = false, data = hashMapOf("error" to "getSystemServiceImpl: $interfaceName not found"))
            val serviceClazz = decompiler.classes.firstOrNull {
                it.smali.contains(".super L${interfaceClazz.fullName.replace('.', '/')}\$Stub;") 
            } ?: return JiapResult(success = false, data = hashMapOf("error" to "getSystemServiceImpl: Service implementation not found"))

            val result = hashMapOf<String, Any>(
                "type" to "list",
                "name" to serviceClazz.fullName,
                "methods-list" to serviceClazz.methods.map { it.toString() },
                "fields-list" to serviceClazz.fields.map { it.toString() }
            )
            return JiapResult(success = true, data = result)
        } catch (e: Exception) {
            LogUtils.error("handleGetSystemServiceImpl", e)
            return JiapResult(success = false, data = hashMapOf("error" to "getSystemServiceImpl: ${e.message}"))
        }
    }
}