package jadx.plugins.jiap.model

enum class JiapError(val code: String, val message: String) {
    SERVER_INTERNAL_ERROR("E001", "Internal error: %s"),
    PORT_IN_USE("E002", "Port %d in use"),
    SERVER_START_FAILED("E003", "Server start failed: %s"),
    SERVER_STOP_FAILED("E004", "Server stop failed: %s"),
    SERVER_RESTART_FAILED("E005", "Server restart failed: %s"),
    JADX_NOT_AVAILABLE("E006", "JADX unavailable"),
    JADX_INIT_FAILED("E007", "JADX init failed: %s"),
    SERVICE_ERROR("E008", "Service error: %s"),
    HEALTH_CHECK_FAILED("E009", "Health check failed: %s"),
    METHOD_NOT_FOUND("E010", "Method not found: %s"),
    MISSING_PARAMETER("E011", "Missing parameter: %s"),
    INVALID_PARAMETER("E012", "Invalid parameter: %s"),
    UNKNOWN_ENDPOINT("E013", "Unknown endpoint: %s"),
    CONNECTION_ERROR("E014", "Connection error: %s");

    fun format(vararg args: Any): String {
        return if (args.isEmpty()) message else message.format(*args)
    }
}
