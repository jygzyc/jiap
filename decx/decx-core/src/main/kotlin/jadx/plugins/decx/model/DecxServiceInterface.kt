package jadx.plugins.decx.model

import jadx.api.JadxDecompiler

interface DecxServiceInterface {
    val decompiler: JadxDecompiler

    val gui: Boolean
        get() = false
}
