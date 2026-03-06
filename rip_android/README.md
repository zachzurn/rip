# rip_android

Android/Kotlin bindings for the Rip receipt markup renderer. Provides a native `.so` via JNI + a Kotlin wrapper with coroutine-based API.

## Prerequisites

- [Rust](https://rustup.rs/)
- Android SDK + NDK (via Android Studio or `sdkmanager`)
- [cargo-ndk](https://github.com/nickelc/cargo-ndk)

```bash
rustup target add aarch64-linux-android x86_64-linux-android
cargo install cargo-ndk
```

## Project structure

```
rip_android/
  src/lib.rs                         # Rust JNI bindings
  kotlin/
    build.gradle.kts                 # Android library module
    src/main/
      AndroidManifest.xml
      jniLibs/                       # Built .so files (git-ignored)
      kotlin/com/zachzurn/rip/
        Rip.kt                      # Public API (suspend functions)
        RipNative.kt                # JNI declarations (internal)
        PixelOutput.kt              # Data class
        RipResources.kt             # Resource loading + image decoding
        RipRenderException.kt       # Exception class
  build_aar.sh                       # Build script
```

## Build

```bash
bash rip_android/build_aar.sh
```

This cross-compiles the Rust code for `arm64-v8a` and `x86_64`, then builds the AAR via Gradle.

Output: `rip_android/kotlin/build/outputs/aar/`

## Publish locally

```bash
cd rip_android/kotlin && ./gradlew publishToMavenLocal
```

Then in your app's `build.gradle.kts`:

```kotlin
dependencies {
    implementation("com.zachzurn:rip:0.1.0")
}
```

## Usage

```kotlin
import com.zachzurn.rip.Rip

// All methods are suspend functions
val html = Rip.renderHtml("## Hello\n---\n| Item |> $5.00 |")
val text = Rip.renderText(markup)
val pixels = Rip.renderPixels(markup)    // PixelOutput(width, height, pixels, dirtyRows)
val raster = Rip.renderRaster(markup)    // 1-bit packed
val escpos = Rip.renderEscpos(markup)    // ByteArray of ESC/POS commands
```

Images and fonts referenced in markup are fetched and decoded automatically.
