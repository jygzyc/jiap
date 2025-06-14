package me.yvesz.server.service

import com.google.gson.Gson
import com.google.gson.JsonObject
import jadx.api.JadxDecompiler
import jadx.api.ResourceType
import me.yvesz.server.model.JadxResult
import org.springframework.beans.factory.annotation.Autowired
import org.springframework.stereotype.Service

@Service
class AndroidAppService @Autowired constructor(
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

    fun handleGetAppManifest(fileId: String): JadxResult<String> {
        getDecompilerInstanceOrError(fileId)?.let { return it }
        return try {
            val manifest = decompilerInstance!!.resources
                ?.stream()
                ?.filter { resourceFile -> resourceFile.type == ResourceType.MANIFEST }
                ?.findFirst()
                ?.orElse(null)
            if (manifest == null){
                JadxResult.Error("AndroidManifest.xml not found")
            }
            val manifestContent = manifest!!.loadContent().text.codeStr
            val result = JsonObject().apply {
                addProperty("name", manifest.originalName)
                addProperty("type", "manifest/xml")
                addProperty("content", manifestContent)
            }
            JadxResult.Success(Gson().toJson(result))
        } catch (e: Exception) {
            JadxResult.Error(e.message ?: "Unknown error")
        }
    }

}