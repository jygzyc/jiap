package me.yvesz.server.service

import com.google.gson.Gson
import jadx.api.JadxDecompiler
import me.yvesz.server.model.JadxResult
import org.springframework.beans.factory.annotation.Autowired
import org.springframework.stereotype.Service

@Service
class AndroidFrameworkService @Autowired constructor(
    private val jadxDecompilerManager: JadxDecompilerManager
) {

    private var decompilerInstance: JadxDecompiler? = null

    private fun getDecompilerInstanceOrError(fileId: String): JadxResult.Error? {
        return if (jadxDecompilerManager.isDecompilerInit(fileId)) {
            decompilerInstance = jadxDecompilerManager.getDecompilerInstance(fileId)
            null
        } else {
            JadxResult.Error("JADX decompiler not initialized for fileId: $fileId")
        }
    }

    fun handleGetSystemServiceImpl(fileId: String, interfaceDescriptor: String): JadxResult<String> {
        getDecompilerInstanceOrError(fileId)?.let { return it }
        return try {
            val cacheInstance = jadxDecompilerManager.getCacheInstance(fileId)
                ?: return JadxResult.Error("No cache instance found for file ID: $fileId")
            
            var serviceClazz = cacheInstance.getServiceImpl(interfaceDescriptor)
            if (serviceClazz != null) {
                JadxResult.Success(Gson().toJson(serviceClazz.toString()))
            } else {
                serviceClazz = decompilerInstance!!.classesWithInners.firstOrNull {
                    ".super ${interfaceDescriptor.replace(".", "/")}\$Stub" in it.smali
                }
                if (serviceClazz != null) {
                    cacheInstance.setServiceMap(interfaceDescriptor, serviceClazz)
                    JadxResult.Success(Gson().toJson(serviceClazz.toString()))
                } else {
                    JadxResult.Error("Service implement not found for $interfaceDescriptor")
                }
            }
        } catch (e: Exception) {
            JadxResult.Error("Error handleGetSystemServiceImpl: ${e.message}", e)
        }
    }

}