package com.terminal.ui.theme

import android.app.Activity
import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.platform.LocalView
import androidx.core.view.WindowCompat

/**
 * Terminal color palette - Dracula theme inspired
 */
object TerminalPalette {
    // Background colors
    val background = Color(0xFF1E1E2E)
    val surface = Color(0xFF282A36)
    val surfaceVariant = Color(0xFF313244)
    
    // Text colors
    val onBackground = Color(0xFFF8F8F2)
    val onSurface = Color(0xFFF8F8F2)
    val onSurfaceVariant = Color(0xFFBAC2DE)
    
    // Accent colors
    val primary = Color(0xFF8BE9FD)      // Cyan
    val secondary = Color(0xFFBD93F9)    // Purple
    val tertiary = Color(0xFF50FA7B)     // Green
    
    // Semantic colors
    val error = Color(0xFFFF5555)        // Red
    val warning = Color(0xFFFFB86C)      // Orange
    val success = Color(0xFF50FA7B)      // Green
    val info = Color(0xFF8BE9FD)         // Cyan
    
    // Terminal specific
    val prompt = Color(0xFF50FA7B)
    val cursor = Color(0xFFF8F8F2)
    val selection = Color(0x4444475A)
}

private val DarkColorScheme = darkColorScheme(
    primary = TerminalPalette.primary,
    secondary = TerminalPalette.secondary,
    tertiary = TerminalPalette.tertiary,
    background = TerminalPalette.background,
    surface = TerminalPalette.surface,
    surfaceVariant = TerminalPalette.surfaceVariant,
    onPrimary = TerminalPalette.background,
    onSecondary = TerminalPalette.background,
    onTertiary = TerminalPalette.background,
    onBackground = TerminalPalette.onBackground,
    onSurface = TerminalPalette.onSurface,
    onSurfaceVariant = TerminalPalette.onSurfaceVariant,
    error = TerminalPalette.error,
    onError = Color.White
)

// Light theme (optional - terminals are usually dark)
private val LightColorScheme = lightColorScheme(
    primary = Color(0xFF006B75),
    secondary = Color(0xFF6E40C9),
    tertiary = Color(0xFF238636),
    background = Color(0xFFF6F8FA),
    surface = Color(0xFFFFFFFF),
    onBackground = Color(0xFF24292F),
    onSurface = Color(0xFF24292F)
)

@Composable
fun TerminalTheme(
    darkTheme: Boolean = true, // Terminals should always be dark by default
    dynamicColor: Boolean = false, // Don't use dynamic colors for terminal
    content: @Composable () -> Unit
) {
    val colorScheme = when {
        dynamicColor && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S -> {
            // Material You dynamic theming (optional)
            DarkColorScheme
        }
        darkTheme -> DarkColorScheme
        else -> LightColorScheme
    }
    
    val view = LocalView.current
    if (!view.isInEditMode) {
        SideEffect {
            val window = (view.context as Activity).window
            window.statusBarColor = colorScheme.background.toArgb()
            window.navigationBarColor = colorScheme.background.toArgb()
            WindowCompat.getInsetsController(window, view).apply {
                isAppearanceLightStatusBars = !darkTheme
                isAppearanceLightNavigationBars = !darkTheme
            }
        }
    }

    MaterialTheme(
        colorScheme = colorScheme,
        content = content
    )
}
