package jadx.plugins.jiap.core

import jadx.plugins.jiap.utils.CacheUtils
import jadx.plugins.jiap.utils.PluginUtils
import jadx.plugins.jiap.model.JiapResult

class RouteHandler(
    private val config: JiapConfig
) {
    fun handle(
        path: String,
        payload: Map<String, Any>,
        page: Int
    ): Map<String, Any> {
        val target = config.routeMap[path] 
            ?: throw IllegalArgumentException("Unknown endpoint: $path")

        validateParams(payload, target.params)

        val data: Any = if (target.cacheable) {
            CacheUtils.get(path, payload) ?: run {
                val result = invokeService(target, payload)
                CacheUtils.put(path, payload, result)
                result
            }
        } else {
            invokeService(target, payload)
        }

        return PluginUtils.createSlice(data, page)
    }

    private fun validateParams(payload: Map<String, Any>, required: Set<String>) {
        required.forEach { param ->
            if (payload[param] == null) {
                throw IllegalArgumentException("Missing required parameter: $param")
            }
        }
    }

    private fun invokeService(target: RouteTarget, payload: Map<String, Any>): Map<String, Any> {
        val method = target.getMethod()
        val args = buildArgs(method, payload)
        @Suppress("UNCHECKED_CAST")
        return (method.invoke(target.service, *args) as JiapResult).data
    }

    private fun buildArgs(method: java.lang.reflect.Method, payload: Map<String, Any>): Array<Any> {
        return method.parameters.map { param ->
            val value = payload[param.name] 
                ?: throw IllegalArgumentException("Missing param: ${param.name}")
            PluginUtils.convertValue(value, param.type)
        }.toTypedArray()
    }


}
