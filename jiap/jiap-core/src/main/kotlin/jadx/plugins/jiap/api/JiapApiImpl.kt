package jadx.plugins.jiap.api

import jadx.api.JadxDecompiler
import jadx.plugins.jiap.service.AndroidAppService
import jadx.plugins.jiap.service.AndroidFrameworkService
import jadx.plugins.jiap.service.CommonService
import jadx.plugins.jiap.service.VulnMiningService
import jadx.plugins.jiap.utils.CacheUtils

/**
 * Default implementation of [JiapApi].
 * Delegates to individual service classes, with optional caching support.
 */
class JiapApiImpl(
    decompiler: JadxDecompiler,
    private val cacheEnabled: Boolean = true
) : JiapApi {

    private val commonService = CommonService(decompiler)
    private val androidAppService = AndroidAppService(decompiler)
    private val androidFrameworkService = AndroidFrameworkService(decompiler)
    private val vulnMiningService = VulnMiningService(decompiler)

    // ==================== Common Service ====================

    override fun getAllClasses(): JiapApiResult {
        return if (cacheEnabled) cached("getAllClasses", emptyMap()) { commonService.handleGetAllClasses() }
        else commonService.handleGetAllClasses()
    }

    override fun getClassInfo(cls: String): JiapApiResult {
        return if (cacheEnabled) cached("getClassInfo", mapOf("cls" to cls)) { commonService.handleGetClassInfo(cls) }
        else commonService.handleGetClassInfo(cls)
    }

    override fun getClassSource(cls: String, smali: Boolean): JiapApiResult {
        return if (cacheEnabled) cached("getClassSource", mapOf("cls" to cls, "smali" to smali)) {
            commonService.handleGetClassSource(cls, smali)
        } else commonService.handleGetClassSource(cls, smali)
    }

    override fun searchClassKey(key: String): JiapApiResult {
        return if (cacheEnabled) cached("searchClassKey", mapOf("key" to key)) { commonService.handleSearchClassKey(key) }
        else commonService.handleSearchClassKey(key)
    }

    override fun searchMethod(mth: String): JiapApiResult {
        return if (cacheEnabled) cached("searchMethod", mapOf("mth" to mth)) { commonService.handleSearchMethod(mth) }
        else commonService.handleSearchMethod(mth)
    }

    override fun getMethodSource(mth: String, smali: Boolean): JiapApiResult {
        return if (cacheEnabled) cached("getMethodSource", mapOf("mth" to mth, "smali" to smali)) {
            commonService.handleGetMethodSource(mth, smali)
        } else commonService.handleGetMethodSource(mth, smali)
    }

    override fun getMethodXref(mth: String): JiapApiResult {
        return if (cacheEnabled) cached("getMethodXref", mapOf("mth" to mth)) { commonService.handleGetMethodXref(mth) }
        else commonService.handleGetMethodXref(mth)
    }

    override fun getFieldXref(fld: String): JiapApiResult {
        return if (cacheEnabled) cached("getFieldXref", mapOf("fld" to fld)) { commonService.handleGetFieldXref(fld) }
        else commonService.handleGetFieldXref(fld)
    }

    override fun getClassXref(cls: String): JiapApiResult {
        return if (cacheEnabled) cached("getClassXref", mapOf("cls" to cls)) { commonService.handleGetClassXref(cls) }
        else commonService.handleGetClassXref(cls)
    }

    override fun getImplementOfInterface(iface: String): JiapApiResult {
        return if (cacheEnabled) cached("getImplementOfInterface", mapOf("iface" to iface)) {
            commonService.handleGetImplementOfInterface(iface)
        } else commonService.handleGetImplementOfInterface(iface)
    }

    override fun getSubclasses(cls: String): JiapApiResult {
        return if (cacheEnabled) cached("getSubclasses", mapOf("cls" to cls)) { commonService.handleGetSubclasses(cls) }
        else commonService.handleGetSubclasses(cls)
    }

    // ==================== Android App Service ====================

    override fun getAppManifest(): JiapApiResult {
        return androidAppService.handleGetAppManifest()
    }

    override fun getMainActivity(): JiapApiResult {
        return androidAppService.handleGetMainActivity()
    }

    override fun getApplication(): JiapApiResult {
        return androidAppService.handleGetApplication()
    }

    override fun getExportedComponents(): JiapApiResult {
        return androidAppService.handleGetExportedComponents()
    }

    override fun getDeepLinks(): JiapApiResult {
        return androidAppService.handleGetDeepLinks()
    }

    override fun getDynamicReceivers(): JiapApiResult {
        return androidAppService.handleGetDynamicReceivers()
    }

    override fun getAllResources(): JiapApiResult {
        return androidAppService.handleGetAllResources()
    }

    override fun getResourceFile(res: String): JiapApiResult {
        return androidAppService.handleGetResourceFile(res)
    }

    override fun getStrings(): JiapApiResult {
        return androidAppService.handleGetStrings()
    }

    // ==================== Android Framework Service ====================

    override fun getSystemServiceImpl(iface: String): JiapApiResult {
        return androidFrameworkService.handleGetSystemServiceImpl(iface)
    }

    // ==================== Cache ====================

    private fun cached(endpoint: String, params: Map<String, Any>, loader: () -> JiapApiResult): JiapApiResult {
        CacheUtils.get(endpoint, params)?.let {
            @Suppress("UNCHECKED_CAST")
            return JiapApiResult(true, it as Map<String, Any>)
        }
        val result = loader()
        if (result.success) {
            CacheUtils.put(endpoint, params, result.data)
        }
        return result
    }
}

