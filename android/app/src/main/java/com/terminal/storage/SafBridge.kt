package com.terminal.storage

import android.content.ContentProvider
import android.content.ContentValues
import android.content.Context
import android.database.Cursor
import android.net.Uri
import android.os.ParcelFileDescriptor
import androidx.documentfile.provider.DocumentFile
import timber.log.Timber
import java.io.FileNotFoundException

/**
 * Bridge between SAF URIs and internal file access.
 * 
 * This provider allows the Rust VFS layer to access SAF-managed
 * files through content URIs.
 */
class SafBridge : ContentProvider() {
    
    override fun onCreate(): Boolean {
        Timber.d("SafBridge provider created")
        return true
    }
    
    override fun query(
        uri: Uri,
        projection: Array<out String>?,
        selection: String?,
        selectionArgs: Array<out String>?,
        sortOrder: String?
    ): Cursor? {
        // Not used - we access files directly
        return null
    }
    
    override fun getType(uri: Uri): String? {
        return context?.contentResolver?.getType(uri)
    }
    
    override fun insert(uri: Uri, values: ContentValues?): Uri? {
        // Not supported
        return null
    }
    
    override fun delete(uri: Uri, selection: String?, selectionArgs: Array<out String>?): Int {
        // Not supported directly - use DocumentFile
        return 0
    }
    
    override fun update(
        uri: Uri,
        values: ContentValues?,
        selection: String?,
        selectionArgs: Array<out String>?
    ): Int {
        // Not supported
        return 0
    }
    
    @Throws(FileNotFoundException::class)
    override fun openFile(uri: Uri, mode: String): ParcelFileDescriptor? {
        val context = context ?: throw FileNotFoundException("Context unavailable")
        
        // The URI should be a SAF document URI
        return try {
            context.contentResolver.openFileDescriptor(uri, mode)
        } catch (e: Exception) {
            Timber.e(e, "Failed to open file: $uri")
            throw FileNotFoundException("Cannot open: $uri")
        }
    }
    
    companion object {
        /**
         * Check if we have access to a SAF tree URI
         */
        fun hasAccess(context: Context, treeUri: Uri): Boolean {
            return try {
                val doc = DocumentFile.fromTreeUri(context, treeUri)
                doc?.canRead() == true
            } catch (e: Exception) {
                Timber.w(e, "No access to: $treeUri")
                false
            }
        }
        
        /**
         * List files in a SAF directory
         */
        fun listFiles(context: Context, treeUri: Uri): List<SafFileInfo> {
            return try {
                val doc = DocumentFile.fromTreeUri(context, treeUri) ?: return emptyList()
                doc.listFiles().mapNotNull { file ->
                    SafFileInfo(
                        name = file.name ?: return@mapNotNull null,
                        uri = file.uri,
                        isDirectory = file.isDirectory,
                        size = file.length(),
                        lastModified = file.lastModified()
                    )
                }
            } catch (e: Exception) {
                Timber.e(e, "Failed to list files: $treeUri")
                emptyList()
            }
        }
        
        /**
         * Read file content from SAF
         */
        fun readFile(context: Context, uri: Uri): ByteArray? {
            return try {
                context.contentResolver.openInputStream(uri)?.use { it.readBytes() }
            } catch (e: Exception) {
                Timber.e(e, "Failed to read: $uri")
                null
            }
        }
        
        /**
         * Write file content to SAF
         */
        fun writeFile(context: Context, uri: Uri, data: ByteArray): Boolean {
            return try {
                context.contentResolver.openOutputStream(uri)?.use { 
                    it.write(data)
                    true
                } ?: false
            } catch (e: Exception) {
                Timber.e(e, "Failed to write: $uri")
                false
            }
        }
        
        /**
         * Create a file in SAF directory
         */
        fun createFile(context: Context, parentUri: Uri, mimeType: String, name: String): Uri? {
            return try {
                val parent = DocumentFile.fromTreeUri(context, parentUri) ?: return null
                parent.createFile(mimeType, name)?.uri
            } catch (e: Exception) {
                Timber.e(e, "Failed to create file: $name in $parentUri")
                null
            }
        }
        
        /**
         * Create a directory in SAF
         */
        fun createDirectory(context: Context, parentUri: Uri, name: String): Uri? {
            return try {
                val parent = DocumentFile.fromTreeUri(context, parentUri) ?: return null
                parent.createDirectory(name)?.uri
            } catch (e: Exception) {
                Timber.e(e, "Failed to create directory: $name")
                null
            }
        }
        
        /**
         * Delete a file/directory in SAF
         */
        fun delete(context: Context, uri: Uri): Boolean {
            return try {
                val doc = DocumentFile.fromSingleUri(context, uri) ?: return false
                doc.delete()
            } catch (e: Exception) {
                Timber.e(e, "Failed to delete: $uri")
                false
            }
        }
    }
}

/**
 * File info from SAF
 */
data class SafFileInfo(
    val name: String,
    val uri: Uri,
    val isDirectory: Boolean,
    val size: Long,
    val lastModified: Long
)
