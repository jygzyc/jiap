package me.yvesz.server.controller

import me.yvesz.server.service.JadxDecompilerManager
import me.yvesz.server.utils.FileUtils
import org.slf4j.LoggerFactory
import org.springframework.beans.factory.annotation.Autowired
import org.springframework.http.HttpStatus
import org.springframework.http.ResponseEntity
import org.springframework.stereotype.Controller
import org.springframework.ui.Model
import org.springframework.web.bind.annotation.*
import org.springframework.web.multipart.MultipartFile
import java.io.File
import java.io.FileOutputStream
import java.net.URL
import java.nio.file.Files
import java.util.*

@Controller
class WebController {
    @Autowired
    private lateinit var jadxDecompilerManager: JadxDecompilerManager
    
    private val logger = LoggerFactory.getLogger(WebController::class.java)
    private val uploadDir = "uploads"
    private val outputDir = "jadx_output"
    
    // File name mapping to store fileId to original filename mapping
    private val fileNameMapping = mutableMapOf<String, String>()

    init {
        // Ensure upload and output directories exist
        FileUtils.createDirectoryIfNotExists(uploadDir)
        FileUtils.createDirectoryIfNotExists(outputDir)
    }

    @GetMapping("/")
    fun index(model: Model): String {
        model.addAttribute("title", "JIAP - Java Intelligent Analysis Platform")
        return "index"
    }

    @PostMapping("/api/file/local_handle")
    @ResponseBody
    fun uploadFile(@RequestParam("file") file: MultipartFile): ResponseEntity<Map<String, Any>> {
        return try {
            if (file.isEmpty) {
                return createErrorResponse(HttpStatus.BAD_REQUEST, "File cannot be empty")
            }

            val originalFileName = file.originalFilename ?: "unknown"
            
            // Create temporary file for validation
            val tempFile = File.createTempFile("upload_", "_${originalFileName}")
            file.transferTo(tempFile)
            
            // Use FileUtils for file validation
            val validationResult = FileUtils.validateFile(tempFile.absolutePath)
            if (!validationResult.isValid) {
                tempFile.delete() // Clean up temporary file
                return createErrorResponse(HttpStatus.BAD_REQUEST, validationResult.message)
            }
            
            // Generate file identifier
            val fileId = FileUtils.getFileIdentifier(tempFile)
            
            // Check if file with same fileId already exists in upload directory
            val targetFile = if (!FileUtils.fileExistsWithId(uploadDir, fileId)) {
                // File doesn't exist, copy to upload directory
                val copyResult = FileUtils.copyFileToDirectory(tempFile, uploadDir, fileId)
                tempFile.delete() // Clean up temporary file
                if (!copyResult.success) {
                    return createErrorResponse(HttpStatus.INTERNAL_SERVER_ERROR, 
                        "Failed to copy file to upload directory: ${copyResult.message}")
                }
                File(copyResult.targetPath!!)
            } else {
                // File already exists, use existing file
                tempFile.delete() // Clean up temporary file
                logger.info("File with id {} already exists in upload directory, using existing file", fileId)
                // Find existing file
                val uploadDirFile = File(uploadDir)
                uploadDirFile.listFiles()?.find { it.name.startsWith(fileId) } ?: tempFile
            }

            val (resultFileId, initResult) = initializeDecompiler(targetFile)
            
            return if (resultFileId != null) {
                fileNameMapping[resultFileId] = originalFileName
                
                ResponseEntity.ok(mapOf(
                    "success" to true,
                    "message" to "File uploaded and decompiler initialized successfully",
                    "fileId" to resultFileId,
                    "originalFileName" to originalFileName,
                    "filePath" to targetFile.absolutePath
                ))
            } else {
                createErrorResponse(HttpStatus.INTERNAL_SERVER_ERROR, "Failed to initialize decompiler: $initResult")
            }
        } catch (e: Exception) {
            logger.error("File upload failed", e)
            createErrorResponse(HttpStatus.INTERNAL_SERVER_ERROR, "File upload failed: ${e.message}")
        }
    }

