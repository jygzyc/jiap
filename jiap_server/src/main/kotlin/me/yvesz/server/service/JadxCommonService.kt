package me.yvesz.server.service

import com.google.gson.Gson
import com.google.gson.JsonArray
import com.google.gson.JsonElement
import com.google.gson.JsonObject
import jadx.api.JadxDecompiler
import jadx.api.JavaClass
import jadx.api.JavaMethod
import jadx.api.JavaNode
import jadx.core.dex.visitors.prepare.CollectConstValues
import me.yvesz.server.model.JadxResult
import me.yvesz.server.utils.CodeUtils
import org.springframework.beans.factory.annotation.Autowired
import org.springframework.stereotype.Service
import org.slf4j.LoggerFactory

/**
 * Service class providing common JADX decompiler operations.
 * Handles class and method analysis, source code retrieval, and cross-reference operations.
 */
@Service
class JadxCommonService @Autowired constructor(
    private val jadxDecompilerManager: JadxDecompilerManager
) {
    
    companion object {
        private val log = LoggerFactory.getLogger(JadxCommonService::class.java)
    }
    
    private var decompilerInstance: JadxDecompiler? = null

    private fun getDecompilerInstanceOrError(fileId: String): JadxResult.Error? {
        return if (jadxDecompilerManager.isDecompilerInit(fileId)) {
            decompilerInstance = jadxDecompilerManager.getDecompilerInstance(fileId)
            null
        } else {
            JadxResult.Error("JADX decompiler not initialized for fileId: $fileId")
        }
    }

    fun handleRemoveAllDecompilers(): JadxResult<String> {
        return try {
            jadxDecompilerManager.clearAllInstances()
            JadxResult.Success(Gson().toJson(mapOf(
                "message" to "All decompiler instances removed successfully"
            )))
        } catch (e: Exception) {
            JadxResult.Error("Error removing all decompiler instances: ${e.message}", e)
        }
    }

    /**
     * Remove JADX decompiler instance and release memory.
     * @param fileId The file identifier for the decompiler instance to remove
     * @return JadxResult indicating success or failure
     */
    fun handleRemoveDecompiler(fileId: String): JadxResult<String> {
        return try {
            val removed = jadxDecompilerManager.removeDecompilerInstance(fileId)
            if (removed) {
                 // Cache is automatically cleared in removeDecompilerInstance
                 JadxResult.Success(Gson().toJson(mapOf(
                     "fileId" to fileId,
                     "message" to "Decompiler instance removed successfully"
                 )))
             } else {
                JadxResult.Error("Failed to remove decompiler instance for fileId: $fileId")
            }
        } catch (e: Exception) {
            JadxResult.Error("Error removing decompiler instance: ${e.message}", e)
        }
    }

    fun handleGetAllClasses(fileId: String): JadxResult<String> {
        getDecompilerInstanceOrError(fileId)?.let { return it }
        return try {
            val classNames = decompilerInstance!!.classesWithInners.map { it.fullName }
            JadxResult.Success(Gson().toJson(classNames))
        } catch (e: Exception) {
            JadxResult.Error("Error handleGetAllClasses: ${e.message}", e)
        }
    }

    fun handleSearchClassByName(fileId: String, classFullName: String): JadxResult<String> {
        getDecompilerInstanceOrError(fileId)?.let { return it }
        return try {
            val clazz = decompilerInstance!!.searchJavaClassOrItsParentByOrigFullName(classFullName)
            if (clazz != null) {
                // Cache the class for future reference
                val cacheInstance = jadxDecompilerManager.getCacheInstance(fileId)
                cacheInstance?.setClassFullNameToAliasMap(classFullName, clazz)
                val result = JsonObject().apply {
                    addProperty("class", clazz.toString())
                    add("methods", JsonArray().apply { 
                        clazz.methods.forEach { method -> add(method.toString()) } 
                    })
                    add("fields", JsonArray().apply { 
                        clazz.fields.forEach { field -> add(field.toString()) } 
                    })
                }
                JadxResult.Success(Gson().toJson(result))
            } else {
                JadxResult.Error("Class not found: $classFullName")
            }
        } catch (e: Exception) {
            JadxResult.Error("Error handleSearchClassByName: ${e.message}", e)
        }
    }

    fun handleSearchMethodByName(fileId: String, methodName: String): JadxResult<String> {
        getDecompilerInstanceOrError(fileId)?.let { return it }
        return try {
            val methods = decompilerInstance!!.classesWithInners.flatMap { clazz ->
                clazz.methods.filter {
                    it.name.lowercase().contains(methodName.lowercase())
                }
            }
            if (methods.isNotEmpty()){
                val cacheInstance = jadxDecompilerManager.getCacheInstance(fileId)
                methods.forEach {
                    cacheInstance?.setMethodToClassMap(it.toString(), it.declaringClass)
                    cacheInstance?.setXrefMap(it.toString(), it.useIn)
                }
                JadxResult.Success(Gson().toJson(methods.map { it.toString() }))
            }else{
                JadxResult.Error("Method not found: $methodName")
            }
        }catch (e: Exception) {
            JadxResult.Error("Error handleSearchMethodByName: ${e.message}", e)
        }
    }

    fun handleListMethodsOfClass(fileId: String, className: String): JadxResult<String> {
        getDecompilerInstanceOrError(fileId)?.let { return it }
        return try {
            val cacheInstance = jadxDecompilerManager.getCacheInstance(fileId)
                ?: return JadxResult.Error("No cache instance found for file ID: $fileId")
            val clazz = cacheInstance.getClassOfAliasName(className) ?:
            decompilerInstance!!.classesWithInners.find {
                it.fullName == className
            } ?: return JadxResult.Error("Class not found: $className")
            return JadxResult.Success(Gson().toJson(clazz.methods.map {it.toString()}))
        }catch (e: Exception){
            JadxResult.Error("Error handleListMethodsOfClass: ${e.message}", e)
        }
    }

    fun handleListFieldsOfClass(fileId: String, className: String): JadxResult<String> {
        getDecompilerInstanceOrError(fileId)?.let { return it }
        return try {
            val cacheInstance = jadxDecompilerManager.getCacheInstance(fileId)
                ?: return JadxResult.Error("No cache instance found for file ID: $fileId")
            val clazz = cacheInstance.getClassOfAliasName(className) ?:
            decompilerInstance!!.classesWithInners.find {
                it.fullName == className
            } ?: return JadxResult.Error("Class not found: $className")
            return JadxResult.Success(Gson().toJson(clazz.fields.map {it.toString()}))
        }catch (e: Exception){
            JadxResult.Error("Error handleListFieldsOfClas: ${e.message}", e)
        }
    }

    fun handleGetClassSource(fileId: String, className: String, isSmali: Boolean): JadxResult<String> {
        getDecompilerInstanceOrError(fileId)?.let { return it }
        return try {
            val cacheInstance = jadxDecompilerManager.getCacheInstance(fileId)
                ?: return JadxResult.Error("No cache instance found for file ID: $fileId")
            
            val clazz = cacheInstance.getClassOfAliasName(className) ?:
            decompilerInstance!!.classesWithInners.find {
                it.fullName == className
            } ?: return JadxResult.Error("Class not found: $className")
            val code = if (isSmali) {
                clazz.smali
            }else{
                clazz.code
            }
            JadxResult.Success(Gson().toJson(code))
        }catch (e: Exception) {
            JadxResult.Error("Error handleGetClassSource: ${e.message}", e)
        }
    }

    fun handleGetMethodSource(fileId: String, methodInfo: String, isSmali: Boolean): JadxResult<String> {
        getDecompilerInstanceOrError(fileId)?.let { return it }
        
        return try {
            val cacheInstance = jadxDecompilerManager.getCacheInstance(fileId)
                ?: return JadxResult.Error("No cache instance found for file ID: $fileId")
            val cachedMethod = cacheInstance.getClassOfMethodInfo(methodInfo)
                ?.methods
                ?.find { it.toString() == methodInfo }
            val (method, declaringClass) = when {
                cachedMethod != null -> cachedMethod to cachedMethod.declaringClass
                else -> findMethodInAllClasses(methodInfo)
            } ?: return JadxResult.Error("Method not found: $methodInfo")
            
            cacheInstance.setMethodToClassMap(methodInfo, declaringClass)
            
            val code = if (isSmali) {
                CodeUtils.extractMethodSmaliCode(method, declaringClass)
            } else {
                method.codeStr
            }
            
            JadxResult.Success(Gson().toJson(code))
        } catch (e: Exception) {
            JadxResult.Error("Error handleGetMethodSource: ${e.message}", e)
        }
    }

    private fun findMethodInAllClasses(methodInfo: String): Pair<JavaMethod, JavaClass>? {
        decompilerInstance?.classesWithInners?.forEach { clazz ->
            clazz.methods.find { it.toString() == methodInfo }?.let { method ->
                return method to clazz
            }
        }
        return null
    }

    private fun processUsage(searchName:String, searchNode: JavaNode, xrefNodes: MutableList<JavaNode>): JsonObject {
        val result = JsonObject().apply {
            addProperty("name", searchName)
        }
        val usageArray = JsonArray()
        
        xrefNodes.groupBy(JavaNode::getTopParentClass).forEach classLoop@{ (topUseClass, nodesInClass) ->
            val codeInfo = topUseClass.codeInfo
            val usePositions = topUseClass.getUsePlacesFor(codeInfo, searchNode)
            if (usePositions.isEmpty()) {
                return@classLoop  // 继续处理下一个类，而不是直接返回
            }
            val code = codeInfo.codeStr
            usePositions.forEach positionLoop@{ pos ->
                val line = CodeUtils.getLineForPos(code, pos)
                if (line.startsWith("import ")) {
                    return@positionLoop
                }
                // 找到对应位置的xrefNode
                val correspondingNode = nodesInClass.firstOrNull() ?: nodesInClass.first()
                val usageNode = JsonObject().apply {
                    addProperty("fullName", correspondingNode.fullName)
                    addProperty("className", topUseClass.fullName)
                    addProperty("codeLineNumber", CodeUtils.getLineNumberForPos(code, pos))
                    addProperty("codeLine", line.trim())
                }
                usageArray.add(usageNode)
            }
        }
        
        result.add("usages", usageArray)
        result.addProperty("totalUsages", usageArray.size())
        return result
    }

    fun handleGetClassXref(fileId: String, className: String): JadxResult<String> {
        getDecompilerInstanceOrError(fileId)?.let { return it }
        return try {
            val cacheInstance = jadxDecompilerManager.getCacheInstance(fileId)
                ?: return JadxResult.Error("No cache instance found for file ID: $fileId")
            var xrefNodes = cacheInstance.getXrefNodes(className)
            val foundClass = cacheInstance.getClassOfAliasName(className)
                ?: decompilerInstance!!.classesWithInners.find { it.fullName == className }
                ?: return JadxResult.Error("Class not found: $className")
            if (xrefNodes == null) {
                xrefNodes = mutableListOf()
                foundClass.decompile()
                val classUseIn = foundClass.useIn
                if (classUseIn.isNotEmpty()) {
                    xrefNodes.addAll(classUseIn)
                }
                
                foundClass.methods.forEach { method ->
                    if (method.isConstructor) {
                        val constructorUseIn = method.useIn
                        if (constructorUseIn.isNotEmpty()) {
                            xrefNodes.addAll(constructorUseIn)
                        }
                    }
                }
                cacheInstance.setXrefMap(className, xrefNodes)
            }
            val result = processUsage(className, foundClass, xrefNodes)
            JadxResult.Success(Gson().toJson(result))
        } catch (e: Exception) {
            JadxResult.Error("Error handleGetClassXref: ${e.message}", e)
        }
    }

    fun handleGetMethodXref(fileId: String, methodInfo: String): JadxResult<String> {
        getDecompilerInstanceOrError(fileId)?.let { return it }
        return try {
            val cacheInstance = jadxDecompilerManager.getCacheInstance(fileId)
                ?: return JadxResult.Error("No cache instance found for file ID: $fileId")
            var xrefNodes = cacheInstance.getXrefNodes(methodInfo)
            val foundMethod = cacheInstance.getClassOfMethodInfo(methodInfo)?.methods?.find {
                    it.toString() == methodInfo
                } ?: decompilerInstance!!.classesWithInners.firstNotNullOfOrNull { clazz ->
                    clazz.methods.find { it.toString() == methodInfo }
            } ?: return JadxResult.Error("Method not found: $methodInfo")
            if (xrefNodes == null) {
                xrefNodes = mutableListOf()
                foundMethod.declaringClass.decompile()
                foundMethod.overrideRelatedMethods.forEach { relatedMethod ->
                    val relatedUseIn = relatedMethod.useIn
                    if (relatedUseIn.isNotEmpty()) {
                        xrefNodes.addAll(relatedUseIn)
                    }
                }
                xrefNodes.addAll(foundMethod.useIn)
                cacheInstance.setXrefMap(methodInfo, xrefNodes)
            }

            val result = processUsage(methodInfo, foundMethod, xrefNodes)
            JadxResult.Success(Gson().toJson(result))
        } catch (e: Exception) {
            JadxResult.Error("Error handleGetMethodXref: ${e.message}", e)
        }
    }

    fun handleGetFieldXref(fileId: String, fieldInfo: String): JadxResult<String> {
        getDecompilerInstanceOrError(fileId)?.let { return it }
        return try {
            val cacheInstance = jadxDecompilerManager.getCacheInstance(fileId)
                ?: return JadxResult.Error("No cache instance found for file ID: $fileId")
            var xrefNodes = cacheInstance.getXrefNodes(fieldInfo)
            val foundField = decompilerInstance!!.classesWithInners.firstNotNullOfOrNull { clazz ->
                clazz.fields.find { it.toString() == fieldInfo }
            } ?: return JadxResult.Error("Field not found: $fieldInfo")
            if (xrefNodes == null) {
                foundField.declaringClass.decompile()
                xrefNodes = mutableListOf()
                val constField = CollectConstValues.getFieldConstValue(foundField.fieldNode) != null
                if (constField and !foundField.fieldNode.accessFlags.isPrivate){
                    xrefNodes.addAll(foundField.useIn)
                }
                xrefNodes.let { cacheInstance.setXrefMap(fieldInfo, it) }
            }
            val result = processUsage(fieldInfo, foundField, xrefNodes)
            JadxResult.Success(Gson().toJson(result))
        } catch (e: Exception) {
            JadxResult.Error("Error getting field xref: ${e.message}", e)
        }
    }

    fun handleGetImplementOfInterface(fileId: String, interfaceName: String): JadxResult<String> {
        getDecompilerInstanceOrError(fileId)?.let { return it }
        return try {
            val cacheInstance = jadxDecompilerManager.getCacheInstance(fileId)
                ?: return JadxResult.Error("No cache instance found for file ID: $fileId")
            val interfaceClazz = cacheInstance.getClassOfAliasName(interfaceName)
                ?: decompilerInstance!!.classesWithInners.find { it.fullName == interfaceName }
                ?: return JadxResult.Error("Interface not found: $interfaceName")

            if (!interfaceClazz.accessInfo.isInterface) {
                return JadxResult.Error("$interfaceName is not an interface")
            }

            cacheInstance.setClassFullNameToAliasMap(interfaceName, interfaceClazz)
            val implementingClasses = decompilerInstance!!.classesWithInners.filter{
                it.smali.contains(".implements ${interfaceClazz.fullName.replace(".", "/")}")
            }

            JadxResult.Success(Gson().toJson(implementingClasses.map {it.toString()}))
        } catch (e: Exception) {
            JadxResult.Error("Error handleGetImplOfInterface: ${e.message}", e)
        }
    }

    fun handleGetSubclasses(fileId: String, className: String): JadxResult<String> {
        getDecompilerInstanceOrError(fileId)?.let { return it }
        return try {
            val cacheInstance = jadxDecompilerManager.getCacheInstance(fileId)
                ?: return JadxResult.Error("No cache instance found for file ID: $fileId")
            val superClazz = cacheInstance.getClassOfAliasName(className)
                ?: decompilerInstance!!.classesWithInners.find { it.fullName == className }
                ?: return JadxResult.Error("superCla not found: $className")

            cacheInstance.setClassFullNameToAliasMap(className, superClazz)

            val subClasses = decompilerInstance!!.classesWithInners.filter{
                it.smali.contains(".super ${superClazz.fullName.replace(".", "/")}")
            }

            JadxResult.Success(Gson().toJson(subClasses.map {it.toString()}))
        } catch (e: Exception) {
            JadxResult.Error("Error handleGetSuperClass: ${e.message}", e)
        }
    }

    /**
     * Test function.
     *
     * @return A JadxResult.
     */
    fun handleTest(): JadxResult<String> {
        return try {
            JadxResult.Success("JADX decompiler service test successful")
        } catch (e: Exception) {
            JadxResult.Error("Error in test function: ${e.message}", e)
        }
    }
}