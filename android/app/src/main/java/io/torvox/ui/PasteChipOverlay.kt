package io.torvox.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlin.math.roundToInt

@Composable
@Suppress("LongParameterList")
fun PasteChipOverlay(
    row: Int,
    col: Int,
    cellWidth: Float,
    cellHeight: Float,
    scrollOffset: Int,
    onPaste: () -> Unit,
    accentColor: Color,
    backgroundColor: Color,
) {
    val visibleRow = (row - scrollOffset).coerceAtLeast(0)
    val chipX = col * cellWidth
    val chipY = (visibleRow + 1) * cellHeight + 4f

    Box(
        modifier =
        Modifier
            .testTag("PasteChipOverlay")
            .offset {
                IntOffset(chipX.roundToInt(), chipY.roundToInt())
            }.clip(RoundedCornerShape(6.dp))
            .background(backgroundColor)
            .border(1.dp, accentColor, RoundedCornerShape(6.dp))
            .clickable { onPaste() }
            .padding(horizontal = 12.dp, vertical = 6.dp),
    ) {
        Text(
            text = "Paste",
            color = accentColor,
            fontSize = 12.sp,
        )
    }
}
