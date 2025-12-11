package jadx.plugins.jiap.service

import jadx.api.plugins.JadxPluginContext
import jadx.core.utils.android.AndroidManifestParser
import jadx.core.utils.android.AppAttribute
import jadx.api.ResourceFile
import jadx.api.ResourceType
import jadx.gui.JadxWrapper
import jadx.gui.ui.MainWindow
import jadx.plugins.jiap.model.JiapServiceInterface
import jadx.plugins.jiap.model.JiapResult
import jadx.plugins.jiap.utils.LogUtils

import java.util.*

class AndroidAppService(override val pluginContext: JadxPluginContext) : JiapServiceInterface{

    fun handleGetAppManifest(): JiapResult {
        try{
            var manifest: ResourceFile?
            if(this.gui){
                val mainWindow: MainWindow = pluginContext.guiContext?.mainFrame as MainWindow
                val jadxWrapper: JadxWrapper = mainWindow.wrapper
                manifest = AndroidManifestParser.getAndroidManifest(jadxWrapper.resources)
            } else {
                manifest = decompiler.resources
                    ?.stream()
                    ?.filter { resourceFile -> resourceFile.type == ResourceType.MANIFEST }
                    ?.findFirst()
                    ?.orElse(null)
            }
            if (manifest == null){
                LogUtils.error("AndroidManifest not found.")
                return JiapResult(success = false, data = hashMapOf("error" to "getAppManifest: AndroidManifest not found."))
            }
            val manifestContent = manifest.loadContent()?.text?.codeStr
            val result = hashMapOf<String, Any>(
                "type" to "code",
                "name" to manifest.originalName,
                "code" to manifestContent as Any
            )
            return JiapResult(success = true, data = result)

        } catch (e: Exception) {
            LogUtils.error("Loading AndroidManifest", e)
            return JiapResult(success = false, data = hashMapOf("error" to "getAppManifest: ${e.message}"))
        }
    }

    fun handleGetMainActivity(): JiapResult {
        try{
            var result = HashMap<String, Any>()
            if(this.gui){
                val mainWindow = pluginContext.guiContext?.mainFrame
                if(mainWindow is MainWindow){
                    val jadxWrapper = mainWindow.wrapper
                    val manifest = AndroidManifestParser.getAndroidManifest(jadxWrapper.resources)
                    if (manifest == null) {
                        LogUtils.error("AndroidManifest not found.")
                        return JiapResult(success = false, data = hashMapOf("error" to "getMainActivity: AndroidManifest not found."))
                    }
                    
                    val parser = AndroidManifestParser(
                        manifest,
                        EnumSet.of(AppAttribute.MAIN_ACTIVITY),
                        jadxWrapper.args.security
                    )
                    
                    val appParams = parser.parse()
                    val mainActivityClass = appParams.getMainActivityJavaClass(decompiler) ?: return JiapResult(
                        success = false,
                        data = hashMapOf("error" to "getMainActivity: Failed to get main activity class.")
                    )

                    result = hashMapOf(
                        "type" to "code",
                        "name" to mainActivityClass.fullName,
                        "code" to mainActivityClass.code
                    )
                } else {
                    LogUtils.warn("Main frame not MainWindow instance")
                }
                return JiapResult(success = true, data = result)
            } else {
                return JiapResult(success = false, data = hashMapOf("error" to "getMainActivity: command mode not support"))
            }
        }catch(e: Exception){
            LogUtils.error("Loading main activity", e)
            return JiapResult(success = false, data = hashMapOf("error" to "getMainActivity: ${e.message}"))
        }
    }
}