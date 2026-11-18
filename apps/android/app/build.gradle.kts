plugins {
    id("com.android.application")
}

android {
    namespace = "app.bansayo.android"
    compileSdk = 36

    defaultConfig {
        applicationId = "app.bansayo.android"
        minSdk = 26
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

dependencies {
    // ComponentActivity is a LifecycleOwner, which CameraX binds to; no AppCompat or Compose.
    implementation("androidx.activity:activity:1.8.0")
    implementation("androidx.camera:camera-core:1.6.1")
    implementation("androidx.camera:camera-camera2:1.6.1")
    implementation("androidx.camera:camera-lifecycle:1.6.1")
    implementation("androidx.camera:camera-view:1.6.1")
    implementation("com.squareup.okhttp3:okhttp:4.12.0")
}
