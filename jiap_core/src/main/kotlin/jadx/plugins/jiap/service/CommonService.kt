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
            return JiapResult(success = false, data = hashMapOf("error" to "handleGetAllClasses: ${e.message}"))
        }
    }

    fun handleGetClassInfo(cls: String): JiapResult {
        return try {
            val clazz = decompiler.searchJavaClassOrItsParentByOrigFullName(cls)
            if (clazz != null) {
                val result = hashMapOf(
                    "type" to "list",
                    "name" to clazz.fullName,
                    "methods-list" to clazz.methods.map { it.toString() },
                    "fields-list" to clazz.fields.map { it.toString() }
                )
                JiapResult(success = true, data = result)
            } else {
                JiapResult(success = false, data = hashMapOf("error" to "handleGetClassInfo: $cls not found"))
            }
        } catch (e: Exception) {
            JiapResult(success = false, data = hashMapOf("error" to "handleGetClassInfo: ${e.message}"))
        }
    }

    fun handleGetClassSource(cls: String, smali: Boolean): JiapResult {
        try {
            val clazz = decompiler.classesWithInners?.find {
                it.fullName == cls
            } ?: return JiapResult(
                success = false,
                data = hashMapOf("error" to "getClassSource: $cls not found")
            )
            clazz.decompile()
            val code = if (smali) clazz.smali else clazz.code
            val result: HashMap<String, Any> = hashMapOf(
                "type" to "code",
                "name" to clazz.fullName,
                "code" to code
            )
            return JiapResult(success = true, data = result)
        } catch (e: Exception) {
            return JiapResult(success = false, data = hashMapOf("error" to "getClassSource: ${e.message}"))
        }
    }

    fun handleSearchClassKey(key: String): JiapResult {
        try {
            if (key.isBlank()) {
                return JiapResult(
                    success = false,
                    data = hashMapOf("error" to "handleSearchClassKey: key cannot be empty")
                )
            }

            val lowerKeyword = key.lowercase()
            val classes = decompiler.classesWithInners?.parallelStream()
                ?.filter { clazz ->
                    clazz.code?.lowercase()?.contains(lowerKeyword) == true
                }
                ?.toList() ?: emptyList()

            val result = hashMapOf(
                "type" to "list",
                "count" to classes.size,
                "classes-list" to classes.map { it.fullName }
            )
            return JiapResult(success = true, data = result)
        } catch (e: Exception) {
            return JiapResult(success = false, data = hashMapOf("error" to "handleSearchClassKey: ${e.message}"))
        }
    }

    fun handleSearchMethod(mth: String): JiapResult {
        try {
            val lowerMethodName = mth.lowercase()
            val mths = decompiler.classesWithInners?.flatMap { clazz ->
                clazz.methods.filter { method ->
                    method.fullName.lowercase().contains(lowerMethodName)
                }
            } ?: emptyList()
            val result = hashMapOf(
                "type" to "list",
                "count" to mths.size,
                "methods-list" to mths.map { it.toString() }
            )
            return JiapResult(success = true, data = result)
        } catch (e: Exception) {
            return JiapResult(success = false, data = hashMapOf("error" to "handleSearchMethod: ${e.message}"))
        }
    }

    fun handleGetMethodSource(mth: String, smali: Boolean): JiapResult {
        try {
            val mthPair = CodeUtils.findMethod(decompiler, mth) ?: return JiapResult(
                success = false,
                data = hashMapOf("error" to "handleGetMethodSource: $mth not found")
            )
            val jcls = mthPair.first
            val jmth = mthPair.second
            jcls.decompile()

            val code = if (smali) CodeUtils.extractMethodSmaliCode(jcls, jmth) else jmth.codeStr
            val result: HashMap<String, Any> = hashMapOf(
                "type" to "code",
                "name" to jmth.toString(),
                "code" to code
            )
            return JiapResult(success = true, data = result)
        } catch (e: Exception) {
            return JiapResult(success = false, data = hashMapOf("error" to "handleGetMethodSource: ${e.message}"))
        }
    }

    fun handleGetMethodXref(mth: String): JiapResult {
        try {
            val mthPair = CodeUtils.findMethod(decompiler, mth)
                ?: return JiapResult(
                    success = false,
                    data = hashMapOf("error" to "handleGetMethodXref: $mth not found")
                )
            val jmth = mthPair.second
            val xrefMap = CodeUtils.buildUsageQuery(decompiler, jmth)
            val xrefNodes = xrefMap.values.flatten().toMutableList()
            val references = processUsage(jmth, xrefNodes)
            val result = hashMapOf<String, Any>(
                "type" to "list",
                "count" to xrefNodes.size,
                "references-list" to references
            )
            return JiapResult(success = true, data = result)
        } catch (e: Exception) {
            return JiapResult(success = false, data = hashMapOf("error" to "handleGetMethodXref: ${e.message}"))
        }
    }

    fun handleGetFieldXref(fld: String): JiapResult {
        try {
            val fldPair = CodeUtils.findField(decompiler, fld)
                ?: return JiapResult(
                    success = false,
                    data = hashMapOf("error" to "handleGetFieldXref: $fld not found")
                )
            val jfld = fldPair.second
            val xrefMap = CodeUtils.buildUsageQuery(decompiler, jfld)
            val xrefNodes = xrefMap.values.flatten().toMutableList()
            val references = processUsage(jfld, xrefNodes)
            val result = hashMapOf<String, Any>(
                "type" to "list",
                "count" to xrefNodes.size,
                "references-list" to references
            )
            return JiapResult(success = true, data = result)
        } catch (e: Exception) {
            return JiapResult(success = false, data = hashMapOf("error" to "handleGetFieldXref: ${e.message}"))
        }
    }

    fun handleGetClassXref(cls: String): JiapResult {
        try {
            val jclazz = decompiler.searchJavaClassOrItsParentByOrigFullName(cls)
                ?: return JiapResult(
                    success = false,
                    data = hashMapOf("error" to "handleGetClassXref: $cls not found")
                )
            val xrefMap = CodeUtils.buildUsageQuery(decompiler, jclazz)
            val xrefNodes = xrefMap.values.flatten().toMutableList()
            val references = processUsage(jclazz, xrefNodes)
            val result = hashMapOf<String, Any>(
                "type" to "list",
                "count" to xrefNodes.size,
                "references-list" to references
            )
            return JiapResult(success = true, data = result)
        } catch (e: Exception) {
            return JiapResult(success = false, data = hashMapOf("error" to "handleGetClassXref: ${e.message}"))
        }
    }

    fun handleGetImplementOfInterface(iface: String): JiapResult {
        return try {
            val interfaceClazz = decompiler.searchJavaClassOrItsParentByOrigFullName(iface)
                ?: return JiapResult(
                    success = false,
                    data = hashMapOf("error" to "handleGetImplementOfInterface: $iface not found")
                )
            val implementingClasses = decompiler.classesWithInners.filter {
                it.smali.contains(".super L${interfaceClazz.fullName.replace('.', '/')};")
            }
            val result = hashMapOf(
                "type" to "list",
                "classes-list" to implementingClasses.map { it.fullName }
            )
            JiapResult(success = true, data = result)
        } catch (e: Exception) {
            JiapResult(success = false, data = hashMapOf("error" to "handleGetImplementOfInterface: ${e.message}"))
        }
    }

    fun handleGetSubclasses(cls: String): JiapResult {
        return try {
            val clazz = decompiler.searchJavaClassOrItsParentByOrigFullName(cls)
                ?: return JiapResult(
                    success = false,
                    data = hashMapOf("error" to "handleGetSubclasses: $cls not found")
                )

            val subClasses = decompiler.classesWithInners.filter {
                it.smali.contains(".super L${clazz.fullName.replace(".", "/")};")
            }
            val result = hashMapOf(
                "type" to "list",
                "classes-list" to subClasses.map { it.fullName }
            )
            JiapResult(success = true, data = result)
        } catch (e: Exception) {
            JiapResult(success = false, data = hashMapOf("error" to "handleGetSubclasses: ${e.message}"))
        }
    }
}