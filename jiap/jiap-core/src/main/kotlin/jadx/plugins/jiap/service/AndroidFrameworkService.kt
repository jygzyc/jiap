package jadx.plugins.jiap.service

import jadx.api.JadxDecompiler
import jadx.plugins.jiap.model.JiapServiceInterface
import jadx.plugins.jiap.api.JiapApiResult

class AndroidFrameworkService(override val decompiler: JadxDecompiler) : JiapServiceInterface {

    fun handleGetSystemServiceImpl(iface: String): JiapApiResult {
        try {
            val interfaceClazz = decompiler.searchJavaClassOrItsParentByOrigFullName(iface)
                ?: return JiapApiResult(success = false, data = hashMapOf("error" to "handleGetSystemServiceImpl: $iface not found"))
            val serviceClazz = decompiler.classes.firstOrNull {
                it.smali.contains(".super L${interfaceClazz.fullName.replace('.', '/')}\$Stub;")
            } ?: return JiapApiResult(success = false, data = hashMapOf("error" to "handleGetSystemServiceImpl: Service implementation not found"))

            val result = hashMapOf<String, Any>(
                "type" to "list",
                "name" to serviceClazz.fullName,
                "methods-list" to serviceClazz.methods.map { it.toString() },
                "fields-list" to serviceClazz.fields.map { it.toString() }
            )
            return JiapApiResult(success = true, data = result)
        } catch (e: Exception) {
            return JiapApiResult(success = false, data = hashMapOf("error" to "handleGetSystemServiceImpl: ${e.message}"))
        }
    }
}
