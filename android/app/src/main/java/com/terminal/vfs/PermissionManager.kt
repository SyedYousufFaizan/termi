package com.terminal.vfs

import android.Manifest
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.Settings
import androidx.activity.result.ActivityResultLauncher
import androidx.core.content.ContextCompat
import timber.log.Timber

/**
 * Manages storage permissions for the terminal.
 */
class PermissionManager(private val context: Context) {
    
    /**
     * Check if we have storage access.
     */
    fun hasStoragePermission(): Boolean {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            // Android 11+: Check MANAGE_EXTERNAL_STORAGE
            Environment.isExternalStorageManager()
        } else {
            // Android 10 and below
            ContextCompat.checkSelfPermission(
                context,
                Manifest.permission.WRITE_EXTERNAL_STORAGE
            ) == PackageManager.PERMISSION_GRANTED
        }
    }
    
    /**
     * Get intent to request all files access (Android 11+).
     */
    fun getAllFilesAccessIntent(): Intent? {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION).apply {
                data = Uri.parse("package:${context.packageName}")
            }
        } else {
            null
        }
    }
    
    /**
     * Persist URI permission after SAF picker.
     */
    fun persistUriPermission(uri: Uri) {
        try {
            val flags = Intent.FLAG_GRANT_READ_URI_PERMISSION or
                    Intent.FLAG_GRANT_WRITE_URI_PERMISSION
            context.contentResolver.takePersistableUriPermission(uri, flags)
            Timber.i("Persisted URI permission: $uri")
        } catch (e: Exception) {
            Timber.e(e, "Failed to persist URI permission")
        }
    }
    
    /**
     * Release URI permission.
     */
    fun releaseUriPermission(uri: Uri) {
        try {
            val flags = Intent.FLAG_GRANT_READ_URI_PERMISSION or
                    Intent.FLAG_GRANT_WRITE_URI_PERMISSION
            context.contentResolver.releasePersistableUriPermission(uri, flags)
            Timber.i("Released URI permission: $uri")
        } catch (e: Exception) {
            Timber.w(e, "Failed to release URI permission")
        }
    }
    
    /**
     * Get list of persisted URI permissions.
     */
    fun getPersistedPermissions(): List<Uri> {
        return context.contentResolver.persistedUriPermissions.map { it.uri }
    }
    
    /**
     * Check if we have permission for a specific tree URI.
     */
    fun hasPermissionFor(treeUri: Uri): Boolean {
        return context.contentResolver.persistedUriPermissions.any {
            it.uri == treeUri && it.isReadPermission
        }
    }
    
    companion object {
        const val REQUEST_CODE_STORAGE = 1001
        const val REQUEST_CODE_SAF_PICKER = 1002
    }
}
