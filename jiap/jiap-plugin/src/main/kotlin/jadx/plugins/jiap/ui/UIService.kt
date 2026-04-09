package jadx.plugins.jiap.ui

import jadx.api.plugins.JadxPluginContext
import jadx.plugins.jiap.api.JiapApi
import jadx.plugins.jiap.api.JiapApiResult
import java.awt.Component
import java.awt.Container
import javax.swing.JTextArea

/**
 * GUI-only service. Only available when running inside JADX GUI.
 */
class UIService(
    private val pluginContext: JadxPluginContext,
    private val api: JiapApi
) {

    private fun findTextArea(component: Component): JTextArea? {
        return when (component) {
            is JTextArea -> component
            is Container -> {
                component.components.forEach { child ->
                    findTextArea(child)?.let { return it }
                }
                null
            }
            else -> null
        }
    }

    fun handleGetSelectedClass(): JiapApiResult {
        try {
            val mainWindow = pluginContext.guiContext?.mainFrame as? jadx.gui.ui.MainWindow
            if (mainWindow == null) {
                return JiapApiResult.fail("handleGetSelectedClass: Not Gui Mode")
            }
            val tabs = mainWindow.tabbedPane
            val index = tabs.selectedIndex
            val className = if (index != -1) tabs.getTitleAt(index) else ""
            val selectedComponent = mainWindow.tabbedPane?.selectedComponent
                ?: return JiapApiResult.fail("handleGetSelectedClass: No selected component")

            val textArea = findTextArea(selectedComponent)
            val result = mapOf<String, Any>(
                "type" to "code",
                "name" to className,
                "code" to (textArea?.selectedText ?: "")
            )
            return JiapApiResult.ok(result)
        } catch (e: Exception) {
            return JiapApiResult.fail("GetSelectedClass: ${e.message}")
        }
    }

    fun handleGetSelectedText(): JiapApiResult {
        try {
            val mainWindow = pluginContext.guiContext?.mainFrame as? jadx.gui.ui.MainWindow
            if (mainWindow == null) {
                return JiapApiResult.fail("handleGetSelectedText: Not Gui Mode")
            }

            val selectedComponent = mainWindow.tabbedPane.selectedComponent
                ?: return JiapApiResult.fail("handleGetSelectedText: No selected component")

            val textArea = findTextArea(selectedComponent)
            val selectedText = textArea?.selectedText

            val result = mapOf<String, Any>(
                "type" to "code",
                "code" to (selectedText ?: "")
            )
            return JiapApiResult.ok(result)
        } catch (e: Exception) {
            return JiapApiResult.fail("GetSelectedText: ${e.message}")
        }
    }
}
