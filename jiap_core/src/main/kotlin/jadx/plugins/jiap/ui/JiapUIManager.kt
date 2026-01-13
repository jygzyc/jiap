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
import java.io.File
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
        val isServerRunning = server.isRunning
        val serverStatus = if (isServerRunning) "Running" else "Stopped"
        
        val isSidecarRunning = sidecarManager.isRunning()
        val sidecarStatus = if (isSidecarRunning) "Running" else "Stopped"
        
        val currentPort = server.currentPort
        val url = PluginUtils.buildServerUrl(port = currentPort)

        // Create main panel with GridBagLayout
        val panel = JPanel(GridBagLayout())
        val gbc = GridBagConstraints()
        gbc.anchor = GridBagConstraints.WEST
        gbc.insets = Insets(5, 5, 5, 5)

        // JIAP Server Status
        gbc.gridx = 0
        gbc.gridy = 0
        panel.add(JLabel("JIAP Server Status:"), gbc)
        gbc.gridx = 1
        panel.add(JLabel(serverStatus), gbc)

        // MCP Sidecar Status
        gbc.gridx = 0
        gbc.gridy = 1
        panel.add(JLabel("MCP Sidecar Status:"), gbc)
        gbc.gridx = 1
        
        panel.add(JLabel(sidecarStatus), gbc)

        // Current Port
        gbc.gridx = 0
        gbc.gridy = 2
        panel.add(JLabel("Current Port:"), gbc)
        gbc.gridx = 1
        panel.add(JLabel(currentPort.toString()), gbc)

        // New Port Input
        gbc.gridx = 0
        gbc.gridy = 3
        panel.add(JLabel("New Port:"), gbc)

        val portTextField = JTextField(currentPort.toString(), 10)
        gbc.gridx = 1
        panel.add(portTextField, gbc)

        // MCP Server Path
        gbc.gridx = 0
        gbc.gridy = 4
        gbc.gridwidth = 1
        panel.add(JLabel("MCP Server Path:"), gbc)

        val mcpPath = PreferencesManager.getMcpPath()
        val mcpPathField = JTextField(mcpPath, 20)
        gbc.gridx = 1
        panel.add(mcpPathField, gbc)

        val browseButton = JButton("Browse")
        gbc.gridx = 2
        panel.add(browseButton, gbc)

        browseButton.addActionListener {
            val fileChooser = JFileChooser()
            if (mcpPath.isNotEmpty()) {
                val f = File(mcpPath)
                if (f.exists()) {
                     fileChooser.selectedFile = f
                } else {
                    fileChooser.currentDirectory = File(System.getProperty("user.home"))
                }
            }
            val selection = fileChooser.showOpenDialog(panel)
            if (selection == JFileChooser.APPROVE_OPTION) {
                mcpPathField.text = fileChooser.selectedFile.absolutePath
            }
        }

        // Health Check URL
        gbc.gridx = 0
        gbc.gridy = 5
        gbc.gridwidth = 3
        panel.add(JLabel("Health Check: $url (MCP Port: ${currentPort + 1})"), gbc)

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
            handleSettingsChange(portTextField.text, mcpPathField.text, currentPort, mcpPath)
        }
    }

    private fun handleSettingsChange(portText: String, mcpPath: String, currentPort: Int, currentMcpPath: String) {
        try {
            val newPort = portText.trim().toInt()
            val newMcpPath = mcpPath.trim()

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

            // Check if settings changed
            if (newPort != currentPort || newMcpPath != currentMcpPath) {
                val confirm = JOptionPane.showConfirmDialog(
                    pluginContext.guiContext?.mainFrame,
                    "Settings changed. Servers will restart. Continue?",
                    "Confirm Changes",
                    JOptionPane.YES_NO_OPTION,
                    JOptionPane.QUESTION_MESSAGE
                )

                if (confirm == JOptionPane.YES_OPTION) {
                    PreferencesManager.setMcpPath(newMcpPath)
                    PreferencesManager.setPort(newPort)
                    
                    val progressDialog = JDialog(pluginContext.guiContext?.mainFrame, "Restarting Servers...", true)
                    val label = JLabel("Applying changes...")
                    label.horizontalAlignment = SwingConstants.CENTER
                    progressDialog.contentPane.add(label)
                    progressDialog.size = java.awt.Dimension(300, 100)
                    progressDialog.setLocationRelativeTo(pluginContext.guiContext?.mainFrame)

                    Thread({
                        var success = false
                        var error: String? = null

                        try {
                            // Stop server (which will also stop sidecar)
                            if (server.isRunning) server.stop()
                            
                            // Start server (which will also start sidecar)
                            success = server.start(newPort)
                        } catch (e: Exception) {
                            error = e.message
                        }

                        SwingUtilities.invokeLater {
                            progressDialog.isVisible = false
                            progressDialog.dispose()

                            if (success) {
                                JOptionPane.showMessageDialog(
                                    pluginContext.guiContext?.mainFrame,
                                    "Settings applied successfully",
                                    "Success",
                                    JOptionPane.INFORMATION_MESSAGE
                                )
                            } else {
                                JOptionPane.showMessageDialog(
                                    pluginContext.guiContext?.mainFrame,
                                    "Failed to apply changes\n${error ?: "Error during server restart"}",
                                    "Error",
                                    JOptionPane.ERROR_MESSAGE
                                )
                            }
                        }
                    }, "JiapUI-SettingsChangeThread").apply {
                        isDaemon = true
                    }.start()

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
            JOptionPane.showMessageDialog(
                pluginContext.guiContext?.mainFrame,
                "Error applying settings: ${e.message}",
                "Error",
                JOptionPane.ERROR_MESSAGE
            )
        }
    }
}
