package com.zachzurn.rip

/**
 * JNI declarations for the native Rip rendering library.
 *
 * These map to `#[no_mangle]` functions in `rip_android/src/lib.rs`.
 * Use [Rip] instead — it wraps these with resource loading and coroutines.
 */
internal object RipNative {
    init {
        System.loadLibrary("rip_android")
    }

    external fun getResources(source: String): ResourceUrls
    external fun renderHtml(source: String): String
    external fun renderText(source: String): String
    external fun renderPixels(source: String, resources: RipResourcesJni?): PixelOutput
    external fun renderRaster(source: String, resources: RipResourcesJni?): PixelOutput
    external fun renderEscpos(source: String, resources: RipResourcesJni?): ByteArray
}
