package terminal.emulator.ui.theme

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import terminal.emulator.R

private const val ANSI_LABELS = "0123456789ABCDEF"

/**
 * Full-theme color editor (ghostty-android ThemeActivity working-copy
 * model): edits a throwaway copy of the theme — the original is untouched
 * until Save/Save-as — with a dirty flag that enables those buttons.
 *
 * Each color row opens [ColorPickerDialog]; the picker's live preview
 * writes into the working copy via [editTarget] + [updating] (ghostty
 * `updating[]` re-entry guard, ColorPickerDialog.java:104), and only the
 * color being edited is fed back — never the theme itself — so there is no
 * recursive recomposition loop.
 */
@Suppress("LongMethod")
@Composable
fun ThemeEditorDialog(
    theme: TerminalTheme,
    isOverwriteExisting: Boolean,
    onSaveAsNew: (TerminalTheme) -> Unit,
    onOverwrite: (TerminalTheme) -> Unit,
    onDismiss: () -> Unit,
) {
    // Working copy (ghostty ThemeActivity :71): a fresh list instance, so
    // the caller's TerminalTheme is never mutated.
    var working by remember(theme) {
        mutableStateOf(
            theme.copy(
                ansi = theme.ansi.toMutableList(),
            ),
        )
    }
    var dirty by remember { mutableStateOf(false) }
    var newThemeName by remember(theme) { mutableStateOf(theme.name) }

    // Guard against re-entrancy: `updating` is only true while a picker is
    // open, and its onColorChange only receives the single color being
    // edited. The docs mention `updating[]` as a single boolean guarding
    // the three picker listeners; applied here to the editor row → picker
    // handoff instead of mutable State, because we treat edits as a
    // function of the copy (drag feedback over a fresh duration is
    // sufficient; recomposition is cheap).
    var updating by remember { mutableStateOf(false) }
    var pickerInitial by remember { mutableStateOf<Color?>(null) }
    var editTarget by remember { mutableStateOf<ColorTarget?>(null) }

    Dialog(onDismissRequest = onDismiss) {
        Surface(
            shape = RoundedCornerShape(16.dp),
            color = MaterialTheme.colorScheme.surfaceContainerHigh,
        ) {
            Column(
                modifier =
                Modifier
                    .fillMaxWidth()
                    .widthIn(max = 420.dp)
                    .padding(16.dp),
            ) {
                Text(
                    stringResource(
                        if (isOverwriteExisting) R.string.edit_theme_title else R.string.create_theme_title,
                        theme.name,
                    ),
                    style = MaterialTheme.typography.titleMedium,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                Spacer(Modifier.height(12.dp))

                // Name for "Save as new": prefilled with the source theme's
                // name; "Overwrite" keeps the original name regardless.
                OutlinedTextField(
                    value = newThemeName,
                    onValueChange = { newThemeName = it },
                    label = { Text(stringResource(R.string.theme_name)) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth().testTag("EditedThemeName"),
                )

                Spacer(Modifier.height(12.dp))

                // Preview: sample terminal output on the working copy's
                // background (ghostty ThemePreviewView idea).
                ThemeSamplePreview(working)

                Spacer(Modifier.height(12.dp))
                Text(
                    stringResource(R.string.tap_a_color_to_edit),
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Spacer(Modifier.height(8.dp))

                Column(
                    modifier =
                    Modifier
                        .fillMaxWidth()
                        .height(340.dp)
                        .verticalScroll(rememberScrollState()),
                ) {
                    ColorRow(
                        label = stringResource(R.string.background),
                        color = working.background,
                        testTag = "EditBackground",
                        onClick = {
                            updating = true
                            pickerInitial = working.background
                            editTarget = ColorTarget.Background
                        },
                    )
                    ColorRow(
                        label = stringResource(R.string.foreground),
                        color = working.foreground,
                        testTag = "EditForeground",
                        onClick = {
                            updating = true
                            pickerInitial = working.foreground
                            editTarget = ColorTarget.Foreground
                        },
                    )
                    ColorRow(
                        label = stringResource(R.string.cursor_label),
                        color = working.cursor,
                        testTag = "EditCursor",
                        onClick = {
                            updating = true
                            pickerInitial = working.cursor
                            editTarget = ColorTarget.Cursor
                        },
                    )
                    ColorRow(
                        label = stringResource(R.string.selection),
                        color = working.selectionBg,
                        testTag = "EditSelection",
                        onClick = {
                            updating = true
                            pickerInitial = working.selectionBg
                            editTarget = ColorTarget.Selection
                        },
                    )
                    ANSI_LABELS.forEachIndexed { index, c ->
                        ColorRow(
                            label = stringResource(R.string.ansi_color, index, c),
                            color = working.ansi[index],
                            testTag = "EditAnsi$index",
                            onClick = {
                                updating = true
                                pickerInitial = working.ansi[index]
                                editTarget = ColorTarget.Ansi(index)
                            },
                        )
                    }
                }

                Spacer(Modifier.height(12.dp))
                Row(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    OutlinedButton(
                        onClick = onDismiss,
                        modifier = Modifier.weight(1f),
                    ) { Text(stringResource(R.string.cancel)) }
                    if (isOverwriteExisting) {
                        FilledTonalButton(
                            onClick = {
                                if (dirty) onOverwrite(working)
                            },
                            enabled = dirty,
                            modifier = Modifier.weight(1f).testTag("OverwriteThemeButton"),
                        ) { Text(stringResource(R.string.overwrite)) }
                    }
                    Button(
                        onClick = { onSaveAsNew(working.copy(name = newThemeName.trim().ifEmpty { theme.name })) },
                        enabled = dirty,
                        modifier = Modifier.weight(1f).testTag("SaveAsNewThemeButton"),
                    ) { Text(stringResource(R.string.save_as_new)) }
                }
            }
        }
    }

    // The one place the working copy is mutated: the picker callback for
    // the single color being edited. `pickerInitial` holds the pre-pop
    // value so the callback has a stable seed for the picker's HSV state.
    val target = editTarget
    val initial = pickerInitial
    if (target != null && initial != null && updating) {
        val targetName =
            when (target) {
                ColorTarget.Background -> stringResource(R.string.background)
                ColorTarget.Foreground -> stringResource(R.string.foreground)
                ColorTarget.Cursor -> stringResource(R.string.cursor_label)
                ColorTarget.Selection -> stringResource(R.string.selection)
                is ColorTarget.Ansi -> stringResource(R.string.ansi_color, target.index, ANSI_LABELS[target.index].toString())
            }
        ColorPickerDialog(
            title =
            stringResource(if (isOverwriteExisting) R.string.edit_theme_title else R.string.create_theme_title, targetName),
            initialColor = initial,
            onColorChange = { color ->
                when (target) {
                    ColorTarget.Background -> working = working.copy(background = color)

                    ColorTarget.Foreground -> working = working.copy(foreground = color)

                    ColorTarget.Cursor -> working = working.copy(cursor = color)

                    ColorTarget.Selection -> working = working.copy(selectionBg = color)

                    is ColorTarget.Ansi -> {
                        val updated = working.ansi.toMutableList()
                        updated[target.index] = color
                        working = working.copy(ansi = updated)
                    }
                }
                dirty = true
            },
            onConfirm = { color ->
                updating = false
                editTarget = null
                pickerInitial = null
            },
            onDismiss = {
                updating = false
                editTarget = null
                pickerInitial = null
            },
        )
    }
}

/** Which color the theme editor is currently editing (drives the picker
 *  callback dispatch and the row labels). */
private sealed interface ColorTarget {
    data object Background : ColorTarget

    data object Foreground : ColorTarget

    data object Cursor : ColorTarget

    data object Selection : ColorTarget

    data class Ansi(val index: Int) : ColorTarget
}

/** One editable color: label + current-color swatch + hex caption. */
@Composable
private fun ColorRow(
    label: String,
    color: Color,
    testTag: String,
    onClick: () -> Unit,
) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier =
        Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .testTag(testTag)
            .padding(vertical = 6.dp),
    ) {
        Box(
            modifier =
            Modifier
                .size(26.dp)
                .clip(RoundedCornerShape(5.dp))
                .background(color)
                .border(1.dp, MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f), RoundedCornerShape(5.dp)),
        )
        Spacer(Modifier.width(10.dp))
        Text(
            label,
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurface,
            modifier = Modifier.weight(1f),
        )
        Text(
            colorToHex(color),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

/** Mini terminal preview: prompt + colored text on the theme background. */
@Composable
private fun ThemeSamplePreview(theme: TerminalTheme) {
    Column(
        modifier =
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(8.dp))
            .background(theme.background)
            .padding(10.dp),
    ) {
        Text(
            "torvox@device:~$ echo hello",
            color = theme.foreground,
            style = MaterialTheme.typography.bodyMedium.copy(fontFamily = FontFamily.Monospace),
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        Text(
            "hello",
            color = theme.ansi[2],
            style = MaterialTheme.typography.bodyMedium.copy(fontFamily = FontFamily.Monospace),
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        Text(
            "warning: disk almost full",
            color = theme.ansi[3],
            style = MaterialTheme.typography.bodyMedium.copy(fontFamily = FontFamily.Monospace),
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        Text(
            "error: command not found",
            color = theme.ansi[1],
            style = MaterialTheme.typography.bodyMedium.copy(fontFamily = FontFamily.Monospace),
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        Spacer(Modifier.height(6.dp))
        Row(modifier = Modifier.fillMaxWidth()) {
            theme.ansi.forEach { color ->
                Box(modifier = Modifier.size(10.dp).background(color))
                Spacer(Modifier.width(2.dp))
            }
        }
        Spacer(Modifier.height(6.dp))
        Box(
            modifier =
            Modifier
                .width(10.dp)
                .height(14.dp)
                .background(theme.cursor),
        )
    }
}
