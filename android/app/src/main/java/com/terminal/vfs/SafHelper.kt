package com.terminal.vfs

import android.content.Context
import android.net.Uri
import androidx.documentfile.provider.DocumentFile
import timber.log.Timber
import java.io.FileNotFoundException

/**
 * Helper class for SAF operations.
 * This class is called from Rust via JNI to perform file operations
 * on SAF-managed storage.
 */
class SafHelper(private val context: Context) {
    
    /**
     * Read file contents.
     */
    fun readFile(uriString: String): ByteArray? {
        return try {
            val uri = Uri.parse(uriString)
            context.contentResolver.openInputStream(uri)?.use { it.readBytes() }
        } catch (e: Exception) {
            Timber.e(e, "Failed to read: $uriString")
            null
        }
    }
    
    /**
     * Write file contents.
     */
    fun writeFile(uriString: String, data: ByteArray): Boolean {
        return try {
            val uri = Uri.parse(uriString)
            context.contentResolver.openOutputStream(uri)?.use {
                it.write(data)
                true
            } ?: false
        } catch (e: Exception) {
            Timber.e(e, "Failed to write: $uriString")
            false
        }
    }
    
    /**
     * Get file metadata.
     */
    fun getMetadata(uriString: String): SafMetadata? {
        return try {
            val uri = Uri.parse(uriString)
            val doc = DocumentFile.fromSingleUri(context, uri) ?: return null
            SafMetadata(
                name = doc.name ?: "",
                size = doc.length(),
                lastModified = doc.lastModified(),
                isDirectory = doc.isDirectory,
                isFile = doc.isFile,
                canRead = doc.canRead(),
                canWrite = doc.canWrite()
            )
        } catch (e: Exception) {
            Timber.e(e, "Failed to get metadata: $uriString")
            null
        }
    }
    
    /**
     * List directory contents.
     */
    fun listDirectory(uriString: String): Array<SafMetadata>? {
        return try {
            val uri = Uri.parse(uriString)
            val dir = DocumentFile.fromTreeUri(context, uri) ?: return null
            dir.listFiles().mapNotNull { file ->
                SafMetadata(
                    name = file.name ?: return@mapNotNull null,
                    size = file.length(),
                    lastModified = file.lastModified(),
                    isDirectory = file.isDirectory,
                    isFile = file.isFile,
                    canRead = file.canRead(),
                    canWrite = file.canWrite(),
                    uri = file.uri.toString()
                )
            }.toTypedArray()
        } catch (e: Exception) {
            Timber.e(e, "Failed to list: $uriString")
            null
        }
    }
    
    /**
     * Create a directory.
     */
    fun createDirectory(parentUri: String, name: String): String? {
        return try {
            val uri = Uri.parse(parentUri)
            val parent = DocumentFile.fromTreeUri(context, uri) ?: return null
            parent.createDirectory(name)?.uri?.toString()
        } catch (e: Exception) {
            Timber.e(e, "Failed to create directory: $name in $parentUri")
            null
        }
    }
    
    /**
     * Create a file.
     */
    fun createFile(parentUri: String, mimeType: String, name: String): String? {
        return try {
            val uri = Uri.parse(parentUri)
            val parent = DocumentFile.fromTreeUri(context, uri) ?: return null
            parent.createFile(mimeType, name)?.uri?.toString()
        } catch (e: Exception) {
            Timber.e(e, "Failed to create file: $name in $parentUri")
            null
        }
    }
    
    /**
     * Delete a file or directory.
     */
    fun delete(uriString: String): Boolean {
        return try {
            val uri = Uri.parse(uriString)
            val doc = DocumentFile.fromSingleUri(context, uri) ?: return false
            doc.delete()
        } catch (e: Exception) {
            Timber.e(e, "Failed to delete: $uriString")
            false
        }
    }
    
    /**
     * Rename/move a file.
     */
    fun rename(uriString: String, newName: String): Boolean {
        return try {
            val uri = Uri.parse(uriString)
            val doc = DocumentFile.fromSingleUri(context, uri) ?: return false
            doc.renameTo(newName)
        } catch (e: Exception) {
            Timber.e(e, "Failed to rename: $uriString to $newName")
            false
        }
    }
    
    /**
     * Check if file exists.
     */
    fun exists(uriString: String): Boolean {
        return try {
            val uri = Uri.parse(uriString)
            val doc = DocumentFile.fromSingleUri(context, uri)
            doc?.exists() == true
        } catch (e: Exception) {
            false
        }
    }
    
    /**
     * Check if path is a directory.
     */
    fun isDirectory(uriString: String): Boolean {
        return try {
            val uri = Uri.parse(uriString)
            val doc = DocumentFile.fromSingleUri(context, uri)
            doc?.isDirectory == true
        } catch (e: Exception) {
            false
        }
    }
}

/**
 * File metadata from SAF.
 */
data class SafMetadata(
    val name: String,
    val size: Long,
    val lastModified: Long,
    val isDirectory: Boolean,
    val isFile: Boolean,
    val canRead: Boolean,
    val canWrite: Boolean,
    val uri: String = ""
)
