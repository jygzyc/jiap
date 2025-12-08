package jadx.plugins.jiap.utils

object ErrorMessages {

    // Server errors
    const val SERVER_ALREADY_RUNNING = "JIAP server is already running on port %d"
    const val SERVER_NOT_STARTED = "JIAP server is not started"
    const val SERVER_START_FAILED = "Failed to start JIAP server"
    const val SERVER_STOP_FAILED = "Failed to stop JIAP server"
    const val SERVER_RESTART_FAILED = "Failed to restart JIAP server"
    const val PORT_IN_USE = "Port %d is already in use"
    const val INVALID_PORT_RANGE = "Port must be between 1024 and 65535"
    const val HEALTH_CHECK_FAILED = "Health check failed"

    // JADX errors
    const val JADX_NOT_AVAILABLE = "JADX decompiler is not available. Please load a project first."
    const val JADX_INIT_TIMEOUT = "JADX initialization timeout after %d seconds"
    const val NO_CLASSES_LOADED = "No classes loaded in JADX"

    // Parameter errors
    const val MISSING_REQUIRED_PARAMETER = "Missing required parameter: %s"
    const val INVALID_PARAMETER_FORMAT = "Invalid parameter format for: %s"
    const val INVALID_PARAMETER_TYPE = "Invalid parameter type for: %s"

    // Service errors
    const val SERVICE_NOT_FOUND = "Service not found: %s"
    const val METHOD_NOT_FOUND = "Method '%s' not found in service: %s"
    const val SERVICE_CALL_FAILED = "Service call failed: %s"
    const val INTERNAL_SERVER_ERROR = "Internal server error: %s"

    // Route errors
    const val UNKNOWN_ENDPOINT = "Unknown endpoint: %s"
    const val INVALID_REQUEST_METHOD = "Invalid request method for endpoint: %s"

    // Utility methods for formatting messages
    fun missingParameter(paramName: String): String {
        return MISSING_REQUIRED_PARAMETER.format(paramName)
    }

    fun invalidParameterType(paramName: String): String {
        return INVALID_PARAMETER_TYPE.format(paramName)
    }

    fun portInUse(port: Int): String {
        return PORT_IN_USE.format(port)
    }

    fun jadxInitTimeout(seconds: Int): String {
        return JADX_INIT_TIMEOUT.format(seconds)
    }
}