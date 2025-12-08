package jadx.plugins.jiap.utils

import org.slf4j.LoggerFactory

object LogUtils {

    private val logger = LoggerFactory.getLogger("JIAP")

    // Log levels
    fun debug(message: String, vararg args: Any) {
        logger.debug("JIAP: " + formatMessage(message, args))
    }

    fun info(message: String, vararg args: Any) {
        logger.info("JIAP: " + formatMessage(message, args))
    }

    fun warn(message: String, vararg args: Any) {
        logger.warn("JIAP: " + formatMessage(message, args))
    }

    fun error(message: String, vararg args: Any) {
        logger.error("JIAP: " + formatMessage(message, args))
    }

    fun error(message: String, e: Exception, vararg args: Any) {
        logger.error("JIAP: " + formatMessage(message, args), e)
    }

    private fun formatMessage(message: String, args: Array<out Any>): String {
        return try {
            message.format(*args)
        } catch (e: Exception) {
            message
        }
    }

    // Messages
    object Msg {
        // Server messages
        const val SERVER_STARTING = "Starting server on port %d"
        const val SERVER_STARTED = "Server started successfully on port %d"
        const val SERVER_STOPPED = "Server stopped successfully"
        const val SERVER_ALREADY_RUNNING = "Server already running on port %d"
        const val SERVER_ALREADY_STOPPED = "Server already stopped"
        const val SERVER_RESTARTING = "Restarting server..."
        const val SERVER_NOT_RUNNING = "Server not running, starting..."

        // Server errors
        const val SERVER_START_FAILED = "Failed to start server"
        const val SERVER_STOP_FAILED = "Failed to stop server"
        const val SERVER_RESTART_FAILED = "Failed to restart server"
        const val PORT_IN_USE = "Port %d is already in use"
        const val INVALID_PORT_RANGE = "Port must be between 1024 and 65535"
        const val HEALTH_CHECK_FAILED = "Health check failed"

        // JADX messages
        const val JADX_NOT_AVAILABLE = "JADX decompiler is not available. Please load a project first."
        const val JADX_INIT_TIMEOUT = "JADX initialization timeout after %d seconds"
        const val NO_CLASSES_LOADED = "No classes loaded in JADX"
        const val FOUND_CLASSES = "Found %d classes, JADX ready"

        // Request messages
        const val PROCESSING_REQUEST = "Processing request: %s"
        const val SERVICE_CALL_SUCCESS = "Service call completed successfully"
        const val SERVICE_CALL_FAILED = "Service call failed"

        // Parameter errors
        const val MISSING_PARAMETER = "Missing required parameter: %s"
        const val INVALID_PARAMETER = "Invalid parameter: %s"

        // Route errors
        const val UNKNOWN_ENDPOINT = "Unknown endpoint: %s"
        const val METHOD_NOT_FOUND = "Method %s not found in %s"

        // Initialization messages
        const val INITIALIZATION_STARTING = "Starting delayed initialization"
        const val INITIALIZATION_SCHEDULED = "Initialization already scheduled"
        const val INITIALIZATION_STOPPED = "Server started, stopping initialization checker"
        const val JADX_READY = "JADX decompiler ready, starting server..."
        const val WAITING_FOR_JADX = "Waiting for JADX decompiler..."

        // Status messages
        const val NO_CLASSES_AVAILABLE = "No classes available"
        const val MAIN_FRAME_NOT_MAIN_WINDOW = "Main frame is not MainWindow instance"
        const val DECOMPILER_NOT_LOADED = "Decompiler not loaded"
        const val JADX_CHECK_FAILED = "JADX availability check failed: %s"

        // Shutdown messages
        const val SHUTDOWN_HOOK_TRIGGERED = "Shutdown hook triggered"
        const val SHUTDOWN_ERROR = "Error during shutdown"
        const val SCHEDULER_SHUTDOWN_ERROR = "Error shutting down scheduler"
        const val INITIALIZATION_CHECK_ERROR = "Error during initialization check"
    }
}