# Flutter Rust Bridge
-keep class com.flutter_rust_bridge.** { *; }

# Rust native libraries
-keep class rust_lib_telegram_photos.** { *; }

# grammers MTProto
-keep class grammers.** { *; }

# SQLite
-keep class org.sqlite.** { *; }

# JNI
-keep class java.lang.** { *; }

# Prevent R8 from stripping interface information needed by the debug protocol
-keepattributes *Annotation*
