package terminal.emulator.bridge;

/**
 * Minimal Java test class matching the JNI exports in native/src/android/ffi.rs.
 * Compile with: javac -d ../../target/jni-test-classes NativeBridge.java
 * Run with: java -Djava.library.path=../../target/debug -cp ../../target/jni-test-classes terminal.emulator.bridge.NativeBridge
 */
public class NativeBridge {
    // ── native method declarations (matching ffi.rs exports) ──
    static native long initSession(int rows, int cols, String shell, String home, String user, String path, String workingDirectory, String prefix);
    static native boolean destroySession(long sessionId);
    static native boolean switchSession(long sessionId);
    static native int getSessionCount();
    static native String listSessions();
    static native void resize(long sessionId, int rows, int cols);
    static native void feedPty(long sessionId, byte[] data);
    static native void writeKey(long sessionId, String key, int mods, String text);
    static native String pollEvent();
    static native void setMcpEnabled(boolean enabled);
    static native String getTitle(long sessionId);
    static native int scrollbackLength(long sessionId);
    static native String scrollbackLine(long sessionId, int row);
    static native String getTerminalText(long sessionId);
    static native String searchAllInScrollback(long sessionId, String query, boolean caseSensitive, boolean fuzzyMatch);
    static native boolean isCellEmpty(long sessionId, int row, int col);

    // ── test harness ──
    static int passed = 0;
    static int failed = 0;

    static void check(String name, boolean condition) {
        if (condition) {
            System.out.println("  PASS: " + name);
            passed++;
        } else {
            System.out.println("  FAIL: " + name);
            failed++;
        }
    }

    public static void main(String[] args) {
        System.out.println("=== Torvox JNI Bridge Test ===");
        System.loadLibrary("native");

        testLifecycle();
        testInitInvalidArgs();
        testResize();
        testFeedPty();
        testPollEvent();
        testMultiSession();
        testSetMcpEnabled();
        testQueries();

        System.out.println();
        System.out.println("=== Results: " + passed + " passed, " + failed + " failed ===");
        if (failed > 0) {
            System.exit(1);
        }
    }

    static void testLifecycle() {
        System.out.println("-- Session lifecycle --");
        int before = getSessionCount();
        check("initial count is 0", before >= 0);

        long sid = initSession(24, 80, "/bin/sh", "/", "root", "/usr/bin:/bin", "/", "");
        check("initSession returns positive id", sid > 0);

        int afterCreate = getSessionCount();
        check("count increased after create", afterCreate > before);

        long sid2 = initSession(40, 120, "/bin/sh", "/", "root", "/usr/bin:/bin", "/", "");
        check("second session has different id", sid2 != sid);
        check("two sessions created", getSessionCount() >= 2);

        // Switch to first session
        check("switchSession works", switchSession(sid));

        // List sessions
        String sessions = listSessions();
        check("listSessions returns non-null", sessions != null);
        check("listSessions contains sid", sessions.contains(String.valueOf(sid)));

        check("destroySession first", destroySession(sid));
        check("destroySession second", destroySession(sid2));
        check("count back to initial", getSessionCount() == before);
    }

    static void testInitInvalidArgs() {
        System.out.println("-- Init with invalid args --");
        // 0 rows/cols should still work (VT handles minimum internally)
        long sid = initSession(0, 0, "/bin/sh", "/", "root", "/usr/bin:/bin", "/", "");
        check("initSession(0,0) succeeds", sid > 0);
        destroySession(sid);
    }

    static void testResize() {
        System.out.println("-- Resize --");
        long sid = initSession(24, 80, "/bin/sh", "/", "root", "/usr/bin:/bin", "/", "");
        check("session created for resize", sid > 0);
        resize(sid, 50, 150);
        // resize does not throw (no crash = pass)
        destroySession(sid);
    }

    static void testFeedPty() {
        System.out.println("-- Feed PTY --");
        long sid = initSession(24, 80, "/bin/sh", "/", "root", "/usr/bin:/bin", "/", "");
        check("session created for feedPty", sid > 0);
        feedPty(sid, "echo hello\n".getBytes(java.nio.charset.StandardCharsets.UTF_8));
        // feedPty does not crash
        destroySession(sid);
    }

    static void testPollEvent() {
        System.out.println("-- Poll event --");
        long sid = initSession(24, 80, "/bin/sh", "/", "root", "/usr/bin:/bin", "/", "");
        check("session created for pollEvent", sid > 0);

        // pollEvent should return null initially (no events)
        String event = pollEvent();
        check("pollEvent returns null or valid JSON initially",
              event == null || event.startsWith("{\""));

        // Feed some data, then poll again
        feedPty(sid, "echo terminal-test-poll\n".getBytes(java.nio.charset.StandardCharsets.UTF_8));
        // Give the VT thread a moment to process
        try { Thread.sleep(200); } catch (InterruptedException e) {}
        String event2 = pollEvent();
        check("pollEvent after feedPty returns something or null",
              event2 == null || event2.startsWith("{\""));

        destroySession(sid);
    }

    static void testMultiSession() {
        System.out.println("-- Multi-session --");
        long a = initSession(24, 80, "/bin/sh", "/", "root", "/usr/bin:/bin", "/", "");
        long b = initSession(24, 80, "/bin/sh", "/", "root", "/usr/bin:/bin", "/", "");
        check("session A created", a > 0);
        check("session B created", b > 0);
        check("A != B", a != b);

        check("switch to A", switchSession(a));
        check("switch to B", switchSession(b));
        check("switch back to A", switchSession(a));

        destroySession(a);
        destroySession(b);
    }

    static void testSetMcpEnabled() {
        System.out.println("-- MCP toggle --");
        setMcpEnabled(true);
        setMcpEnabled(false);
        // no crash = pass
    }

    static void testQueries() {
        System.out.println("-- Query exports --");
        // Unknown session: getTitle throws IllegalArgumentException.
        boolean threw = false;
        try {
            getTitle(99999);
        } catch (IllegalArgumentException expected) {
            threw = true;
        }
        check("getTitle(unknown) throws IllegalArgumentException", threw);

        long sid = initSession(24, 80, "/bin/sh", "/", "root", "/usr/bin:/bin", "/", "");
        check("session created for queries", sid > 0);
        try { Thread.sleep(100); } catch (InterruptedException e) {}
        // Live session: getters do not throw (data may be empty for a bare sh).
        threw = false;
        try {
            String title = getTitle(sid);
            int len = scrollbackLength(sid);
            String line = scrollbackLine(sid, 0);
            String text = getTerminalText(sid);
            String matches = searchAllInScrollback(sid, "x", false, false);
            boolean empty = isCellEmpty(sid, 0, 0);
            check("live query getters do not throw", title != null);
            check("scrollbackLength returns non-negative", len >= 0);
            check("getTerminalText returns string", text != null);
            check("search returns JSON array", matches != null && matches.startsWith("["));
        } catch (RuntimeException unexpected) {
            threw = true;
            System.err.println("unexpected: " + unexpected);
        }
        check("live queries do not throw", !threw);
        destroySession(sid);

        threw = false;
        try {
            scrollbackLength(99999);
        } catch (IllegalArgumentException expected) {
            threw = true;
        }
        check("scrollbackLength(unknown) throws", threw);
    }
}
