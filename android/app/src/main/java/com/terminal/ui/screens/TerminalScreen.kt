package com.terminal.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.terminal.TerminalApplication
import com.terminal.ui.viewmodels.TerminalViewModel
import kotlinx.coroutines.launch

/**
 * Main terminal screen with output display and input handling.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun TerminalScreen(
    viewModel: TerminalViewModel = viewModel()
) {
    val uiState by viewModel.uiState.collectAsState()
    val listState = rememberLazyListState()
    val scope = rememberCoroutineScope()
    val focusRequester = remember { FocusRequester() }
    
    // Auto-scroll to bottom when new output arrives
    LaunchedEffect(uiState.outputLines.size) {
        if (uiState.outputLines.isNotEmpty()) {
            listState.animateScrollToItem(uiState.outputLines.size - 1)
        }
    }
    
    // Request focus on launch
    LaunchedEffect(Unit) {
        focusRequester.requestFocus()
    }
    
    Scaffold(
        topBar = {
            TerminalTopBar(
                sessionState = uiState.sessionState,
                isConnected = uiState.isConnected,
                onNewSession = { viewModel.createSession() },
                onCloseSession = { viewModel.closeSession() }
            )
        }
    ) { paddingValues ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(paddingValues)
                .background(TerminalColors.background)
        ) {
            // Error banner
            if (uiState.error != null) {
                ErrorBanner(
                    message = uiState.error!!,
                    onDismiss = { viewModel.clearError() }
                )
            }
            
            // Native library warning
            if (!TerminalApplication.isNativeLibraryLoaded) {
                ErrorBanner(
                    message = "Native library not loaded. Terminal unavailable.",
                    onDismiss = {}
                )
            }
            
            // Terminal output
            LazyColumn(
                state = listState,
                modifier = Modifier
                    .weight(1f)
                    .fillMaxWidth()
                    .padding(horizontal = 8.dp),
                contentPadding = PaddingValues(vertical = 8.dp)
            ) {
                items(uiState.outputLines) { line ->
                    TerminalLine(text = line)
                }
            }
            
            // Input area
            TerminalInput(
                value = uiState.inputText,
                onValueChange = { viewModel.updateInput(it) },
                onSubmit = { 
                    viewModel.sendCommand()
                    scope.launch {
                        if (uiState.outputLines.isNotEmpty()) {
                            listState.animateScrollToItem(uiState.outputLines.size - 1)
                        }
                    }
                },
                focusRequester = focusRequester,
                enabled = uiState.isConnected
            )
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun TerminalTopBar(
    sessionState: String,
    isConnected: Boolean,
    onNewSession: () -> Unit,
    onCloseSession: () -> Unit
) {
    TopAppBar(
        title = {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Text(
                    text = "Terminal",
                    color = TerminalColors.text
                )
                
                // Connection indicator
                Box(
                    modifier = Modifier
                        .size(8.dp)
                        .background(
                            if (isConnected) Color(0xFF4CAF50) else Color(0xFFF44336),
                            shape = androidx.compose.foundation.shape.CircleShape
                        )
                )
                
                // Session state badge
                if (sessionState.isNotEmpty()) {
                    Surface(
                        color = when (sessionState) {
                            "Active" -> Color(0xFF2E7D32)
                            "Restored" -> Color(0xFF1565C0)
                            "Checkpointed" -> Color(0xFFF57C00)
                            else -> Color(0xFF757575)
                        },
                        shape = MaterialTheme.shapes.small
                    ) {
                        Text(
                            text = sessionState,
                            color = Color.White,
                            fontSize = 10.sp,
                            modifier = Modifier.padding(horizontal = 6.dp, vertical = 2.dp)
                        )
                    }
                }
            }
        },
        actions = {
            if (isConnected) {
                IconButton(onClick = onCloseSession) {
                    Text("✕", color = TerminalColors.text)
                }
            } else {
                TextButton(onClick = onNewSession) {
                    Text("New", color = TerminalColors.accent)
                }
            }
        },
        colors = TopAppBarDefaults.topAppBarColors(
            containerColor = TerminalColors.surface
        )
    )
}

@Composable
private fun TerminalLine(text: String) {
    Text(
        text = text,
        style = TextStyle(
            fontFamily = FontFamily.Monospace,
            fontSize = 13.sp,
            color = TerminalColors.text,
            lineHeight = 18.sp
        ),
        modifier = Modifier.fillMaxWidth()
    )
}

@Composable
private fun TerminalInput(
    value: String,
    onValueChange: (String) -> Unit,
    onSubmit: () -> Unit,
    focusRequester: FocusRequester,
    enabled: Boolean
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(TerminalColors.inputBackground)
            .padding(horizontal = 8.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        // Prompt
        Text(
            text = "$ ",
            style = TextStyle(
                fontFamily = FontFamily.Monospace,
                fontSize = 14.sp,
                color = TerminalColors.prompt
            )
        )
        
        // Input field
        BasicTextField(
            value = value,
            onValueChange = onValueChange,
            modifier = Modifier
                .weight(1f)
                .focusRequester(focusRequester),
            textStyle = TextStyle(
                fontFamily = FontFamily.Monospace,
                fontSize = 14.sp,
                color = TerminalColors.text
            ),
            cursorBrush = SolidColor(TerminalColors.cursor),
            keyboardOptions = KeyboardOptions(
                keyboardType = KeyboardType.Ascii,
                imeAction = ImeAction.Send
            ),
            keyboardActions = KeyboardActions(
                onSend = { onSubmit() }
            ),
            enabled = enabled,
            singleLine = true
        )
    }
}

@Composable
private fun ErrorBanner(
    message: String,
    onDismiss: () -> Unit
) {
    Surface(
        color = Color(0xFFB71C1C),
        modifier = Modifier.fillMaxWidth()
    ) {
        Row(
            modifier = Modifier
                .padding(horizontal = 16.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text(
                text = message,
                color = Color.White,
                modifier = Modifier.weight(1f),
                fontSize = 13.sp
            )
            
            TextButton(onClick = onDismiss) {
                Text("Dismiss", color = Color.White)
            }
        }
    }
}

/**
 * Terminal color scheme (Dracula-inspired)
 */
object TerminalColors {
    val background = Color(0xFF1E1E2E)
    val surface = Color(0xFF282A36)
    val inputBackground = Color(0xFF21222C)
    val text = Color(0xFFF8F8F2)
    val prompt = Color(0xFF50FA7B)
    val accent = Color(0xFF8BE9FD)
    val cursor = Color(0xFFF8F8F2)
    val error = Color(0xFFFF5555)
}
