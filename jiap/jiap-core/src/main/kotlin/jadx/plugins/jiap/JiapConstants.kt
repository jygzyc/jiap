package jadx.plugins.jiap

object JiapConstants {
    const val DEFAULT_PORT: Int = 25419
    val SUPPORTED_CACHE_MODES = setOf("memory", "disk")
    const val DEFAULT_CACHE_MODE = "disk"

    fun getVersion(): String {
        try {
            val props = java.util.Properties()
            val input = JiapConstants::class.java.getResourceAsStream("/version.properties")
            if (input != null) {
                props.load(input)
                input.close()
                return props.getProperty("version", "dev")
            }
        } catch (_: Exception) {}
        return System.getenv("JIAP_VERSION") ?: "dev"
    }
}
