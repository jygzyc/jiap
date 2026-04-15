package jadx.plugins.decx.model

enum class DecxError(val code: String, val message: String) {
    SERVER_INTERNAL_ERROR("E001", "Internal error: %s"),
    SERVICE_ERROR("E002", "Service error: %s"),
    HEALTH_CHECK_FAILED("E003", "Health check failed: %s"),
    METHOD_NOT_FOUND("E004", "Method not found: %s"),
    INVALID_PARAMETER("E005", "Invalid parameter: %s");

    fun format(vararg args: Any): String {
        return if (args.isEmpty()) message else message.format(*args)
    }
}