    @PostMapping("/api/file/remote_handle")
    @ResponseBody
    fun processUrl(@RequestBody request: Map<String, String>): ResponseEntity<Map<String, Any>> {
        val url = request["url"]
        if (url.isNullOrBlank()) {
            return createErrorResponse(HttpStatus.BAD_REQUEST, "URL cannot be empty")
        }

        return try {
            // Use FileUtils to process URL
            val processResult = FileUtils.processFilePath(url)
            if (!processResult.success || processResult.file == null) {
                return createErrorResponse(HttpStatus.BAD_REQUEST, processResult.message)
            }
            
            val localFile = processResult.file
            val originalFileName = processResult.originalFileName
            val protocol = processResult.protocol
            
            // Use FileUtils for file validation
            val validationResult = FileUtils.validateFile(localFile.absolutePath)
            if (!validationResult.isValid) {
                return createErrorResponse(HttpStatus.BAD_REQUEST, validationResult.message)
            }
            
            // Generate file identifier
            val fileId = FileUtils.getFileIdentifier(localFile)
            
            // Check if file with same fileId already exists in upload directory
            val targetFile = if (!FileUtils.fileExistsWithId(uploadDir, fileId)) {
                // File doesn't exist, copy to upload directory
                val copyResult = FileUtils.copyFileToDirectory(localFile, uploadDir, fileId)
                if (!copyResult.success) {
                    return createErrorResponse(HttpStatus.INTERNAL_SERVER_ERROR, 
                        "Failed to copy file to upload directory: ${copyResult.message}")
                }
                File(copyResult.targetPath!!)
            } else {
                // File already exists, use existing file
                logger.info("File with id {} already exists in upload directory, using existing file", fileId)
                // Find existing file
                val uploadDirFile = File(uploadDir)
                uploadDirFile.listFiles()?.find { it.name.startsWith(fileId) } ?: localFile
            }

            val (resultFileId, initResult) = initializeDecompiler(targetFile)
            
            return if (resultFileId != null) {
                fileNameMapping[resultFileId] = originalFileName
                
                ResponseEntity.ok(mapOf(
                    "success" to true,
                    "message" to "URL file downloaded and decompiler initialized successfully",
                    "fileId" to resultFileId,
                    "originalFileName" to originalFileName,
                    "originalUrl" to url,
                    "filePath" to targetFile.absolutePath,
                    "protocol" to protocol
                ))
            } else {
                createErrorResponse(HttpStatus.INTERNAL_SERVER_ERROR, "Failed to initialize decompiler: $initResult")
            }
        } catch (e: Exception) {
            logger.error("URL processing failed: $url", e)
            createErrorResponse(HttpStatus.INTERNAL_SERVER_ERROR, "File download failed: ${e.message}")
        }
    }

    /**
     * Get file list
     */
    @GetMapping("/api/file/list")
    @ResponseBody
    fun getFileList(): ResponseEntity<Map<String, Any>> {
        return try {
            val fileIds = jadxDecompilerManager.getInitializedFileIds()
            val fileList = fileIds.map { fileId ->
                mapOf(
                    "fileId" to fileId,
                    "fileName" to (fileNameMapping[fileId] ?: "unknown_${fileId.take(8)}")
                )
            }
            
            ResponseEntity.ok(mapOf(
                "success" to true,
                "files" to fileList,
                "count" to fileList.size
            ))
        } catch (e: Exception) {
            logger.error("Failed to get file list: ${e.message}", e)
            createErrorResponse(HttpStatus.INTERNAL_SERVER_ERROR, "Failed to get file list: ${e.message}")
        }
    }
    
    /**
     * Delete file interface
     */
    @DeleteMapping("/api/file/delete")
    @ResponseBody
    fun deleteFile(@RequestBody request: Map<String, String>): ResponseEntity<Map<String, Any>> {
        val fileId = request["id"]
        if (fileId.isNullOrBlank()) {
            return createErrorResponse(HttpStatus.BAD_REQUEST, "fileId parameter is required")
        }

        return try {
            val instanceRemoved = jadxDecompilerManager.removeDecompilerInstance(fileId)
            val fileDeleted = FileUtils.deleteFilesByPrefix(uploadDir, fileId)
            fileNameMapping.remove(fileId)
            
            if (instanceRemoved) {
                ResponseEntity.ok(mapOf(
                    "success" to true,
                    "message" to "File and decompiler instance deleted successfully",
                    "fileId" to fileId,
                    "fileDeleted" to fileDeleted
                ))
            } else {
                createErrorResponse(HttpStatus.NOT_FOUND, "FileId not found: $fileId")
            }
        } catch (e: Exception) {
            logger.error("Failed to delete file: ${e.message}", e)
            createErrorResponse(HttpStatus.INTERNAL_SERVER_ERROR, "Failed to delete file: ${e.message}")
        }
    }

    private fun initializeDecompiler(file: File): Pair<String?, String> {
        return jadxDecompilerManager.initJadxDecompiler(
            file.absolutePath,
            outputDir,
            true
        )
    }
    
    private fun createErrorResponse(status: HttpStatus, message: String): ResponseEntity<Map<String, Any>> {
        return ResponseEntity.status(status).body(mapOf(
            "success" to false,
            "message" to message
        ))
    }
}
