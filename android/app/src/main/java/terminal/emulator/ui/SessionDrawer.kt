package terminal.emulator.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Keyboard
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import terminal.emulator.R
import terminal.emulator.SessionInfo

@Composable
fun SessionDrawer(
    sessions: List<SessionInfo>,
    activeSessionId: Long,
    onSwitchSession: (Long) -> Unit,
    onCloseSession: (Long) -> Unit,
    onAddSession: () -> Unit,
    onRefreshSessions: () -> Unit,
    onSettings: () -> Unit,
    onSearch: () -> Unit,
    onKeyboardToggle: () -> Unit,
    onResetTerminal: () -> Unit,
    onClose: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val backgroundColor = MaterialTheme.colorScheme.surfaceVariant
    val textColor = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val surface = MaterialTheme.colorScheme.surface
    LaunchedEffect(Unit) {
        onRefreshSessions()
    }

    Column(
        modifier =
        modifier
            .fillMaxHeight()
            // 小屏/横屏可用宽度有限：抽屉占屏幕宽度的 84%，
            // 上限 320.dp，下限 240.dp，保证按钮行与会话项不溢出。
            .widthIn(min = 240.dp, max = 320.dp)
            .fillMaxWidth(0.84f)
            .background(backgroundColor)
            .testTag("SessionDrawer")
            // Requires AndroidManifest `windowSoftInputMode="adjustNothing"` —
            // imePadding adds keyboard-height bottom padding; adjustNothing
            // prevents the framework from resizing the activity (which would
            // change terminal grid rows/cols).  WindowInsets(0.dp) in Compose
            // alone does NOT prevent View.setImeWindowInsets() from modifying
            // mPaddingBottom at the View layer.
            .imePadding()
            .navigationBarsPadding(),
    ) {
        Spacer(modifier = Modifier.height(8.dp))

        SessionDrawerHeader(
            onClose = onClose,
            onAddSession = onAddSession,
            textColor = textColor,
            accent = accent,
        )

        Spacer(modifier = Modifier.height(8.dp))

        LazyColumn(
            modifier = Modifier.weight(1f),
            verticalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            itemsIndexed(sessions, key = { _, session -> session.id }) { index, session ->
                val sessionNumber = stringResource(R.string.session_number, index + 1)
                SessionItem(
                    title = sessionNumber,
                    subtitle =
                    session.directory.ifEmpty {
                        session.title.takeIf { it != sessionNumber }.orEmpty()
                    },
                    isActive = session.id == activeSessionId,
                    onClick = {
                        onSwitchSession(session.id)
                        onClose()
                    },
                    onClose = {
                        onCloseSession(session.id)
                    },
                    accent = accent,
                    surface = surface,
                    textColor = textColor,
                )
            }
        }

        SessionDrawerActions(
            onClose = onClose,
            onSearch = onSearch,
            onKeyboardToggle = onKeyboardToggle,
            onResetTerminal = onResetTerminal,
            onSettings = onSettings,
            textColor = textColor,
        )

        Spacer(modifier = Modifier.height(16.dp))
    }
}

@Composable
private fun SessionDrawerHeader(
    onClose: () -> Unit,
    onAddSession: () -> Unit,
    textColor: Color,
    accent: Color,
) {
    Row(
        modifier =
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = stringResource(R.string.sessions),
            color = textColor.copy(alpha = 0.7f),
            fontSize = 12.sp,
            fontWeight = FontWeight.Medium,
        )
        Icon(
            imageVector = Icons.Default.Add,
            contentDescription = stringResource(R.string.cd_new_session),
            tint = accent,
            modifier =
            Modifier
                .size(24.dp)
                .testTag("AddSessionButton")
                .clip(CircleShape)
                .clickable {
                    onClose()
                    onAddSession()
                }.padding(2.dp),
        )
    }
}

@Composable
private fun SessionDrawerActions(
    onClose: () -> Unit,
    onSearch: () -> Unit,
    onKeyboardToggle: () -> Unit,
    onResetTerminal: () -> Unit,
    onSettings: () -> Unit,
    textColor: Color,
) {
    Row(
        modifier =
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 12.dp, vertical = 6.dp),
        horizontalArrangement = Arrangement.SpaceEvenly,
    ) {
        DrawerActionButton(
            icon = Icons.Default.Search,
            label = stringResource(R.string.text_search),
            onClick = {
                onClose()
                onSearch()
            },
            textColor = textColor,
            testTag = "SearchButton",
        )
        DrawerActionButton(
            icon = Icons.Default.Keyboard,
            label = stringResource(R.string.toggle_keyboard),
            onClick = {
                onClose()
                onKeyboardToggle()
            },
            textColor = textColor,
            testTag = "KeyboardToggle",
        )
        DrawerActionButton(
            icon = Icons.Default.Refresh,
            label = stringResource(R.string.reset_terminal),
            onClick = {
                onClose()
                onResetTerminal()
            },
            textColor = textColor,
            testTag = "ResetTerminalButton",
        )
        DrawerActionButton(
            icon = Icons.Default.Settings,
            label = stringResource(R.string.settings_button),
            onClick = {
                onClose()
                onSettings()
            },
            textColor = textColor,
            testTag = "SettingsButton",
        )
    }
}

@Composable
private fun SessionItem(
    title: String,
    subtitle: String,
    isActive: Boolean,
    onClick: () -> Unit,
    onClose: () -> Unit,
    accent: Color,
    surface: Color,
    textColor: Color,
) {
    val itemBackgroundColor = if (isActive) surface else Color.Transparent
    val titleColor = if (isActive) textColor else textColor.copy(alpha = 0.7f)

    Row(
        modifier =
        Modifier
            .testTag("SessionItem")
            .fillMaxWidth()
            .padding(horizontal = 8.dp)
            .clip(RoundedCornerShape(8.dp))
            .background(itemBackgroundColor)
            .clickable(onClick = onClick)
            .padding(horizontal = 12.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(
            modifier =
            Modifier
                .size(8.dp)
                .clip(CircleShape)
                .background(if (isActive) accent else textColor.copy(alpha = 0.4f)),
        )
        Spacer(modifier = Modifier.width(12.dp))
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = title,
                color = titleColor,
                fontSize = 14.sp,
                fontWeight = FontWeight.Medium,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (subtitle.isNotEmpty()) {
                Text(
                    text = subtitle,
                    color = titleColor.copy(alpha = 0.7f),
                    fontSize = 12.sp,
                    fontStyle = FontStyle.Italic,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        Icon(
            imageVector = Icons.Default.Close,
            contentDescription = stringResource(R.string.cd_close_session),
            tint = textColor.copy(alpha = 0.6f),
            modifier =
            Modifier
                .size(18.dp)
                .clip(CircleShape)
                .clickable(onClick = onClose)
                .padding(2.dp),
        )
    }
}

@Composable
private fun DrawerActionButton(
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    label: String,
    onClick: () -> Unit,
    textColor: Color,
    testTag: String,
) {
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        modifier =
        Modifier
            .clip(RoundedCornerShape(8.dp))
            .clickable(onClick = onClick)
            .padding(horizontal = 12.dp, vertical = 8.dp)
            .testTag(testTag),
    ) {
        Icon(
            imageVector = icon,
            contentDescription = label,
            tint = textColor.copy(alpha = 0.8f),
            modifier = Modifier.size(22.dp),
        )
        Spacer(modifier = Modifier.height(4.dp))
        Text(
            text = label,
            color = textColor.copy(alpha = 0.7f),
            fontSize = 11.sp,
        )
    }
}
