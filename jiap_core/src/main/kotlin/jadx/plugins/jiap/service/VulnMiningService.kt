package jadx.plugins.jiap.service

import jadx.api.plugins.JadxPluginContext
import jadx.plugins.jiap.model.JiapServiceInterface
import jadx.plugins.jiap.model.JiapResult
import jadx.plugins.jiap.utils.CodeUtils
import jadx.api.JavaMethod
import jadx.core.dex.nodes.MethodNode
import jadx.core.dex.instructions.InvokeNode
import jadx.core.dex.instructions.InsnType
import jadx.core.dex.instructions.IndexInsnNode
import jadx.core.dex.instructions.args.RegisterArg
import jadx.core.dex.instructions.args.InsnArg
import jadx.core.dex.info.FieldInfo

class VulnMiningService(override val pluginContext: JadxPluginContext) : JiapServiceInterface {

    /**
     * Scan for dynamically registered BroadcastReceivers.
     * Looks for 'registerReceiver' calls in the code.
     */
    fun handleGetDynamicReceivers(): JiapResult {
        try {
            val receivers = mutableListOf<HashMap<String, Any>>()
            decompiler.classesWithInners.forEach { jcls ->
                if ("registerReceiver" !in jcls.smali) return@forEach
                for (jmth in jcls.methods) {
                    val mthCode = jmth.codeStr
                    if ("registerReceiver" !in mthCode) continue

                    receivers.add(hashMapOf(
                        "class" to jcls.fullName,
                        "method" to mthCode
                    ))
                }
            }
            return JiapResult(true, hashMapOf(
                "type" to "list",
                "count" to receivers.size,
                "code-list" to receivers
            ))
        } catch (e: Exception) {
            return JiapResult(false, hashMapOf("error" to "handleGetDynamicReceivers error: ${e.message}"))
        }
    }
}
