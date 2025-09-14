package jadx.plugins.jiap.ui

import jadx.api.plugins.JadxPluginContext
import jadx.api.plugins.gui.JadxGuiContext
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


class JiapUIManager(private val pluginContext: JadxPluginContext) {

    companion object {
        private val logger = LoggerFactory.getLogger(JiapUIManager::class.java)
    }

    fun initializeGuiComponents(guiContext: JadxGuiContext) {
        guiContext.addMenuAction("JIAP Config", showConfigWindow())
    }


}