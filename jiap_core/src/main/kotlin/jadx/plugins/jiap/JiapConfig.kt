package jadx.plugins.jiap

import jadx.api.plugins.JadxPluginContext
import jadx.plugins.jiap.service.CommonService
import jadx.plugins.jiap.service.AndroidFrameworkService
import jadx.plugins.jiap.service.AndroidAppService
import kotlin.collections.mapOf

/**
 * Configuration class for JIAP services and routing.
 * This class centralizes all service instances and route mappings.
 */
class JiapConfig(
    private val pluginContext: JadxPluginContext
) {

    // Service instances
    val commonService: CommonService = CommonService(pluginContext)
    val androidFrameworkService: AndroidFrameworkService = AndroidFrameworkService(pluginContext)
    val androidAppService: AndroidAppService = AndroidAppService(pluginContext)

    // Route mappings
    val routeMap: Map<String, RouteTarget>
        get() = mapOf(
            // Common Service
            "/api/jiap/get_all_classes" to RouteTarget(
                service = commonService,
                methodName = "handleGetAllClasses",
                cacheable = true
            ),
            "/api/jiap/search_method" to RouteTarget(
                service = commonService,
                methodName = "handleSearchMethod",
                params = setOf("method"),
                cacheable = true
            ),
            "/api/jiap/get_class_info" to RouteTarget(
                service = commonService,
                methodName = "handleGetClassInfo",
                params = setOf("class"),
                cacheable = true
            ),
            "/api/jiap/get_method_xref" to RouteTarget(
                service = commonService,
                methodName = "handleGetMethodXref",
                params = setOf("method"),
                cacheable = true
            ),
            "/api/jiap/get_class_xref" to RouteTarget(
                service = commonService,
                methodName = "handleGetClassXref",
                params = setOf("class"),
                cacheable = true
            ),
            "/api/jiap/get_implement" to RouteTarget(
                service = commonService,
                methodName = "handleGetImplementOfInterface",
                params = setOf("interface"),
                cacheable = true
            ),
            "/api/jiap/get_sub_classes" to RouteTarget(
                service = commonService,
                methodName = "handleGetSubclasses",
                params = setOf("class"),
                cacheable = true
            ),
            "/api/jiap/get_class_source" to RouteTarget(
                service = commonService,
                methodName = "handleGetClassSource",
                params = setOf("class", "smali")
            ),
            "/api/jiap/get_method_source" to RouteTarget(
                service = commonService,
                methodName = "handleGetMethodSource",
                params = setOf("method", "smali")
            ),
            "/api/jiap/selected_text" to RouteTarget(
                service = commonService,
                methodName = "handleGetSelectedText"
            ),
            "/api/jiap/selected_class" to RouteTarget(
                service = commonService,
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

            // Android Framework Service
            "/api/jiap/get_system_service_impl" to RouteTarget(
                service = androidFrameworkService,
                methodName = "handleGetSystemServiceImpl",
                params = setOf("interface")
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
    val cacheable: Boolean = false  // 默认不缓存，只有明确标记的才缓存
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