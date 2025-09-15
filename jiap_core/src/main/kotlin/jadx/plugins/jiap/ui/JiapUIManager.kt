package jadx.plugins.jiap.ui

import jadx.api.plugins.JadxPluginContext
import jadx.api.plugins.gui.JadxGuiContext
import jadx.api.gui.tree.ITreeNode
import jadx.plugins.jiap.JiapServer
import jadx.plugins.jiap.utils.JiapConstants
import jadx.plugins.jiap.utils.PreferencesManager
import org.slf4j.LoggerFactory
import java.awt.BorderLayout
import java.awt.Dimension
import java.awt.FlowLayout
import java.awt.event.ActionEvent
import java.awt.event.ActionListener
import javax.swing.BorderFactory
import javax.swing.JButton
import javax.swing.JFrame
import javax.swing.JPanel
import javax.swing.JOptionPane
import javax.swing.JTextField
import javax.swing.JLabel
import java.net.HttpURLConnection
import java.net.URL
import java.util.function.Predicate
import java.util.function.Consumer
import kotlin.concurrent.thread


class JiapUIManager(
    private val pluginContext: JadxPluginContext,
    private val server: JiapServer
) {

    companion object {
        private val logger = LoggerFactory.getLogger(JiapUIManager::class.java)
    }

    fun initializeGuiComponents(guiContext: JadxGuiContext) {
        guiContext.addTreePopupMenuEntry(
            "Restart Server",
            { true },
            { node ->
                restartServer() 
            }
        )
        guiContext.addTreePopupMenuEntry(
            "Server Status",
            { true }, { node ->
                showServerStatus() 
            })
    }

    private fun restartServer() {
        if (server.isRunning()) {
            Thread {
                server.restart()
            }.start()
        } else {
            server.start()
        }
    }

    private fun showServerStatus() {
        val isRunning = server.isRunning()
        val status = if (isRunning) "Running" else "Stopped"
        val currentPort = server.getCurrentPort()
        val url = if(isRunning) "http://127.0.0.1:" + currentPort + "/" else "N/A"

        val message = """
            Server Status: $status
            Port: $currentPort
            Health Check: $url
        """.trimIndent()

        JOptionPane.showMessageDialog(
            pluginContext.guiContext?.mainFrame,
            message,
            "JIAP Server Status",
            JOptionPane.INFORMATION_MESSAGE
        )
    }
}