package jadx.plugins.decx.service

import jadx.api.JadxDecompiler
import jadx.plugins.decx.model.DecxServiceInterface
import jadx.plugins.decx.api.DecxApiResult

class AndroidFrameworkService(override val decompiler: JadxDecompiler) : DecxServiceInterface {

    fun handleGetSystemServiceImpl(iface: String): DecxApiResult {
        try {
            val interfaceClazz = decompiler.searchJavaClassOrItsParentByOrigFullName(iface)
                ?: return DecxApiResult(success = false, data = hashMapOf("error" to "handleGetSystemServiceImpl: $iface not found"))
            val serviceClazz = decompiler.classes.firstOrNull {
                it.smali.contains(".super L${interfaceClazz.fullName.replace('.', '/')}\$Stub;")
            } ?: return DecxApiResult(success = false, data = hashMapOf("error" to "handleGetSystemServiceImpl: Service implementation not found"))

            val result = hashMapOf<String, Any>(
                "type" to "list",
                "name" to serviceClazz.fullName,
                "methods-list" to serviceClazz.methods.map { it.toString() },
                "fields-list" to serviceClazz.fields.map { it.toString() }
            )
            return DecxApiResult(success = true, data = result)
        } catch (e: Exception) {
            return DecxApiResult(success = false, data = hashMapOf("error" to "handleGetSystemServiceImpl: ${e.message}"))
        }
    }
}
