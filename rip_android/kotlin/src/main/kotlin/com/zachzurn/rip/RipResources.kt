package com.zachzurn.rip

import android.graphics.BitmapFactory
import java.io.File
import java.util.concurrent.ConcurrentHashMap

/**
 * Discovered resource URLs from parsing markup.
 * Constructed by the JNI layer from Rust's `collect_resources()`.
 */
data class ResourceUrls(
    val fonts: Array<String>,
    val images: Array<String>,
) {
    fun isEmpty(): Boolean = fonts.isEmpty() && images.isEmpty()
}

/**
 * Decoded image data in luma8 format (0=black, 255=white).
 * Passed to the JNI layer for rendering.
 */
internal class RipImageData(
    @JvmField val width: Int,
    @JvmField val height: Int,
    @JvmField val pixels: ByteArray,
)

/**
 * Assembled resources ready for the JNI rendering functions.
 */
internal class RipResourcesJni(
    @JvmField val images: HashMap<String, RipImageData>,
    @JvmField val fonts: HashMap<String, ByteArray>,
)

/**
 * Internal resource loading with basePath resolution, in-memory caching,
 * and disk caching.
 */
internal object RipResourceLoader {

    private var basePath: String = ""
    private var cachePath: String = ""

    // In-memory cache: resolved URL → decoded resource (image or font bytes)
    private val imageCache = ConcurrentHashMap<String, RipImageData>()
    private val fontCache = ConcurrentHashMap<String, ByteArray>()

    fun configure(basePath: String?, cachePath: String?) {
        if (basePath != null) this.basePath = basePath
        if (cachePath != null) this.cachePath = cachePath
    }

    fun clearCache() {
        imageCache.clear()
        fontCache.clear()
    }

    /**
     * Discover resource URLs in markup, fetch and decode them.
     * Returns null if the markup references no external resources.
     */
    suspend fun loadIfNeeded(source: String): RipResourcesJni? {
        val urls = RipNative.getResources(source)
        if (urls.isEmpty()) return null

        val images = HashMap<String, RipImageData>()
        val fonts = HashMap<String, ByteArray>()

        for (url in urls.images) {
            val resolved = resolveUrl(url)
            // Check in-memory cache
            val cached = imageCache[resolved]
            if (cached != null) {
                images[url] = cached
                continue
            }
            val bytes = fetchBytesWithCache(resolved)
            val decoded = decodeImageToLuma8(bytes)
            imageCache[resolved] = decoded
            images[url] = decoded
        }

        for (url in urls.fonts) {
            val resolved = resolveUrl(url)
            val cached = fontCache[resolved]
            if (cached != null) {
                fonts[url] = cached
                continue
            }
            val bytes = fetchBytesWithCache(resolved)
            fontCache[resolved] = bytes
            fonts[url] = bytes
        }

        return RipResourcesJni(images, fonts)
    }

    /**
     * Resolve a relative URL against the configured basePath.
     */
    private fun resolveUrl(url: String): String {
        if (url.startsWith("http://") || url.startsWith("https://")) return url
        if (basePath.isEmpty()) return url
        val base = if (basePath.endsWith("/")) basePath else "$basePath/"
        return base + url
    }

    /**
     * Fetch bytes with disk cache support.
     */
    private suspend fun fetchBytesWithCache(resolvedUrl: String): ByteArray {
        // Try disk cache first
        val diskFile = diskCacheFile(resolvedUrl)
        if (diskFile != null && diskFile.exists()) {
            return diskFile.readBytes()
        }

        val bytes = fetchBytes(resolvedUrl)

        // Write to disk cache
        if (diskFile != null) {
            try {
                diskFile.parentFile?.mkdirs()
                diskFile.writeBytes(bytes)
            } catch (_: Exception) {
                // Silently ignore cache write failures
            }
        }

        return bytes
    }

    /**
     * Get the disk cache file for a URL, or null if cachePath is not set.
     */
    private fun diskCacheFile(url: String): File? {
        if (cachePath.isEmpty()) return null
        val safe = url.replace(Regex("[^a-zA-Z0-9._-]"), "_")
        return File(cachePath, safe)
    }

    /**
     * Load raw bytes from a URL or local file path.
     *
     * - HTTP/HTTPS URLs → fetched over the network
     * - Everything else → read from local filesystem
     */
    private suspend fun fetchBytes(url: String): ByteArray {
        if (url.startsWith("http://") || url.startsWith("https://")) {
            val connection = java.net.URL(url).openConnection() as java.net.HttpURLConnection
            try {
                connection.requestMethod = "GET"
                connection.connect()
                return connection.inputStream.readBytes()
            } finally {
                connection.disconnect()
            }
        }
        // Local file path
        return File(url).readBytes()
    }

    /**
     * Decode image bytes (PNG, JPEG, WebP, etc.) to luma8 grayscale.
     */
    private fun decodeImageToLuma8(bytes: ByteArray): RipImageData {
        val bitmap = BitmapFactory.decodeByteArray(bytes, 0, bytes.size)
            ?: throw RipRenderException("Failed to decode image")

        val width = bitmap.width
        val height = bitmap.height
        val argb = IntArray(width * height)
        bitmap.getPixels(argb, 0, width, 0, 0, width, height)
        bitmap.recycle()

        val luma = ByteArray(width * height)
        for (i in argb.indices) {
            val pixel = argb[i]
            val r = (pixel shr 16) and 0xFF
            val g = (pixel shr 8) and 0xFF
            val b = pixel and 0xFF
            luma[i] = (0.299 * r + 0.587 * g + 0.114 * b).toInt().toByte()
        }

        return RipImageData(width, height, luma)
    }
}
