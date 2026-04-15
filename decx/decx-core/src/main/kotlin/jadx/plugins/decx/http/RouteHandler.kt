package jadx.plugins.decx.http

import io.javalin.http.Context
import io.javalin.http.bodyAsClass
import jadx.plugins.decx.api.DecxApi
import jadx.plugins.decx.api.DecxApiResult
import jadx.plugins.decx.utils.CacheUtils
import jadx.plugins.decx.utils.LogUtils
import jadx.plugins.decx.utils.PluginUtils

/**
 * Maps HTTP routes to [DecxApi] methods.
 * This is the HTTP transport layer — pure routing, no business logic.
 */
class RouteHandler(private val api: DecxApi) {

    fun handle(path: String, payload: Map<String, Any>, page: Int): Map<String, Any> {
        LogUtils.info("[API] $path: $payload")
        val result = dispatch(path, payload)
        return PluginUtils.createSlice(result.data, page)
    }

    private fun dispatch(path: String, payload: Map<String, Any>): DecxApiResult {
        return when (path) {
            // Common Service
            "/api/decx/get_all_classes" -> api.getAllClasses()
            "/api/decx/get_class_info" -> requireParam(payload, "cls") { api.getClassInfo(it) }
            "/api/decx/get_class_source" -> requireParams(payload, "cls", "smali") { p ->
                api.getClassSource(p["cls"] as String, p["smali"] as Boolean)
            }
            "/api/decx/search_class_key" -> requireParam(payload, "key") { api.searchClassKey(it) }
            "/api/decx/search_method" -> requireParam(payload, "mth") { api.searchMethod(it) }
            "/api/decx/get_method_source" -> requireParams(payload, "mth", "smali") { p ->
                api.getMethodSource(p["mth"] as String, p["smali"] as Boolean)
            }
            "/api/decx/get_method_xref" -> requireParam(payload, "mth") { api.getMethodXref(it) }
            "/api/decx/get_field_xref" -> requireParam(payload, "fld") { api.getFieldXref(it) }
            "/api/decx/get_class_xref" -> requireParam(payload, "cls") { api.getClassXref(it) }
            "/api/decx/get_implement" -> requireParam(payload, "iface") { api.getImplementOfInterface(it) }
            "/api/decx/get_sub_classes" -> requireParam(payload, "cls") { api.getSubclasses(it) }

            // Android App Service
            "/api/decx/get_aidl" -> api.getAidlInterfaces()
            "/api/decx/get_app_manifest" -> api.getAppManifest()
            "/api/decx/get_main_activity" -> api.getMainActivity()
            "/api/decx/get_application" -> api.getApplication()
            "/api/decx/get_exported_components" -> api.getExportedComponents()
            "/api/decx/get_deep_links" -> api.getDeepLinks()
            "/api/decx/get_dynamic_receivers" -> api.getDynamicReceivers()
            "/api/decx/get_all_resources" -> api.getAllResources()
            "/api/decx/get_resource_file" -> requireParam(payload, "res") { api.getResourceFile(it) }
            "/api/decx/get_strings" -> api.getStrings()

            // Android Framework Service
            "/api/decx/get_system_service_impl" -> requireParam(payload, "iface") { api.getSystemServiceImpl(it) }

            else -> throw IllegalArgumentException("Unknown endpoint: $path")
        }
    }

    private fun requireParam(payload: Map<String, Any>, param: String, action: (String) -> DecxApiResult): DecxApiResult {
        val value = payload[param] as? String
            ?: return DecxApiResult.fail("Missing required parameter: $param")
        return action(value)
    }

    private fun requireParams(
        payload: Map<String, Any>,
        p1: String, p2: String,
        action: (Map<String, Any>) -> DecxApiResult
    ): DecxApiResult {
        val v1 = payload[p1] ?: return DecxApiResult.fail("Missing required parameter: $p1")
        val v2 = payload[p2] ?: return DecxApiResult.fail("Missing required parameter: $p2")
        return action(mapOf(p1 to v1, p2 to v2))
    }
}
