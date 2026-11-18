// AGP on the buildscript classpath (the plugins-DSL marker
// artifact is not needed). AGP 9 compiles Kotlin itself, so there is no separate Kotlin plugin.
buildscript {
    repositories {
        google()
        mavenCentral()
    }
    dependencies {
        classpath("com.android.tools.build:gradle:9.2.1")
    }
}
