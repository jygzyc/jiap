package jadx.plugins.jiap.core

import jadx.api.plugins.JadxPluginContext
import jadx.plugins.jiap.service.CommonService
import jadx.plugins.jiap.service.AndroidFrameworkService
import jadx.plugins.jiap.service.AndroidAppService
import jadx.plugins.jiap.service.VulnMiningService
import jadx.plugins.jiap.service.UIService
import kotlin.collections.mapOf

/**
 * Configuration class for JIAP services and routing.
 * This class centralizes all service instances and route mappings.
 */
class JiapConfig(
    pluginContext: JadxPluginContext
) {

    // Service instances
    val commonService: CommonService = CommonService(pluginContext)
    val androidFrameworkService: AndroidFrameworkService = AndroidFrameworkService(pluginContext)
    val androidAppService: AndroidAppService = AndroidAppService(pluginContext)
    val vulnMiningService: VulnMiningService = VulnMiningService(pluginContext)
    val uiService: UIService = UIService(pluginContext)

    // Route mappings
    val routeMap: Map<String, RouteTarget>
        get() = mapOf(
            // Common Service
            "/api/jiap/get_all_classes" to RouteTarget(
                service = commonService,
                methodName = "handleGetAllClasses",
                cacheable = true
            ),
            "/api/jiap/get_class_info" to RouteTarget(
                service = commonService,
                methodName = "handleGetClassInfo",
                params = setOf("cls"),
                cacheable = true
            ),
            "/api/jiap/get_class_source" to RouteTarget(
                service = commonService,
                methodName = "handleGetClassSource",
                params = setOf("cls", "smali")
            ),
            "/api/jiap/search_class_key" to RouteTarget(
                service = commonService,
                methodName = "handleSearchClassKey",
                params = setOf("key"),
                cacheable = true
            ),
            "/api/jiap/search_method" to RouteTarget(
                service = commonService,
                methodName = "handleSearchMethod",
                params = setOf("mth"),
                cacheable = true
            ),
            "/api/jiap/get_method_xref" to RouteTarget(
                service = commonService,
                methodName = "handleGetMethodXref",
                params = setOf("mth"),
                cacheable = true
            ),
            "/api/jiap/get_field_xref" to RouteTarget(
                service = commonService,
                methodName = "handleGetFieldXref",
                params = setOf("fld"),
                cacheable = true
            ),
            "/api/jiap/get_class_xref" to RouteTarget(
                service = commonService,
                methodName = "handleGetClassXref",
                params = setOf("cls"),
                cacheable = true
            ),
            "/api/jiap/get_implement" to RouteTarget(
                service = commonService,
                methodName = "handleGetImplementOfInterface",
                params = setOf("iface"),
                cacheable = true
            ),
            "/api/jiap/get_sub_classes" to RouteTarget(
                service = commonService,
                methodName = "handleGetSubclasses",
                params = setOf("cls"),
                cacheable = true
            ),
            "/api/jiap/get_method_source" to RouteTarget(
                service = commonService,
                methodName = "handleGetMethodSource",
                params = setOf("mth", "smali")
            ),

            // UI Service
            "/api/jiap/selected_text" to RouteTarget(
                service = uiService,
                methodName = "handleGetSelectedText"
            ),
            "/api/jiap/selected_class" to RouteTarget(
                service = uiService,
                methodName = "handleGetSelectedClass"
            ),

            // Android App Service
            "/api/jiap/get_app_manifest" to RouteTarget(
                service = androidAppService,
                methodName = "handleGetAppManifest"
            ),
            "/api/jiap/get_main_activity" to RouteTarget(
                service = androidAppService,
                methodName = "handleGetMainActivity"
            ),
            "/api/jiap/get_application" to RouteTarget(
                service = androidAppService,
                methodName = "handleGetApplication"
            ),
            "/api/jiap/get_exported_components" to RouteTarget(
                service = androidAppService,
                methodName = "handleGetExportedComponents"
            ),
            "/api/jiap/get_deep_links" to RouteTarget(
                service = androidAppService,
                methodName = "handleGetDeepLinks"
            ),

            // Android Framework Service
            "/api/jiap/get_system_service_impl" to RouteTarget(
                service = androidFrameworkService,
                methodName = "handleGetSystemServiceImpl",
                params = setOf("iface")
            ),

            // Vulnerability Mining Service
            "/api/jiap/get_dynamic_receivers" to RouteTarget(
                service = vulnMiningService,
                methodName = "handleGetDynamicReceivers"
            )
        )

    /**
     * Get all available API endpoints
     */
    fun getApiEndpoints(): Set<String> = routeMap.keys

    /**
     * Get service class name for an endpoint
     */
    fun getServiceName(endpoint: String): String? {
        return routeMap[endpoint]?.service?.let {
            it::class.simpleName
        }
    }
}

/**
 * Data class representing a route target with service instance,
 * method name, required parameters, and cache flag.
 */
data class RouteTarget(
    val service: Any,
    val methodName: String,
    val params: Set<String> = emptySet(),
    val cacheable: Boolean = false
) {
    // Cached method reference, lazily initialized
    @Volatile
    private var cachedMethod: java.lang.reflect.Method? = null

    // Get method reference with caching
    fun getMethod(): java.lang.reflect.Method {
        // Double-checked locking pattern
        if (cachedMethod == null) {
            synchronized(this) {
                if (cachedMethod == null) {
                    val serviceClass = service::class.java
                    val method = serviceClass.methods.find {
                        it.name == methodName
                    } ?: throw NoSuchMethodException(
                        "Method $methodName not found in ${serviceClass.simpleName}"
                    )
                    cachedMethod = method
                }
            }
        }
        return cachedMethod!!
    }
}