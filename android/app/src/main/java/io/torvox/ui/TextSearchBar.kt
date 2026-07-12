package io.torvox.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.KeyboardArrowUp
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEvent
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.platform.SoftwareKeyboardController
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.torvox.R

private fun isWideChar(ch: Char): Boolean {
    val type = Character.getType(ch)
    return type == Character.OTHER_SYMBOL.toInt() ||
        type == Character.LETTER_NUMBER.toInt() ||
        type == Character.ENCLOSING_MARK.toInt() ||
        ch.code in 0x1100..0x115F ||
        ch.code in 0x2E80..0x9FFF ||
        ch.code in 0xA000..0xA4CF ||
        ch.code in 0xAC00..0xD7AF ||
        ch.code in 0xF900..0xFAFF ||
        ch.code in 0xFE30..0xFE6F ||
        ch.code in 0xFF01..0xFF60 ||
        ch.code in 0xFFE0..0xFFE6 ||
        ch.code in 0x20000..0x2FA1F ||
        ch.code in 0x30000..0x3134F
}

private fun charCellWidth(ch: Char): Int = if (isWideChar(ch)) 2 else 1

private fun charIndexToCellColumn(
    line: String,
    charIndex: Int,
): Int {
    var col = 0
    for (i in 0 until charIndex.coerceAtMost(line.length)) {
        col += charCellWidth(line[i])
    }
    return col
}

fun findMatches(
    text: String,
    query: String,
    matchCase: Boolean = false,
): List<SearchResult> {
    if (query.isEmpty()) return emptyList()
    val lines = text.split("\n")
    val results = mutableListOf<SearchResult>()
    for ((lineIndex, line) in lines.withIndex()) {
        val searchLine = if (matchCase) line else line.lowercase()
        val searchQuery = if (matchCase) query else query.lowercase()
        var startIndex = 0
        while (true) {
            val foundIndex = searchLine.indexOf(searchQuery, startIndex)
            if (foundIndex == -1) break
            results.add(
                SearchResult(
                    lineIndex = lineIndex,
                    startIndex = charIndexToCellColumn(line, foundIndex),
                    endIndex = charIndexToCellColumn(line, foundIndex + query.length),
                ),
            )
            startIndex = foundIndex + 1
        }
    }
    return results
}

@Composable
fun TextSearchBar(
    query: String,
    onQueryChange: (String) -> Unit,
    resultCount: Int,
    currentResultIndex: Int,
    onPrevious: () -> Unit,
    onNext: () -> Unit,
    onClose: () -> Unit,
    caseSensitive: Boolean = false,
    onCaseSensitiveToggle: (Boolean) -> Unit = {},
    autoCaseSensitive: Boolean = false,
    fuzzyMatch: Boolean = false,
    onFuzzyMatchToggle: (Boolean) -> Unit = {},
    modifier: Modifier = Modifier,
) {
    val focusRequester = remember { FocusRequester() }
    val keyboardController = LocalSoftwareKeyboardController.current
    val escapeHandler: (KeyEvent) -> Boolean = {
        if (it.type == KeyEventType.KeyUp && it.key == Key.Escape) {
            keyboardController?.hide()
            onClose()
            true
        } else {
            false
        }
    }

    LaunchedEffect(Unit) { focusRequester.requestFocus() }

    Row(
        modifier =
        modifier
            .fillMaxWidth()
            .height(56.dp)
            .background(MaterialTheme.colorScheme.surfaceContainerHigh)
            .padding(horizontal = 8.dp, vertical = 0.dp)
            .onPreviewKeyEvent(escapeHandler),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        SearchTextField(
            query = query,
            onQueryChange = onQueryChange,
            focusRequester = focusRequester,
            onNext = onNext,
            modifier = Modifier.weight(1f),
        )

        SearchToggleButtons(
            caseSensitive = caseSensitive,
            autoCaseSensitive = autoCaseSensitive,
            fuzzyMatch = fuzzyMatch,
            onCaseSensitiveToggle = onCaseSensitiveToggle,
            onFuzzyMatchToggle = onFuzzyMatchToggle,
        )

        SearchResultCounter(query, resultCount, currentResultIndex)
        SearchNavButtons(resultCount, onPrevious, onNext)
        SearchCloseButton(onClose, keyboardController)
    }
}

