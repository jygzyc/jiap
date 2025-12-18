package jadx.plugins.jiap.service

import jadx.gui.ui.MainWindow
import jadx.api.JavaClass
import jadx.api.JavaMethod
import jadx.api.JavaNode
import jadx.api.plugins.JadxPluginContext

import jadx.plugins.jiap.model.JiapResult
import jadx.plugins.jiap.model.JiapServiceInterface
import jadx.plugins.jiap.utils.CodeUtils
import jadx.plugins.jiap.utils.LogUtils
import jadx.plugins.jiap.utils.CacheUtils
import kotlin.collections.forEach
import java.awt.Component
import java.awt.Container
import javax.swing.JTextArea

class CommonService(override val pluginContext: JadxPluginContext) : JiapServiceInterface {

    private fun processUsage(searchNode: JavaNode, xrefNodes: MutableList<JavaNode>): HashMap<String, Any> {
        val usageHashMap = hashMapOf<String, Any>()
        xrefNodes.groupBy(JavaNode::getTopParentClass).forEach classLoop@{ (topUseClass, nodesInClass) ->
            val codeInfo = topUseClass.codeInfo
            val usePositions = topUseClass.getUsePlacesFor(codeInfo, searchNode)
            if (usePositions.isEmpty()) {
                return@classLoop
            }
            val code = codeInfo.codeStr
            usePositions.forEach positionLoop@{ pos ->
                val line = CodeUtils.getLineForPos(code, pos)
                if (line.trim().startsWith("import ")) {
                    return@positionLoop
                }
                val correspondingNode = nodesInClass.firstOrNull() ?: nodesInClass.first()
                val codeLineNumber = CodeUtils.getLineNumberForPos(code, pos)
                usageHashMap["${correspondingNode.fullName.hashCode()}$codeLineNumber"] = hashMapOf(
                    "fullName" to correspondingNode.fullName,
                    "className" to topUseClass.fullName,
                    "codeLineNumber" to codeLineNumber,
                    "codeLine" to line.trim()
                )
            }
        }
        return usageHashMap
    }

    fun handleGetAllClasses(): JiapResult {
        try {
            val classes = decompiler.classesWithInners.map { it.fullName }
            val result = hashMapOf(
                "type" to "list",
                "count" to classes.size,
                "classes-list" to classes
            )
            return JiapResult(success = true, data = result)
        } catch (e: Exception) {
            LogUtils.error("handleGetAllClasses", e)
            return JiapResult(success = false, data = hashMapOf("error" to "handleGetAllClasses: ${e.message}"))
        }
    }

    fun handleGetClassInfo(className: String): JiapResult {
        return try {
            val clazz = decompiler.searchJavaClassOrItsParentByOrigFullName(className)
            if (clazz != null) {
                val result = hashMapOf(
                    "type" to "list",
                    "name" to clazz.fullName,
                    "methods-list" to clazz.methods.map { it.toString() },
                    "fields-list" to clazz.fields.map { it.toString() }
                )
                JiapResult(success = true, data = result)
            } else {
                JiapResult(success = false, data = hashMapOf("error" to "handleGetClassInfo: $className not found"))
            }
        } catch (e: Exception) {
            LogUtils.error("handleGetClassInfo", e)
            JiapResult(success = false, data = hashMapOf("error" to "handleGetClassInfo: ${e.message}"))
        }
    }

    fun handleGetClassSource(className: String, isSmali: Boolean): JiapResult {
        try {
            val clazz = decompiler.classesWithInners?.find {
                it.fullName == className
            } ?: return JiapResult(
                success = false,
                data = hashMapOf("error" to "getClassSource: $className not found")
            )
            clazz.decompile()
            val code = if (isSmali) clazz.smali else clazz.code
            val result: HashMap<String, Any> = hashMapOf(
                "type" to "code",
                "name" to clazz.fullName,
                "code" to code
            )
            return JiapResult(success = true, data = result)
        } catch (e: Exception) {
            LogUtils.error("handleGetClassSource", e)
            return JiapResult(success = false, data = hashMapOf("error" to "getClassSource: ${e.message}"))
        }
    }

    fun handleSearchClassKey(keyword: String): JiapResult {
        try {
            val lowerKeyword = keyword.lowercase()
            val classes = decompiler.classesWithInners?.parallelStream()?.filter { clazz ->
                clazz.code.lowercase().contains(lowerKeyword)
            } ?: emptyList()
            val result = hashMapOf(
                "type" to "list",
                "count" to classes.size,
                "classes-list" to classes.map { it.fullName }
            )
            return JiapResult(success = true, data = result)
        } catch (e: Exception) {
            LogUtils.error("handleSearchClassKey", e)
            return JiapResult(success = false, data = hashMapOf("error" to "handleSearchClassKey: ${e.message}"))
        }
    }

    fun handleSearchMethod(methodName: String): JiapResult {
        try {
            val lowerMethodName = methodName.lowercase()
            val methods = decompiler.classesWithInners?.flatMap { clazz ->
                clazz.methods.filter { mth ->
                    mth.fullName.lowercase().contains(lowerMethodName)
                }
            } ?: emptyList()
            val result = hashMapOf(
                "type" to "list",
                "count" to methods.size,
                "methods-list" to methods.map { it.toString() }
            )
            return JiapResult(success = true, data = result)
        } catch (e: Exception) {
            LogUtils.error("handleSearchMethod", e)
            return JiapResult(success = false, data = hashMapOf("error" to "handleSearchMethod: ${e.message}"))
        }
    }

