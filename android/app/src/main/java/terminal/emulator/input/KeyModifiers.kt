package terminal.emulator.input

import android.view.KeyCharacterMap
import android.view.KeyEvent

/**
 * 修饰键掩码：**上游 `key.Mods` 的原始位**，全仓唯一一种表示。
 *
 * 曾经有两套并存：应用自定义位（SHIFT=1/ALT=2/CTRL=4/META=8）与上游位
 * （SHIFT=1/CTRL=2/ALT=4/SUPER=8）。两套形状相近而位值不同，每个转换点都是一次
 * 可能错位的机会，而鼠标与按键路径还各走一套。现统一为上游位：原生侧
 * `Mods::from_bits_retain` 与 `encode_modifiers` 消费同一套。
 */
object KeyModifiers {
    const val SHIFT = 1
    const val CTRL = 2
    const val ALT = 4
    const val SUPER = 8

    /**
     * 仅取工具栏粘滞状态。布局感知路径中 Shift 已并入生成的字符，故不进掩码。
     */
    fun fromStickyStates(ctrlState: ModifierState, altState: ModifierState): Int {
        var mask = 0
        if (ctrlState.isActive()) {
            mask = mask or CTRL
        }
        if (altState.isActive()) {
            mask = mask or ALT
        }
        return mask
    }

    /**
     * 硬件按键的完整掩码：物理按键状态与工具栏粘滞状态取并。
     *
     * 右 Alt（AltGr）不计入 [ALT]：它是布局的字符级修饰，`getUnicodeChar` 已把
     * 它并入产出的字符，上行发不发送 Meta 前缀由终端侧的工具条粘滞 Alt 决定。
     * `isAltPressed` 不区分左右，必须显式读 `META_ALT_RIGHT_ON`——
     * 拦截路径（`TerminalViewModel.handleLayoutAwareHardwareKey`）注释里
     * 「左 Alt 交给编码器路径」在这里才是真正生效的地方。
     */
    fun fromKeyEvent(event: KeyEvent, ctrlState: ModifierState, altState: ModifierState): Int {
        var mask = fromStickyStates(ctrlState, altState)
        if (event.isShiftPressed) mask = mask or SHIFT
        if (event.isCtrlPressed) mask = mask or CTRL
        val rightAltPressed = event.metaState and KeyEvent.META_ALT_RIGHT_ON != 0
        if (event.isAltPressed && !rightAltPressed) mask = mask or ALT
        if (event.isMetaPressed) mask = mask or SUPER
        return mask
    }

    /**
     * Android [KeyEvent.metaState] → 本掩码。触摸与滚轮路径没有 KeyEvent 的修饰
     * 查询可用，只能读 metaState（鼠标编码直接用这套位）。
     */
    fun ghosttyMods(metaState: Int): Int {
        var mask = 0
        if (metaState and KeyEvent.META_SHIFT_ON != 0) mask = mask or SHIFT
        if (metaState and KeyEvent.META_CTRL_ON != 0) mask = mask or CTRL
        if (metaState and KeyEvent.META_ALT_ON != 0) mask = mask or ALT
        if (metaState and KeyEvent.META_META_ON != 0) mask = mask or SUPER
        return mask
    }
}

/**
 * 死键（组合符）状态机。
 *
 * AZERTY、US-International、Nordic 等布局靠组合符打出 `è é ü ñ @ €`：组合键本身
 * 不产生任何字符，要等下一次击键才与它合成。丢弃组合键会让这些布局只能打出裸字母
 * （`e` 而不是 `è`），而组合键不产生输出在协议上完全正常，肉眼看不出少了什么。
 *
 * [compose] 默认取 Android 键位表的 `getDeadChar`；可注入以便单测不依赖平台实现。
 */
class DeadKeyState(private val compose: (Int, Int) -> Int = KeyCharacterMap::getDeadChar) {
    /** 待合成的组合符；0 表示无。 */
    var accent: Int = 0
        private set

    /**
     * [KeyEvent.getUnicodeChar] 的返回值是否为死键。
     *
     * 死键的值带 `COMBINING_ACCENT` 最高位，故作为有符号整数恒为负——不能靠
     * `> 0` 之外的判断顺带识别。
     */
    fun isDead(unicodeChar: Int): Boolean = unicodeChar and KeyCharacterMap.COMBINING_ACCENT != 0

    /** 记下死键。连按两个组合键以最后一个为准：前者没有可合成的对象。 */
    fun hold(unicodeChar: Int) {
        accent = unicodeChar and KeyCharacterMap.COMBINING_ACCENT_MASK
    }

    /** 取出并作废待合成的组合符：合成只对紧接着的那一次击键生效。 */
    fun take(): Int = accent.also { accent = 0 }

    /** [accent] 与 [base] 合成；无待合成组合符或键位表里没有该组合时返回 0。 */
    fun composeWith(accent: Int, base: Int): Int = if (accent == 0) 0 else compose(accent, base)
}
