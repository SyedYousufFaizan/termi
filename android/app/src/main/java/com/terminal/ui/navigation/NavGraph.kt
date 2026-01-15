package com.terminal.ui.navigation

import androidx.compose.runtime.Composable
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import com.terminal.ui.screens.TerminalScreen

/**
 * Navigation routes for the app.
 */
object Routes {
    const val TERMINAL = "terminal"
    const val SETTINGS = "settings"
    const val ABOUT = "about"
    const val FILE_PICKER = "file_picker"
}

/**
 * Main navigation graph.
 */
@Composable
fun AppNavGraph(
    navController: NavHostController = rememberNavController(),
    startDestination: String = Routes.TERMINAL
) {
    NavHost(
        navController = navController,
        startDestination = startDestination
    ) {
        composable(Routes.TERMINAL) {
            TerminalScreen()
        }
        
        // Settings and About screens can be added later
        // For now, just the terminal screen
    }
}
