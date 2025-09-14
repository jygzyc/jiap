package jadx.plugins.jiap.ui

import java.awt.Dimension
import javax.swing.JFrame

object ConfigWindow {

    private const val WINDOW_WIDTH = 600
    private const val WINDOW_HEIGHT = 400

    fun show() {
        val frame = JFrame("JIAP Configuration").apply {
            defaultCloseOperation = JFrame.DISPOSE_ON_CLOSE
            setSize(400, 300)
            minimumSize = Dimension(200, 150)
            setLocationRelativeTo(null)
        }
        
    }
}