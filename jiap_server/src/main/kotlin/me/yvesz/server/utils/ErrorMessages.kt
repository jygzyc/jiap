package me.yvesz.server.utils

object ErrorMessages {
    const val MISSING_FILEID_PARAM = "Missing 'id'(The first 8 characters (hexadecimal) of the file's MD5 value)"
    const val MISSING_CLASS_PARAM = "Missing 'class' parameter in JSON body"
    const val MISSING_INTERFACE_PARAM = "Missing 'interface' parameter in JSON body"
    const val MISSING_METHOD_PARAM = "Missing 'method' parameter in JSON body"
    const val MISSING_FIELD_PARAM = "Missing 'field' parameter in JSON body"
    const val INVALID_SMALI_PARAM = "Missing or invalid 'smali' parameter in JSON body"
}
