package jadx.plugins.jiap.service

import jadx.plugins.jiap.model.JiapResult
import org.slf4j.LoggerFactory

import jadx.api.plugins.JadxPluginContext
import jadx.core.utils.android.AndroidManifestParser
import jadx.api.ResourceFile
import jadx.api.ResourceType
import jadx.gui.JadxWrapper
import jadx.gui.ui.MainWindow
import jadx.plugins.jiap.model.JiapServiceInterface

class AndroidAppService(override val pluginContext: JadxPluginContext) : JiapServiceInterface{
    
    companion object {
        private val logger = LoggerFactory.getLogger(AndroidAppService::class.java)
    }

    private val decompiler = pluginContext.decompiler
    
    fun handleGetAppManifest(): JiapResult {
        try{
            var manifest: ResourceFile?
            if(this.gui){
                val mainWindow: MainWindow = pluginContext.guiContext?.mainFrame as MainWindow
                val jadxWrapper: JadxWrapper = mainWindow.wrapper
                manifest = AndroidManifestParser.getAndroidManifest(jadxWrapper.resources)
            } else {
                manifest = decompiler!!.resources
                    ?.stream()
                    ?.filter { resourceFile -> resourceFile.type == ResourceType.MANIFEST }
                    ?.findFirst()
                    ?.orElse(null)
            }
            if (manifest == null){
                logger.error("JIAP Error: AndroidManifest.xml not found.")
                return JiapResult(success = false, data = hashMapOf("error" to "getAppManifest: AndroidManifest not found."))
            }
            val manifestContent = manifest.loadContent()?.text?.codeStr
            val result = hashMapOf<String, Any>(
                "name" to manifest.originalName,
                "type" to "manifest/xml",
                "content" to manifestContent as Any
            )
            return JiapResult(success = true, data = result)

        } catch (e: Exception) {
            logger.error("JIAP Error: load AndroidManifest.xml", e)
            return JiapResult(success = false, data = hashMapOf("error" to "getAppManifest: ${e.message}"))
        }
    }

    // fun handleGetMainActivity(): JiapResult {
    //     try{
    //         if(isGui){
    //             val wrapper: JadxWrapper = mainWindow.getWrapper()
    //             val resources: List<ResourceFile> = wrapper.getResources()
    //             val parser = AndroidManifestParser(
    //                 AndroidMainifestParser.getAndroidManifest(resources),
    //                 EnumSet.of(AppAttribute.MAIN_ACTIVITY),
    //                 wrapper.args.security
    //             )
    //             if(!parser.isManifestFound()){
    //                 logger.error("[x]JIAP Error: AndroidManifest.xml not found.")
    //                 return JiapResult(success = false, data = hashMapOf("error" to "AndroidManifest.xml not found."))
    //             }
    //             val results = parser.parse()
    //             if(results.mainActivity == null){
    //                 logger.error("[x]JIAP Error: Main activity not found from AndroidManifest.")
    //                 return JiapResult(success = false, data = hashMapOf("error" to "Main activity not found from AndroidManifest."))
    //             }
    //             val mainActivity = results.getMainActivityJavaClass(wrapper.decompiler)
    //         } else {
    //             val manifest = decompiler.resources
    //                 ?.stream()
    //                 ?.filter { resourceFile -> resourceFile.type == ResourceType.MANIFEST }
    //                 ?.findFirst()
    //                 ?.orElse(null)
    //             if (manifest == null){
    //                 logger.error("[x]JIAP Error: AndroidManifest.xml not found.")
    //                 return JiapResult(success = false, data = hashMapOf("error" to "AndroidManifest.xml not found."))
    //             }
    //             val mainActivity = AndroidManifestParser.getMainActivity(manifest)
    //             if (mainActivity == null) {
    //                 logger.error("[x]JIAP Error: Main activity not found in AndroidManifest.xml.")
    //                 return JiapResult(success = false, data = hashMapOf("error" to "Main activity not found in AndroidManifest.xml."))
    //             }
    //         }
    //         val result = hashMapOf<String, Any>(
    //             "type" to "main-activity",
    //             "mainActivity" to ""
    //         )
    //         return JiapResult(success = true, data = result)

    //     } catch (e: Exception) {
    //         logger.error("[x]JIAP Error:load main activity", e)
    //         return JiapResult(success = false, data = hashMapOf("error" to "getMainActivity: ${e.message}"))
    //     }
    // }
}