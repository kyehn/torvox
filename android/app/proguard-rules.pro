# JNA (UniFFI runtime dependency)
-keep class com.sun.jna.** { *; }
-dontwarn java.awt.**
-dontwarn sun.awt.**

# UniFFI bridge types
-keep class io.torvox.bridge.** { *; }
