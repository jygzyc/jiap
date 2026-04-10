package jadx.plugins.jiap.model

import jadx.api.JadxDecompiler

interface JiapServiceInterface {
    val decompiler: JadxDecompiler

    val gui: Boolean
        get() = false
}
