package com.terminal

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.ui.Modifier
import com.terminal.ui.screens.TerminalScreen
import com.terminal.ui.theme.TerminalTheme
import timber.log.Timber

/**
 * Main entry point for the Terminal app.
 */
class MainActivity : ComponentActivity() {
    
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        
        Timber.d("MainActivity created")
        
        // Check if native library is loaded
        if (!TerminalApplication.isNativeLibraryLoaded) {
            Timber.e("Native library not loaded - terminal will not work")
        }
        
        setContent {
            TerminalTheme {
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = MaterialTheme.colorScheme.background
                ) {
                    TerminalScreen()
                }
            }
        }
    }
    
    override fun onStart() {
        super.onStart()
        Timber.d("MainActivity started")
    }
    
    override fun onStop() {
        super.onStop()
        Timber.d("MainActivity stopped")
        // Sessions continue in TerminalService
    }
    
    override fun onDestroy() {
        super.onDestroy()
        Timber.d("MainActivity destroyed")
    }
}
