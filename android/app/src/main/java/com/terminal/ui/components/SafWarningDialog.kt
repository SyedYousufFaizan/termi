package com.terminal.ui.components

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.terminal.R

/**
 * Dialog showing SAF limitations warning.
 */
@Composable
fun SafWarningDialog(
    onDismiss: () -> Unit,
    onLearnMore: () -> Unit = {},
    modifier: Modifier = Modifier
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = {
            Text(
                text = "External Storage Limitations",
                fontWeight = FontWeight.Bold
            )
        },
        text = {
            Column(
                verticalArrangement = Arrangement.spacedBy(12.dp)
            ) {
                Text(
                    text = "Due to Android's Storage Access Framework (SAF) restrictions, " +
                            "some Unix operations are not available on external storage:"
                )
                
                Column(
                    modifier = Modifier.padding(start = 16.dp),
                    verticalArrangement = Arrangement.spacedBy(4.dp)
                ) {
                    LimitationItem("chmod / chown - Permission changes")
                    LimitationItem("symlinks / hardlinks - Link creation")
                    LimitationItem("Native execution - Can't run binaries")
                    LimitationItem("File locking - Advisory locks")
                }
                
                Text(
                    text = "For full Unix compatibility, use the internal storage " +
                            "(/data/data/...) for development work."
                )
            }
        },
        confirmButton = {
            TextButton(onClick = onDismiss) {
                Text("Got it")
            }
        },
        dismissButton = {
            TextButton(onClick = onLearnMore) {
                Text("Learn More")
            }
        },
        modifier = modifier
    )
}

@Composable
private fun LimitationItem(text: String) {
    Row {
        Text("• ", color = MaterialTheme.colorScheme.error)
        Text(text, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

/**
 * Inline warning for SAF paths.
 */
@Composable
fun SafWarningBanner(
    warning: String?,
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier
) {
    if (warning != null) {
        Surface(
            color = MaterialTheme.colorScheme.errorContainer,
            modifier = modifier.fillMaxWidth()
        ) {
            Row(
                modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                horizontalArrangement = Arrangement.SpaceBetween
            ) {
                Text(
                    text = "⚠ $warning",
                    color = MaterialTheme.colorScheme.onErrorContainer,
                    modifier = Modifier.weight(1f)
                )
                TextButton(
                    onClick = onDismiss,
                    contentPadding = PaddingValues(horizontal = 8.dp)
                ) {
                    Text("Dismiss")
                }
            }
        }
    }
}
