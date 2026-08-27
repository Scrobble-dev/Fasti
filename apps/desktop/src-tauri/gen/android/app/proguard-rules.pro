# Add project specific ProGuard rules here.
# You can control the set of applied configuration files using the
# proguardFiles setting in build.gradle.
#
# For more details, see
#   http://developer.android.com/guide/developing/tools/proguard.html

# If your project uses WebView with JS, uncomment the following
# and specify the fully qualified class name to the JavaScript interface
# class:
#-keepclassmembers class fqcn.of.javascript.interface.for.webview {
#   public *;
#}

# Uncomment this to preserve the line number information for
# debugging stack traces.
#-keepattributes SourceFile,LineNumberTable

# If you keep the line number information, uncomment this to
# hide the original source file name.
#-renamesourcefileattribute SourceFile

# The Rust side's JNI_OnLoad registers native methods against
# io.crates.keyring.Keyring's companion object by exact class/method name;
# without this, R8 can strip or rename the class in a release build and the
# native binding breaks at runtime.
-keep class io.crates.keyring.Keyring {
    *;
}
-keep class io.crates.keyring.Keyring$Companion {
    *;
}
