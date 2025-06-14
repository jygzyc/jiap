package me.yvesz.server.model

sealed class JadxResult<out T> {
    data class Success<out T>(val data: T) : JadxResult<T>()
    data class Error(val error: String, val cause: Throwable? = null) : JadxResult<Nothing>()
}