import java.util.Properties

plugins {
    id("com.android.application")
}

val rustDir = rootProject.file("rust")
val jniOut = file("src/main/jniLibs")
val ndkVer = "30.0.16248370"

// Android SDK location: local.properties first, then the usual environment variables (CI), then ~/Android/Sdk.
val sdkDir: File = run {
    val lp = rootProject.file("local.properties")
    val local = if (lp.exists()) Properties().apply { lp.inputStream().use { load(it) } }.getProperty("sdk.dir") else null
    File(local ?: System.getenv("ANDROID_HOME") ?: System.getenv("ANDROID_SDK_ROOT") ?: File(System.getProperty("user.home"), "Android/Sdk").path)
}

// Release signing comes from environment variables. Without them the release build uses the debug key.
//   SIGNING_STORE_FILE      path to the .jks / .p12 keystore
//   SIGNING_STORE_PASSWORD  keystore password
//   SIGNING_KEY_ALIAS       key alias
//   SIGNING_KEY_PASSWORD    key password (optional, defaults to the keystore password)
fun env(name: String): String? = System.getenv(name)?.takeIf { it.isNotBlank() }
val releaseStore: File? = env("SIGNING_STORE_FILE")?.let { file(it) }?.takeIf { it.isFile }

// Builds the Rust engine (libribbit.so) for every shipped ABI before Android packaging.
val cargoBuild by tasks.registering(Exec::class) {
    group = "rust"
    description = "Build the Rust native engine with cargo-ndk"
    workingDir = rustDir
    val cargo = File(System.getProperty("user.home"), ".cargo/bin/cargo").takeIf { it.exists() }?.path ?: "cargo"
    environment("ANDROID_NDK_HOME", File(sdkDir, "ndk/$ndkVer").path)
    commandLine(
        cargo, "ndk",
        "-t", "arm64-v8a", "-t", "armeabi-v7a", "-t", "x86_64",
        "-P", "26",
        "-o", jniOut.path,
        "build", "--release",
    )
    inputs.dir(rustDir.resolve("src"))
    inputs.file(rustDir.resolve("Cargo.toml"))
    outputs.dir(jniOut)
}

tasks.named("preBuild") { dependsOn(cargoBuild) }

android {
    namespace = "com.riyadm.ribbit.revived"
    compileSdk = 37
    ndkVersion = ndkVer

    defaultConfig {
        applicationId = "com.riyadm.ribbit.revived"
        minSdk = 26
        targetSdk = 37
        versionCode = 3
        versionName = "1.2.0"
        ndk { abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64") }
    }

    signingConfigs {
        if (releaseStore != null) {
            create("release") {
                storeFile = releaseStore
                storePassword = env("SIGNING_STORE_PASSWORD")
                keyAlias = env("SIGNING_KEY_ALIAS")
                keyPassword = env("SIGNING_KEY_PASSWORD") ?: env("SIGNING_STORE_PASSWORD")
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            signingConfig = signingConfigs.getByName(if (releaseStore != null) "release" else "debug")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    packaging { jniLibs { useLegacyPackaging = false } }
}

kotlin {
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
    }
}