@Composable
private fun SearchToggleButtons(
    caseSensitive: Boolean,
    autoCaseSensitive: Boolean,
    fuzzyMatch: Boolean,
    onCaseSensitiveToggle: (Boolean) -> Unit,
    onFuzzyMatchToggle: (Boolean) -> Unit,
) {
    val aaColor =
        when {
            caseSensitive -> MaterialTheme.colorScheme.primary
            autoCaseSensitive -> MaterialTheme.colorScheme.primary.copy(alpha = 0.6f)
            else -> MaterialTheme.colorScheme.onSurfaceVariant
        }

    IconButton(
        onClick = { onCaseSensitiveToggle(!caseSensitive) },
        modifier = Modifier.size(32.dp).testTag("SearchCaseSensitive"),
    ) {
        Text(
            text = "Aa",
            fontSize = 13.sp,
            color = aaColor,
        )
    }

    IconButton(
        onClick = { onFuzzyMatchToggle(!fuzzyMatch) },
        modifier =
        Modifier
            .size(32.dp)
            .testTag("SearchFuzzyMatch")
            .semantics {
                contentDescription = if (fuzzyMatch) "Disable fuzzy match" else "Enable fuzzy match"
            },
    ) {
        Text(
            text = "~",
            fontSize = 13.sp,
            color =
            if (fuzzyMatch) {
                MaterialTheme.colorScheme.primary
            } else {
                MaterialTheme.colorScheme.onSurfaceVariant
            },
        )
    }
}

@Composable
private fun SearchResultCounter(
    query: String,
    resultCount: Int,
    currentResultIndex: Int,
) {
    Spacer(modifier = Modifier.width(4.dp))
    if (query.isNotEmpty()) {
        Text(
            text =
            if (resultCount == 0) {
                stringResource(R.string.search_no_results)
            } else {
                stringResource(R.string.search_result_of, currentResultIndex + 1, resultCount)
            },
            fontSize = 12.sp,
            color =
            if (resultCount == 0) {
                MaterialTheme.colorScheme.error
            } else {
                MaterialTheme.colorScheme.onSurfaceVariant
            },
            modifier = Modifier.testTag("SearchResultCount"),
        )
    }
}

@Composable
private fun SearchNavButtons(
    resultCount: Int,
    onPrevious: () -> Unit,
    onNext: () -> Unit,
) {
    Spacer(modifier = Modifier.width(4.dp))
    IconButton(
        onClick = onPrevious,
        enabled = resultCount > 0,
        modifier = Modifier.size(32.dp).testTag("SearchPrevious"),
    ) {
        Icon(
            imageVector = Icons.Filled.KeyboardArrowUp,
            contentDescription = stringResource(R.string.search_previous),
            tint =
            if (resultCount > 0) {
                MaterialTheme.colorScheme.onSurface
            } else {
                MaterialTheme.colorScheme.onSurface.copy(alpha = 0.3f)
            },
            modifier = Modifier.size(20.dp),
        )
    }

    IconButton(
        onClick = onNext,
        enabled = resultCount > 0,
        modifier = Modifier.size(32.dp).testTag("SearchNext"),
    ) {
        Icon(
            imageVector = Icons.Filled.KeyboardArrowDown,
            contentDescription = stringResource(R.string.search_next),
            tint =
            if (resultCount > 0) {
                MaterialTheme.colorScheme.onSurface
            } else {
                MaterialTheme.colorScheme.onSurface.copy(alpha = 0.3f)
            },
            modifier = Modifier.size(20.dp),
        )
    }
}

@Composable
private fun SearchTextField(
    query: String,
    onQueryChange: (String) -> Unit,
    focusRequester: FocusRequester,
    onNext: () -> Unit,
    modifier: Modifier = Modifier,
) {
    OutlinedTextField(
        value = query,
        onValueChange = onQueryChange,
        modifier =
        modifier
            .focusRequester(focusRequester)
            .testTag("SearchTextField"),
        placeholder = {
            Text(
                text = stringResource(R.string.search_placeholder),
                color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.5f),
                fontSize = 14.sp,
            )
        },
        singleLine = true,
        textStyle = MaterialTheme.typography.bodyMedium.copy(color = MaterialTheme.colorScheme.onSurface),
        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Next),
        keyboardActions =
        KeyboardActions(
            onNext = { onNext() },
        ),
        colors =
        OutlinedTextFieldDefaults.colors(
            focusedBorderColor = MaterialTheme.colorScheme.primary,
            unfocusedBorderColor = MaterialTheme.colorScheme.outline,
            cursorColor = MaterialTheme.colorScheme.primary,
        ),
    )
}

@Composable
private fun SearchCloseButton(
    onClose: () -> Unit,
    keyboardController: SoftwareKeyboardController?,
) {
    IconButton(
        onClick = {
            keyboardController?.hide()
            onClose()
        },
        modifier = Modifier.size(32.dp).testTag("SearchClose"),
    ) {
        Icon(
            imageVector = Icons.Default.Close,
            contentDescription = stringResource(R.string.search_close),
            tint = MaterialTheme.colorScheme.onSurface,
            modifier = Modifier.size(20.dp),
        )
    }
}
