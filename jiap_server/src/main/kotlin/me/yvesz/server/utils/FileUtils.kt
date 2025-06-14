package me.yvesz.server.utils

import java.io.File
import java.security.MessageDigest
import java.nio.file.Files
import java.nio.file.Paths
import java.nio.file.StandardCopyOption
import java.net.URL
import org.slf4j.LoggerFactory

/**
 * Utility class for file operations including validation, hashing, and file management.
 * 
 * This class provides essential file operations for the JADX server including
 * file validation, hash calculation, and file cleanup operations.
 */
class FileUtils {
    
    companion object {
        private val log = LoggerFactory.getLogger(FileUtils::class.java)
        private const val HASH_ALGORITHM = "SHA-256"
        
        const val MAX_FILE_SIZE = 500 * 1024 * 1024 // 500MB
        val SUPPORTED_EXTENSIONS = setOf("apk", "jar", "dex", "aar")

        /**
         * Generates a unique identifier for a file based on its SHA-256 hash.
         * 
         * @param file The file to generate identifier for
         * @return A truncated hash string (first 8 characters of each byte in hex)
         * @throws IllegalArgumentException if file doesn't exist or is a directory
         */
        fun getFileIdentifier(file: File): String {
            if (!file.exists() || file.isDirectory) {
                throw IllegalArgumentException("Invalid file: Path does not exist or is a directory.")
            }
            val digest = MessageDigest.getInstance(HASH_ALGORITHM)
            file.inputStream().use { stream ->
                val buffer = ByteArray(8192)
                var bytesRead: Int
                while (stream.read(buffer).also { bytesRead = it } != -1) {
                    digest.update(buffer, 0, bytesRead)
                }
            }
            return digest.digest().joinToString("") { "%02x".format(it) }.take(8)
        }
        
        /**
         * Validates if a file extension is supported.
         * 
         * @param extension The file extension to validate
         * @return True if the extension is supported, false otherwise
         */
        private fun isValidFileExtension(extension: String): Boolean {
            return SUPPORTED_EXTENSIONS.contains(extension.lowercase())
        }
        
        /**
         * Validates if a file size is within the allowed limit.
         * 
         * @param file The file to check
         * @return True if file size is within limit, false otherwise
         */
        private fun isValidFileSize(file: File): Boolean {
            return file.length() <= MAX_FILE_SIZE
        }
        
        /**
         * Extracts the file extension from a filename.
         * 
         * @param fileName The filename to extract extension from
         * @return The file extension without the dot, or empty string if no extension
         */
        private fun getFileExtension(fileName: String): String {
            return fileName.substringAfterLast(".", "")
        }
        
        /**
         * Deletes all files in a directory that start with the specified prefix.
         * 
         * @param directory The directory path to search in
         * @param fileId The prefix to match files against
         * @return True if at least one file was deleted, false otherwise
         */
        fun deleteFilesByPrefix(directory: String, fileId: String): Boolean {
            return try {
                val uploadPath = Paths.get(directory)
                if (!Files.exists(uploadPath)) {
                    log.warn("Directory does not exist: $directory")
                    return false
                }
                
                var fileDeleted = false
                Files.list(uploadPath).use { stream ->
                    val files = stream.filter { it.fileName.toString().startsWith(fileId) }.toList()
                    
                    files.forEach { filePath ->
                        try {
                            Files.delete(filePath)
                            fileDeleted = true
                            log.info("Deleted file: ${filePath.toAbsolutePath()}")
                        } catch (e: Exception) {
                            log.warn("Failed to delete file: ${filePath.toAbsolutePath()}", e)
                        }
                    }
                }
                fileDeleted
            } catch (e: Exception) {
                log.error("Error deleting files with prefix $fileId in directory $directory", e)
                false
            }
        }
        
        /**
         * Performs comprehensive validation on a file including existence, type, and size checks.
         * 
         * @param filePath The path of the file to validate
         * @return FileValidationResult containing validation status and message
         */
        fun validateFile(filePath: String): FileValidationResult {
            val file = File(filePath)
            
            if (!file.exists()) {
                return FileValidationResult(false, "File does not exist: $filePath")
            }
            
            if (!file.isFile) {
                return FileValidationResult(false, "Path is not a file: $filePath")
            }
            
            val extension = getFileExtension(file.name)
            if (!isValidFileExtension(extension)) {
                return FileValidationResult(false, 
                    "Unsupported file type. Supported types: ${SUPPORTED_EXTENSIONS.joinToString(", ")}")
            }
            
            if (!isValidFileSize(file)) {
                return FileValidationResult(false, 
                    "File size exceeds maximum limit of ${MAX_FILE_SIZE / (1024 * 1024)}MB")
            }
            
            return FileValidationResult(true, "File validation passed")
        }
        
        /**
         * Creates a directory if it doesn't exist.
         * 
         * @param directoryPath The path of the directory to create
         * @return True if directory was created or already exists, false if creation failed
         */
        fun createDirectoryIfNotExists(directoryPath: String): Boolean {
            return try {
                val directory = File(directoryPath)
                if (!directory.exists()) {
                    log.info("Creating directory: {}", directory.absolutePath)
                    directory.mkdirs()
                } else {
                    true
                }
            } catch (e: Exception) {
                log.error("Failed to create directory: $directoryPath", e)
                false
            }
        }
        
        /**
         * Creates a directory if it doesn't exist.
         * 
         * @param directory The File object representing the directory to create
         * @return True if directory was created or already exists, false if creation failed
         */
        fun createDirectoryIfNotExists(directory: File): Boolean {
            return try {
                if (!directory.exists()) {
                    log.info("Creating directory: {}", directory.absolutePath)
                    directory.mkdirs()
                } else {
                    true
                }
            } catch (e: Exception) {
                log.error("Failed to create directory: ${directory.absolutePath}", e)
                false
            }
        }
        
        /**
         * Checks if a file with the given fileId already exists in the specified directory.
         * 
         * @param directory The directory to search in
         * @param fileId The file identifier to check for
         * @return True if a file with the fileId prefix exists, false otherwise
         */
        fun fileExistsWithId(directory: String, fileId: String): Boolean {
            return try {
                val uploadPath = Paths.get(directory)
                if (!Files.exists(uploadPath)) {
                    return false
                }
                
                Files.list(uploadPath).use { stream ->
                    stream.anyMatch { it.fileName.toString().startsWith(fileId) }
                }
            } catch (e: Exception) {
                log.error("Error checking file existence with id $fileId in directory $directory", e)
                false
            }
        }
        
        /**
         * Copies a file to the specified directory with a new name format: fileId_originalFileName.
         * 
         * @param sourceFile The source file to copy
         * @param targetDirectory The target directory to copy to
         * @param fileId The file identifier to use as prefix
         * @return CopyResult containing success status and target file path
         */
        fun copyFileToDirectory(sourceFile: File, targetDirectory: String, fileId: String): CopyResult {
            return try {
                // Create target directory if it doesn't exist
                if (!createDirectoryIfNotExists(targetDirectory)) {
                    return CopyResult(false, null, "Failed to create target directory: $targetDirectory")
                }
                
                val targetFileName = "${fileId}_${sourceFile.name}"
                val targetFile = File(targetDirectory, targetFileName)
                
                // Copy the file
                Files.copy(sourceFile.toPath(), targetFile.toPath())
                
                log.info("File copied successfully: ${sourceFile.absolutePath} -> ${targetFile.absolutePath}")
                CopyResult(true, targetFile.absolutePath, "File copied successfully")
                
            } catch (e: Exception) {
                log.error("Error copying file ${sourceFile.absolutePath} to directory $targetDirectory", e)
                CopyResult(false, null, "Failed to copy file: ${e.message}")
            }
        }
        
        /**
         * Downloads a file from the given URL to a temporary directory.
         * 
         * @param url The URL to download from
         * @return DownloadResult containing the downloaded file and original filename
         */
        fun downloadFileFromUrl(url: String): DownloadResult {
            return try {
                val urlObj = URL(url)
                val fileName = urlObj.path.substringAfterLast('/').ifEmpty { "downloaded_file" }
                val tempFile = File.createTempFile("jadx_download_", "_$fileName")
                
                log.info("Downloading file from URL: $url")
                urlObj.openStream().use { input ->
                    Files.copy(input, tempFile.toPath(), StandardCopyOption.REPLACE_EXISTING)
                }
                
                log.info("File downloaded successfully: ${tempFile.absolutePath}")
                DownloadResult(true, tempFile, fileName, "File downloaded successfully")
            } catch (e: Exception) {
                log.error("Failed to download file from URL: $url", e)
                DownloadResult(false, null, "", "Failed to download file from URL: ${e.message}")
            }
        }
        
        /**
         * Processes a file path based on its protocol and returns the local file and original filename.
         * 
         * @param filePath The file path which can be a local path, file:// URL, or http/https URL
         * @return FileProcessResult containing the local file, original filename, and protocol type
         */
        fun processFilePath(filePath: String): FileProcessResult {
            return try {
                when {
                    filePath.startsWith("file://") -> {
                        // Handle file:// protocol
                        val localPath = filePath.removePrefix("file://")
                        val file = File(localPath)
                        if (!file.exists()) {
                            FileProcessResult(false, null, "", "file", "File does not exist: $localPath")
                        } else {
                            FileProcessResult(true, file, file.name, "file", "File processed successfully")
                        }
                    }
                    filePath.startsWith("http://") || filePath.startsWith("https://") -> {
                        // Handle HTTP/HTTPS protocol
                        val protocol = if (filePath.startsWith("https://")) "https" else "http"
                        val downloadResult = downloadFileFromUrl(filePath)
                        if (downloadResult.success && downloadResult.file != null) {
                            FileProcessResult(true, downloadResult.file, downloadResult.originalFileName, protocol, downloadResult.message)
                        } else {
                            FileProcessResult(false, null, "", protocol, downloadResult.message)
                        }
                    }
                    else -> {
                        // Handle regular file path
                        val file = File(filePath)
                        if (!file.exists()) {
                            FileProcessResult(false, null, "", "local", "File does not exist: $filePath")
                        } else {
                            FileProcessResult(true, file, file.name, "local", "File processed successfully")
                        }
                    }
                }
            } catch (e: Exception) {
                log.error("Error processing file path: $filePath", e)
                FileProcessResult(false, null, "", "unknown", "Error processing file path: ${e.message}")
            }
        }
        

        
        /**
         * Data class representing the result of file copy operation.
         * 
         * @property success Whether the copy operation was successful
         * @property targetPath The path of the copied file (null if failed)
         * @property message Descriptive message about the copy result
         */
        data class CopyResult(
            val success: Boolean,
            val targetPath: String?,
            val message: String
        )
        
        /**
         * Data class representing the result of file download operation.
         * 
         * @property success Whether the download was successful
         * @property file The downloaded file (null if failed)
         * @property originalFileName The original filename from the URL
         * @property message Descriptive message about the download result
         */
        data class DownloadResult(
            val success: Boolean,
            val file: File?,
            val originalFileName: String,
            val message: String
        )
        
        /**
         * Data class representing the result of file path processing.
         * 
         * @property success Whether the processing was successful
         * @property file The processed local file (null if failed)
         * @property originalFileName The original filename
         * @property protocol The protocol type (file, http, https, local)
         * @property message Descriptive message about the processing result
         */
        data class FileProcessResult(
            val success: Boolean,
            val file: File?,
            val originalFileName: String,
            val protocol: String,
            val message: String
        )
        
        /**
         * Data class representing the result of file validation.
         * 
         * @property isValid Whether the file passed validation
         * @property message Descriptive message about the validation result
         */
        data class FileValidationResult(
            val isValid: Boolean,
            val message: String
        )
    }
}