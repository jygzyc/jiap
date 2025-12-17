package jadx.plugins.jiap.ui

import jadx.api.plugins.JadxPluginContext
import jadx.api.plugins.gui.JadxGuiContext
import jadx.plugins.jiap.JiapServer
import jadx.plugins.jiap.utils.JiapConstants
import jadx.plugins.jiap.utils.LogUtils
import jadx.plugins.jiap.utils.PluginUtils
import java.awt.GridBagConstraints
import java.awt.GridBagLayout
import java.awt.Insets

import javax.swing.*



class JiapUIManager(
    private val pluginContext: JadxPluginContext,
    private val server: JiapServer
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
        // Run in background thread to avoid blocking UI
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
        val isRunning = server.isRunning
        val status = if (isRunning) "Running" else "Stopped"
        val currentPort = server.currentPort
        val url = PluginUtils.buildServerUrl()

        // Create main panel with GridBagLayout
        val panel = JPanel(GridBagLayout())
        val gbc = GridBagConstraints()
        gbc.anchor = GridBagConstraints.WEST
        gbc.insets = Insets(5, 5, 5, 5)

        // Server Status
        gbc.gridx = 0
        gbc.gridy = 0
        panel.add(JLabel("Server Status:"), gbc)
        gbc.gridx = 1
        panel.add(JLabel(status), gbc)

        // Current Port
        gbc.gridx = 0
        gbc.gridy = 1
        panel.add(JLabel("Current Port:"), gbc)
        gbc.gridx = 1
        panel.add(JLabel(currentPort.toString()), gbc)

        // New Port Input
        gbc.gridx = 0
        gbc.gridy = 2
        panel.add(JLabel("New Port:"), gbc)

        val portTextField = JTextField(currentPort.toString(), 10)
        gbc.gridx = 1
        panel.add(portTextField, gbc)

        // Health Check URL
        gbc.gridx = 0
        gbc.gridy = 3
        gbc.gridwidth = 2
        panel.add(JLabel("Health Check: $url"), gbc)

        // Create dialog
        val options = arrayOf("OK", "Cancel")
        val result = JOptionPane.showOptionDialog(
            pluginContext.guiContext?.mainFrame,
            panel,
            "JIAP Server Status",
            JOptionPane.DEFAULT_OPTION,
            JOptionPane.INFORMATION_MESSAGE,
            null,
            options,
            options[0]
        )

        // Handle user response
        if (result == 0) { // OK button
            handlePortChange(portTextField.text, currentPort)
        }
    }

    private fun handlePortChange(portText: String, currentPort: Int) {
        try {
            val newPort = portText.trim().toInt()

            // Validate port range
            if (newPort !in 1024..65535) {
                JOptionPane.showMessageDialog(
                    pluginContext.guiContext?.mainFrame,
                    "Port must be between 1024 and 65535",
                    "Invalid Port",
                    JOptionPane.ERROR_MESSAGE
                )
                return
            }

            // Check if port changed
            if (newPort != currentPort) {
                val confirm = JOptionPane.showConfirmDialog(
                    pluginContext.guiContext?.mainFrame,
                    "Server will restart on port $newPort. Continue?",
                    "Confirm Port Change",
                    JOptionPane.YES_NO_OPTION,
                    JOptionPane.QUESTION_MESSAGE
                )

                if (confirm == JOptionPane.YES_OPTION) {
                    // Show progress dialog
                    val progressDialog = JDialog(pluginContext.guiContext?.mainFrame as JFrame?, "Restarting Server...", true)
                    val label = JLabel("Restarting server on port $newPort...")
                    label.horizontalAlignment = SwingConstants.CENTER
                    progressDialog.contentPane.add(label)
                    progressDialog.size = java.awt.Dimension(300, 100)
                    progressDialog.setLocationRelativeTo(pluginContext.guiContext?.mainFrame)

                    // Run server operations in background thread
                    Thread({
                        var success = false
                        var error: String? = null

                        try {
                            // Stop server if running
                            if (server.isRunning) {
                                server.stop()
                            }

                            // Start server on new port
                            success = server.start(newPort)
                        } catch (e: Exception) {
                            error = e.message
                        }

                        // Show result on UI thread
                        SwingUtilities.invokeLater {
                            progressDialog.isVisible = false
                            progressDialog.dispose()

                            if (success) {
                                JOptionPane.showMessageDialog(
                                    pluginContext.guiContext?.mainFrame,
                                    "Server successfully started on port $newPort",
                                    "Server Restarted",
                                    JOptionPane.INFORMATION_MESSAGE
                                )
                            } else {
                                JOptionPane.showMessageDialog(
                                    pluginContext.guiContext?.mainFrame,
                                    "Failed to start server on port $newPort\n${error ?: "Port may be in use or invalid"}",
                                    "Error",
                                    JOptionPane.ERROR_MESSAGE
                                )
                            }
                        }
                    }, "JiapUI-PortChangeThread").apply {
                        isDaemon = true
                    }.start()

                    // Show progress dialog
                    SwingUtilities.invokeLater {
                        progressDialog.isVisible = true
                    }
                }
            }
        } catch (e: NumberFormatException) {
            JOptionPane.showMessageDialog(
                pluginContext.guiContext?.mainFrame,
                "Invalid port number: $portText",
                "Error",
                JOptionPane.ERROR_MESSAGE
            )
        } catch (e: Exception) {
            LogUtils.error(JiapConstants.ErrorCode.SERVER_INTERNAL_ERROR, "Error changing port", e)
            JOptionPane.showMessageDialog(
                pluginContext.guiContext?.mainFrame,
                "Error changing port: ${e.message}",
                "Error",
                JOptionPane.ERROR_MESSAGE
            )
        }
    }
}