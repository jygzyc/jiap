package jadx.plugins.jiap.http

import io.javalin.http.Context
import io.javalin.http.bodyAsClass
import jadx.plugins.jiap.api.JiapApi
import jadx.plugins.jiap.api.JiapApiResult
import jadx.plugins.jiap.utils.CacheUtils
import jadx.plugins.jiap.utils.LogUtils
import jadx.plugins.jiap.utils.PluginUtils

/**
 * Maps HTTP routes to [JiapApi] methods.
 * This is the HTTP transport layer — pure routing, no business logic.
 */
class RouteHandler(private val api: JiapApi) {

    fun handle(path: String, payload: Map<String, Any>, page: Int): Map<String, Any> {
        LogUtils.info("[API] $path: $payload")
        val result = dispatch(path, payload)
        return PluginUtils.createSlice(result.data, page)
    }

    private fun dispatch(path: String, payload: Map<String, Any>): JiapApiResult {
        return when (path) {
            // Common Service
            "/api/jiap/get_all_classes" -> api.getAllClasses()
            "/api/jiap/get_class_info" -> requireParam(payload, "cls") { api.getClassInfo(it) }
            "/api/jiap/get_class_source" -> requireParams(payload, "cls", "smali") { p ->
                api.getClassSource(p["cls"] as String, p["smali"] as Boolean)
            }
            "/api/jiap/search_class_key" -> requireParam(payload, "key") { api.searchClassKey(it) }
            "/api/jiap/search_method" -> requireParam(payload, "mth") { api.searchMethod(it) }
            "/api/jiap/get_method_source" -> requireParams(payload, "mth", "smali") { p ->
                api.getMethodSource(p["mth"] as String, p["smali"] as Boolean)
            }
            "/api/jiap/get_method_xref" -> requireParam(payload, "mth") { api.getMethodXref(it) }
            "/api/jiap/get_field_xref" -> requireParam(payload, "fld") { api.getFieldXref(it) }
            "/api/jiap/get_class_xref" -> requireParam(payload, "cls") { api.getClassXref(it) }
            "/api/jiap/get_implement" -> requireParam(payload, "iface") { api.getImplementOfInterface(it) }
            "/api/jiap/get_sub_classes" -> requireParam(payload, "cls") { api.getSubclasses(it) }

            // Android App Service
            "/api/jiap/get_aidl" -> api.getAidlInterfaces()
            "/api/jiap/get_app_manifest" -> api.getAppManifest()
            "/api/jiap/get_main_activity" -> api.getMainActivity()
            "/api/jiap/get_application" -> api.getApplication()
            "/api/jiap/get_exported_components" -> api.getExportedComponents()
            "/api/jiap/get_deep_links" -> api.getDeepLinks()
            "/api/jiap/get_dynamic_receivers" -> api.getDynamicReceivers()
            "/api/jiap/get_all_resources" -> api.getAllResources()
            "/api/jiap/get_resource_file" -> requireParam(payload, "res") { api.getResourceFile(it) }
            "/api/jiap/get_strings" -> api.getStrings()

            // Android Framework Service
            "/api/jiap/get_system_service_impl" -> requireParam(payload, "iface") { api.getSystemServiceImpl(it) }

            else -> throw IllegalArgumentException("Unknown endpoint: $path")
        }
    }

    private fun requireParam(payload: Map<String, Any>, param: String, action: (String) -> JiapApiResult): JiapApiResult {
        val value = payload[param] as? String
            ?: return JiapApiResult.fail("Missing required parameter: $param")
        return action(value)
    }

    private fun requireParams(
        payload: Map<String, Any>,
        p1: String, p2: String,
        action: (Map<String, Any>) -> JiapApiResult
    ): JiapApiResult {
        val v1 = payload[p1] ?: return JiapApiResult.fail("Missing required parameter: $p1")
        val v2 = payload[p2] ?: return JiapApiResult.fail("Missing required parameter: $p2")
        return action(mapOf(p1 to v1, p2 to v2))
    }
}
