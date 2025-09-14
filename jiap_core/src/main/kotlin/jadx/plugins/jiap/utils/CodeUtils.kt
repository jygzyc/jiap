package jadx.plugins.jiap.utils

import jadx.api.JavaClass
import jadx.api.JavaMethod
import jadx.core.dex.instructions.args.ArgType
import org.slf4j.LoggerFactory
import java.util.regex.Pattern


object CodeUtils {
    private val logger = LoggerFactory.getLogger(CodeUtils::class.java)

    fun getLineForPos(code: String, pos: Int): String {
        val start = getLineStartForPos(code, pos)
        val end = getLineEndForPos(code, pos)
        return code.substring(start, end)
    }

    private fun getLineStartForPos(code: String, pos: Int): Int {
        val start = getNewLinePosBefore(code, pos)
        return if (start == -1) 0 else start + 1
    }

    private fun getLineEndForPos(code: String, pos: Int): Int {
        val end = getNewLinePosAfter(code, pos)
        return if (end == -1) code.length else end
    }

    private fun getNewLinePosAfter(code: String, startPos: Int): Int {
        val pos = code.indexOf('\n', startPos)
        if (pos != -1) {
            // check for '\r\n'
            val prev = pos - 1
            if (code[prev] == '\r') {
                return prev
            }
        }
        return pos
    }

    private fun getNewLinePosBefore(code: String, startPos: Int): Int {
        return code.lastIndexOf('\n', startPos)
    }

    fun getLineNumberForPos(code: String, pos: Int): Int {
        if (pos < 0 || pos >= code.length) {
            return 1
        }
        return code.substring(0, pos).count { it == '\n' } + 1
    }

    fun extractMethodSmaliCode(clazz: JavaClass, mth: JavaMethod): String {
        val classSmaliCode = clazz.smali ?: return "".also {
            logger.warn("Smali code not available for class: {}", clazz.fullName)
        }

        try {
            val smaliSignature = buildSmaliSignature(mth)
            logger.debug("Looking for Smali signature: {} in class {}", smaliSignature, clazz.fullName)

            val patternString = "(^\\s*\\.method[^\n]*${Regex.escape(smaliSignature)}.*?^\\s*\\.end method)"
            val pattern = Pattern.compile(patternString, Pattern.DOTALL or Pattern.MULTILINE)
            val matcher = pattern.matcher(classSmaliCode)

            return if (matcher.find()) {
                matcher.group(1).trimIndent()
            } else {
                logger.warn("Smali method body not found for signature: {} in class: {}", smaliSignature, clazz.fullName)
                ""
            }
        } catch (e: Exception) {
            logger.error("Error extracting Smali code for method {} in class {}: {}", mth.name, clazz.fullName, e.message)
            return ""
        }
    }

    /**
     * Builds the Smali method signature (name + descriptor) from a JavaMethod object.
     *
     * @param mth The JavaMethod object.
     * @return The Smali method signature string.
     */
    private fun buildSmaliSignature(mth: JavaMethod): String {
        val methodName = mth.name
        val parameters = mth.arguments.joinToString("") {
            javaTypeToSmaliDescriptor(it)
        }
        val returnType = javaTypeToSmaliDescriptor(mth.returnType)

        val signature = "$methodName($parameters)$returnType"
        logger.debug("Built Smali signature: {}", signature)

        return signature
    }

    /**
     * Converts a Jadx ArgType object representing a Java type to its Smali descriptor string.
     *
     * @param type The ArgType object.
     * @return The Smali type descriptor string.
     */
    private fun javaTypeToSmaliDescriptor(type: ArgType): String {
        return when {
            type.isPrimitive -> when (type.primitiveType) {
                ArgType.BOOLEAN.primitiveType -> "Z"
                ArgType.BYTE.primitiveType -> "B"
                ArgType.SHORT.primitiveType -> "S"
                ArgType.CHAR.primitiveType -> "C"
                ArgType.INT.primitiveType -> "I"
                ArgType.LONG.primitiveType -> "J"
                ArgType.FLOAT.primitiveType -> "F"
                ArgType.DOUBLE.primitiveType -> "D"
                ArgType.VOID.primitiveType -> "V"
                else -> throw IllegalArgumentException("Unknown primitive type: ${type.primitiveType}")
            }
            type.isArray -> "[${javaTypeToSmaliDescriptor(type.arrayElement!!)}"
            type.isObject -> "L${type.`object`.replace('.', '/')};"
            else -> throw IllegalArgumentException("Unknown ArgType: $type")
        }
    }
}