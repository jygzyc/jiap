package jadx.plugins.jiap.service

import org.slf4j.LoggerFactory

import jadx.gui.ui.MainWindow
import jadx.gui.JadxWrapper
import jadx.api.JadxDecompiler
import jadx.api.JavaClass
import jadx.api.JavaMethod
import jadx.api.plugins.JadxPluginContext

import jadx.plugins.jiap.model.JiapResult
import jadx.plugins.jiap.model.JiapServiceInterface
import jadx.plugins.jiap.utils.CodeUtils

class CommonService(override val pluginContext: JadxPluginContext) : JiapServiceInterface {
    
    companion object {
        private val logger = LoggerFactory.getLogger(CommonService::class.java)
    }
    val decompiler: JadxDecompiler = pluginContext.decompiler
    
    fun handleGetAllClasses(): JiapResult{
        try{
            val classes = decompiler.classesWithInners.map { it.fullName }
            val result = hashMapOf(
                "type" to "class-list",
                "count" to classes.size,
                "classes" to classes
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
                "type" to "class",
                "code" to code
            )
            return JiapResult(success = true, data = result)
        }catch (e: Exception){
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
            if (clazz == null || method == null){
                return JiapResult(success = false, data = hashMapOf("error" to "getMethodSource: $methodName not found"))
            }
            val code = if (isSmali) CodeUtils.extractMethodSmaliCode(clazz, method ) else method.codeStr
            val result: HashMap<String, Any> = hashMapOf(
                "type" to "method",
                "code" to code
            )
            return JiapResult(success = true, data = result)
        }catch (e: Exception){
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

    fun handleSearchClassByName(classFullName: String): JiapResult {
        return try {
            val clazz = decompiler.searchJavaClassOrItsParentByOrigFullName(classFullName)
            if (clazz != null) {
                val result = hashMapOf(
                    "type" to "class",
                    "methods" to clazz.methods.map { it.toString() },
                    "fields" to clazz.fields.map { it.toString() }
                )
                JiapResult(success = true, data = result)
            } else {
                JiapResult(success = false, data = hashMapOf("error" to "searchClassByName: $classFullName not found"))
            }
        } catch (e: Exception) {
            logger.error("JIAP Error: search class by name", e)
            JiapResult(success = false, data = hashMapOf("error" to "searchClassByName: ${e.message}"))
        }
    }

    fun handleListMethodsOfClass(className: String): JiapResult {
        try{
            return JiapResult(success = true, data = hashMapOf())
        }catch (e: Exception){
            logger.error("JIAP Error: list methods of class", e)
            return JiapResult(success = false, data = hashMapOf("error" to "listMethodsOfClass: ${e.message}"))
        }
    }

    // fun handleListFieldsOfClass(fileId: String, className: String): JadxResult<List<String>> {
    //     logger.info("Listing fields of class: $className")
    //     return JadxResult(success = true, data = emptyList())
    // }

    
    // fun handleSearchMethodByName(fileId: String, methodName: String): JadxResult<String> {
    //     logger.info("Searching method by name: $methodName")
    //     return JadxResult(success = true, data = "Method not found")
    // }
    
    // fun handleGetMethodXref(fileId: String, methodInfo: String): JadxResult<List<String>> {
    //     logger.info("Getting method xref for: $methodInfo")
    //     return JadxResult(success = true, data = emptyList())
    // }
    
    // fun handleGetClassXref(fileId: String, className: String): JadxResult<List<String>> {
    //     logger.info("Getting class xref for: $className")
    //     return JadxResult(success = true, data = emptyList())
    // }
    
    // fun handleGetFieldXref(fileId: String, fieldInfo: String): JadxResult<List<String>> {
    //     logger.info("Getting field xref for: $fieldInfo")
    //     return JadxResult(success = true, data = emptyList())
    // }
    
    // fun handleGetImplementOfInterface(fileId: String, interfaceName: String): JadxResult<List<String>> {
    //     logger.info("Getting interface implementations for: $interfaceName")
    //     return JadxResult(success = true, data = emptyList())
    // }
    
    // fun handleGetSubclasses(fileId: String, className: String): JadxResult<List<String>> {
    //     logger.info("Getting subclasses for: $className")
    //     return JadxResult(success = true, data = emptyList())
    // }
    

}