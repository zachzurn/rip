package com.zachzurn.rip

/**
 * Pixel rendering output.
 *
 * For [Rip.renderPixels]: row-major 8-bit grayscale, length = width × height.
 * For [Rip.renderRaster]: 1-bit packed MSB-first, length = ceil(width/8) × height.
 *
 * @property width Image width in pixels.
 * @property height Image height in pixels.
 * @property pixels Raw pixel data.
 * @property dirtyRows Which rows contain non-background content (length = height).
 */
data class PixelOutput(
    val width: Int,
    val height: Int,
    val pixels: ByteArray,
    val dirtyRows: BooleanArray,
) {
    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is PixelOutput) return false
        return width == other.width && height == other.height &&
            pixels.contentEquals(other.pixels) && dirtyRows.contentEquals(other.dirtyRows)
    }

    override fun hashCode(): Int {
        var result = width
        result = 31 * result + height
        result = 31 * result + pixels.contentHashCode()
        result = 31 * result + dirtyRows.contentHashCode()
        return result
    }
}
