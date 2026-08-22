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

    // ------------------------------------------------------------------
    // TODO (Phase 1d — wires up to rust/src/vfs/health.rs::PermissionProbe)
    //
    // The Rust side already has the full state machine implemented and
    // unit-tested (PermissionState::{Valid,Stale,Revoked,NotApplicable},
    // HealthMonitor::scan/scan_needs_attention — see
    // rust/src/vfs/health.rs). What's missing is the Kotlin-side
    // implementation of the `PermissionProbe` trait that feeds it real
    // data, since that trait boundary exists specifically so the state
    // machine could be tested on host without a JVM. Concretely:
    //
    // 1. Add a JNI-exported function here (or in SafHelper.kt, wherever
    //    the existing JNI bridge conventions live) that Rust can call per
    //    mount, following the same safe-wrapper pattern already used
    //    elsewhere — do NOT call raw JNI, go through jni_safe.rs.
    //
    // 2. Distinguish Stale vs Revoked (this is the part that needs real
    //    logic, not just a boolean):
    //      - `Revoked`: the URI is no longer in
    //        `getPersistedPermissions()` at all — user turned it off in
    //        Settings, or the storage provider app was uninstalled.
    //      - `Stale`: the URI IS still in `getPersistedPermissions()`
    //        (so `hasPermissionFor` returns true) but an actual operation
    //        against it fails (e.g. `DocumentFile.fromTreeUri(...).exists()`
    //        returns false, or a query throws). This is the "survived a
    //        reboot but didn't fully come back" case and is usually
    //        recoverable by re-taking the persistable permission without
    //        showing the picker again.
    //
    // 3. Suggested shape:
    //
    //      fun checkHealth(treeUri: Uri): PermissionHealthResult {
    //          val stillListed = hasPermissionFor(treeUri)
    //          if (!stillListed) return PermissionHealthResult.REVOKED
    //          return try {
    //              val doc = DocumentFile.fromTreeUri(context, treeUri)
    //              if (doc != null && doc.exists()) PermissionHealthResult.VALID
    //              else PermissionHealthResult.STALE
    //          } catch (e: SecurityException) {
    //              PermissionHealthResult.STALE
    //          }
    //      }
    //
    //    ...then map PermissionHealthResult to the Rust PermissionState
    //    enum values at the JNI boundary.
    //
    // 4. Call this from a startup health-check pass (e.g. in
    //    TerminalApplication.onCreate or a splash/init screen) and surface
    //    `MountHealth.suggested_action` as a dismissible banner — see
    //    SessionStateBanner.kt for the existing banner pattern to reuse
    //    rather than inventing a new UI component for this.
    //
    // See .cursor/skills/wire-permission-health-check.md for a fuller
    // walkthrough including the JNI signature to use.
    // ------------------------------------------------------------------

    companion object {
        const val REQUEST_CODE_STORAGE = 1001
        const val REQUEST_CODE_SAF_PICKER = 1002
    }
}
