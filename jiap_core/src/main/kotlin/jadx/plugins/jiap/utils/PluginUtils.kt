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
                       port: Int = PreferencesManager.getPort(),
                       running: Boolean = true): String {
        return if (running) "http://$ipAddress:$port/" else "N/A"
    }
}