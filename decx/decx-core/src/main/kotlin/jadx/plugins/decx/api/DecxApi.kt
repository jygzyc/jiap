package jadx.plugins.decx.api

import jadx.api.JadxDecompiler

/**
 * Type-safe API interface for Decx core services.
 * Server and Plugin both consume this API — it has zero HTTP/transport dependencies.
 */
interface DecxApi {

    // ==================== Common Service ====================

    fun getAllClasses(): DecxApiResult
    fun getClassInfo(cls: String): DecxApiResult
    fun getClassSource(cls: String, smali: Boolean): DecxApiResult
    fun searchClassKey(key: String): DecxApiResult
    fun searchMethod(mth: String): DecxApiResult
    fun getMethodSource(mth: String, smali: Boolean): DecxApiResult
    fun getMethodXref(mth: String): DecxApiResult
    fun getFieldXref(fld: String): DecxApiResult
    fun getClassXref(cls: String): DecxApiResult
    fun getImplementOfInterface(iface: String): DecxApiResult
    fun getSubclasses(cls: String): DecxApiResult

    // ==================== Android App Service ====================

    fun getAidlInterfaces(): DecxApiResult
    fun getAppManifest(): DecxApiResult
    fun getMainActivity(): DecxApiResult
    fun getApplication(): DecxApiResult
    fun getExportedComponents(): DecxApiResult
    fun getDeepLinks(): DecxApiResult
    fun getDynamicReceivers(): DecxApiResult
    fun getAllResources(): DecxApiResult
    fun getResourceFile(res: String): DecxApiResult
    fun getStrings(): DecxApiResult

    // ==================== Android Framework Service ====================

    fun getSystemServiceImpl(iface: String): DecxApiResult
}

/**
 * Unified result type for all API calls.
 */
data class DecxApiResult(
    val success: Boolean,
    val data: Map<String, Any>
) {
    companion object {
        fun ok(data: Map<String, Any>) = DecxApiResult(true, data)
        fun fail(message: String) = DecxApiResult(false, mapOf("error" to message))
    }
}
