package jadx.plugins.decx.service

import jadx.api.JadxDecompiler
import jadx.api.ResourceFile
import jadx.api.ResourceType
import jadx.core.utils.android.AndroidManifestParser
import jadx.core.utils.android.AppAttribute
import jadx.plugins.decx.model.DecxServiceInterface
import jadx.plugins.decx.api.DecxApiResult
import org.w3c.dom.Document
import org.w3c.dom.Element
import org.w3c.dom.NodeList
import java.io.ByteArrayInputStream
import java.util.*
import javax.xml.parsers.DocumentBuilderFactory

class AndroidAppService(override val decompiler: JadxDecompiler) : DecxServiceInterface {

    private fun getAppManifest(): ResourceFile? {
        return decompiler.resources
            ?.stream()
            ?.filter { resourceFile -> resourceFile.type == ResourceType.MANIFEST }
            ?.findFirst()
            ?.orElse(null)
    }

    private fun getManifestDocument(): Document? {
        val manifest = getAppManifest() ?: return null
        val manifestContent = manifest.loadContent()?.text?.codeStr ?: return null
        val factory = DocumentBuilderFactory.newInstance()
        val builder = factory.newDocumentBuilder()
        return builder.parse(ByteArrayInputStream(manifestContent.toByteArray(Charsets.UTF_8)))
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

    fun handleGetAppManifest(): DecxApiResult {
        try {
            val manifest = getAppManifest() ?: return DecxApiResult(
                success = false,
                data = hashMapOf("error" to "handleGetAppManifest: AndroidManifest not found.")
            )
            val manifestContent = manifest.loadContent()?.text?.codeStr ?: ""
            val result = hashMapOf<String, Any>(
                "type" to "code",
                "name" to manifest.originalName,
                "code" to manifestContent
            )
            return DecxApiResult(success = true, data = result)
        } catch (e: Exception) {
            return DecxApiResult(success = false, data = hashMapOf("error" to "handleGetAppManifest: ${e.message}"))
        }
    }

    fun handleGetMainActivity(): DecxApiResult {
        try {
            val manifest = getAppManifest() ?: return DecxApiResult(
                success = false,
                data = hashMapOf("error" to "handleGetMainActivity: AndroidManifest not found.")
            )
            val parser = AndroidManifestParser(
                manifest,
                EnumSet.of(AppAttribute.MAIN_ACTIVITY),
                decompiler.args.security
            )
            val appParams = parser.parse()
            val mainActivityName = appParams.getMainActivity() ?: return DecxApiResult(
                success = false,
                data = hashMapOf(
                    "error" to "handleGetMainActivity: No MAIN/LAUNCHER Activity found in AndroidManifest.xml."
                )
            )

            val mainActivityClass = appParams.getMainActivityJavaClass(decompiler) ?: return DecxApiResult(
                success = false,
                data = hashMapOf(
                    "error" to "handleGetMainActivity: Failed to find Main Activity class '$mainActivityName'."
                )
            )
            
            val result: HashMap<String, Any> = hashMapOf(
                "type" to "code",
                "name" to mainActivityClass.fullName,
                "code" to mainActivityClass.code
            )
            return DecxApiResult(success = true, data = result)
        } catch (e: Exception) {
            return DecxApiResult(success = false, data = hashMapOf("error" to "handleGetMainActivity: ${e.message}"))
        }
    }

    fun handleGetApplication(): DecxApiResult {
        try {
            val manifest = getAppManifest() ?: return DecxApiResult(
                success = false,
                data = hashMapOf("error" to "handleGetApplication: AndroidManifest not found.")
            )
            val parser = AndroidManifestParser(
                manifest,
                EnumSet.of(AppAttribute.APPLICATION),
                decompiler.args.security
            )
            if (!parser.isManifestFound()) {
                return DecxApiResult(
                    success = false,
                    data = hashMapOf("error" to "handleGetApplication: AndroidManifest not found.")
                )
            }
            val appParams = parser.parse()
            val applicationClass = appParams.getApplicationJavaClass(decompiler) ?: return DecxApiResult(
                success = false,
                data = hashMapOf("error" to "handleGetApplication: Failed to get application class.")
            )
            val result: HashMap<String, Any> = hashMapOf(
                "type" to "code",
                "name" to applicationClass.fullName,
                "code" to applicationClass.code
            )
            return DecxApiResult(success = true, data = result)
        } catch (e: Exception) {
            return DecxApiResult(success = false, data = hashMapOf("error" to "handleGetApplication: ${e.message}"))
        }
    }

    fun handleGetExportedComponents(): DecxApiResult {
        try {
            val doc = getManifestDocument() ?: return DecxApiResult(
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
            return DecxApiResult(true, hashMapOf(
                "type" to "list",
                "count" to components.size,
                "components-list" to components
            ))
        } catch (e: Exception) {
            return DecxApiResult(false, hashMapOf("error" to "handleGetExportedComponents: ${e.message}"))
        }
    }

    fun handleGetDeepLinks(): DecxApiResult {
        try {
            val manifestDoc = getManifestDocument()
                ?: return DecxApiResult(false, hashMapOf("error" to "handleGetDeepLinks: Manifest not found"))
            
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
            
            return DecxApiResult(true, hashMapOf(
                "type" to "list",
                "count" to deepLinks.size,
                "deeplinks-list" to deepLinks
            ))
        } catch (e: Exception) {
            return DecxApiResult(false, hashMapOf("error" to "handleGetDeepLinks: ${e.message}"))
        }
    }

    fun handleGetDynamicReceivers(): DecxApiResult {
        try {
            val receivers = mutableListOf<HashMap<String, Any>>()
            decompiler.classesWithInners.forEach { jcls ->
                if ("registerReceiver" !in jcls.smali) return@forEach
                for (jmth in jcls.methods) {
                    val mthCode = jmth.codeStr
                    if ("registerReceiver" !in mthCode) continue

                    receivers.add(hashMapOf(
                        "class" to jcls.fullName,
                        "method" to mthCode
                    ))
                }
            }
            return DecxApiResult(true, hashMapOf(
                "type" to "list",
                "count" to receivers.size,
                "code-list" to receivers
            ))
        } catch (e: Exception) {
            return DecxApiResult(false, hashMapOf("error" to "handleGetDynamicReceivers error: ${e.message}"))
        }
    }
    
    fun handleGetAllResources(): DecxApiResult {
        try {
            val resources = decompiler.resources
                ?: return DecxApiResult(false, hashMapOf("error" to "handleGetAllResources: No resources found."))
            val fileNames = mutableListOf<String>()
            for (resFile in resources) {
                try {
                    if (resFile.deobfName == "resources.arsc") {
                        resFile.loadContent()?.subFiles?.forEach { sub ->
                            fileNames.add(sub.name)
                        }
                    }
                    fileNames.add(resFile.deobfName)
                } catch (_: Exception) {
                    fileNames.add(resFile.deobfName)
                }
            }
            if (fileNames.isEmpty()) {
                return DecxApiResult(false, hashMapOf("error" to "handleGetAllResources: No resources found."))
            }
            return DecxApiResult(true, hashMapOf(
                "type" to "list",
                "count" to fileNames.size,
                "resources-list" to fileNames
            ))
        } catch (e: Exception) {
            return DecxApiResult(false, hashMapOf("error" to "handleGetAllResources: ${e.message}"))
        }
    }

    fun handleGetResourceFile(res: String): DecxApiResult {
        try {
            val resources = decompiler.resources
                ?: return DecxApiResult(false, hashMapOf("error" to "handleGetResourceFile: No resources found."))
            for (resFile in resources) {
                if (resFile.deobfName == res) {
                    val content = resFile.loadContent()?.text?.codeStr ?: continue
                    return DecxApiResult(true, hashMapOf("type" to "code", "name" to res, "code" to content))
                }
                if (resFile.deobfName == "resources.arsc") {
                    for (sub in resFile.loadContent()?.subFiles ?: continue) {
                        if (sub.name == res) {
                            val content = sub.text?.codeStr ?: continue
                            return DecxApiResult(true, hashMapOf("type" to "code", "name" to res, "code" to content))
                        }
                    }
                }
            }
            return DecxApiResult(false, hashMapOf("error" to "handleGetResourceFile: Resource '$res' not found."))
        } catch (e: Exception) {
            return DecxApiResult(false, hashMapOf("error" to "handleGetResourceFile: ${e.message}"))
        }
    }

    fun handleGetAidlInterfaces(): DecxApiResult {
        try {
            // JADX uses '.' for inner classes in fullName (e.g. "IFoo.Stub"), but smali uses '$'
            val stubClasses = decompiler.classesWithInners.filter { clazz ->
                clazz.fullName.endsWith(".Stub")
            }
            val aidlInterfaces = stubClasses.map { stub ->
                val interfaceName = stub.fullName.removeSuffix(".Stub")
                // smali uses '$' for inner classes: Lcom/example/IFoo$Stub;
                val stubSmaliName = stub.fullName.replace(".Stub", "\$Stub")
                    .replace('.', '/')
                val implClasses = decompiler.classesWithInners.filter { clazz ->
                    clazz.fullName != stub.fullName &&
                    !clazz.fullName.endsWith(".Proxy") &&
                    clazz.smali.contains(".super L$stubSmaliName")
                }
                hashMapOf(
                    "interface" to interfaceName,
                    "stub" to stub.fullName,
                    "implements" to implClasses.map { it.fullName }
                )
            }
            val result = hashMapOf(
                "type" to "list",
                "count" to aidlInterfaces.size,
                "interfaces-list" to aidlInterfaces
            )
            return DecxApiResult(success = true, data = result)
        } catch (e: Exception) {
            return DecxApiResult(success = false, data = hashMapOf("error" to "handleGetAidlInterfaces: ${e.message}"))
        }
    }

    fun handleGetStrings(): DecxApiResult {
        try {
            val resources = decompiler.resources
                ?: return DecxApiResult(false, hashMapOf("error" to "handleGetStrings: No resources found."))
            val stringsList = resources.mapNotNull { resFile ->
                runCatching {
                    when (resFile.deobfName) {
                        "resources.arsc" -> {
                            val sub = resFile.loadContent()?.subFiles?.find { it.name == "res/values/strings.xml" }
                            sub?.text?.codeStr?.let { content -> hashMapOf("file" to sub.name, "content" to content) }
                        }
                        "res/values/strings.xml" -> {
                            resFile.loadContent()?.text?.codeStr
                                ?.let { content -> hashMapOf("file" to resFile.deobfName, "content" to content) }
                        }
                        else -> null
                    }
                }.getOrNull()
            }
            if (stringsList.isEmpty()) return DecxApiResult(false, hashMapOf("error" to "handleGetStrings: No strings.xml resource found."))
            return DecxApiResult(true, hashMapOf("type" to "list", "count" to stringsList.size, "strings-list" to stringsList))
        } catch (e: Exception) {
            return DecxApiResult(false, hashMapOf("error" to "handleGetStrings: ${e.message}"))
        }
    }
}
