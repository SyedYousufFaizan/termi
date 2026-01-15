package com.terminal.utils

import android.content.Context
import android.widget.Toast
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * Extension functions for common operations.
 */

/**
 * Show a short toast message.
 */
fun Context.toast(message: String) {
    Toast.makeText(this, message, Toast.LENGTH_SHORT).show()
}

/**
 * Show a long toast message.
 */
fun Context.longToast(message: String) {
    Toast.makeText(this, message, Toast.LENGTH_LONG).show()
}

/**
 * Debounce a suspend function.
 */
fun <T> debounce(
    delayMs: Long = 300L,
    scope: CoroutineScope,
    action: suspend (T) -> Unit
): (T) -> Unit {
    var debounceJob: Job? = null
    return { param: T ->
        debounceJob?.cancel()
        debounceJob = scope.launch {
            delay(delayMs)
            action(param)
        }
    }
}

/**
 * Throttle a function to execute at most once per interval.
 */
fun <T> throttle(
    intervalMs: Long = 300L,
    scope: CoroutineScope,
    action: suspend (T) -> Unit
): (T) -> Unit {
    var lastExecutionTime = 0L
    return { param: T ->
        val currentTime = System.currentTimeMillis()
        if (currentTime - lastExecutionTime >= intervalMs) {
            lastExecutionTime = currentTime
            scope.launch { action(param) }
        }
    }
}

/**
 * Safe substring that doesn't throw.
 */
fun String.safeSubstring(startIndex: Int, endIndex: Int = length): String {
    val start = startIndex.coerceIn(0, length)
    val end = endIndex.coerceIn(start, length)
    return substring(start, end)
}

/**
 * Convert bytes to human-readable size.
 */
fun Long.toHumanReadableSize(): String {
    if (this < 1024) return "$this B"
    val kb = this / 1024.0
    if (kb < 1024) return String.format("%.1f KB", kb)
    val mb = kb / 1024.0
    if (mb < 1024) return String.format("%.1f MB", mb)
    val gb = mb / 1024.0
    return String.format("%.1f GB", gb)
}