    fun handleGetMethodSource(methodName: String, isSmali: Boolean): JiapResult {
        try {
            val mthPair = CodeUtils.findMethod(decompiler, methodName) ?: return JiapResult(
                success = false,
                data = hashMapOf("error" to "handleGetMethodSource: $methodName not found")
            )
            val clazz = mthPair.first
            val method = mthPair.second
            clazz.decompile()

            val code = if (isSmali) CodeUtils.extractMethodSmaliCode(clazz, method) else method.codeStr
            val result: HashMap<String, Any> = hashMapOf(
                "type" to "code",
                "name" to method.toString(),
                "code" to code
            )
            return JiapResult(success = true, data = result)
        } catch (e: Exception) {
            LogUtils.error("handleGetMethodSource", e)
            return JiapResult(success = false, data = hashMapOf("error" to "handleGetMethodSource: ${e.message}"))
        }
    }



    fun handleGetMethodXref(methodName: String): JiapResult {
        try {
            val mthPair = CodeUtils.findMethod(decompiler, methodName)
            ?: return JiapResult(success = false, data = hashMapOf("error" to "handleGetMethodXref: $methodName not found"))
            val method = mthPair.second
            val xrefNodes = mutableListOf<JavaNode>()
            method.declaringClass.decompile()
            method.overrideRelatedMethods.forEach { relatedMethod ->
                val relatedUseIn = relatedMethod.useIn
                if (relatedUseIn.isNotEmpty()) {
                    xrefNodes.addAll(relatedUseIn)
                }
            }
            xrefNodes.addAll(method.useIn)
            val references = processUsage(method, xrefNodes)
            val result = hashMapOf<String, Any>(
                "type" to "list",
                "count" to xrefNodes.size,
                "references-list" to references
            )
            return JiapResult(success = true, data = result)
        } catch (e: Exception) {
            LogUtils.error("handleGetMethodXref", e)
            return JiapResult(success = false, data = hashMapOf("error" to "handleGetMethodXref: ${e.message}"))
        }
    }

    fun handleGetClassXref(className: String): JiapResult {
        try {
            val clazz = decompiler.searchJavaClassOrItsParentByOrigFullName(className) 
            ?: return JiapResult(success = false, data = hashMapOf("error" to "handleGetClassXref: $className not found"))
            val xrefNodes = mutableListOf<JavaNode>()
            clazz.decompile()
            val classUseIn = clazz.useIn
            if(classUseIn.isNotEmpty()){
                xrefNodes.addAll(classUseIn)
            }
            clazz.methods.forEach { method ->
                if(method.isConstructor){
                    val constructorUseIn = method.useIn
                    if(constructorUseIn.isNotEmpty()){
                        xrefNodes.addAll(constructorUseIn)
                    }
                } 
            }
            val references = processUsage(clazz, xrefNodes)
            val result = hashMapOf<String, Any>(
                "type" to "list",
                "count" to xrefNodes.size,
                "references-list" to references
            )
            return JiapResult(success = true, data = result)
        } catch (e: Exception) {
            LogUtils.error("handleGetClassXref", e)
            return JiapResult(success = false, data = hashMapOf("error" to "handleGetClassXref: ${e.message}"))
        }
    }

    fun handleGetImplementOfInterface(interfaceName: String): JiapResult {
        return try {
            val interfaceClazz = decompiler.searchJavaClassOrItsParentByOrigFullName(interfaceName)
            ?: return JiapResult(success = false, data = hashMapOf("error" to "handleGetImplementOfInterface: $interfaceName not found"))
            val implementingClasses = decompiler.classesWithInners.filter {
                it.smali.contains(".super L${interfaceClazz.fullName.replace('.', '/')};") ?: false
            }
            val result = hashMapOf(
                "type" to "list",
                "classes-list" to implementingClasses.map { it.fullName }
            )
            JiapResult(success = true, data = result)
        } catch (e: Exception) {
            LogUtils.error("handleGetImplementOfInterface", e)
            JiapResult(success = false, data = hashMapOf("error" to "handleGetImplementOfInterface: ${e.message}"))
        }
    }

    fun handleGetSubclasses(className: String): JiapResult {
        return try {
            val clazz = decompiler.searchJavaClassOrItsParentByOrigFullName(className) 
            ?: return JiapResult(success = false, data = hashMapOf("error" to "handleGetSubclasses: $className not found"))

            val subClasses = decompiler.classesWithInners.filter {
                it.smali.contains(".super L${clazz.fullName.replace(".", "/")};")
            }
            val result = hashMapOf(
                "type" to "list",
                "classes-list" to subClasses.map { it.fullName }
            )
            JiapResult(success = true, data = result)
        } catch (e: Exception) {
            LogUtils.error("handleGetSubclasses", e)
            JiapResult(success = false, data = hashMapOf("error" to "handleGetSubclasses: ${e.message}"))
        }
    }

    // UI Mode

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
        }catch (e:Exception){
            LogUtils.error("handleGetSelectedClass", e)
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
            LogUtils.error("handleGetSelectedText", e)
            return JiapResult(success = false, data = hashMapOf("error" to "handleGetSelectedText: ${e.message}"))
        }
    }
}