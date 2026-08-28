package terminal.emulator.ui

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
import terminal.emulator.R

@Composable
fun TextSearchBar(
    query: String,
    onQueryChange: (String) -> Unit,
    resultCount: Int,
    currentResultIndex: Int,
    onPrevious: () -> Unit,
    onNext: () -> Unit,
    onClose: () -> Unit,
    modifier: Modifier = Modifier,
    caseSensitive: Boolean = false,
    onCaseSensitiveToggle: (Boolean) -> Unit = {},
    autoCaseSensitive: Boolean = false,
    fuzzyMatch: Boolean = false,
    onFuzzyMatchToggle: (Boolean) -> Unit = {},
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
    val fuzzyMatchDescription =
        if (fuzzyMatch) {
            stringResource(R.string.disable_fuzzy_match)
        } else {
            stringResource(R.string.enable_fuzzy_match)
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
                contentDescription = fuzzyMatchDescription
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
    SearchNavButton(
        imageVector = Icons.Filled.KeyboardArrowUp,
        description = stringResource(R.string.search_previous),
        resultCount = resultCount,
        onClick = onPrevious,
        testTag = "SearchPrevious",
    )
    SearchNavButton(
        imageVector = Icons.Filled.KeyboardArrowDown,
        description = stringResource(R.string.search_next),
        resultCount = resultCount,
        onClick = onNext,
        testTag = "SearchNext",
    )
}

/** Arrow step button for the search bar, dimmed when there are no results. */
@Composable
private fun SearchNavButton(
    imageVector: androidx.compose.ui.graphics.vector.ImageVector,
    description: String,
    resultCount: Int,
    onClick: () -> Unit,
    testTag: String,
) {
    IconButton(
        onClick = onClick,
        enabled = resultCount > 0,
        modifier = Modifier.size(32.dp).testTag(testTag),
    ) {
        Icon(
            imageVector = imageVector,
            contentDescription = description,
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
