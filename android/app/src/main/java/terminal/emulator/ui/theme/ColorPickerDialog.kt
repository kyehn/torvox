package terminal.emulator.ui.theme

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import terminal.emulator.R

/** Format a [Color] as #RRGGBB for the hex field / row captions. */
internal fun colorToHex(color: Color): String {
    val rgb = color.toArgb() and 0xFFFFFF
    return "#" + rgb.toString(16).padStart(6, '0').uppercase()
}

/** Parse #RRGGBB (leading '#' optional) into an opaque [Color]. */
internal fun colorFromHex(hex: String): Color {
    // ARGB as a Long (Int literal 0xFF000000 would still be Int — but we
    // need the OR of a runtime value, so build the Long then convert via
    // toInt(): `Color(Int)` is the ARGB constructor, `Color(Long)` would
    // treat the value as a packed color-space ULong (wrong).
    val argb = 0xFF000000L or (hex.removePrefix("#").toLong(16))
    return Color(argb.toInt())
}

/**
 * HSV color picker (ghostty-android ColorPickerDialog.java:46 pattern, pure
 * Compose — no library): SV saturation/value field + hue bar + #RRGGBB hex
 * input.
 *
 * All controls write to one HSV source of truth, so there is no
 * listener-reentry loop of the Java original; its `updating[]` guard
 * (:104) lives here as `hexFocused` — the hex field is only rewritten
 * programmatically when the user is not typing in it. [onColorChange] gives
 * the caller live preview of every drag; [onConfirm] returns the final
 * color.
 */
