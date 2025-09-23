package jadx.plugins.jiap.service

import org.slf4j.LoggerFactory

import jadx.gui.ui.MainWindow
import jadx.gui.JadxWrapper
import jadx.api.JadxDecompiler
import jadx.api.JavaClass
import jadx.api.JavaMethod
import jadx.api.JavaNode
import jadx.api.plugins.JadxPluginContext

import jadx.plugins.jiap.model.JiapResult
import jadx.plugins.jiap.model.JiapServiceInterface
import jadx.plugins.jiap.utils.CodeUtils
import kotlin.collections.forEach
import java.awt.Component
import java.awt.Container
import javax.swing.JTextArea

class CommonService(override val pluginContext: JadxPluginContext) : JiapServiceInterface {

    companion object {
        private val logger = LoggerFactory.getLogger(CommonService::class.java)
    }

    val decompiler: JadxDecompiler = pluginContext.decompiler

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
            logger.error("JIAP Error: load classes", e)
            return JiapResult(success = false, data = hashMapOf("error" to "getAllClasses: ${e.message}"))
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
            val code = if (isSmali) clazz.smali else clazz.code
            val result: HashMap<String, Any> = hashMapOf(
                "type" to "code",
                "name" to clazz.fullName,
                "code" to code
            )
            return JiapResult(success = true, data = result)
        } catch (e: Exception) {
            logger.error("JIAP Error: get class source", e)
            return JiapResult(success = false, data = hashMapOf("error" to "getClassSource: ${e.message}"))
        }
    }

    fun handleGetMethodSource(methodName: String, isSmali: Boolean): JiapResult {
        try {
            var method: JavaMethod? = null
            var clazz: JavaClass? = null
            val tmpResult = findMethodInAllClasses(methodName)
            if (tmpResult != null) {
                clazz = tmpResult.first
                method = tmpResult.second
            }
            if (clazz == null || method == null) {
                return JiapResult(
                    success = false,
                    data = hashMapOf("error" to "getMethodSource: $methodName not found")
                )
            }
            val code = if (isSmali) CodeUtils.extractMethodSmaliCode(clazz, method) else method.codeStr
            val result: HashMap<String, Any> = hashMapOf(
                "type" to "code",
                "name" to method.toString(),
                "code" to code
            )
            return JiapResult(success = true, data = result)
        } catch (e: Exception) {
            logger.error("JIAP Error: get method source", e)
            return JiapResult(success = false, data = hashMapOf("error" to "getMethodSource: ${e.message}"))
        }
    }

    private fun findMethodInAllClasses(methodName: String): Pair<JavaClass, JavaMethod>? {
        decompiler.classesWithInners?.forEach { clazz ->
            clazz.methods.find { it.toString() == methodName }?.let { method ->
                return clazz to method
            }
        }
        return null
    }

    fun handleSearchClassByName(className: String): JiapResult {
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
                JiapResult(success = false, data = hashMapOf("error" to "searchClassByName: $className not found"))
            }
        } catch (e: Exception) {
            logger.error("JIAP Error: search class by name", e)
            JiapResult(success = false, data = hashMapOf("error" to "searchClassByName: ${e.message}"))
        }
    }

    fun handleListMethodsOfClass(className: String): JiapResult {
        try {
            val methodsList = decompiler.classesWithInners?.find {
                it.fullName == className
            }?.methods?.map {
                it.toString()
            } ?: emptyList()
            val result = hashMapOf(
                "type" to "list",
                "count" to methodsList.size,
                "methods-list" to methodsList
            )
            return JiapResult(success = true, data = result)
        } catch (e: Exception) {
            logger.error("JIAP Error: list methods of class", e)
            return JiapResult(success = false, data = hashMapOf("error" to "listMethodsOfClass: ${e.message}"))
        }
    }

    fun handleGetMethodXref(methodName: String): JiapResult {
        try {
            val method = decompiler.classesWithInners.firstNotNullOfOrNull { clazz ->
                clazz.methods.find { it.toString() == methodName }
            } ?: return JiapResult(success = false, data = hashMapOf("error" to "getMethodXref: $methodName not found"))
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
            logger.error("JIAP Error: get method xref", e)
            return JiapResult(success = false, data = hashMapOf("error" to "getMethodXref: ${e.message}"))
        }
    }

    fun handleGetClassXref(className: String): JiapResult {
        try {
            val clazz = decompiler.classesWithInners.find { it.fullName == className }
                ?: return JiapResult(success = false, data = hashMapOf("error" to "getClassXref: $className not found"))
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
            logger.error("JIAP Error: get class xref", e)
            return JiapResult(success = false, data = hashMapOf("error" to "getClassXref: ${e.message}"))
        }
    }

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
                if (line.startsWith("import ")) {
                    return@positionLoop
                }
                val correspondingNode = nodesInClass.firstOrNull() ?: nodesInClass.first()
                val codeLineNumber = CodeUtils.getLineNumberForPos(code, pos)
                usageHashMap["${correspondingNode.fullName.hashCode().toString()}${codeLineNumber.toString()}"] = hashMapOf(
                    "fullName" to correspondingNode.fullName,
                    "className" to topUseClass.fullName,
                    "codeLineNumber" to codeLineNumber,
                    "codeLine" to line.trim()
                )
            }
        }
        return usageHashMap
    }

    fun handleGetImplementOfInterface(interfaceName: String): JiapResult {
        try {
            val interfaceClazz = decompiler.classesWithInners.find { 
                it.fullName == interfaceName 
            } ?: return JiapResult(success = false, data = hashMapOf("error" to "getImplementOfInterface: $interfaceName not found"))

            if (!interfaceClazz.accessInfo.isInterface) {
                return JiapResult(success = false, data = hashMapOf("error" to "getImplementOfInterface: $interfaceName is not a interface"))
            }

            val implementingClasses = decompiler.classesWithInners.filter{
                it.smali.contains(".implements ${interfaceClazz.fullName.replace(".", "/")}")
            }
            val result = hashMapOf(
                "type" to "list"
                "classes-list" to implementingClasses.map { it.fullName }
            )
            return JiapResult(success = true, data = result)            
        } catch (e: Exception) {
            logger.error("JIAP Error: get implement of interface", e)
            return JiapResult(success = false, data = hashMapOf("error" to "getImplementOfInterface: ${e.message}"))
        }
    }

    fun handleGetSubclasses(className: String): JiapResult {
        try {
            val clazz = decompiler.classesWithInners.find { 
                it.fullName == className 
            } ?: return JiapResult(success = false, data = hashMapOf("error" to "getSubclasses: $className not found"))
            val subClasses = decompiler.classesWithInners.filter {
                it.smali.contains(".super ${clazz.fullName.replace(".", "/")}")
            }
            val result = hashMapOf(
                "type" to "list",
                "classes-list" to subClasses.map { it.fullName }
            )
            return JiapResult(success = true, data = result)            
        } catch (e: Exception) {
            logger.error("JIAP Error: get implement of interface", e)
            return JiapResult(success = false, data = hashMapOf("error" to "getImplementOfInterface: ${e.message}"))
        }
    }

    // UI Mode
    fun handleGetSelectedText(): JiapResult {
        try {
            val mainWindow = pluginContext.guiContext?.mainFrame
            if (mainWindow !is MainWindow) {
                return JiapResult(success = false, data = hashMapOf("error" to "GetSelectedText: Not Gui Mode"))
            }

            val selectedComponent = mainWindow.tabbedPane?.selectedComponent
                ?: return JiapResult(success = false, data = hashMapOf("error" to "GetSelectedText: No selected component"))

            val textArea = findTextArea(selectedComponent)
            val selectedText = textArea?.selectedText

            val result = hashMapOf<String, Any>(
                "type" to "code",
                "selectedText" to (selectedText ?: "")
            )
            return JiapResult(success = true, data = result)
        } catch (e: Exception) {
            logger.error("JIAP Error: get selected text", e)
            return JiapResult(success = false, data = hashMapOf("error" to "GetSelectedText: ${e.message}"))
        }
    }

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
}