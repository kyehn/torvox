package terminal.emulator.runtime

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class RunCommandPayloadTest {
    private val json = Json { ignoreUnknownKeys = true }

    private fun parse(payload: String): Map<String, String> {
        val obj = json.parseToJsonElement(payload).jsonObject
        return buildMap {
            put("exit_code", obj.getValue("exit_code").jsonPrimitive.int.toString())
            put("err_code", obj.getValue("err_code").jsonPrimitive.int.toString())
            put("stdout", obj.getValue("stdout").jsonPrimitive.content)
            put("stderr", obj.getValue("stderr").jsonPrimitive.content)
        }
    }

    @Test
    fun `normal exit carries exit_code and err_code 0`() {
        val json = parse(runCommandPayload(0, 0, "hello", ""))
        assertEquals("0", json["exit_code"])
        assertEquals("0", json["err_code"])
        assertEquals("hello", json["stdout"])
        assertEquals("", json["stderr"])
    }

    @Test
    fun `non-zero exit is not an app failure`() {
        // termux: exitCode non-zero does NOT mean execution failed.
        val json = parse(runCommandPayload(127, 0, "", "command not found"))
        assertEquals("127", json["exit_code"])
        assertEquals("0", json["err_code"])
    }

    @Test
    fun `timeout keeps exit_code -1 with err_code 1`() {
        val json = parse(runCommandPayload(-1, 1, "", ""))
        assertEquals("-1", json["exit_code"])
        assertEquals("1", json["err_code"])
    }

    @Test
    fun `exception maps to err_code 2`() {
        val json = parse(runCommandPayload(-1, 2, "", "IllegalArgumentException: boom"))
        assertEquals("-1", json["exit_code"])
        assertEquals("2", json["err_code"])
        assertTrue(json["stderr"].orEmpty().contains("boom"))
    }

    @Test
    fun `payload is valid JSON with escaped content`() {
        val json = parse(runCommandPayload(0, 0, "line1\nline2 \"quoted\" \\ path", "err\tline"))
        assertEquals("line1\nline2 \"quoted\" \\ path", json["stdout"])
        assertEquals("err\tline", json["stderr"])
    }

    // ── round-227 T4b: full C0 control escaping (spec d4) ────────────────

    @Test
    fun `backspace and formfeed are escaped`() {
        // \b (0x08) and \f (0x0C) were previously embedded raw, producing
        // invalid JSON that strict parsers reject.
        val json = parse(runCommandPayload(0, 0, "a\u0008b", "c\u000Cd"))
        assertEquals("a\u0008b", json["stdout"])
        assertEquals("c\u000Cd", json["stderr"])
    }

    @Test
    fun `every C0 control byte produces valid JSON`() {
        // All 0x00-0x1F bytes must round-trip through the payload as
        // valid JSON (parsing proves the escaping is well-formed).
        for (code in 0..0x1F) {
            val value = code.toChar().toString()
            val json = parse(runCommandPayload(0, 0, value, value))
            assertEquals(value, json["stdout"])
            assertEquals(value, json["stderr"])
        }
    }

    @Test
    fun `nul byte survives round-trip`() {
        val json = parse(runCommandPayload(0, 0, "a\u0000b", ""))
        assertEquals("a\u0000b", json["stdout"])
    }

    @Test
    fun `timeout and exception are distinguishable by err_code alone`() {
        val timeout = parse(runCommandPayload(-1, 1, "", ""))
        val exception = parse(runCommandPayload(-1, 2, "", "Class: msg"))
        assertEquals("-1", timeout["exit_code"])
        assertEquals("-1", exception["exit_code"])
        assertEquals("1", timeout["err_code"])
        assertEquals("2", exception["err_code"])
    }

    @Test
    fun `exit_code is clamped to 0-255 per spec d4`() {
        // 256 → 0 (wrap)
        val wrap = parse(runCommandPayload(256, 0, "", ""))
        assertEquals("0", wrap["exit_code"])
        // 512 → 0
        val large = parse(runCommandPayload(512, 0, "", ""))
        assertEquals("0", large["exit_code"])
        // 255 stays 255
        val max = parse(runCommandPayload(255, 0, "", ""))
        assertEquals("255", max["exit_code"])
        // -1 (timeout) stays -1
        val timeout = parse(runCommandPayload(-1, 1, "", ""))
        assertEquals("-1", timeout["exit_code"])
        // Normal exit 7 unchanged
        val normal = parse(runCommandPayload(7, 0, "", ""))
        assertEquals("7", normal["exit_code"])
    }
}
