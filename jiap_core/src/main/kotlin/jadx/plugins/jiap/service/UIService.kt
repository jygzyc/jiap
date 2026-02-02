package jadx.plugins.jiap.service

import jadx.gui.ui.MainWindow
import jadx.api.JavaNode
import jadx.api.plugins.JadxPluginContext

import jadx.plugins.jiap.model.JiapResult
import jadx.plugins.jiap.model.JiapServiceInterface
import jadx.plugins.jiap.utils.CodeUtils
import kotlin.collections.forEach
import java.awt.Component
import java.awt.Container
import javax.swing.JTextArea

class UIService(override val pluginContext: JadxPluginContext) : JiapServiceInterface {

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

    fun handleGetSelectedClass(): JiapResult {
        try {
            val mainWindow = pluginContext.guiContext?.mainFrame
            if (mainWindow !is MainWindow) {
                return JiapResult(success = false, data = hashMapOf("error" to "handleGetSelectedClass: Not Gui Mode"))
            }
            val tabs = mainWindow.tabbedPane
            val index = tabs.selectedIndex
            val className = if (index != -1) tabs.getTitleAt(index) else ""
            val selectedComponent = mainWindow.tabbedPane?.selectedComponent
                ?: return JiapResult(
                    success = false,
                    data = hashMapOf("error" to "handleGetSelectedClass: No selected component")
                )

            val textArea = findTextArea(selectedComponent)
            val result = hashMapOf<String, Any>(
                "type" to "code",
                "name" to className,
                "code" to (textArea?.selectedText ?: "")
            )
            return JiapResult(success = true, data = result)
        } catch (e: Exception) {
            return JiapResult(success = false, data = hashMapOf("error" to "GetSelectedClass: ${e.message}"))
        }
    }

    fun handleGetSelectedText(): JiapResult {
        try {
            val mainWindow = pluginContext.guiContext?.mainFrame
            if (mainWindow !is MainWindow) {
                return JiapResult(success = false, data = hashMapOf("error" to "handleGetSelectedText: Not Gui Mode"))
            }

            val selectedComponent = mainWindow.tabbedPane?.selectedComponent
                ?: return JiapResult(
                    success = false,
                    data = hashMapOf("error" to "handleGetSelectedText: No selected component")
                )

            val textArea = findTextArea(selectedComponent)
            val selectedText = textArea?.selectedText

            val result = hashMapOf<String, Any>(
                "type" to "code",
                "code" to (selectedText ?: "")
            )
            return JiapResult(success = true, data = result)
        } catch (e: Exception) {
            return JiapResult(success = false, data = hashMapOf("error" to "handleGetSelectedText: ${e.message}"))
        }
    }
}