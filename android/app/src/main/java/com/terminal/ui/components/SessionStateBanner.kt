package com.terminal.ui.components

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.expandVertically
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.terminal.R
import com.terminal.core.TerminalEngine

/**
 * Banner showing the current session state.
 * Displays Active, Checkpointed, Restored, or Failed status.
 */
@Composable
fun SessionStateBanner(
    state: Int,
    visible: Boolean = true,
    modifier: Modifier = Modifier
) {
    AnimatedVisibility(
        visible = visible && state != TerminalEngine.SessionState.ACTIVE,
        enter = expandVertically(),
        exit = shrinkVertically()
    ) {
        val (backgroundColor, icon, message) = when (state) {
            TerminalEngine.SessionState.CHECKPOINTED -> Triple(
                Color(0xFFF57C00),
                "⏸",
                "Session checkpointed - will restore on resume"
            )
            TerminalEngine.SessionState.RESTORED -> Triple(
                Color(0xFF1565C0),
                "↺",
                "Session restored from checkpoint"
            )
            TerminalEngine.SessionState.FAILED -> Triple(
                Color(0xFFB71C1C),
                "⚠",
                "Session failed - tap to restart"
            )
            else -> Triple(
                Color(0xFF2E7D32),
                "●",
                "Session active"
            )
        }

        Surface(
            color = backgroundColor,
            modifier = modifier.fillMaxWidth()
        ) {
            Row(
                modifier = Modifier
                    .padding(horizontal = 16.dp, vertical = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Text(
                    text = icon,
                    fontSize = 16.sp,
                    color = Color.White
                )
                Text(
                    text = message,
                    color = Color.White,
                    fontSize = 13.sp
                )
            }
        }
    }
}

/**
 * Compact session state indicator (just the badge).
 */
@Composable
fun SessionStateIndicator(
    state: Int,
    modifier: Modifier = Modifier
) {
    val (color, label) = when (state) {
        TerminalEngine.SessionState.ACTIVE -> Color(0xFF2E7D32) to "Active"
        TerminalEngine.SessionState.CHECKPOINTED -> Color(0xFFF57C00) to "Checkpointed"
        TerminalEngine.SessionState.RESTORED -> Color(0xFF1565C0) to "Restored"
        TerminalEngine.SessionState.FAILED -> Color(0xFFB71C1C) to "Failed"
        else -> Color(0xFF757575) to "Unknown"
    }

    Surface(
        color = color,
        shape = MaterialTheme.shapes.small,
        modifier = modifier
    ) {
        Text(
            text = label,
            color = Color.White,
            fontSize = 10.sp,
            modifier = Modifier.padding(horizontal = 6.dp, vertical = 2.dp)
        )
    }
}
