package com.terminal

import android.app.Application
import android.app.NotificationChannel
import android.app.NotificationManager
import android.os.Build
import com.terminal.core.TerminalEngine
import timber.log.Timber

/**
 * Application class for the Terminal app.
 * 
 * Responsibilities:
 * - Initialize Timber logging
 * - Load native library
 * - Create notification channels
 */
class TerminalApplication : Application() {
    
    companion object {
        const val CHANNEL_TERMINAL_SERVICE = "terminal_service"
        
        /** Whether the native library loaded successfully */
        var isNativeLibraryLoaded = false
            private set
    }
    
    override fun onCreate() {
        super.onCreate()
        
        // Initialize Timber for logging
        if (BuildConfig.DEBUG) {
            Timber.plant(Timber.DebugTree())
        } else {
            // In release, plant a tree that only logs warnings and errors
            Timber.plant(ReleaseTree())
        }
        
        Timber.i("Terminal application starting...")
        
        // Load native library
        loadNativeLibrary()
        
        // Create notification channels
        createNotificationChannels()
        
        Timber.i("Terminal application initialized")
    }
    
    private fun loadNativeLibrary() {
        isNativeLibraryLoaded = TerminalEngine.loadNativeLibrary()
        
        if (isNativeLibraryLoaded) {
            // Initialize the engine
            TerminalEngine.initialize()
                .onSuccess { 
                    Timber.i("Terminal engine v${TerminalEngine.getVersion()} ready")
                }
                .onFailure { e ->
                    Timber.e(e, "Failed to initialize terminal engine")
                }
        } else {
            Timber.e("CRITICAL: Failed to load native library!")
        }
    }
    
    private fun createNotificationChannels() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val manager = getSystemService(NotificationManager::class.java)
            
            // Terminal service channel
            val terminalChannel = NotificationChannel(
                CHANNEL_TERMINAL_SERVICE,
                "Terminal Service",
                NotificationManager.IMPORTANCE_LOW
            ).apply {
                description = "Keeps terminal sessions alive in background"
                setShowBadge(false)
            }
            
            manager.createNotificationChannel(terminalChannel)
            Timber.d("Notification channels created")
        }
    }
    
    /**
     * Release tree that only logs warnings and errors
     */
    private class ReleaseTree : Timber.Tree() {
        override fun log(priority: Int, tag: String?, message: String, t: Throwable?) {
            if (priority >= android.util.Log.WARN) {
                // In production, you might want to send to crashlytics
                android.util.Log.println(priority, tag ?: "Terminal", message)
            }
        }
    }
}
