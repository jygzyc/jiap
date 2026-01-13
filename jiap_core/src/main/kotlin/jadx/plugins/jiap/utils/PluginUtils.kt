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

    /**
     * Calculate total pages needed for pagination
     */
    private fun calculateTotalPages(responseMap: Map<String, Any>, type: String?): Int {
        return when (type) {
            "list" -> {
                // Find all list fields
                val listFields = responseMap.filter { (key, value) ->
                    key.endsWith("-list") && value is List<*>
                }

                if (listFields.isNotEmpty()) {
                    listFields.values.maxOf { list ->
                        @Suppress("UNCHECKED_CAST")
                        (list as List<*>).size
                    }.let { size ->
                        (size + 1000 - 1) / 1000  // 1000 items per page
                    }
                } else {
                    1
                }
            }
            "code" -> {
                // Check for code or content field
                val codeField = responseMap["code"]
                if (codeField != null) {
                    val codeLines = codeField.toString().split('\n')
                    (codeLines.size + 1000 - 1) / 1000  // 1000 lines per page
                } else {
                    1
                }
            }
            else -> 1
        }
    }

    /**
     * Create sliced data for the given page
     */
    private fun createSliceData(responseMap: Map<String, Any>, page: Int, type: String?): Map<String, Any> {
        val sliceData = mutableMapOf<String, Any>()

        // Process each field in the response based on type
        responseMap.forEach { (key, value) ->
            when (type) {
                "list" -> {
                    if (key.endsWith("-list") && value is List<*>) {
                        // Slice the list
                        @Suppress("UNCHECKED_CAST")
                        val list = value as List<*>
                        val start = (page - 1) * 1000  // 1000 items per slice
                        val end = (start + 1000).coerceAtMost(list.size)
                        sliceData[key] = list.subList(start, end)
                    } else {
                        sliceData[key] = value
                    }
                }
                "code" -> {
                    if (key == "code") {
                        // Slice the code
                        val codeLines = value.toString().split('\n')
                        val startLine = (page - 1) * 1000  // 1000 lines per slice
                        val endLine = (startLine + 1000).coerceAtMost(codeLines.size)
                        val codeSlice = codeLines.subList(startLine, endLine).joinToString("\n")

                        // Truncate if too long
                        sliceData[key] = if (codeSlice.length > 60000) {
                            codeSlice.take(60000)
                        } else {
                            codeSlice
                        }
                    } else {
                        sliceData[key] = value
                    }
                }
                else -> {
                    // Copy other fields as-is (including type, count, name, etc.)
                    sliceData[key] = value
                }
            }
        }

        return sliceData
    }
}