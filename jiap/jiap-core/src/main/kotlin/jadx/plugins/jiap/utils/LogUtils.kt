package jadx.plugins.jiap.utils

import org.slf4j.LoggerFactory
import jadx.plugins.jiap.model.JiapError

object LogUtils {

    private val logger = LoggerFactory.getLogger("JIAP")

    fun debug(message: String, vararg args: Any) {
        logger.debug("[JIAP] $message", *args)
    }

    fun info(message: String, vararg args: Any) {
        logger.info("[JIAP] $message", *args)
    }

    fun warn(message: String, vararg args: Any) {
        logger.warn("[JIAP] $message", *args)
    }

    fun error(error: JiapError, vararg args: Any) {
        logger.error("[JIAP] [{}] {}", error.code, error.format(*args))
    }

    fun error(error: JiapError, e: Exception, vararg args: Any) {
        logger.error("[JIAP] [{}] {}", error.code, error.format(*args), e)
    }
}
