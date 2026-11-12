-dontoptimize
-dontobfuscate
-keep class terminal.emulator.** { *; }
-keepclassmembers class * {
    @dagger.hilt.android.lifecycle.HiltViewModel <init>(...);
}
-keepattributes *Annotation*, RuntimeVisibleAnnotations, RuntimeInvisibleAnnotations, RuntimeVisibleParameterAnnotations, RuntimeInvisibleParameterAnnotations, DefaultAnnotation, AnnotationDefault, Retention
-keepattributes Signature
-keepattributes InnerClasses
-keepattributes EnclosingMethod

-keep class io.cucumber.** { *; }
-keep class terminal.emulator.cucumber.** { *; }

# Release 构建剥离调试级日志代码（DESIGN 日志节「Release 版本构建时应该剥离
# DEBUG 和 TRACE 日志代码」）：调试级的调用点连同其消息构造一起移除，故 release
# 的 APK 里既没有这些日志，也没有它们的字符串拼接开销。
# `assumenosideeffects` 要求被移除的调用没有副作用——本仓的 `LogUtil.debug`
# 只做「拼消息 → 写 logcat」，无副作用。
# 信息、警告与错误保留：它们是排障时真正需要的信息（且 release 的可选等级
# 列表也只到「信息及以上」，见 `LogLevel.releaseLevels`）。
-assumenosideeffects class terminal.emulator.runtime.LogUtil {
    public static void debug(...);
}