@Suppress("LongMethod")
@Composable
fun ColorPickerDialog(
    title: String,
    initialColor: Color,
    onColorChange: (Color) -> Unit,
    onConfirm: (Color) -> Unit,
    onDismiss: () -> Unit,
) {
    // HSV seeded once from the initial color (ghostty ColorPickerDialog :62).
    // The dialog keeps its own state while open; `initialColor` is frozen by
    // the caller (ThemeEditorDialog's `pickerInitial`), so live preview
    // feedback can never re-seed the picker.
    val initialHsv =
        remember(initialColor) {
            val hsv = FloatArray(3)
            android.graphics.Color.colorToHSV(initialColor.toArgb(), hsv)
            hsv
        }
    var hue by remember { mutableFloatStateOf(initialHsv[0]) }
    var sat by remember { mutableFloatStateOf(initialHsv[1]) }
    var value by remember { mutableFloatStateOf(initialHsv[2]) }
    val currentColor = remember(hue, sat, value) { Color.hsv(hue, sat, value) }
    var hexText by remember { mutableStateOf(colorToHex(initialColor)) }
    var hexFocused by remember { mutableStateOf(false) }
    // Skip the first emission: opening must not fire a bogus "change".
    var firstEmission by remember { mutableStateOf(true) }

    // Live preview while dragging.
    LaunchedEffect(currentColor) {
        if (firstEmission) {
            firstEmission = false
            return@LaunchedEffect
        }
        onColorChange(currentColor)
    }

    // Canonicalize the hex field after slider changes — but never while the
    // user is typing (updating[] guard pattern).
    LaunchedEffect(currentColor) {
        if (!hexFocused) hexText = colorToHex(currentColor)
    }

    Dialog(onDismissRequest = onDismiss) {
        Surface(
            shape = RoundedCornerShape(16.dp),
            color = MaterialTheme.colorScheme.surfaceContainerHigh,
        ) {
            Column(
                modifier =
                Modifier
                    .fillMaxWidth()
                    .widthIn(max = 380.dp)
                    .padding(16.dp),
            ) {
                Text(title, style = MaterialTheme.typography.titleMedium)
                Spacer(Modifier.height(12.dp))
                Row(verticalAlignment = Alignment.CenterVertically) {
                    SwatchWithCaption("Old", initialColor, Modifier.weight(1f))
                    Spacer(Modifier.width(10.dp))
                    SwatchWithCaption("New", currentColor, Modifier.weight(1f))
                    Spacer(Modifier.width(10.dp))
                    Text(
                        colorToHex(currentColor),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                Spacer(Modifier.height(12.dp))
                SvField(hue = hue, sat = sat, value = value, onSatValue = { s, v ->
                    sat = s
                    value = v
                })
                Spacer(Modifier.height(10.dp))
                HueBar(hue = hue, onHue = { hue = it })
                Spacer(Modifier.height(14.dp))
                OutlinedTextField(
                    value = hexText,
                    onValueChange = { raw ->
                        // Only 0-9A-F survive (ghostty HexFilter :366).
                        val digits =
                            raw.removePrefix("#").uppercase()
                                .filter { it.isDigit() || it in 'A'..'F' }
                                .take(6)
                        hexText = "#$digits"
                        if (digits.length == 6) {
                            val typed = colorFromHex(digits)
                            val hsv = FloatArray(3)
                            android.graphics.Color.colorToHSV(typed.toArgb(), hsv)
                            hue = hsv[0]
                            sat = hsv[1]
                            value = hsv[2]
                        }
                    },
                    label = { Text("Hex") },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Ascii),
                    modifier = Modifier
                        .fillMaxWidth()
                        .onFocusChanged { hexFocused = it.isFocused }
                        .testTag("HexInput"),
                )
                Spacer(Modifier.height(12.dp))
                Row(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    OutlinedButton(onClick = onDismiss, modifier = Modifier.weight(1f)) {
                        Text(stringResource(R.string.cancel))
                    }
                    Button(
                        onClick = { onConfirm(currentColor) },
                        modifier = Modifier.weight(1f).testTag("ConfirmColorButton"),
                    ) { Text(stringResource(R.string.save)) }
                }
            }
        }
    }
}

@Composable
private fun SwatchWithCaption(
    caption: String,
    color: Color,
    modifier: Modifier = Modifier,
) {
    Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = modifier) {
        Box(
            modifier =
            Modifier
                .size(36.dp)
                .clip(RoundedCornerShape(6.dp))
                .background(color)
                .border(1.dp, Color.Black.copy(alpha = 0.15f), RoundedCornerShape(6.dp)),
        )
        Spacer(Modifier.height(2.dp))
        Text(
            caption,
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

/**
 * SV (saturation/value) square: hue fixed, x = saturation, y = 1 - value.
 * Drawn as the canonical two-gradient composite: white→hue horizontally
 * (full value), then transparent→black vertically (value falloff).
 */
@Composable
private fun SvField(
    hue: Float,
    sat: Float,
    value: Float,
    onSatValue: (Float, Float) -> Unit,
) {
    val hueColor = Color.hsv(hue, 1f, 1f)
    Canvas(
        modifier =
        Modifier
            .fillMaxWidth()
            .height(180.dp)
            .clip(RoundedCornerShape(8.dp))
            .pointerInput(Unit) {
                fun update(pos: Offset) {
                    onSatValue(
                        (pos.x / size.width).coerceIn(0f, 1f),
                        (1f - pos.y / size.height).coerceIn(0f, 1f),
                    )
                }
                detectTapGestures { update(it) }
                detectDragGestures(onDrag = { change, _ ->
                    change.consume()
                    update(change.position)
                })
            },
    ) {
        drawRect(brush = Brush.horizontalGradient(listOf(Color.White, hueColor)))
        drawRect(brush = Brush.verticalGradient(listOf(Color.Transparent, Color.Black)))
        val thumb = Offset(sat * size.width, (1f - value) * size.height)
        drawCircle(Color.Black, radius = 8f, center = thumb)
        drawCircle(Color.White, radius = 8f, center = thumb)
        drawCircle(Color.Black, radius = 7f, center = thumb, style = Stroke(width = 1.5f))
    }
}

/** Hue bar: 0-360° rainbow gradient with a thumb marker. */
@Composable
private fun HueBar(
    hue: Float,
    onHue: (Float) -> Unit,
) {
    Canvas(
        modifier =
        Modifier
            .fillMaxWidth()
            .height(22.dp)
            .clip(RoundedCornerShape(6.dp))
            .pointerInput(Unit) {
                fun update(pos: Offset) = onHue((pos.x / size.width * 360f).coerceIn(0f, 360f))
                detectTapGestures { update(it) }
                detectDragGestures(onDrag = { change, _ ->
                    change.consume()
                    update(change.position)
                })
            },
    ) {
        drawRect(
            brush =
            Brush.horizontalGradient(
                listOf(
                    Color.Red,
                    Color.Yellow,
                    Color.Green,
                    Color.Cyan,
                    Color.Blue,
                    Color.Magenta,
                    Color.Red,
                ),
            ),
        )
        val x = hue / 360f * size.width
        drawLine(Color.White, Offset(x, 1f), Offset(x, size.height - 1f), strokeWidth = 2f)
        drawLine(Color.Black, Offset(x - 1f, 1f), Offset(x - 1f, size.height - 1f), strokeWidth = 1f)
        drawLine(Color.Black, Offset(x + 1f, 1f), Offset(x + 1f, size.height - 1f), strokeWidth = 1f)
    }
}
