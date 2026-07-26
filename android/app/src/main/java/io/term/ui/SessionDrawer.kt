package io.term.ui

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
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import io.term.R
import io.term.TerminalViewModel

@Composable
fun SessionDrawer(
    viewModel: TerminalViewModel,
    onSettings: () -> Unit,
    onSearch: () -> Unit,
    onClose: () -> Unit,
) {
    val state by viewModel.state.collectAsState()
    val backgroundColor = MaterialTheme.colorScheme.surfaceVariant
    val textColor = MaterialTheme.colorScheme.onSurfaceVariant
    val accent = MaterialTheme.colorScheme.primary
    val surface = MaterialTheme.colorScheme.surface

    Column(
        modifier =
        Modifier
            .fillMaxHeight()
            .width(280.dp)
            .background(backgroundColor)
            .testTag("SessionDrawer")
            .imePadding()
            .navigationBarsPadding(),
    ) {
        Spacer(modifier = Modifier.height(8.dp))

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
                        viewModel.createSession()
                    }.padding(2.dp),
            )
        }

        Spacer(modifier = Modifier.height(8.dp))

        LazyColumn(
            modifier = Modifier.weight(1f),
            verticalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            items(state.sessions) { session ->
                SessionItem(
                    title = session.title,
                    isActive = session.id == state.activeSessionId,
                    onClick = {
                        viewModel.switchSession(session.id)
                        onClose()
                    },
                    onClose = {
                        viewModel.closeSession(session.id)
                    },
                    accent = accent,
                    surface = surface,
                    textColor = textColor,
                )
            }
        }

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

        Spacer(modifier = Modifier.height(16.dp))
    }
}

@Composable
private fun SessionItem(
    title: String,
    isActive: Boolean,
    onClick: () -> Unit,
    onClose: () -> Unit,
    accent: Color,
    surface: Color,
    textColor: Color,
) {
    val bgColor = if (isActive) surface else Color.Transparent
    val titleColor = if (isActive) textColor else textColor.copy(alpha = 0.7f)

    Row(
        modifier =
        Modifier
            .testTag("SessionItem")
            .fillMaxWidth()
            .padding(horizontal = 8.dp)
            .clip(RoundedCornerShape(8.dp))
            .background(bgColor)
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
        Text(
            text = title,
            color = titleColor,
            fontSize = 14.sp,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
        if (!isActive) {
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
