package terminal.emulator.ui

import androidx.annotation.StringRes
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
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import terminal.emulator.R
import terminal.emulator.SessionInfo

/** 抽屉里每个会话的关闭按钮 testTag 前缀：测试据此关闭指定会话，无需按位置猜节点。 */
const val CLOSE_SESSION_TAG_PREFIX = "CloseSession-"

@Composable
fun SessionDrawer(
    sessions: List<SessionInfo>,
    activeSessionId: Long,
    onSwitchSession: (Long) -> Unit,
    onCloseSession: (Long) -> Unit,
    onAddSession: () -> Unit,
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

    Column(
        modifier =
        modifier
            .fillMaxHeight()
            // 抽屉宽度 = 抽屉可用宽度的 75%，上限 280.dp。
            // `fillMaxWidth(0.75f)` 交给下游的是 min=max=0.75×W 的单点约束，`widthIn`
            // 把该点夹进区间。
            // 下限 MUST NOT 设置：ZTE 折叠屏外屏可用宽度只有约 236dp，0.75×236≈177dp；
            // 历史上的 240dp 下限会把抽屉抬宽到比可用宽度还宽，已移除且不得重新引入。
            // 按钮行与会话项在 177dp 上不溢出，无需下限保底；上限 280.dp 收窄宽屏
            // （0.75×411dp≈308dp），并给终端留出可辨认的一条。
            .fillMaxWidth(0.75f)
            .widthIn(max = 280.dp)
            .background(backgroundColor)
            .testTag("SessionDrawer")
            // 需要 AndroidManifest 的 `windowSoftInputMode="adjustNothing"`
            // ——imePadding 会加入键盘高度的底部内边距；adjustNothing
            // 则阻止框架调整 Activity 尺寸（那会改变终端网格的行列）。
            // 仅在 Compose 侧用 WindowInsets(0.dp)
            // 并不能阻止 View.setImeWindowInsets() 在视图层修改 mPaddingBottom。
            .imePadding()
            .navigationBarsPadding(),
    ) {
        Spacer(modifier = Modifier.height(8.dp))

        Text(
            text = stringResource(R.string.sessions),
            color = textColor.copy(alpha = 0.7f),
            fontSize = 12.sp,
            fontWeight = FontWeight.Medium,
            modifier = Modifier.padding(horizontal = 16.dp),
        )

        Spacer(modifier = Modifier.height(8.dp))

        LazyColumn(
            modifier = Modifier.weight(1f),
            verticalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            itemsIndexed(sessions, key = { _, session -> session.sessionId }) { index, session ->
                val label =
                    sessionItemLabel(
                        indexLabel = stringResource(R.string.session_index, index + 1),
                        title = session.title,
                        workingDirectory = session.workingDirectory,
                    )
                SessionItem(
                    label = label,
                    isActive = session.sessionId == activeSessionId,
                    closeTestTag = "$CLOSE_SESSION_TAG_PREFIX${session.sessionId}",
                    onClick = {
                        onSwitchSession(session.sessionId)
                        onClose()
                    },
                    onClose = {
                        onCloseSession(session.sessionId)
                    },
                    accent = accent,
                    surface = surface,
                    textColor = textColor,
                )
            }
        }

        AddSessionButton(
            onClick = {
                onClose()
                onAddSession()
            },
            textColor = textColor,
            accent = accent,
        )

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

/**
 * 会话列表底部的「新建会话」按钮。
 *
 * 位置按 `DESIGN.md` 侧边面板节：新增入口 MUST 位于会话列表底部，MUST NOT 放在标题栏。
 */
@Composable
private fun AddSessionButton(onClick: () -> Unit, textColor: Color, accent: Color) {
    Row(
        modifier =
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 12.dp)
            .clip(RoundedCornerShape(8.dp))
            .clickable(onClick = onClick)
            .padding(vertical = 6.dp)
            .testTag("AddSessionButton"),
        horizontalArrangement = Arrangement.Center,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            imageVector = Icons.Default.Add,
            contentDescription = stringResource(R.string.cd_new_session),
            tint = accent,
            modifier = Modifier.size(18.dp),
        )
        Spacer(modifier = Modifier.width(8.dp))
        Text(
            text = stringResource(R.string.new_session),
            color = textColor,
            fontSize = 13.sp,
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
    // 四个动作排成两行两列：两行的布局完全相同，只有数据不同。用一份列表 + chunked
    // 生成，避免把同一段 Row/weight 样板抄两遍，改每行个数也只改常量。
    // 顺序按 `DESIGN.md` 侧边面板节：重置终端 文本搜索 / 显示、隐藏输入法 设置。
    val actions =
        listOf(
            DrawerAction(Icons.Default.Refresh, R.string.reset_terminal, "ResetTerminalButton", onResetTerminal),
            DrawerAction(Icons.Default.Search, R.string.text_search, "SearchButton", onSearch),
            DrawerAction(Icons.Default.Keyboard, R.string.toggle_keyboard, "KeyboardToggle", onKeyboardToggle),
            DrawerAction(Icons.Default.Settings, R.string.settings_button, "SettingsButton", onSettings),
        )
    Column(
        modifier =
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 12.dp, vertical = 2.dp),
        verticalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        actions.chunked(ACTIONS_PER_ROW).forEach { row ->
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceEvenly,
            ) {
                row.forEach { action ->
                    DrawerActionButton(
                        icon = action.icon,
                        label = stringResource(action.labelRes),
                        onClick = {
                            onClose()
                            action.onClick()
                        },
                        textColor = textColor,
                        testTag = action.testTag,
                        modifier = Modifier.weight(1f),
                    )
                }
            }
        }
    }
}

