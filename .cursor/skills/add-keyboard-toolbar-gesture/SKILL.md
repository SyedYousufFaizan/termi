---
name: "add-keyboard-toolbar-gesture"
description: "Worked example for adding sticky-Ctrl modifier and swipe-based history cycling to CommandToolbar.kt. Use when implementing the remaining Phase 1c keyboard UX items."
icon: "code"
color: "blue"
---

This is the Phase 1c "cheapest, highest-visibility win" item — pure
Compose UI, no Rust changes needed. `CommandToolbar.kt` already has
Esc/Home/End buttons and the basic Ctrl+C/D/Z/Tab/arrow buttons; this
skill covers the two remaining pieces.

## 1. Sticky-Ctrl modifier

Goal: tap "Ctrl" once, it visually arms, the next character typed sends
that character's control code instead of the literal character, then it
disarms automatically.

```kotlin
// In TerminalScreen.kt (or wherever CommandToolbar is hosted) —
// this state needs to live above the toolbar since it also affects how
// regular keyboard input is interpreted, not just toolbar buttons:
var ctrlArmed by remember { mutableStateOf(false) }

// Toolbar button:
ToolbarButton(
    text = "Ctrl",
    // Use a different visual state when armed — e.g. filled vs outlined —
    // so the user has clear feedback the modifier is "waiting."
    onClick = { ctrlArmed = !ctrlArmed }
)

// Wherever regular character input is sent to the PTY (likely in
// TerminalViewModel or the text input handler in TerminalView.kt):
fun sendChar(c: Char) {
    if (ctrlArmed) {
        val controlCode = (c.uppercaseChar().code - 'A'.code + 1)
        if (controlCode in 1..26) {
            onCommand(controlCode.toChar().toString())
        }
        ctrlArmed = false  // disarm after one use
    } else {
        onCommand(c.toString())
    }
}
```

Check `TerminalViewModel.kt` and `TerminalView.kt` for where character
input actually currently gets sent to the PTY before wiring this in — the
sketch above assumes a single `sendChar`/`onCommand` chokepoint, which may
need confirming against the actual current implementation.

## 2. Swipe-based history cycling

Goal: swipe left/right on the toolbar area to cycle command history,
as an alternative to repeatedly tapping ↑/↓.

```kotlin
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.ui.input.pointer.pointerInput

Row(
    modifier = modifier
        .fillMaxWidth()
        .pointerInput(Unit) {
            detectHorizontalDragGestures { change, dragAmount ->
                change.consume()
                if (dragAmount > 20) onArrowUp()   // swipe right → older
                else if (dragAmount < -20) onArrowDown()  // swipe left → newer
            }
        }
        // ...existing modifiers (background, horizontalScroll, padding)
) { /* existing buttons */ }
```

Watch out: `horizontalScroll` and `detectHorizontalDragGestures` on the
same `Row` will fight each other for the drag gesture. Test this — you
may need to move the gesture detector to a dedicated thin strip above the
toolbar rather than the scrollable button row itself, so scrolling the
button row and swiping for history don't conflict.

## 3. Testing

Compose UI gestures are hard to unit test meaningfully without a device
or `@Composable` test harness. At minimum:
- Manually verify on an emulator or device that Ctrl-arming visually
  updates and correctly sends control codes for a few letters (try
  Ctrl+R for reverse-search, Ctrl+L for clear — both have visible,
  checkable effects in a real shell).
- Verify the swipe gesture doesn't break existing horizontal scroll of
  the button row.
- If Gradle/instrumented tests are available in your environment, add a
  `androidTest` case; otherwise, explicitly note that this was only
  reviewed for logical correctness, not run, per
  `.cursor/rules/20-android-kotlin.mdc`.
