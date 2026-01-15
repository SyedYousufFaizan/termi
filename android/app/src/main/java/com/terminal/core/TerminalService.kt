package com.terminal.core

import android.app.Notification
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.os.Binder
import android.os.IBinder
import androidx.core.app.NotificationCompat
import com.terminal.MainActivity
import com.terminal.R
import com.terminal.TerminalApplication
import kotlinx.coroutines.*
import timber.log.Timber
import java.io.File

/**
 * Foreground service that keeps terminal sessions alive.
 * 
 * Android aggressively kills background processes. This service
 * ensures terminal sessions survive when the app is in background.
 * 
 * Design: "Expect to die, not hope to survive"
 * - Periodically checkpoint state
 * - Can restore sessions after process death
 */
class TerminalService : Service() {
    
    private val binder = LocalBinder()
    private val scope = CoroutineScope(Dispatchers.IO + SupervisorJob())
    
    /** Session manager instance */
    lateinit var sessionManager: SessionManager
        private set
    
    /** Checkpoint job */
    private var checkpointJob: Job? = null
    
    inner class LocalBinder : Binder() {
        fun getService(): TerminalService = this@TerminalService
    }
    
    override fun onCreate() {
        super.onCreate()
        Timber.i("TerminalService created")
        
        // Initialize session manager
        val checkpointDir = File(filesDir, "checkpoints")
        sessionManager = SessionManager(checkpointDir, scope)
        
        // Start periodic checkpointing
        startCheckpointLoop()
    }
    
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        Timber.d("TerminalService started")
        
        // Start as foreground service
        startForeground(NOTIFICATION_ID, createNotification())
        
        return START_STICKY
    }
    
    override fun onBind(intent: Intent?): IBinder {
        Timber.d("TerminalService bound")
        return binder
    }
    
    override fun onDestroy() {
        super.onDestroy()
        Timber.i("TerminalService destroyed")
        
        // Cancel checkpoint job
        checkpointJob?.cancel()
        
        // Checkpoint and close all sessions
        runBlocking {
            sessionManager.checkpointAll()
            sessionManager.closeAll()
        }
        
        scope.cancel()
    }
    
    /**
     * Start periodic checkpointing (every 30 seconds).
     */
    private fun startCheckpointLoop() {
        checkpointJob = scope.launch {
            while (isActive) {
                delay(CHECKPOINT_INTERVAL_MS)
                
                try {
                    sessionManager.checkpointAll()
                    Timber.d("Periodic checkpoint completed")
                } catch (e: Exception) {
                    Timber.e(e, "Periodic checkpoint failed")
                }
            }
        }
    }
    
    /**
     * Create the foreground service notification.
     */
    private fun createNotification(): Notification {
        val pendingIntent = PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )
        
        val sessionCount = if (::sessionManager.isInitialized) {
            sessionManager.getAllSessions().size
        } else 0
        
        return NotificationCompat.Builder(this, TerminalApplication.CHANNEL_TERMINAL_SERVICE)
            .setContentTitle("Terminal Running")
            .setContentText("$sessionCount active session(s)")
            .setSmallIcon(R.drawable.ic_terminal)
            .setContentIntent(pendingIntent)
            .setOngoing(true)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .setCategory(NotificationCompat.CATEGORY_SERVICE)
            .build()
    }
    
    /**
     * Update notification with current session count.
     */
    fun updateNotification() {
        val notification = createNotification()
        val manager = getSystemService(NOTIFICATION_SERVICE) as android.app.NotificationManager
        manager.notify(NOTIFICATION_ID, notification)
    }
    
    companion object {
        private const val NOTIFICATION_ID = 1001
        private const val CHECKPOINT_INTERVAL_MS = 30_000L
    }
}