/** 每行动作数。[SessionDrawerActions] 的四个动作按此分块生成行。 */
private const val ACTIONS_PER_ROW = 2

/** 抽屉底部的一个动作：图标、文案资源、testTag 与回调。 */
private class DrawerAction(
    val icon: ImageVector,
    @StringRes val labelRes: Int,
    val testTag: String,
    val onClick: () -> Unit,
)

/**
 * 会话条目文案：`[序号] 标题`——序号加粗、终端标题斜体，两者同行。
 *
 * 与 termux 无会话名时的渲染一致（`TermuxSessionsListViewController.getView` 的
 * `numberPart + sessionTitlePart`）：termux 只在有会话名时才换行，本应用没有会话名，
 * 因此标题始终跟在序号后。标题为空时只剩序号，不占第二行。
 *
 * 标题为空而 [workingDirectory] 已知时改显目录：多数 shell 不设标题，此时目录是
 * 唯一能区分两个会话的信息。两者都空（shell 尚未上报任何 OSC）时只留序号——
 * 补一个「未知」只是噪声。
 */
internal fun sessionItemLabel(indexLabel: String, title: String, workingDirectory: String = ""): AnnotatedString =
    buildAnnotatedString {
        withStyle(SpanStyle(fontWeight = FontWeight.Bold)) { append("$indexLabel ") }
        val secondary = title.ifEmpty { workingDirectory }
        if (secondary.isNotEmpty()) {
            withStyle(SpanStyle(fontStyle = FontStyle.Italic)) { append(secondary) }
        }
    }

@Composable
private fun SessionItem(
    label: AnnotatedString,
    isActive: Boolean,
    closeTestTag: String,
    onClick: () -> Unit,
    onClose: () -> Unit,
    accent: Color,
    surface: Color,
    textColor: Color,
) {
    val itemBackgroundColor = if (isActive) surface else Color.Transparent
    val labelColor = if (isActive) textColor else textColor.copy(alpha = 0.7f)

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
        Text(
            text = label,
            color = labelColor,
            fontSize = 14.sp,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
        Icon(
            imageVector = Icons.Default.Close,
            contentDescription = stringResource(R.string.cd_close_session),
            tint = textColor.copy(alpha = 0.6f),
            modifier =
            Modifier
                .size(18.dp)
                .clip(CircleShape)
                .testTag(closeTestTag)
                .clickable(onClick = onClose)
                .padding(2.dp),
        )
    }
}

@Composable
private fun DrawerActionButton(
    icon: ImageVector,
    label: String,
    onClick: () -> Unit,
    textColor: Color,
    testTag: String,
    modifier: Modifier = Modifier,
) {
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        modifier =
        modifier
            .clip(RoundedCornerShape(8.dp))
            .clickable(onClick = onClick)
            .padding(horizontal = 4.dp, vertical = 4.dp)
            .testTag(testTag),
    ) {
        Icon(
            imageVector = icon,
            contentDescription = label,
            tint = textColor.copy(alpha = 0.8f),
            modifier = Modifier.size(20.dp),
        )
        Spacer(modifier = Modifier.height(2.dp))
        Text(
            text = label,
            color = textColor.copy(alpha = 0.7f),
            fontSize = 11.sp,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}
