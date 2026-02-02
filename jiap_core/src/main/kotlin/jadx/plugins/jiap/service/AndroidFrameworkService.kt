package jadx.plugins.jiap.service

import jadx.api.plugins.JadxPluginContext

import jadx.plugins.jiap.model.JiapServiceInterface
import jadx.plugins.jiap.model.JiapResult

class AndroidFrameworkService(override val pluginContext: JadxPluginContext) : JiapServiceInterface{
    
    fun handleGetSystemServiceImpl(iface: String): JiapResult {
        try {
            val interfaceClazz = decompiler.searchJavaClassOrItsParentByOrigFullName(iface)
                ?: return JiapResult(success = false, data = hashMapOf("error" to "handleGetSystemServiceImpl: $iface not found"))
            val serviceClazz = decompiler.classes.firstOrNull {
                it.smali.contains(".super L${interfaceClazz.fullName.replace('.', '/')}\$Stub;") 
            } ?: return JiapResult(success = false, data = hashMapOf("error" to "handleGetSystemServiceImpl: Service implementation not found"))

            val result = hashMapOf<String, Any>(
                "type" to "list",
                "name" to serviceClazz.fullName,
                "methods-list" to serviceClazz.methods.map { it.toString() },
                "fields-list" to serviceClazz.fields.map { it.toString() }
            )
            return JiapResult(success = true, data = result)
        } catch (e: Exception) {
            return JiapResult(success = false, data = hashMapOf("error" to "handleGetSystemServiceImpl: ${e.message}"))
        }
    }
}