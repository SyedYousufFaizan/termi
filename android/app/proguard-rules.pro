# ProGuard rules for Next-Gen Terminal

# Keep JNI native methods
-keepclasseswithmembernames class * {
    native <methods>;
}

# Keep TerminalEngine class for JNI
-keep class com.terminal.core.TerminalEngine { *; }
-keep class com.terminal.core.TerminalException { *; }

# Keep all classes that have native methods
-keepclasseswithmembers class * {
    native <methods>;
}

# Keep Kotlin metadata for reflection
-keep class kotlin.Metadata { *; }

# Keep Compose classes
-keep class androidx.compose.** { *; }

# Keep data classes for serialization
-keepclassmembers class * {
    @kotlinx.serialization.Serializable <fields>;
}

# Keep coroutines
-keepnames class kotlinx.coroutines.internal.MainDispatcherFactory {}
-keepnames class kotlinx.coroutines.CoroutineExceptionHandler {}

# Keep Timber
-dontwarn org.jetbrains.annotations.**

# Debugging - keep source file names and line numbers
-keepattributes SourceFile,LineNumberTable
-renamesourcefileattribute SourceFile
