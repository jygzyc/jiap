package jadx.plugins.jiap.model

enum class McpError(val code: String, val message: String) {
    PYTHON_NOT_FOUND("M001", "Python executable not found"),
    SIDECAR_SCRIPT_NOT_FOUND("M002", "Sidecar script not found: %s"),
    SIDECAR_START_FAILED("M003", "Sidecar start failed: %s"),
    SIDECAR_PROCESS_ERROR("M004", "Sidecar process error: %s"),
    SIDECAR_STOP_FAILED("M005", "Sidecar stop failed: %s");

    fun format(vararg args: Any): String {
        return if (args.isEmpty()) message else message.format(*args)
    }
}
