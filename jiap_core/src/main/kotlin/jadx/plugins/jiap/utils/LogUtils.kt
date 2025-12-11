package jadx.plugins.jiap.utils

import org.slf4j.LoggerFactory

object LogUtils {

    private val logger = LoggerFactory.getLogger("JIAP")

    fun debug(message: String, vararg args: Any) {
        logger.debug("[JIAP] $message", *args)
    }

    fun info(message: String, vararg args: Any) {
        logger.info("[JIAP] $message", *args)
    }

    fun warn(errorCode: String, message: String, vararg args: Any) {
        logger.warn("[JIAP] [{}] $message", errorCode, *args)
    }

    fun error(errorCode: String, message: String, vararg args: Any) {
        logger.error("[JIAP] [{}] $message", errorCode, *args)
    }

    fun error(errorCode: String, message: String, e: Exception, vararg args: Any) {
        logger.error("[JIAP] [{}] $message", errorCode, *args, e)
    }

    fun warn(message: String, vararg args: Any) {
        logger.warn("[JIAP] $message", *args)
    }

    fun error(message: String, vararg args: Any) {
        logger.error("[JIAP] $message", *args)
    }

    fun error(message: String, e: Exception, vararg args: Any) {
        logger.error("[JIAP] $message", *args, e)
    }
}