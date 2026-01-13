package jadx.plugins.jiap

import java.io.File

object JiapConstants {
    const val DEFAULT_PORT: Int = 25419
    val DEFAULT_MCP_SIDECAR_SCRIPT: String = getDefaultMcpScriptPath()

    private fun getDefaultMcpScriptPath(): String {
        return File(File(System.getProperty("user.home"), ".jiap/mcp"), "jiap_mcp_server.py").absolutePath
    }
}
