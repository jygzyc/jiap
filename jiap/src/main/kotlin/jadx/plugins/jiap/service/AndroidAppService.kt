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
import org.w3c.dom.Document
import org.w3c.dom.Element
import org.w3c.dom.NodeList
import java.io.ByteArrayInputStream
import javax.xml.parsers.DocumentBuilderFactory
import java.util.*

class AndroidAppService(override val pluginContext: JadxPluginContext) : JiapServiceInterface {

    private fun getAppManifest(): ResourceFile? {
        return try {
            var manifest: ResourceFile?
            if (this.gui) {
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
            manifest
        } catch (e: Exception) {
            throw e
        }
    }

    private fun getManifestDocument(): Document? {
        try {
            val manifest = getAppManifest() ?: return null
            val manifestContent = manifest.loadContent()?.text?.codeStr ?: return null
            val factory = DocumentBuilderFactory.newInstance()
            val builder = factory.newDocumentBuilder()
            return builder.parse(ByteArrayInputStream(manifestContent.toByteArray(Charsets.UTF_8)))
        } catch (e: Exception) {
            throw e
        }
    }

    private fun getTargetSdkVersion(doc: Document): Int {
        val usesSdks = doc.getElementsByTagName("uses-sdk")
        if (usesSdks.length > 0) {
            val item = usesSdks.item(0) as Element
            val target = item.getAttribute("android:targetSdkVersion")
            if (target.isNotEmpty()) return target.toIntOrNull() ?: 1
            val min = item.getAttribute("android:minSdkVersion")
            if (min.isNotEmpty()) return min.toIntOrNull() ?: 1
        }
        return 1
    }

    private fun getAppPackageFromManifest(manifest: ResourceFile): String? {
        return try {
            val manifestContent = manifest.loadContent()?.text?.codeStr ?: return null
            val factory = DocumentBuilderFactory.newInstance()
            val builder = factory.newDocumentBuilder()
            val doc = builder.parse(ByteArrayInputStream(manifestContent.toByteArray(Charsets.UTF_8)))
            val manifestElement = doc.getElementsByTagName("manifest").item(0) as? Element
            manifestElement?.getAttribute("package")
        } catch (e: Exception) {
            null
        }
    }

    fun handleGetAppManifest(): JiapResult {
        try {
            val manifest = getAppManifest() ?: return JiapResult(
                success = false,
                data = hashMapOf("error" to "handleGetAppManifest: AndroidManifest not found.")
            )
            val manifestContent = manifest.loadContent()?.text?.codeStr
            val result = hashMapOf<String, Any>(
                "type" to "code",
                "name" to manifest.originalName,
                "code" to manifestContent as Any
            )
            return JiapResult(success = true, data = result)
        } catch (e: Exception) {
            return JiapResult(success = false, data = hashMapOf("error" to "handleGetAppManifest: ${e.message}"))
        }
    }

    fun handleGetMainActivity(): JiapResult {
        try {
            var result: HashMap<String, Any>
            val manifest = getAppManifest() ?: return JiapResult(
                success = false,
                data = hashMapOf("error" to "handleGetAppMainActivity: AndroidManifest not found.")
            )
            var parser: AndroidManifestParser?
            if (this.gui) {
                val mainWindow: MainWindow = pluginContext.guiContext?.mainFrame as MainWindow
                val jadxWrapper: JadxWrapper = mainWindow.wrapper
                parser = AndroidManifestParser(
                    manifest,
                    EnumSet.of(AppAttribute.MAIN_ACTIVITY),
                    jadxWrapper.args.security
                )
            } else {
                parser = AndroidManifestParser(
                    manifest,
                    EnumSet.of(AppAttribute.MAIN_ACTIVITY),
                    decompiler.args.security
                )
            }
            val appParams = parser.parse()
            val mainActivityName = appParams.getMainActivity() ?: return JiapResult(
                success = false,
                data = hashMapOf(
                    "error" to "handleGetMainActivity: No MAIN/LAUNCHER Activity found in AndroidManifest.xml."
                )
            )

            var mainActivityClass = appParams.getMainActivityJavaClass(decompiler)?: return JiapResult(
                success = false,
                data = hashMapOf(
                    "error" to "handleGetMainActivity: Failed to find Main Activity class '$mainActivityName'."
                )
            )
            
            result = hashMapOf(
                "type" to "code",
                "name" to mainActivityClass.fullName,
                "code" to mainActivityClass.code
            )
            return JiapResult(success = true, data = result)
        } catch (e: Exception) {
            return JiapResult(success = false, data = hashMapOf("error" to "handleGetMainActivity: ${e.message}"))
        }
    }

    fun handleGetApplication(): JiapResult {
        try {
            val manifest = getAppManifest() ?: return JiapResult(
                success = false,
                data = hashMapOf("error" to "handleGetApplication: AndroidManifest not found.")
            )
            var parser: AndroidManifestParser?
            if (this.gui) {
                val mainWindow: MainWindow = pluginContext.guiContext?.mainFrame as MainWindow
                val jadxWrapper: JadxWrapper = mainWindow.wrapper
                parser = AndroidManifestParser(
                    manifest,
                    EnumSet.of(AppAttribute.APPLICATION),
                    jadxWrapper.args.security
                )
            } else {
                parser = AndroidManifestParser(
                    manifest,
                    EnumSet.of(AppAttribute.APPLICATION),
                    decompiler.args.security
                )
            }
            if(!parser.isManifestFound()) {
                return JiapResult(
                    success = false,
                    data = hashMapOf("error" to "handleGetApplication: AndroidManifest not found.")
                )
            }
            val appParams = parser.parse()
            val applicationClass = appParams.getApplicationJavaClass(decompiler) ?: return JiapResult(
                success = false,
                data = hashMapOf("error" to "handleGetApplication: Failed to get application class.")
            )
            var result: HashMap<String, Any> = hashMapOf(
                "type" to "code",
                "name" to applicationClass.fullName,
                "code" to applicationClass.code
            )
            return JiapResult(success = true, data = result)
        } catch (e: Exception) {
            return JiapResult(success = false, data = hashMapOf("error" to "handleGetApplication: ${e.message}"))
        }
    }

    fun handleGetExportedComponents(): JiapResult {
        try {
            val doc = getManifestDocument() ?: return JiapResult(
                false, 
                hashMapOf("error" to "handleGetExportedComponents: Manifest not found")
            )
            val components = mutableListOf<HashMap<String, Any>>()
            val tags = listOf("activity", "service", "receiver", "provider")
            val targetSdk = getTargetSdkVersion(doc)

            for (tag in tags) {
                val nodeList: NodeList = doc.getElementsByTagName(tag)
                for (i in 0 until nodeList.length) {
                    val element = nodeList.item(i) as Element
                    val exportedAttr = element.getAttribute("android:exported")
                    val intentFilters = element.getElementsByTagName("intent-filter")
                    
                    val hasIntentFilterWithAction = (0 until intentFilters.length).any { j ->
                        val filter = intentFilters.item(j) as Element
                        filter.getElementsByTagName("action").length > 0
                    }

                    val isExported = when {
                        exportedAttr.equals("true", ignoreCase = true) -> true
                        exportedAttr.equals("false", ignoreCase = true) -> false
                        hasIntentFilterWithAction -> true
                        tag == "provider" && exportedAttr.isEmpty() && targetSdk < 17 -> true
                        else -> false
                    }

                    if (!isExported) continue

                    val name = element.getAttribute("android:name")
                    val componentMap = hashMapOf<String, Any>(
                        "name" to name,
                        "type" to tag
                    )
                    
                    val permission = element.getAttribute("android:permission")
                    if (permission.isNotEmpty()) componentMap["permission"] = permission
                    
                    if (tag == "activity") {
                        val launchMode = element.getAttribute("android:launchMode")
                        if (launchMode.isNotEmpty()) componentMap["launchMode"] = launchMode
                        val taskAffinity = element.getAttribute("android:taskAffinity")
                        if (taskAffinity.isNotEmpty()) componentMap["taskAffinity"] = taskAffinity
                    }
                    
                    if (tag == "provider") {
                        val authorities = element.getAttribute("android:authorities")
                        if (authorities.isNotEmpty()) componentMap["authorities"] = authorities
                        val readPermission = element.getAttribute("android:readPermission")
                        val writePermission = element.getAttribute("android:writePermission")
                        if (readPermission.isNotEmpty()) componentMap["readPermission"] = readPermission
                        if (writePermission.isNotEmpty()) componentMap["writePermission"] = writePermission
                        val grantUriPermissions = element.getAttribute("android:grantUriPermissions")
                        if (grantUriPermissions.equals("true", ignoreCase = true)) {
                            componentMap["grantUriPermissions"] = true
                        }
                    }
                    
                    if (intentFilters.length > 0) {
                        val parsedFilters = mutableListOf<HashMap<String, Any>>()
                        for (j in 0 until intentFilters.length) {
                            val filter = intentFilters.item(j) as Element
                            val filterMap = hashMapOf<String, Any>()
                            
                            val actionNodes = filter.getElementsByTagName("action")
                            val actions = (0 until actionNodes.length).mapNotNull { k ->
                                val action = (actionNodes.item(k) as Element).getAttribute("android:name")
                                action.takeIf { it.isNotEmpty() }
                            }
                            if (actions.isEmpty()) continue
                            filterMap["actions"] = actions
                            
                            val categoryNodes = filter.getElementsByTagName("category")
                            val categories = (0 until categoryNodes.length).mapNotNull { k ->
                                val category = (categoryNodes.item(k) as Element).getAttribute("android:name")
                                category.takeIf { it.isNotEmpty() }
                            }
                            if (categories.isNotEmpty()) filterMap["categories"] = categories
                            
                            val dataNodes = filter.getElementsByTagName("data")
                            val dataList = (0 until dataNodes.length).mapNotNull { k ->
                                val dataElement = dataNodes.item(k) as Element
                                val dataMap = hashMapOf<String, String>()
                                listOf("scheme", "host", "path", "pathPrefix", "pathPattern", "mimeType").forEach { attr ->
                                    val value = dataElement.getAttribute("android:$attr")
                                    if (value.isNotEmpty()) dataMap[attr] = value
                                }
                                dataMap.takeIf { it.isNotEmpty() }
                            }
                            if (dataList.isNotEmpty()) filterMap["data"] = dataList
                            
                            parsedFilters.add(filterMap)
                        }
                        if (parsedFilters.isNotEmpty()) componentMap["intentFilters"] = parsedFilters
                    }
                    
                    components.add(componentMap)
                }
            }
            return JiapResult(true, hashMapOf(
                "type" to "list",
                "count" to components.size,
                "components-list" to components
            ))
        } catch (e: Exception) {
            return JiapResult(false, hashMapOf("error" to "handleGetExportedComponents: ${e.message}"))
        }
    }

    fun handleGetDeepLinks(): JiapResult {
        return try {
            val manifestDoc = getManifestDocument()
                ?: return JiapResult(false, hashMapOf("error" to "handleGetDeepLinks: Manifest not found"))
            
            val deepLinks = mutableListOf<HashMap<String, Any>>()
            val nodeList = manifestDoc.getElementsByTagName("intent-filter")
            
            for (i in 0 until nodeList.length) {
                val intentFilter = nodeList.item(i) as? Element ?: continue
                
                if (!isDeepLinkIntentFilter(intentFilter)) continue
                
                val componentName = getComponentName(intentFilter) ?: continue
                val dataElements = extractDataElements(intentFilter)
                
                dataElements.forEach { dataMap ->
                    if (dataMap["scheme"].isNullOrEmpty()) return@forEach
                    
                    deepLinks.add(hashMapOf(
                        "component" to componentName,
                        "scheme" to (dataMap["scheme"] ?: ""),
                        "host" to (dataMap["host"] ?: ""),
                        "port" to (dataMap["port"] ?: ""),
                        "path" to (dataMap["path"] ?: ""),
                        "pathPrefix" to (dataMap["pathPrefix"] ?: ""),
                        "pathPattern" to (dataMap["pathPattern"] ?: ""),
                        "mimeType" to (dataMap["mimeType"] ?: "")
                    ))
                }
            }
            
            JiapResult(true, hashMapOf(
                "type" to "list",
                "count" to deepLinks.size,
                "deeplinks-list" to deepLinks
            ))
        } catch (e: Exception) {
            JiapResult(false, hashMapOf("error" to "handleGetDeepLinks: ${e.message}"))
        }
    }
    
    private fun isDeepLinkIntentFilter(intentFilter: Element): Boolean {
        val hasViewAction = intentFilter.getElementsByTagName("action").let { nodes ->
            (0 until nodes.length).any { i ->
                (nodes.item(i) as? Element)?.getAttribute("android:name") == "android.intent.action.VIEW"
            }
        }
        
        val categories = intentFilter.getElementsByTagName("category")
        val hasBrowsable = (0 until categories.length).any { i ->
            (categories.item(i) as? Element)?.getAttribute("android:name") == "android.intent.category.BROWSABLE"
        }
        val hasDefault = (0 until categories.length).any { i ->
            (categories.item(i) as? Element)?.getAttribute("android:name") == "android.intent.category.DEFAULT"
        }
        
        return hasViewAction && hasBrowsable && hasDefault
    }
    
    private fun getComponentName(intentFilter: Element): String? {
        val parent = intentFilter.parentNode as? Element ?: return null
        return parent.getAttribute("android:name").takeIf { it.isNotEmpty() }
    }
    
    private fun extractDataElements(intentFilter: Element): List<Map<String, String>> {
        val dataTags = intentFilter.getElementsByTagName("data")
        val dataList = mutableListOf<Map<String, String>>()
        
        for (i in 0 until dataTags.length) {
            val data = dataTags.item(i) as? Element ?: continue
            
            dataList.add(mapOf(
                "scheme" to data.getAttribute("android:scheme"),
                "host" to data.getAttribute("android:host"),
                "port" to data.getAttribute("android:port"),
                "path" to data.getAttribute("android:path"),
                "pathPrefix" to data.getAttribute("android:pathPrefix"),
                "pathPattern" to data.getAttribute("android:pathPattern"),
                "mimeType" to data.getAttribute("android:mimeType")
            ))
        }
        
        return dataList
    }  
}
