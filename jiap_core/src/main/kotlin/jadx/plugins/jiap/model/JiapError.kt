package jadx.plugins.jiap.model

enum class JiapError(val code: String, val message: String) {
    SERVER_INTERNAL_ERROR("E001", "Internal error: %s"),
    PORT_IN_USE("E002", "Port %d in use"),
    SERVER_START_FAILED("E003", "Server start failed: %s"),
    SERVER_STOP_FAILED("E004", "Server stop failed: %s"),
    SERVER_RESTART_FAILED("E005", "Server restart failed: %s"),
    JADX_NOT_AVAILABLE("E006", "JADX unavailable"),
    JADX_INIT_FAILED("E007", "JADX init failed: %s"),
    PYTHON_NOT_FOUND("E008", "Python executable not found"),
    SIDECAR_SCRIPT_NOT_FOUND("E009", "Sidecar script not found: %s"),
    SIDECAR_START_FAILED("E010", "Sidecar start failed: %s"),
    SIDECAR_PROCESS_ERROR("E011", "Sidecar process error: %s"),
    SIDECAR_STOP_FAILED("E012", "Sidecar stop failed: %s"),
    SERVICE_ERROR("E013", "Service error: %s"),
    HEALTH_CHECK_FAILED("E014", "Health check failed: %s"),
    METHOD_NOT_FOUND("E015", "Method not found: %s"),
    MISSING_PARAMETER("E016", "Missing parameter: %s"),
    INVALID_PARAMETER("E017", "Invalid parameter: %s"),
    UNKNOWN_ENDPOINT("E018", "Unknown endpoint: %s"),
    CONNECTION_ERROR("E019", "Connection error: %s");

    fun format(vararg args: Any): String {
        return if (args.isEmpty()) message else message.format(*args)
    }
}
