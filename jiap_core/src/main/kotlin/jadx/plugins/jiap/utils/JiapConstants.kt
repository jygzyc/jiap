package jadx.plugins.jiap.utils

object JiapConstants {
    const val DEFAULT_PORT: Int = 25419

    object ErrorCode {
        const val SERVER_INTERNAL_ERROR = "E001"
        const val PORT_IN_USE = "E002"
        const val JADX_NOT_AVAILABLE = "E003"
        const val HEALTH_CHECK_FAILED = "E004"
        const val MISSING_PARAMETER = "E005"
        const val SERVICE_CALL_FAILED = "E006"
        const val UNKNOWN_ENDPOINT = "E007"
        const val INVALID_PARAMETER = "E008"
    }

    object Messages {
        // Server messages
        const val SERVER_STARTING = "Starting server"
        const val SERVER_RESTARTING = "Restarting server"
        const val SERVER_STARTED = "Server started successfully"
        const val SERVER_STOPPED = "Server stopped successfully"
        const val SERVER_RUNNING = "Server already running"
        const val SERVER_START_FAILED = "Server started failed"
        const val SERVER_STOP_FAILED = "Server stopped failed"
        const val SERVER_RESTART_FAILED = "Server restarting failed"
        const val SERVER_INIT_CHECK_FAILED = "Server initialization check failed"

        // Jadx messages
        const val JADX_NOT_AVAILABLE = "JADX decompiler is not available. Please load a project first."
        const val JADX_INIT_FAILED = "JADX decompiler initialization failed"

        // Health check messages
        const val HEALTH_CHECK_FAILED = "Health check failed"

        const val SERVICE_CALL_FAILED = "Service call failed"
        const val METHOD_NOT_FOUND = "Method not found: %s"
        const val UNKNOWN_ENDPOINT = "Unknown endpoint: %s"

        // Parameter errors
        const val MISSING_PARAMETER = "Missing required parameter: %s"
        const val INVALID_PARAMETER = "Invalid parameter type: %s"
    }
}

