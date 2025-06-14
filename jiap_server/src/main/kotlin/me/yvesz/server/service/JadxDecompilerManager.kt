package me.yvesz.server.service

import jadx.api.JadxDecompiler
import jadx.api.JadxArgs
import org.slf4j.LoggerFactory
import org.springframework.stereotype.Component
import java.io.File
import java.util.concurrent.ConcurrentHashMap
import me.yvesz.server.model.JadxDataCache
import me.yvesz.server.utils.FileUtils

@Component
class JadxDecompilerManager {

    private val log = LoggerFactory.getLogger(JadxDecompilerManager::class.java)
    
    private val decompilerInstances = ConcurrentHashMap<String, JadxDecompiler>()
    private val cacheInstances = ConcurrentHashMap<String, JadxDataCache>()

    fun getDecompilerInstance(fileId: String): JadxDecompiler? {
        return decompilerInstances[fileId] ?: run {
            log.warn("JADX decompiler not initialized for fileId: {}", fileId)
            null
        }
    }
    
    /**
     * Get cache instance for the specified file ID
     * @param fileId The unique identifier for the file
     * @return JadxDataCache instance or null if not found
     */
    fun getCacheInstance(fileId: String): JadxDataCache? {
        return cacheInstances[fileId]
    }

    fun isDecompilerInit(fileId: String): Boolean {
        return decompilerInstances.containsKey(fileId)
    }

    fun getInitializedFileIds(): Set<String> {
        return decompilerInstances.keys.toSet()
    }

    private fun createJadxArgs(inputPath: File, outputPath: File): JadxArgs {
        val jadxArgs = JadxArgs().apply {
            setInputFile(inputPath)
            outDir = outputPath
            isCfgOutput = false
            isRawCFGOutput = false
            isShowInconsistentCode = true
            isUseImports = false
            isDebugInfo = true
            isInsertDebugLines = false
            isExtractFinally = true
            isInlineAnonymousClasses = true
            isInlineMethods = true
            isAllowInlineKotlinLambda = true
            isMoveInnerClasses = true
            isSkipResources = false
            isSkipSources = false
            isIncludeDependencies = false
            isDeobfuscationOn = false
            isEscapeUnicode = false
            isReplaceConsts = true
            isRestoreSwitchOverString = true
            isUseDxInput = false
            isSkipXmlPrettyPrint = false
            threadsCount = Runtime.getRuntime().availableProcessors() * 2
        }
        return jadxArgs
    }

    fun initJadxDecompiler(inputPath: String, outputPath: String, useCache: Boolean): Pair<String?, String> {
        log.info("Initializing JADX decompiler with input: '{}', output: '{}'", inputPath, outputPath)
        try {
            val inputFile = File(inputPath)
            if (!inputFile.exists()) {
                val errorMsg = "Input file does not exist: $inputPath"
                log.error(errorMsg)
                return Pair(null, errorMsg)
            }

            val fileId = FileUtils.getFileIdentifier(inputFile)

            if (decompilerInstances.containsKey(fileId)) {
                log.info("JADX decompiler already initialized for file hash: {}", fileId)
                return Pair(fileId, "Decompiler already initialized for this file")
            }

            val fileIdOutputDir = File(outputPath, fileId)
            FileUtils.createDirectoryIfNotExists(fileIdOutputDir)
            
            val jadxArgs = createJadxArgs(inputFile, fileIdOutputDir)
            val decompiler = JadxDecompiler(jadxArgs)
            decompiler.load()
            decompilerInstances[fileId] = decompiler

            if (useCache) {
                log.info("Initializing JADX data cache for file: {}", fileId)
                val cacheInstance = JadxDataCache().apply {
                    initCache()
                }
                cacheInstances[fileId] = cacheInstance
            } else {
                log.info("Disk cache is disabled for file: {}", fileId)
            }
            
            val successMsg = "JADX decompiler initialized successfully for file: $fileId"
            log.info(successMsg)
            return Pair(fileId, successMsg)
        } catch (e: Exception) {
            val errorMsg = "Failed to initialize JADX decompiler: ${e.message}"
            log.error(errorMsg, e)
            return Pair(null, errorMsg)
        }
    }

    fun removeDecompilerInstance(fileId: String): Boolean {
        return try {
            decompilerInstances.remove(fileId)?.let { decompiler ->
                decompiler.close()
                log.info("Removed JADX decompiler instance for file: {}", fileId)
                
                // Clear cache instance as well
                cacheInstances[fileId]?.let { cache ->
                    cache.clearCache()
                    log.info("Cache cleared for file ID: {}", fileId)
                }
                cacheInstances.remove(fileId)
                
                true
            } ?: false
        } catch (e: Exception) {
            log.error("Failed to remove JADX decompiler instance for file: {}", fileId, e)
            false
        }
    }

    fun clearAllInstances() {
        decompilerInstances.values.forEach { decompiler ->
            try {
                decompiler.close()
            } catch (e: Exception) {
                log.error("Error closing Jadx decompiler", e)
            }
        }
        decompilerInstances.clear()

        // Clear all cache instances
        cacheInstances.values.forEach { cache ->
            try {
                cache.clearCache()
            } catch (e: Exception) {
                log.error("Error clearing cache", e)
            }
        }
        cacheInstances.clear()

        log.info("All Jadx decompiler and cache instances cleared")
    }
}