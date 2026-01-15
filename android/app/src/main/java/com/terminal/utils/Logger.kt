package com.terminal.utils

import com.terminal.core.TerminalEngine
import timber.log.Timber

/**
 * Logger wrapper that logs to both Timber and native logger.
 */
object Logger {
    
    fun d(tag: String, message: String) {
        Timber.tag(tag).d(message)
        TerminalEngine.log(TerminalEngine.LogLevel.DEBUG, "[$tag] $message")
    }
    
    fun i(tag: String, message: String) {
        Timber.tag(tag).i(message)
        TerminalEngine.log(TerminalEngine.LogLevel.INFO, "[$tag] $message")
    }
    
    fun w(tag: String, message: String) {
        Timber.tag(tag).w(message)
        TerminalEngine.log(TerminalEngine.LogLevel.WARN, "[$tag] $message")
    }
    
    fun e(tag: String, message: String, throwable: Throwable? = null) {
        if (throwable != null) {
            Timber.tag(tag).e(throwable, message)
        } else {
            Timber.tag(tag).e(message)
        }
        TerminalEngine.log(TerminalEngine.LogLevel.ERROR, "[$tag] $message")
    }
    
    fun d(message: String) = d("Terminal", message)
    fun i(message: String) = i("Terminal", message)
    fun w(message: String) = w("Terminal", message)
    fun e(message: String, throwable: Throwable? = null) = e("Terminal", message, throwable)
}
