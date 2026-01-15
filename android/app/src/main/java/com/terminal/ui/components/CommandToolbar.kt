package com.terminal.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * Toolbar with quick command buttons (Ctrl+C, Tab, etc.)
 */
@Composable
fun CommandToolbar(
    onCtrlC: () -> Unit,
    onCtrlD: () -> Unit,
    onCtrlZ: () -> Unit,
    onTab: () -> Unit,
    onArrowUp: () -> Unit,
    onArrowDown: () -> Unit,
    modifier: Modifier = Modifier
) {
    Row(
        modifier = modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.surfaceVariant)
            .horizontalScroll(rememberScrollState())
            .padding(horizontal = 4.dp, vertical = 4.dp),
        horizontalArrangement = Arrangement.spacedBy(4.dp)
    ) {
        ToolbarButton(text = "Ctrl+C", onClick = onCtrlC)
        ToolbarButton(text = "Ctrl+D", onClick = onCtrlD)
        ToolbarButton(text = "Ctrl+Z", onClick = onCtrlZ)
        ToolbarButton(text = "Tab", onClick = onTab)
        ToolbarButton(text = "↑", onClick = onArrowUp)
        ToolbarButton(text = "↓", onClick = onArrowDown)
        
        Spacer(modifier = Modifier.width(8.dp))
        
        // Common keys
        ToolbarButton(text = "|", onClick = { /* pipe */ })
        ToolbarButton(text = "/", onClick = { /* slash */ })
        ToolbarButton(text = "-", onClick = { /* dash */ })
        ToolbarButton(text = "~", onClick = { /* tilde */ })
    }
}

@Composable
private fun ToolbarButton(
    text: String,
    onClick: () -> Unit
) {
    FilledTonalButton(
        onClick = onClick,
        modifier = Modifier.height(36.dp),
        contentPadding = PaddingValues(horizontal = 12.dp, vertical = 4.dp),
        colors = ButtonDefaults.filledTonalButtonColors(
            containerColor = MaterialTheme.colorScheme.surface
        )
    ) {
        Text(
            text = text,
            fontFamily = FontFamily.Monospace,
            fontSize = 12.sp
        )
    }
}

/**
 * Extended toolbar with more options.
 */
@Composable
fun ExtendedCommandToolbar(
    onCommand: (String) -> Unit,
    modifier: Modifier = Modifier
) {
    Column(modifier = modifier) {
        // Control keys row
        CommandToolbar(
            onCtrlC = { onCommand("\u0003") },  // ETX
            onCtrlD = { onCommand("\u0004") },  // EOT
            onCtrlZ = { onCommand("\u001A") },  // SUB
            onTab = { onCommand("\t") },
            onArrowUp = { onCommand("\u001B[A") },
            onArrowDown = { onCommand("\u001B[B") }
        )
        
        // Quick commands row
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .background(MaterialTheme.colorScheme.surfaceVariant)
                .horizontalScroll(rememberScrollState())
                .padding(horizontal = 4.dp, vertical = 4.dp),
            horizontalArrangement = Arrangement.spacedBy(4.dp)
        ) {
            QuickCommandButton("ls -la") { onCommand("ls -la\n") }
            QuickCommandButton("cd ..") { onCommand("cd ..\n") }
            QuickCommandButton("pwd") { onCommand("pwd\n") }
            QuickCommandButton("clear") { onCommand("clear\n") }
            QuickCommandButton("exit") { onCommand("exit\n") }
        }
    }
}

@Composable
private fun QuickCommandButton(
    text: String,
    onClick: () -> Unit
) {
    OutlinedButton(
        onClick = onClick,
        modifier = Modifier.height(32.dp),
        contentPadding = PaddingValues(horizontal = 8.dp, vertical = 2.dp)
    ) {
        Text(
            text = text,
            fontFamily = FontFamily.Monospace,
            fontSize = 11.sp
        )
    }
}
