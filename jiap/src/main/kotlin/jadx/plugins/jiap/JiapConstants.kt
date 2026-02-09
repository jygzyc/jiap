package jadx.plugins.jiap

import java.io.File

object JiapConstants {
    const val DEFAULT_PORT: Int = 25419
    val DEFAULT_MCP_SIDECAR_SCRIPT: String = getDefaultMcpScriptPath()
    val SUPPORTED_CACHE_MODES = setOf("memory", "disk")
    const val DEFAULT_CACHE_MODE = "disk"

    private fun getDefaultMcpScriptPath(): String {
        return File(System.getProperty("user.home"), ".jiap/mcp").absolutePath
    }
}
