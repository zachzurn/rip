package com.zachzurn.rip

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * Rip receipt markup renderer (native Rust via JNI).
 *
 * All methods are suspend functions. Resources (images, fonts)
 * referenced in markup are fetched and decoded automatically.
 *
 * ```kotlin
 * Rip.configure(basePath = "https://cdn.example.com/assets/", cachePath = cacheDir.path)
 * val html = Rip.renderHtml("## Hello\n---\n| Item |> $5.00 |")
 * ```
 */
object Rip {

    /**
     * Set global options for resource loading.
     *
     * @param basePath Base path for resolving relative resource URLs.
     * @param cachePath Directory for disk caching fetched resources.
     */
    fun configure(basePath: String? = null, cachePath: String? = null) {
        RipResourceLoader.configure(basePath, cachePath)
    }

    /**
     * Clear the in-memory resource cache.
     */
    fun clearCache() {
        RipResourceLoader.clearCache()
    }

    /**
     * Render markup to a standalone HTML document.
     *
     * No external resources are fetched — images become `<img>` tags,
     * QR codes and barcodes are inline SVG.
     */
    suspend fun renderHtml(source: String): String = withContext(Dispatchers.Default) {
        RipNative.renderHtml(source)
    }

    /**
     * Render markup to plain text (monospace ASCII).
     */
    suspend fun renderText(source: String): String = withContext(Dispatchers.Default) {
        RipNative.renderText(source)
    }

    /**
     * Render markup to 8-bit grayscale pixels (anti-aliased).
     *
     * Referenced images and fonts are fetched automatically.
     */
    suspend fun renderPixels(source: String): PixelOutput = withContext(Dispatchers.IO) {
        val resources = RipResourceLoader.loadIfNeeded(source)
        RipNative.renderPixels(source, resources)
    }

    /**
     * Render markup to 1-bit packed pixels (thresholded black/white).
     *
     * Output is MSB-first, `ceil(width/8)` bytes per row.
     * Referenced images and fonts are fetched automatically.
     */
    suspend fun renderRaster(source: String): PixelOutput = withContext(Dispatchers.IO) {
        val resources = RipResourceLoader.loadIfNeeded(source)
        RipNative.renderRaster(source, resources)
    }

    /**
     * Render markup to ESC/POS binary commands for thermal printers.
     *
     * Referenced images and fonts are fetched automatically.
     */
    suspend fun renderEscpos(source: String): ByteArray = withContext(Dispatchers.IO) {
        val resources = RipResourceLoader.loadIfNeeded(source)
        RipNative.renderEscpos(source, resources)
    }
}
