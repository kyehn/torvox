package terminal.emulator.bell

enum class BellMode(val id: Int, val displayName: String) {
    SOUND(0, "Sound"),
    VIBRATE(1, "Vibrate"),
    SCREEN_FLASH(2, "Screen Flash"),
    SILENT(3, "Silent"),
    ;

    companion object {
        fun fromId(id: Int): BellMode = entries.find { it.id == id } ?: SOUND
    }
}
