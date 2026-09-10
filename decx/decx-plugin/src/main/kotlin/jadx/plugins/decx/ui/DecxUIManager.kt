package jadx.plugins.decx.ui

import jadx.api.plugins.JadxPluginContext
import jadx.api.plugins.gui.JadxGuiContext
import jadx.plugins.decx.server.DecxServer
import jadx.plugins.decx.utils.PluginUtils
import jadx.plugins.decx.utils.PreferencesManager
import java.awt.FlowLayout
import javax.swing.*
import javax.swing.Timer

class DecxUIManager(
    private val pluginContext: JadxPluginContext,
    private val server: DecxServer
) {
    private var portField: JTextField? = null

    // Refreshable UI components
    private var decxStatusLabel: JLabel? = null
    private var urlLabel: JLabel? = null

    fun initializeGuiComponents(guiContext: JadxGuiContext) {
        guiContext.addMenuAction("DECX Settings") {
            showSettingsDialog()
        }
    }

    private fun showSettingsDialog() {
        val panel = buildPanel()

        val refreshTimer = Timer(1000) { refreshStatus() }
        refreshTimer.start()

        val dialog = JDialog(pluginContext.guiContext?.mainFrame, "DECX Settings", true)
        dialog.defaultCloseOperation = JDialog.DISPOSE_ON_CLOSE

        val okBtn = JButton("OK")
        val cancelBtn = JButton("Cancel")
        val buttonBar = JPanel(FlowLayout(FlowLayout.RIGHT))
        buttonBar.add(okBtn)
        buttonBar.add(cancelBtn)

        // Wrap the settings panel in a scroll pane so it never gets clipped
        val scrollPane = JScrollPane(panel)
        scrollPane.border = BorderFactory.createEmptyBorder()
        scrollPane.verticalScrollBarPolicy = JScrollPane.VERTICAL_SCROLLBAR_AS_NEEDED
        scrollPane.horizontalScrollBarPolicy = JScrollPane.HORIZONTAL_SCROLLBAR_NEVER

        val content = dialog.contentPane
        content.layout = java.awt.BorderLayout()
        content.add(scrollPane, java.awt.BorderLayout.CENTER)
        content.add(buttonBar, java.awt.BorderLayout.SOUTH)

        var saved = false
        okBtn.addActionListener {
            saved = true
            dialog.dispose()
        }
        cancelBtn.addActionListener { dialog.dispose() }
        dialog.rootPane.defaultButton = okBtn

        dialog.pack()

        // Clamp the dialog size: not too small (buttons visible), not absurdly tall
        val screenBounds = pluginContext.guiContext?.mainFrame?.graphicsConfiguration?.bounds
        val maxW = (screenBounds?.width ?: 600) - 100
        val maxH = (screenBounds?.height ?: 800) - 100
        val prefW = dialog.preferredSize.width.coerceIn(420, maxW)
        val prefH = dialog.preferredSize.height.coerceIn(250, maxH)
        dialog.preferredSize = java.awt.Dimension(prefW, prefH)
        dialog.setSize(prefW, prefH)
        dialog.minimumSize = java.awt.Dimension(420, 250)

        dialog.isResizable = true
        dialog.setLocationRelativeTo(pluginContext.guiContext?.mainFrame)
        dialog.isVisible = true

        refreshTimer.stop()

        if (saved) {
            saveSettings()
        }
    }

    private fun buildPanel(): JPanel {
        val panel = JPanel()
        panel.layout = BoxLayout(panel, BoxLayout.Y_AXIS)
        panel.border = BorderFactory.createEmptyBorder(10, 10, 10, 10)

        // Server Status
        val statusTitle = JLabel("Server Status")
        statusTitle.font = statusTitle.font.deriveFont(java.awt.Font.BOLD, 12f)
        statusTitle.alignmentX = java.awt.Component.LEFT_ALIGNMENT
        panel.add(statusTitle)

        decxStatusLabel = JLabel()
        decxStatusLabel!!.alignmentX = java.awt.Component.LEFT_ALIGNMENT
        panel.add(decxStatusLabel)

        urlLabel = JLabel()
        urlLabel!!.alignmentX = java.awt.Component.LEFT_ALIGNMENT
        panel.add(urlLabel)

        panel.add(Box.createVerticalStrut(10))

        // Port Setting
        val portTitle = JLabel("Port Setting")
        portTitle.font = portTitle.font.deriveFont(java.awt.Font.BOLD, 12f)
        portTitle.alignmentX = java.awt.Component.LEFT_ALIGNMENT
        panel.add(portTitle)

        portField = JTextField(PreferencesManager.getPort().toString(), 8)
        panel.add(constrainHeight(createRowWithComponent("New Port:", portField!!)))

        refreshStatus()
        return panel
    }

    private fun refreshStatus() {
        SwingUtilities.invokeLater {
            val isServerRunning = server.isRunning
            val currentPort = PreferencesManager.getPort()
            val url = PluginUtils.buildServerUrl(port = currentPort)

            decxStatusLabel?.text = "DECX:  ${if (isServerRunning) "Running" else "Stopped"}"
            urlLabel?.text = "URL:   $url"
        }
    }

    private fun constrainHeight(component: JComponent): JComponent {
        val pref = component.preferredSize
        component.maximumSize = java.awt.Dimension(Int.MAX_VALUE, pref.height)
        return component
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
            restartServer(newPort)
        }
    }

    private fun restartServer(newPort: Int) {
        Thread {
            try {
                server.stop()
                Thread.sleep(500)
                server.start(newPort)
                SwingUtilities.invokeLater {
                    JOptionPane.showMessageDialog(
                        pluginContext.guiContext?.mainFrame,
                        "Server restarted on port $newPort",
                        "Success",
                        JOptionPane.INFORMATION_MESSAGE
                    )
                    refreshStatus()
                }
            } catch (e: Exception) {
                SwingUtilities.invokeLater {
                    JOptionPane.showMessageDialog(
                        pluginContext.guiContext?.mainFrame,
                        "Failed to restart server: ${e.message}",
                        "Error",
                        JOptionPane.ERROR_MESSAGE
                    )
                }
            }
        }.apply { isDaemon = true }.start()
    }
}
