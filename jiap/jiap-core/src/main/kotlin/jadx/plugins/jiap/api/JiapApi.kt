package jadx.plugins.jiap.api

import jadx.api.JadxDecompiler

/**
 * Type-safe API interface for JIAP core services.
 * Server and Plugin both consume this API — it has zero HTTP/transport dependencies.
 */
interface JiapApi {

    // ==================== Common Service ====================

    fun getAllClasses(): JiapApiResult
    fun getClassInfo(cls: String): JiapApiResult
    fun getClassSource(cls: String, smali: Boolean): JiapApiResult
    fun searchClassKey(key: String): JiapApiResult
    fun searchMethod(mth: String): JiapApiResult
    fun getMethodSource(mth: String, smali: Boolean): JiapApiResult
    fun getMethodXref(mth: String): JiapApiResult
    fun getFieldXref(fld: String): JiapApiResult
    fun getClassXref(cls: String): JiapApiResult
    fun getImplementOfInterface(iface: String): JiapApiResult
    fun getSubclasses(cls: String): JiapApiResult

    // ==================== Android App Service ====================

    fun getAidlInterfaces(): JiapApiResult
    fun getAppManifest(): JiapApiResult
    fun getMainActivity(): JiapApiResult
    fun getApplication(): JiapApiResult
    fun getExportedComponents(): JiapApiResult
    fun getDeepLinks(): JiapApiResult
    fun getDynamicReceivers(): JiapApiResult
    fun getAllResources(): JiapApiResult
    fun getResourceFile(res: String): JiapApiResult
    fun getStrings(): JiapApiResult

    // ==================== Android Framework Service ====================

    fun getSystemServiceImpl(iface: String): JiapApiResult
}

/**
 * Unified result type for all API calls.
 */
data class JiapApiResult(
    val success: Boolean,
    val data: Map<String, Any>
) {
    companion object {
        fun ok(data: Map<String, Any>) = JiapApiResult(true, data)
        fun fail(message: String) = JiapApiResult(false, mapOf("error" to message))
    }
}
