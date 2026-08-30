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
