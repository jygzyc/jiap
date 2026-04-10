package jadx.plugins.jiap.utils

object PluginUtils {

    private val trueValues = setOf("true", "1", "yes", "on")
    private val falseValues = setOf("false", "0", "no", "off")

    fun convertValue(value: Any?, targetType: Class<*>): Any {
        if (value == null) {
            return getDefaultValue(targetType)
        }

        // If already the correct type, return as-is
        if (targetType.isInstance(value)) {
            return value
        }

        return when (targetType) {
            Boolean::class.java, java.lang.Boolean.TYPE -> toBoolean(value)
            String::class.java -> value.toString()
            Int::class.java, java.lang.Integer.TYPE -> toInt(value)
            else -> value
        }
    }

    private fun getDefaultValue(targetType: Class<*>): Any {
        return when (targetType) {
            Boolean::class.java, java.lang.Boolean.TYPE -> false
            String::class.java -> ""
            Int::class.java, java.lang.Integer.TYPE -> 0
            else -> throw IllegalArgumentException(
                "Cannot convert null to type ${targetType.simpleName}"
            )
        }
    }

    private fun toBoolean(value: Any): Boolean {
        return when (value) {
            is Boolean -> value
            is String -> {
                val lower = value.lowercase().trim()
                trueValues.contains(lower)
            }
            is Number -> value.toInt() != 0
            else -> false
        }
    }

    private fun toInt(value: Any): Int {
        return when (value) {
            is Number -> value.toInt()
            is String -> {
                val trimmed = value.trim()
                try {
                    when {
                        trimmed.startsWith("0x") || trimmed.startsWith("0X") -> {
                            trimmed.substring(2).toInt(16)
                        }
                        trimmed.startsWith("0b") || trimmed.startsWith("0B") -> {
                            trimmed.substring(2).toInt(2)
                        }
                        else -> trimmed.toInt()
                    }
                } catch (e: NumberFormatException) {
                    throw IllegalArgumentException(
                        "Cannot convert '$value' to integer"
                    )
                }
            }
            else -> 0
        }
    }

    private fun getLocalIpAddress(): String {
        var socket: java.net.Socket? = null
        try {
            socket = java.net.Socket()
            socket.connect(java.net.InetSocketAddress("8.8.8.8", 53), 1000)
            return socket.localAddress.hostAddress ?: "127.0.0.1"
        } catch (e: Exception) {
            return "127.0.0.1"
        } finally {
            socket?.close()
        }
    }

    fun buildServerUrl(ipAddress: String = getLocalIpAddress(),
                       port: Int,
                       running: Boolean = true): String {
        if (port !in 1..65535) {
            throw IllegalArgumentException("Port must be between 1 and 65535, got: $port")
        }
        return if (running) "http://$ipAddress:$port" else "N/A"
    }

    /**
     * Create a slice of the response for the given page
     * This method handles pagination for large data sets
     */
    fun createSlice(response: Any, page: Int): Map<String, Any> {
        when (response) {
            is Map<*, *> -> {
                @Suppress("UNCHECKED_CAST")
                val responseMap = response as Map<String, Any>

                // Determine total pages needed based on type
                val type = responseMap["type"] as? String
                val totalPages = calculateTotalPages(responseMap, type)

                // Create the slice data
                val sliceData = createSliceData(responseMap, page, type)

                // Wrap slice with pagination info
                return mapOf(
                    "data" to sliceData,
                    "page" to page,
                    "total" to totalPages
                )
            }
            else -> {
                // Non-Map response, wrap as single page
                return mapOf(
                    "data" to response,
                    "page" to 1,
                    "total" to 1
                )
            }
        }
    }

    private const val MAX_OUTPUT_LENGTH = 50000
    private const val INITIAL_SLICE_SIZE = 1000
    private const val MIN_SLICE_SIZE = 10

    private fun calculateTotalPages(responseMap: Map<String, Any>, type: String?): Int {
        val sliceSize = findOptimalSliceSize(responseMap, type)
        val count = getTotalCount(responseMap, type)
        return if (count <= 0) 1 else (count + sliceSize - 1) / sliceSize
    }

    private fun getTotalCount(responseMap: Map<String, Any>, type: String?): Int = when (type) {
        "list" -> getMaxListSize(responseMap)
        "code" -> responseMap["code"]?.toString()?.split('\n')?.size ?: 0
        else -> 0
    }

    private fun getMaxListSize(responseMap: Map<String, Any>): Int {
        @Suppress("UNCHECKED_CAST")
        return responseMap
            .filter { (k, v) -> k.endsWith("-list") && v is List<*> }
            .map { (_, v) -> (v as List<*>).size }
            .maxOrNull() ?: 0
    }

    private fun findOptimalSliceSize(responseMap: Map<String, Any>, type: String?): Int {
        var low = MIN_SLICE_SIZE
        var high = INITIAL_SLICE_SIZE
        var bestSize = MIN_SLICE_SIZE

        repeat(10) {
            if (low > high) return@repeat
            val mid = (low + high) / 2
            val testData = createSliceWithSize(responseMap, 1, type, mid)

            if (gson.toJson(testData).length <= MAX_OUTPUT_LENGTH) {
                bestSize = mid
                low = mid + 1
            } else {
                high = mid - 1
            }
        }

        return bestSize
    }

    private fun createSliceData(responseMap: Map<String, Any>, page: Int, type: String?): Map<String, Any> {
        val sliceSize = findOptimalSliceSize(responseMap, type)
        return createSliceWithSize(responseMap, page, type, sliceSize)
    }

    private fun createSliceWithSize(
        responseMap: Map<String, Any>,
        page: Int,
        type: String?,
        sliceSize: Int
    ): Map<String, Any> = responseMap.mapValues { (key, value) ->
        when (type) {
            "list" -> sliceList(key, value, page, sliceSize)
            "code" -> sliceCode(key, value, page, sliceSize)
            else -> value
        }
    }

    private fun sliceList(key: String, value: Any, page: Int, sliceSize: Int): Any {
        if (!key.endsWith("-list") || value !is List<*>) return value
        @Suppress("UNCHECKED_CAST")
        val list = value as List<*>
        val start = (page - 1) * sliceSize
        if (start >= list.size) return emptyList<Any>()
        val end = (start + sliceSize).coerceAtMost(list.size)
        return list.subList(start, end)
    }

    private fun sliceCode(key: String, value: Any, page: Int, sliceSize: Int): Any {
        if (key != "code") return value
        val lines = value.toString().split('\n')
        val start = (page - 1) * sliceSize
        if (start >= lines.size) return ""
        val end = (start + sliceSize).coerceAtMost(lines.size)
        val code = lines.subList(start, end).joinToString("\n")
        return code.take(MAX_OUTPUT_LENGTH)
    }

    private val gson by lazy { com.google.gson.Gson() }
}