-dontoptimize
-dontobfuscate
-keep class io.torvox.** { *; }
-keepclassmembers class * {
    @dagger.hilt.android.lifecycle.HiltViewModel <init>(...);
}
-keepattributes *Annotation*, RuntimeVisibleAnnotations, RuntimeInvisibleAnnotations, RuntimeVisibleParameterAnnotations, RuntimeInvisibleParameterAnnotations, DefaultAnnotation, AnnotationDefault, Retention
-keepattributes Signature
-keepattributes InnerClasses
-keepattributes EnclosingMethod

-dontwarn com.sun.jna.**
-keep class com.sun.jna.** { *; }
-keep class * implements com.sun.jna.Library { *; }
-keepclassmembers class * implements com.sun.jna.Library {
    <methods>;
}
-keep class com.sun.jna.**$* { *; }

-keep class io.cucumber.** { *; }
-keep class io.torvox.cucumber.** { *; }
