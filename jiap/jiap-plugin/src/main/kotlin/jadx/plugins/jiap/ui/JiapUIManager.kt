package jadx.plugins.jiap.ui

import jadx.api.plugins.JadxPluginContext
import jadx.api.plugins.gui.JadxGuiContext
import jadx.plugins.jiap.api.JiapApi
import jadx.plugins.jiap.http.JiapServer
import jadx.plugins.jiap.mcp.McpPreferences
import jadx.plugins.jiap.mcp.SidecarProcessManager
import jadx.plugins.jiap.utils.PluginUtils
import jadx.plugins.jiap.utils.PreferencesManager
import java.awt.FlowLayout
import javax.swing.*

class JiapUIManager(
    private val pluginContext: JadxPluginContext,
    private val server: JiapServer,
    private val api: JiapApi,
    private val uiService: UIService
) {
    private var mcpAutoStartCheckbox: JCheckBox? = null
    private var portField: JTextField? = null
    private val sidecarManager = SidecarProcessManager(PreferencesManager.getPort())

    init {
        McpPreferences // ensure loaded
    }

    fun initializeGuiComponents(guiContext: JadxGuiContext) {
        guiContext.addMenuAction("JIAP Settings") {
            showSettingsDialog()
        }
    }

    private fun showSettingsDialog() {
        val isServerRunning = server.isRunning
        val isMcpRunning = sidecarManager.isRunning()
        val currentPort = PreferencesManager.getPort()
        val url = PluginUtils.buildServerUrl(port = currentPort)

        val panel = JPanel()
        panel.layout = BoxLayout(panel, BoxLayout.Y_AXIS)
        panel.border = BorderFactory.createEmptyBorder(10, 10, 10, 10)

        // Server Status
        val statusTitle = JLabel("Server Status")
        statusTitle.font = statusTitle.font.deriveFont(java.awt.Font.BOLD, 12f)
        statusTitle.alignmentX = java.awt.Component.LEFT_ALIGNMENT
        panel.add(statusTitle)

        panel.add(createRow("JIAP:", if (isServerRunning) "Running" else "Stopped"))
        panel.add(createRow("MCP:", if (isMcpRunning) "Running" else "Stopped"))
        panel.add(createRow("URL:", url))

        panel.add(Box.createVerticalStrut(10))

        // Port Setting
        val portTitle = JLabel("Port Setting")
        portTitle.font = portTitle.font.deriveFont(java.awt.Font.BOLD, 12f)
        portTitle.alignmentX = java.awt.Component.LEFT_ALIGNMENT
        panel.add(portTitle)

        portField = JTextField(currentPort.toString(), 8)
        panel.add(createRowWithComponent("New Port:", portField!!))

        panel.add(Box.createVerticalStrut(10))

        // MCP Settings
        val mcpTitle = JLabel("MCP Settings")
        mcpTitle.font = mcpTitle.font.deriveFont(java.awt.Font.BOLD, 12f)
        mcpTitle.alignmentX = java.awt.Component.LEFT_ALIGNMENT
        panel.add(mcpTitle)

        mcpAutoStartCheckbox = JCheckBox("Auto-start MCP with JIAP")
        mcpAutoStartCheckbox!!.isSelected = McpPreferences.getAutoStart()
        mcpAutoStartCheckbox!!.alignmentX = java.awt.Component.LEFT_ALIGNMENT
        panel.add(mcpAutoStartCheckbox)

        panel.add(Box.createVerticalStrut(10))

        // MCP Control Buttons
        val buttonPanel = JPanel(FlowLayout(FlowLayout.LEFT))
        buttonPanel.alignmentX = java.awt.Component.LEFT_ALIGNMENT
        val startBtn = JButton("Start MCP")
        startBtn.isEnabled = !isMcpRunning
        startBtn.addActionListener { startMcp() }
        val stopBtn = JButton("Stop MCP")
        stopBtn.isEnabled = isMcpRunning
        stopBtn.addActionListener { stopMcp() }
        buttonPanel.add(startBtn)
        buttonPanel.add(stopBtn)
        panel.add(buttonPanel)

        val result = JOptionPane.showConfirmDialog(
            pluginContext.guiContext?.mainFrame,
            panel,
            "JIAP Settings",
            JOptionPane.OK_CANCEL_OPTION,
            JOptionPane.PLAIN_MESSAGE
        )

        if (result == JOptionPane.OK_OPTION) {
            saveSettings()
        }
    }

    private fun createRow(label: String, value: String): JPanel {
        val row = JPanel(FlowLayout(FlowLayout.LEFT))
        row.alignmentX = java.awt.Component.LEFT_ALIGNMENT
        row.add(JLabel("$label $value"))
        return row
    }

    private fun createRowWithComponent(label: String, component: JComponent): JPanel {
        val row = JPanel(FlowLayout(FlowLayout.LEFT))
        row.alignmentX = java.awt.Component.LEFT_ALIGNMENT
        row.add(JLabel(label))
        row.add(component)
        return row
    }

    private fun saveSettings() {
        val newPort = portField?.text?.trim()?.toIntOrNull()
        if (newPort != null && newPort != PreferencesManager.getPort() && newPort in 1024..65535) {
            PreferencesManager.setPort(newPort)
            restartServers(newPort)
            return
        }

        mcpAutoStartCheckbox?.let {
            McpPreferences.setAutoStart(it.isSelected)
        }
    }

    private fun restartServers(newPort: Int) {
        Thread {
            try {
                val mcpWasRunning = sidecarManager.isRunning()
                sidecarManager.stop()
                server.stop()
                Thread.sleep(500)
                sidecarManager.updatePort(newPort)
                server.start(newPort)
                if (mcpWasRunning || McpPreferences.getAutoStart()) {
                    Thread.sleep(1000)
                    sidecarManager.start()
                }
                SwingUtilities.invokeLater {
                    JOptionPane.showMessageDialog(
                        pluginContext.guiContext?.mainFrame,
                        "Servers restarted on port $newPort",
                        "Success",
                        JOptionPane.INFORMATION_MESSAGE
                    )
                }
            } catch (e: Exception) {
                SwingUtilities.invokeLater {
                    JOptionPane.showMessageDialog(
                        pluginContext.guiContext?.mainFrame,
                        "Failed to restart servers: ${e.message}",
                        "Error",
                        JOptionPane.ERROR_MESSAGE
                    )
                }
            }
        }.apply { isDaemon = true }.start()
    }

    private fun startMcp() {
        Thread {
            val success = sidecarManager.start()
            SwingUtilities.invokeLater {
                JOptionPane.showMessageDialog(
                    pluginContext.guiContext?.mainFrame,
                    if (success) "MCP Server started" else "Failed to start MCP Server",
                    "MCP",
                    if (success) JOptionPane.INFORMATION_MESSAGE else JOptionPane.ERROR_MESSAGE
                )
            }
        }.apply { isDaemon = true }.start()
    }

    private fun stopMcp() {
        Thread {
            sidecarManager.stop()
            SwingUtilities.invokeLater {
                JOptionPane.showMessageDialog(
                    pluginContext.guiContext?.mainFrame,
                    "MCP Server stopped",
                    "MCP",
                    JOptionPane.INFORMATION_MESSAGE
                )
            }
        }.apply { isDaemon = true }.start()
    }
}
