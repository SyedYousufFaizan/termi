package com.terminal.vfs

import android.content.Context
import android.net.Uri
import com.terminal.core.TerminalEngine
import com.terminal.utils.PreferencesManager
import timber.log.Timber

/**
 * Manages virtual filesystem mounts.
 * 
 * Maps SAF tree URIs to mount points that appear as paths in the terminal.
 */
class MountManager(
    private val context: Context,
    private val preferencesManager: PreferencesManager
) {
    
    /** Active mounts: path -> SAF URI */
    private val mounts = mutableMapOf<String, Uri>()
    
    /** SAF helper for operations */
    private val safHelper = SafHelper(context)
    
    init {
        // Restore persisted mounts
        restorePersistedMounts()
    }
    
    /**
     * Mount a SAF tree at a virtual path.
     */
    fun mount(virtualPath: String, treeUri: Uri): Boolean {
        val normalizedPath = normalizePath(virtualPath)
        
        // Check if already mounted
        if (mounts.containsKey(normalizedPath)) {
            Timber.w("Path already mounted: $normalizedPath")
            return false
        }
        
        // Verify we have access
        if (!safHelper.exists(treeUri.toString())) {
            Timber.e("Cannot access URI: $treeUri")
            return false
        }
        
        mounts[normalizedPath] = treeUri
        preferencesManager.addPersistedUri(treeUri.toString())
        
        Timber.i("Mounted $treeUri at $normalizedPath")
        return true
    }
    
    /**
     * Unmount a virtual path.
     */
    fun unmount(virtualPath: String): Boolean {
        val normalizedPath = normalizePath(virtualPath)
        val uri = mounts.remove(normalizedPath)
        
        if (uri != null) {
            preferencesManager.removePersistedUri(uri.toString())
            Timber.i("Unmounted: $normalizedPath")
            return true
        }
        
        return false
    }
    
    /**
     * Resolve a virtual path to a SAF URI.
     */
    fun resolve(virtualPath: String): Pair<Uri, String>? {
        val normalizedPath = normalizePath(virtualPath)
        
        // Find the longest matching mount point
        var bestMatch: String? = null
        var bestMatchUri: Uri? = null
        
        for ((mountPath, uri) in mounts) {
            if (normalizedPath.startsWith(mountPath) && 
                (bestMatch == null || mountPath.length > bestMatch.length)) {
                bestMatch = mountPath
                bestMatchUri = uri
            }
        }
        
        if (bestMatch != null && bestMatchUri != null) {
            val relativePath = normalizedPath.removePrefix(bestMatch).trimStart('/')
            return bestMatchUri to relativePath
        }
        
        return null
    }
    
    /**
     * Check if a path is on SAF storage.
     */
    fun isSafPath(virtualPath: String): Boolean {
        return resolve(virtualPath) != null
    }
    
    /**
     * Get VFS capability warning for a path.
     */
    fun getLimitationWarning(virtualPath: String): String? {
        return if (isSafPath(virtualPath)) {
            TerminalEngine.getLimitationWarning(isSaf = true)
        } else {
            null
        }
    }
    
    /**
     * Check if an operation is supported on a path.
     */
    fun supportsOperation(virtualPath: String, operation: Int): Boolean {
        val isSaf = isSafPath(virtualPath)
        return TerminalEngine.supportsOperation(virtualPath, operation, isSaf)
    }
    
    /**
     * Get all current mounts.
     */
    fun getMounts(): Map<String, Uri> = mounts.toMap()
    
    /**
     * Restore mounts from preferences.
     */
    private fun restorePersistedMounts() {
        val persistedUris = preferencesManager.getPersistedUris()
        
        for (uriString in persistedUris) {
            try {
                val uri = Uri.parse(uriString)
                // Generate a mount path from the URI
                val displayName = safHelper.getMetadata(uriString)?.name ?: "external"
                val mountPath = "/mnt/$displayName"
                
                if (!mounts.containsKey(mountPath)) {
                    mounts[mountPath] = uri
                    Timber.d("Restored mount: $mountPath -> $uri")
                }
            } catch (e: Exception) {
                Timber.w(e, "Failed to restore mount: $uriString")
                preferencesManager.removePersistedUri(uriString)
            }
        }
    }
    
    private fun normalizePath(path: String): String {
        return "/" + path.trim('/').replace("//", "/")
    }
}
