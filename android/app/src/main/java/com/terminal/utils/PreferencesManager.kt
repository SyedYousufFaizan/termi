package com.terminal.utils

import android.content.Context
import android.content.SharedPreferences
import androidx.core.content.edit
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/**
 * Manages app preferences.
 */
class PreferencesManager(context: Context) {
    
    private val prefs: SharedPreferences = context.getSharedPreferences(
        PREFS_NAME,
        Context.MODE_PRIVATE
    )
    
    // Font size
    private val _fontSize = MutableStateFlow(prefs.getInt(KEY_FONT_SIZE, DEFAULT_FONT_SIZE))
    val fontSize: StateFlow<Int> = _fontSize.asStateFlow()
    
    fun setFontSize(size: Int) {
        prefs.edit { putInt(KEY_FONT_SIZE, size.coerceIn(8, 24)) }
        _fontSize.value = size
    }
    
    // Shell path
    var shellPath: String
        get() = prefs.getString(KEY_SHELL_PATH, DEFAULT_SHELL) ?: DEFAULT_SHELL
        set(value) = prefs.edit { putString(KEY_SHELL_PATH, value) }
    
    // Initial directory
    var initialDirectory: String
        get() = prefs.getString(KEY_INITIAL_DIR, DEFAULT_INITIAL_DIR) ?: DEFAULT_INITIAL_DIR
        set(value) = prefs.edit { putString(KEY_INITIAL_DIR, value) }
    
    // Keep screen on
    var keepScreenOn: Boolean
        get() = prefs.getBoolean(KEY_KEEP_SCREEN_ON, false)
        set(value) = prefs.edit { putBoolean(KEY_KEEP_SCREEN_ON, value) }
    
    // Vibrate on bell
    var vibrateOnBell: Boolean
        get() = prefs.getBoolean(KEY_VIBRATE_ON_BELL, true)
        set(value) = prefs.edit { putBoolean(KEY_VIBRATE_ON_BELL, value) }
    
    // Show SAF warnings
    var showSafWarnings: Boolean
        get() = prefs.getBoolean(KEY_SHOW_SAF_WARNINGS, true)
        set(value) = prefs.edit { putBoolean(KEY_SHOW_SAF_WARNINGS, value) }
    
    // Persisted SAF URIs
    fun getPersistedUris(): Set<String> {
        return prefs.getStringSet(KEY_PERSISTED_URIS, emptySet()) ?: emptySet()
    }
    
    fun addPersistedUri(uri: String) {
        val current = getPersistedUris().toMutableSet()
        current.add(uri)
        prefs.edit { putStringSet(KEY_PERSISTED_URIS, current) }
    }
    
    fun removePersistedUri(uri: String) {
        val current = getPersistedUris().toMutableSet()
        current.remove(uri)
        prefs.edit { putStringSet(KEY_PERSISTED_URIS, current) }
    }
    
    companion object {
        private const val PREFS_NAME = "terminal_prefs"
        
        private const val KEY_FONT_SIZE = "font_size"
        private const val KEY_SHELL_PATH = "shell_path"
        private const val KEY_INITIAL_DIR = "initial_dir"
        private const val KEY_KEEP_SCREEN_ON = "keep_screen_on"
        private const val KEY_VIBRATE_ON_BELL = "vibrate_on_bell"
        private const val KEY_SHOW_SAF_WARNINGS = "show_saf_warnings"
        private const val KEY_PERSISTED_URIS = "persisted_uris"
        
        const val DEFAULT_FONT_SIZE = 13
        const val DEFAULT_SHELL = "/system/bin/sh"
        const val DEFAULT_INITIAL_DIR = ""
    }
}
