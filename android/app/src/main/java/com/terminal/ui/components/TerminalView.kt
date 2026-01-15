package com.terminal.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * ANSI color codes to Compose colors
 */
object AnsiColors {
    // Standard colors (0-7)
    val black = Color(0xFF282A36)
    val red = Color(0xFFFF5555)
    val green = Color(0xFF50FA7B)
    val yellow = Color(0xFFF1FA8C)
    val blue = Color(0xFF6272A4)
    val magenta = Color(0xFFFF79C6)
    val cyan = Color(0xFF8BE9FD)
    val white = Color(0xFFF8F8F2)
    
    // Bright colors (8-15)
    val brightBlack = Color(0xFF6272A4)
    val brightRed = Color(0xFFFF6E6E)
    val brightGreen = Color(0xFF69FF94)
    val brightYellow = Color(0xFFFFFFA5)
    val brightBlue = Color(0xFFD6ACFF)
    val brightMagenta = Color(0xFFFF92DF)
    val brightCyan = Color(0xFFA4FFFF)
    val brightWhite = Color(0xFFFFFFFF)
    
    fun fromCode(code: Int, bright: Boolean = false): Color {
        return when (code) {
            0 -> if (bright) brightBlack else black
            1 -> if (bright) brightRed else red
            2 -> if (bright) brightGreen else green
            3 -> if (bright) brightYellow else yellow
            4 -> if (bright) brightBlue else blue
            5 -> if (bright) brightMagenta else magenta
            6 -> if (bright) brightCyan else cyan
            7 -> if (bright) brightWhite else white
            else -> white
        }
    }
    
    /**
     * Parse 256-color code
     */
    fun from256(code: Int): Color {
        return when {
            code < 16 -> fromCode(code % 8, code >= 8)
            code < 232 -> {
                // 216 color cube (6x6x6)
                val n = code - 16
                val r = (n / 36) * 51
                val g = ((n / 6) % 6) * 51
                val b = (n % 6) * 51
                Color(r, g, b)
            }
            else -> {
                // 24 grayscale
                val gray = (code - 232) * 10 + 8
                Color(gray, gray, gray)
            }
        }
    }
}

/**
 * Parsed ANSI text with styling
 */
data class StyledText(
    val text: String,
    val foreground: Color = AnsiColors.white,
    val background: Color? = null,
    val bold: Boolean = false,
    val italic: Boolean = false,
    val underline: Boolean = false
)

/**
 * Parse ANSI escape sequences and return styled text segments
 */
fun parseAnsi(text: String): List<StyledText> {
    val result = mutableListOf<StyledText>()
    var currentFg = AnsiColors.white
    var currentBg: Color? = null
    var bold = false
    var italic = false
    var underline = false
    
    val buffer = StringBuilder()
    var i = 0
    
    while (i < text.length) {
        if (text[i] == '\u001b' && i + 1 < text.length && text[i + 1] == '[') {
            // Found escape sequence
            if (buffer.isNotEmpty()) {
                result.add(StyledText(buffer.toString(), currentFg, currentBg, bold, italic, underline))
                buffer.clear()
            }
            
            // Find end of sequence
            var j = i + 2
            while (j < text.length && text[j] !in 'A'..'Z' && text[j] !in 'a'..'z') {
                j++
            }
            
            if (j < text.length) {
                val command = text[j]
                val params = text.substring(i + 2, j)
                
                if (command == 'm') {
                    // SGR (Select Graphic Rendition)
                    val codes = if (params.isEmpty()) listOf(0) else params.split(';').mapNotNull { it.toIntOrNull() }
                    
                    var codeIdx = 0
                    while (codeIdx < codes.size) {
                        when (val code = codes[codeIdx]) {
                            0 -> {
                                // Reset
                                currentFg = AnsiColors.white
                                currentBg = null
                                bold = false
                                italic = false
                                underline = false
                            }
                            1 -> bold = true
                            3 -> italic = true
                            4 -> underline = true
                            22 -> bold = false
                            23 -> italic = false
                            24 -> underline = false
                            in 30..37 -> currentFg = AnsiColors.fromCode(code - 30, bold)
                            38 -> {
                                // Extended foreground
                                if (codeIdx + 2 < codes.size && codes[codeIdx + 1] == 5) {
                                    currentFg = AnsiColors.from256(codes[codeIdx + 2])
                                    codeIdx += 2
                                }
                            }
                            39 -> currentFg = AnsiColors.white
                            in 40..47 -> currentBg = AnsiColors.fromCode(code - 40)
                            48 -> {
                                // Extended background
                                if (codeIdx + 2 < codes.size && codes[codeIdx + 1] == 5) {
                                    currentBg = AnsiColors.from256(codes[codeIdx + 2])
                                    codeIdx += 2
                                }
                            }
                            49 -> currentBg = null
                            in 90..97 -> currentFg = AnsiColors.fromCode(code - 90, bright = true)
                            in 100..107 -> currentBg = AnsiColors.fromCode(code - 100, bright = true)
                        }
                        codeIdx++
                    }
                }
                
                i = j + 1
            } else {
                i++
            }
        } else {
            buffer.append(text[i])
            i++
        }
    }
    
    if (buffer.isNotEmpty()) {
        result.add(StyledText(buffer.toString(), currentFg, currentBg, bold, italic, underline))
    }
    
    return result
}

/**
 * Convert styled text to AnnotatedString
 */
fun List<StyledText>.toAnnotatedString(): AnnotatedString {
    return buildAnnotatedString {
        for (segment in this@toAnnotatedString) {
            val style = SpanStyle(
                color = segment.foreground,
                background = segment.background ?: Color.Transparent,
                fontWeight = if (segment.bold) FontWeight.Bold else FontWeight.Normal
            )
            pushStyle(style)
            append(segment.text)
            pop()
        }
    }
}

/**
 * A single terminal line with ANSI color support
 */
@Composable
fun ColoredTerminalLine(
    text: String,
    modifier: Modifier = Modifier,
    fontSize: Int = 13
) {
    val annotatedString = remember(text) {
        parseAnsi(text).toAnnotatedString()
    }
    
    BasicText(
        text = annotatedString,
        style = TextStyle(
            fontFamily = FontFamily.Monospace,
            fontSize = fontSize.sp,
            lineHeight = (fontSize + 5).sp
        ),
        modifier = modifier
    )
}

/**
 * Calculate terminal dimensions based on container size
 */
@Composable
fun rememberTerminalDimensions(
    containerWidth: Dp,
    containerHeight: Dp,
    fontSizeSp: Int = 13
): Pair<Int, Int> {
    val density = LocalDensity.current
    
    return remember(containerWidth, containerHeight, fontSizeSp) {
        with(density) {
            // Approximate character dimensions for monospace
            val charWidth = (fontSizeSp * 0.6).dp.toPx()
            val charHeight = (fontSizeSp * 1.4).dp.toPx()
            
            val cols = (containerWidth.toPx() / charWidth).toInt().coerceIn(20, 500)
            val rows = (containerHeight.toPx() / charHeight).toInt().coerceIn(5, 200)
            
            cols to rows
        }
    }
}
