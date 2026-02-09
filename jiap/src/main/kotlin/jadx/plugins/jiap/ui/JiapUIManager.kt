package jadx.plugins.jiap.ui

import jadx.api.plugins.JadxPluginContext
import jadx.api.plugins.gui.JadxGuiContext
import jadx.plugins.jiap.core.JiapServer
import jadx.plugins.jiap.JiapConstants
import jadx.plugins.jiap.model.JiapError
import jadx.plugins.jiap.utils.LogUtils
import jadx.plugins.jiap.utils.PluginUtils
import jadx.plugins.jiap.utils.PreferencesManager
import jadx.plugins.jiap.core.SidecarProcessManager
import java.awt.GridBagConstraints
import java.awt.GridBagLayout
import java.awt.Insets
import javax.swing.*

class JiapUIManager(
    private val pluginContext: JadxPluginContext,
    private val server: JiapServer,
    private val sidecarManager: SidecarProcessManager
) {

    fun initializeGuiComponents(guiContext: JadxGuiContext) {
        guiContext.addMenuAction("JIAP Server Status") {
            showServerStatus()
        }
        guiContext.addMenuAction("JIAP Server Restart") {
            restartServer()
        }
    }

    private fun restartServer() {
        Thread({
            if (server.isRunning) {
                server.restart()
            } else {
                server.start()
            }
        }, "JiapUI-RestartThread").apply {
            isDaemon = true
        }.start()
    }

    private fun showServerStatus() {
        val panel = createStatusPanel()
        val result = JOptionPane.showOptionDialog(
            pluginContext.guiContext?.mainFrame,
            panel,
            "JIAP Server Status",
            JOptionPane.DEFAULT_OPTION,
            JOptionPane.INFORMATION_MESSAGE,
            null,
            arrayOf("OK", "Cancel"),
            "OK"
        )

        if (result == 0) {
            val statusPanel = panel as JPanel
            val portField = statusPanel.components
                .filterIsInstance<JTextField>()
                .firstOrNull { it.isEditable }
            portField?.let { handleSettingsChange(it.text, server.currentPort) }
        }
    }

    private fun createStatusPanel(): JPanel {
        val isServerRunning = server.isRunning
        val serverStatus = if (isServerRunning) "Running" else "Stopped"
        val isSidecarRunning = sidecarManager.isRunning()
        val sidecarStatus = if (isSidecarRunning) "Running" else "Stopped"
        val currentPort = server.currentPort
        val url = PluginUtils.buildServerUrl(port = currentPort)

        val panel = JPanel(GridBagLayout())
        val gbc = GridBagConstraints()
        gbc.anchor = GridBagConstraints.WEST
        gbc.insets = Insets(5, 5, 5, 5)

        addLabelField(panel, gbc, 0, "JIAP Server Status:", serverStatus)
        addLabelField(panel, gbc, 1, "MCP Server Status:", sidecarStatus)
        addLabelField(panel, gbc, 2, "Current Port:", currentPort.toString())

        val portTextField = JTextField(currentPort.toString(), 10)
        addComponent(panel, gbc, 3, "New Port:", portTextField)

        val mcpPath = PreferencesManager.getMcpPath()
        val mcpPathField = JTextField(mcpPath, 20)
        mcpPathField.isEditable = false
        addComponent(panel, gbc, 4, "MCP Server Path:", mcpPathField)

        gbc.gridx = 0
        gbc.gridy = 5
        gbc.gridwidth = 3
        panel.add(JLabel("Health Check: $url (MCP Port: ${currentPort + 1})"), gbc)

        return panel
    }

    private fun addLabelField(panel: JPanel, gbc: GridBagConstraints, row: Int, labelText: String, value: String) {
        gbc.gridx = 0
        gbc.gridy = row
        gbc.gridwidth = 1
        panel.add(JLabel(labelText), gbc)
        gbc.gridx = 1
        panel.add(JLabel(value), gbc)
    }

    private fun addComponent(panel: JPanel, gbc: GridBagConstraints, row: Int, labelText: String, component: JComponent) {
        gbc.gridx = 0
        gbc.gridy = row
        gbc.gridwidth = 1
        panel.add(JLabel(labelText), gbc)
        gbc.gridx = 1
        gbc.gridwidth = 1
        panel.add(component, gbc)
    }

    private fun handleSettingsChange(portText: String, currentPort: Int) {
        try {
            val newPort = portText.trim().toInt()

            if (newPort !in 1024..65535) {
                showErrorDialog("Port must be between 1024 and 65535", "Invalid Port")
                return
            }

            if (newPort != currentPort) {
                if (showConfirmDialog("Settings changed. Servers will restart. Continue?", "Confirm Changes")) {
                    applyPortChange(newPort)
                }
            }
        } catch (e: NumberFormatException) {
            showErrorDialog("Invalid port number: $portText", "Error")
        } catch (e: Exception) {
            showErrorDialog("Error applying settings: ${e.message}", "Error")
        }
    }

    private fun showConfirmDialog(message: String, title: String): Boolean {
        return JOptionPane.showConfirmDialog(
            pluginContext.guiContext?.mainFrame,
            message,
            title,
            JOptionPane.YES_NO_OPTION,
            JOptionPane.QUESTION_MESSAGE
        ) == JOptionPane.YES_OPTION
    }

    private fun showErrorDialog(message: String, title: String) {
        JOptionPane.showMessageDialog(
            pluginContext.guiContext?.mainFrame,
            message,
            title,
            JOptionPane.ERROR_MESSAGE
        )
    }

    private fun applyPortChange(newPort: Int) {
        PreferencesManager.setPort(newPort)
        showProgressDialog {
            var success = false
            var error: String? = null

            try {
                if (server.isRunning) server.stop()
                success = server.start(newPort)
            } catch (e: Exception) {
                error = e.message
            }

            if (success) {
                showErrorDialog("Settings applied successfully", "Success", JOptionPane.INFORMATION_MESSAGE)
            } else {
                showErrorDialog("Failed to apply changes\n${error ?: "Error during server restart"}", "Error")
            }
        }
    }

    private fun showProgressDialog(block: () -> Unit) {
        val progressDialog = JDialog(pluginContext.guiContext?.mainFrame, "Restarting Servers...", true)
        val label = JLabel("Applying changes...")
        label.horizontalAlignment = SwingConstants.CENTER
        progressDialog.contentPane.add(label)
        progressDialog.size = java.awt.Dimension(300, 100)
        progressDialog.setLocationRelativeTo(pluginContext.guiContext?.mainFrame)

        Thread(block, "JiapUI-ProgressDialog").apply {
            isDaemon = true
        }.start()

        SwingUtilities.invokeLater {
            progressDialog.isVisible = true
        }
    }

    private fun showErrorDialog(message: String, title: String, messageType: Int) {
        JOptionPane.showMessageDialog(
            pluginContext.guiContext?.mainFrame,
            message,
            title,
            messageType
        )
    }
}
