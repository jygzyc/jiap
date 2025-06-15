package me.yvesz.server.controller

import me.yvesz.server.service.JadxCommonService
import me.yvesz.server.service.AndroidFrameworkService
import me.yvesz.server.service.AndroidAppService
import me.yvesz.server.model.JadxResult
import me.yvesz.server.utils.ErrorMessages
import org.slf4j.LoggerFactory
import org.springframework.http.HttpStatus
import org.springframework.http.MediaType
import org.springframework.http.ResponseEntity
import org.springframework.web.bind.annotation.*
import org.springframework.validation.annotation.Validated

/**
 * RESTful API controller for JADX decompiler operations.
 * Provides endpoints for class and method analysis, source code retrieval, and cross-references.
 */
@RestController
@RequestMapping("/api/jadx")
@Validated
class JadxController(
    private val jadxCommonService: JadxCommonService,
    private val androidFrameworkService: AndroidFrameworkService,
    private val androidAppService: AndroidAppService
) {
    companion object {
        private val log = LoggerFactory.getLogger(JadxController::class.java)
    }

    /**
     * Removes JADX decompiler instance and releases memory.
     */
    @PostMapping("/remove_decompiler")
    fun removeDecompiler(@RequestBody payload: Map<String, Any>): ResponseEntity<Any> {
        val fileId = extractStringParam(payload, "id")
            ?: return createBadRequestResponse(ErrorMessages.MISSING_FILEID_PARAM)
        
        log.info("Request received: removeDecompiler for fileId: {}", fileId)
        return handleServiceResult(jadxCommonService.handleRemoveDecompiler(fileId))
    }

    /**
     * Removes all JADX decompiler instances and releases memory.
     */
    @PostMapping("/remove_all_decompilers")
    fun removeAllDecompilers(@RequestBody(required = false) payload: Map<String, Any>): ResponseEntity<Any> {
        log.info("Request received: removeAllDecompilers")
        return handleServiceResult(jadxCommonService.handleRemoveAllDecompilers())
    }

    /**
     * Retrieves all available classes from the decompiled APK.
     */
    @PostMapping("/get_all_classes")
    fun getAllClasses(@RequestBody payload: Map<String, Any>): ResponseEntity<Any> {
        val fileId = extractStringParam(payload, "id")
            ?: return createBadRequestResponse(ErrorMessages.MISSING_FILEID_PARAM)
        log.info("Request received: getAllClasses ")
        return handleServiceResult(jadxCommonService.handleGetAllClasses(fileId))
    }

    /**
     * Searches for a specific class by its full name.
     */
    @PostMapping("/search_class_by_name")
    fun searchClassByName(@RequestBody payload: Map<String, Any>): ResponseEntity<Any> {
        val fileId = extractStringParam(payload, "id")
            ?: return createBadRequestResponse(ErrorMessages.MISSING_FILEID_PARAM)
        val classFullName = extractStringParam(payload, "class")
            ?: return createBadRequestResponse(ErrorMessages.MISSING_CLASS_PARAM)
        
        log.info("Request received: searchClassByName for class '{}'", classFullName)
        return handleServiceResult(jadxCommonService.handleSearchClassByName(fileId, classFullName))
    }

    /**
     * Searches for methods by name across all classes.
     */
    @PostMapping("/search_method_by_name")
    fun searchMethodByName(@RequestBody payload: Map<String, Any>): ResponseEntity<Any> {
        val fileId = extractStringParam(payload, "id")
            ?: return createBadRequestResponse(ErrorMessages.MISSING_FILEID_PARAM)
        val methodName = extractStringParam(payload, "method")
            ?: return createBadRequestResponse(ErrorMessages.MISSING_METHOD_PARAM)
        
        log.info("Request received: searchMethodByName for method '{}'", methodName)
        return handleServiceResult(jadxCommonService.handleSearchMethodByName(fileId, methodName))
    }

    /**
     * Lists all methods within a specific class.
     */
    @PostMapping("/list_methods_of_class")
    fun listMethodsOfClass(@RequestBody payload: Map<String, Any>): ResponseEntity<Any> {
        val fileId = extractStringParam(payload, "id")
            ?: return createBadRequestResponse(ErrorMessages.MISSING_FILEID_PARAM)
        val className = extractStringParam(payload, "class")
            ?: return createBadRequestResponse(ErrorMessages.MISSING_CLASS_PARAM)
        
        log.info("Request received: listMethodsOfClass for class '{}'", className)
        return handleServiceResult(jadxCommonService.handleListMethodsOfClass(fileId, className))
    }

    /**
     * Lists all fields within a specific class.
     */
    @PostMapping("/list_fields_of_class")
    fun listFieldsOfClass(@RequestBody payload: Map<String, Any>): ResponseEntity<Any> {
        val fileId = extractStringParam(payload, "id")
            ?: return createBadRequestResponse(ErrorMessages.MISSING_FILEID_PARAM)
        val className = extractStringParam(payload, "class")
            ?: return createBadRequestResponse(ErrorMessages.MISSING_CLASS_PARAM)
        
        log.info("Request received: listFieldsOfClass for class '{}'", className)
        return handleServiceResult(jadxCommonService.handleListFieldsOfClass(fileId, className))
    }

    /**
     * Retrieves the source code of a specific class.
     */
    @PostMapping("/get_class_source")
    fun getClassSource(@RequestBody payload: Map<String, Any>): ResponseEntity<Any> {
        val fileId = extractStringParam(payload, "id")
            ?: return createBadRequestResponse(ErrorMessages.MISSING_FILEID_PARAM)

        val className = extractStringParam(payload, "class")
            ?: return createBadRequestResponse(ErrorMessages.MISSING_CLASS_PARAM)
        
        val isSmali = extractBooleanParam(payload, "smali")
            ?: return createBadRequestResponse(ErrorMessages.INVALID_SMALI_PARAM)
        
        log.info("Request received: getClassSource for class '{}', smali={}", className, isSmali)
        return handleServiceResult(jadxCommonService.handleGetClassSource(fileId, className, isSmali))
    }

    /**
     * Retrieves the source code of a specific method.
     */
    @PostMapping("/get_method_source")
    fun getMethodSource(@RequestBody payload: Map<String, Any>): ResponseEntity<Any> {
        val fileId = extractStringParam(payload, "id")
            ?: return createBadRequestResponse(ErrorMessages.MISSING_FILEID_PARAM)
        val methodInfo = extractStringParam(payload, "method")
            ?: return createBadRequestResponse(ErrorMessages.MISSING_METHOD_PARAM)
        
        val isSmali = extractBooleanParam(payload, "smali")
            ?: return createBadRequestResponse(ErrorMessages.INVALID_SMALI_PARAM)
        
        log.info("Request received: getMethodSource for method '{}', smali={}", methodInfo, isSmali)
        return handleServiceResult(jadxCommonService.handleGetMethodSource(fileId, methodInfo, isSmali))
    }

    /**
     * Retrieves cross-references (usage locations) for a specific method.
     */
    @PostMapping("/get_method_xref")
    fun getMethodXref(@RequestBody payload: Map<String, Any>): ResponseEntity<Any> {
        val fileId = extractStringParam(payload, "id")
            ?: return createBadRequestResponse(ErrorMessages.MISSING_FILEID_PARAM)
        val methodInfo = extractStringParam(payload, "method")
            ?: return createBadRequestResponse(ErrorMessages.MISSING_METHOD_PARAM)
        
        log.info("Request received: getMethodXref for method '{}'", methodInfo)
        return handleServiceResult(jadxCommonService.handleGetMethodXref(fileId, methodInfo))
    }

    /**
     * Retrieves cross-references (usage locations) for a specific class.
     */
    @PostMapping("/get_class_xref")
    fun getClassXref(@RequestBody payload: Map<String, Any>): ResponseEntity<Any> {
        val fileId = extractStringParam(payload, "id")
            ?: return createBadRequestResponse(ErrorMessages.MISSING_FILEID_PARAM)
        val className = extractStringParam(payload, "class")
            ?: return createBadRequestResponse(ErrorMessages.MISSING_CLASS_PARAM)
        
        log.info("Request received: getClassXref for class '{}'", className)
        return handleServiceResult(jadxCommonService.handleGetClassXref(fileId, className))
    }

    /**
     * Retrieves cross-references (usage locations) for a specific field.
     */
    @PostMapping("/get_field_xref")
    fun getFieldXref(@RequestBody payload: Map<String, Any>): ResponseEntity<Any> {
        val fileId = extractStringParam(payload, "id")
            ?: return createBadRequestResponse(ErrorMessages.MISSING_FILEID_PARAM)
        val fieldInfo = extractStringParam(payload, "field")
            ?: return createBadRequestResponse(ErrorMessages.MISSING_FIELD_PARAM)
        
        log.info("Request received: getFieldXref for field '{}'", fieldInfo)
        return handleServiceResult(jadxCommonService.handleGetFieldXref(fileId, fieldInfo))
    }

    /**
     * Retrieves implementing for a specific interface.
     */
    @PostMapping("/get_interface_impl")
    fun getImplementOfInterface(@RequestBody payload: Map<String, Any>): ResponseEntity<Any> {
        val fileId = extractStringParam(payload, "id")
            ?: return createBadRequestResponse(ErrorMessages.MISSING_FILEID_PARAM)
        val interfaceName = extractStringParam(payload, "interface")
            ?: return createBadRequestResponse(ErrorMessages.MISSING_INTERFACE_PARAM)

        log.info("Request received: getImplementOfInterface for interface '{}'", interfaceName)
        return handleServiceResult(jadxCommonService.handleGetImplementOfInterface(fileId, interfaceName))
    }

    /**
     * Retrieve subclasses for a specific superclass.
     */
    @PostMapping("/get_subclasses")
    fun getSubclasses(@RequestBody payload: Map<String, Any>): ResponseEntity<Any> {
        val fileId = extractStringParam(payload, "id")
            ?: return createBadRequestResponse(ErrorMessages.MISSING_FILEID_PARAM)
        val className = extractStringParam(payload, "class")
            ?: return createBadRequestResponse(ErrorMessages.MISSING_CLASS_PARAM)

        log.info("Request received: getSubclasses for class '{}'", className)
        return handleServiceResult(jadxCommonService.handleGetSubclasses(fileId, className))
    }

    /**
     * Retrieve subclasses for a specific superclass.
     */
    @PostMapping("/get_system_service_impl")
    fun getSystemServiceImpl(@RequestBody payload: Map<String, Any>): ResponseEntity<Any> {
        val fileId = extractStringParam(payload, "id")
            ?: return createBadRequestResponse(ErrorMessages.MISSING_FILEID_PARAM)
        val className = extractStringParam(payload, "class")
            ?: return createBadRequestResponse(ErrorMessages.MISSING_CLASS_PARAM)

        log.info("Request received: getSystemServiceImpl for class '{}'", className)
        return handleServiceResult(androidFrameworkService.handleGetSystemServiceImpl(fileId, className))
    }

    /**
     * Retrieve AndroidManifest for an App.
     */
    @PostMapping("/get_app_manifest")
    fun getAppManifest(@RequestBody payload: Map<String, Any>): ResponseEntity<Any> {
        val fileId = extractStringParam(payload, "id")
            ?: return createBadRequestResponse(ErrorMessages.MISSING_FILEID_PARAM)
        log.info("Request received: getAppManifest")
        return handleServiceResult(androidAppService.handleGetAppManifest(fileId))
    }

    /**
     * Test endpoint for debugging and validation purposes.
     * Verifies that the JADX decompiler service is working correctly.
     */
    @PostMapping("/test")
    fun test(@RequestBody(required = false) payload: Map<String, Any>): ResponseEntity<Any> {
        log.debug("Request received: test endpoint")
        return handleServiceResult(jadxCommonService.handleTest())
    }

    // Helper methods

    /**
     * Safely extracts a string parameter from the request payload.
     */
    private fun extractStringParam(payload: Map<String, Any>, key: String): String? {
        return payload[key] as? String
    }

    /**
     * Safely extracts a boolean parameter from the request payload.
     */
    private fun extractBooleanParam(payload: Map<String, Any>, key: String): Boolean? {
        return payload[key] as? Boolean
    }

    /**
     * Creates a standardized bad request response.
     */
    private fun createBadRequestResponse(message: String): ResponseEntity<Any> {
        log.warn("Bad request: {}", message)
        return ResponseEntity.status(HttpStatus.BAD_REQUEST).body(mapOf("error" to message))
    }

    /**
     * Handles service results and converts them to appropriate HTTP responses.
     */
    private fun handleServiceResult(result: JadxResult<*>): ResponseEntity<Any> {
        return when (result) {
            is JadxResult.Success -> {
                log.debug("Operation completed successfully")
                ResponseEntity.ok()
                    .contentType(MediaType.APPLICATION_JSON)
                    .body(result.data)
            }
            is JadxResult.Error -> {
                log.error("Operation failed: {}", result.error, result.cause)
                ResponseEntity.status(HttpStatus.INTERNAL_SERVER_ERROR)
                    .contentType(MediaType.APPLICATION_JSON)
                    .body(mapOf("error" to result.error))
            }
        }
    }
}
