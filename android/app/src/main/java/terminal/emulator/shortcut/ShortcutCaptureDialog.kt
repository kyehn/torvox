package terminal.emulator.shortcut

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import terminal.emulator.R

/**
 * Compose dialog that records one hardware-keyboard shortcut from the
 * physical keyboard (reterminal ShortcutCaptureDialog.kt:47 pattern).
 *
 * Behavior:
 * - A [FocusRequester] is attached to the dialog modifier; the request is
 *   (re-)issued from [LaunchedEffect] so the first hardware key press is
 *   not eaten by window activation.
 * - [onPreviewKeyEvent] captures the native Android [android.view.KeyEvent]
 *   via the Compose wrapper's nativeKeyEvent property, then converts to
 *   [ShortcutBinding]. Ctrl/Shift/Alt/Meta chords are supported; a bare
 *   modifier key, key auto-repeat, and reserved system keys are ignored.
 * - [conflictDetector] vetoes a key that another action already uses so
 *   two actions can never share one chord.
 */
@Composable
fun ShortcutCaptureDialog(
    current: ShortcutBinding,
    conflictDetector: (ShortcutBinding) -> Boolean,
    onDismiss: () -> Unit,
    onSave: (ShortcutBinding) -> Unit,
) {
    var captured by remember { mutableStateOf(current) }
    var feedback by remember { mutableStateOf<String?>(null) }
    val focusRequester = remember { FocusRequester() }
    LaunchedEffect(Unit) { focusRequester.requestFocus() }

    AlertDialog(
        onDismissRequest = onDismiss,
        modifier =
        Modifier
            .focusRequester(focusRequester)
            .onPreviewKeyEvent { composeEvent ->
                // Extract the native android.view.KeyEvent from the Compose wrapper.
                // nativeKeyEvent is a member property of androidx.compose.ui.input.key.KeyEvent.
                @Suppress("UNCHECKED_CAST")
                val nativeEvent = try {
                    composeEvent.javaClass
                        .getMethod("getNativeKeyEvent")
                        .invoke(composeEvent) as? android.view.KeyEvent
                } catch (_: Exception) {
                    null
                }

                if (nativeEvent != null) {
                    captureKeyEvent(nativeEvent, captured, conflictDetector) { newBinding, newFeedback ->
                        captured = newBinding
                        feedback = newFeedback
                    }
                }
                true
            },
        title = { Text(stringResource(R.string.shortcut_capture_title)) },
        text = {
            Column {
                Text(
                    text = stringResource(R.string.shortcut_capture_prompt),
                    style = MaterialTheme.typography.bodyMedium,
                )
                Spacer(Modifier.height(16.dp))
                Text(
                    text = captured.toDisplayString(),
                    style = MaterialTheme.typography.headlineSmall,
                    textAlign = TextAlign.Center,
                    modifier = Modifier.fillMaxWidth(),
                )
                feedback?.let { msg ->
                    Spacer(Modifier.height(8.dp))
                    Text(
                        text = msg,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.error,
                        textAlign = TextAlign.Center,
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
            }
        },
        confirmButton = {
            TextButton(
                onClick = { onSave(captured) },
                modifier = Modifier.testTag("ShortcutCaptureSave"),
            ) {
                Text(stringResource(R.string.save))
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text(stringResource(R.string.cancel))
            }
        },
    )
}

/**
 * Process a native [android.view.KeyEvent] and update captured binding + feedback.
 */
private fun captureKeyEvent(
    event: android.view.KeyEvent,
    current: ShortcutBinding,
    conflictDetector: (ShortcutBinding) -> Boolean,
    onResult: (ShortcutBinding, String?) -> Unit,
) {
    if (event.action != android.view.KeyEvent.ACTION_DOWN || event.repeatCount > 0) {
        return
    }
    if (event.keyCode in ShortcutBinding.RESERVED_KEY_CODES) {
        onResult(current, "Reserved key")
        return
    }
    if (!event.isCtrlPressed && !event.isShiftPressed && !event.isAltPressed && !event.isMetaPressed) {
        onResult(current, "Press a modifier + key")
        return
    }
    val binding = ShortcutBinding(
        key = event.keyCode,
        ctrl = event.isCtrlPressed,
        shift = event.isShiftPressed,
        alt = event.isAltPressed,
        meta = event.isMetaPressed,
    )
    if (conflictDetector(binding)) {
        onResult(current, "Already in use")
        return
    }
    onResult(binding, null)
}
