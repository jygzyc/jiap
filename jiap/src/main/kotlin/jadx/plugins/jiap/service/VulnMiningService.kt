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
    // TODO: implement vulnerability mining related APIs here
}
