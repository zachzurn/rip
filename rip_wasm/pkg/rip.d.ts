/**
 * Pixel output from image rendering.
 */
export interface PixelOutput {
  /** Image width in pixels. */
  width: number;
  /** Image height in pixels. */
  height: number;
  /**
   * Pixel data.
   *
   * For `renderPixels`: row-major 8-bit grayscale, length = width × height.
   * For `renderRaster`: 1-bit packed MSB-first, length = ceil(width/8) × height.
   */
  pixels: Uint8Array;
  /** Which rows contain non-background content (length = height). */
  dirtyRows: boolean[];
}

/**
 * Rip receipt markup renderer (WASM-powered).
 *
 * All methods are static and async. WASM is lazy-initialized on first call.
 * Resources (images, fonts) referenced in markup are fetched automatically.
 *
 * @example
 * ```typescript
 * import { Rip } from 'rip-js';
 *
 * const html = await Rip.renderHtml("## Hello\n---\nItem | $5.00");
 * ```
 */
export class Rip {
  /**
   * Render markup to a standalone HTML document.
   *
   * No external resources are fetched — images become `<img>` tags,
   * QR codes and barcodes are inline SVG.
   */
  static renderHtml(source: string): Promise<string>;

  /**
   * Render markup to plain text (monospace ASCII).
   *
   * Images, QR codes, and barcodes are rendered as text placeholders.
   */
  static renderText(source: string): Promise<string>;

  /**
   * Render markup to 8-bit grayscale pixels (anti-aliased).
   *
   * Referenced images and fonts are fetched automatically.
   */
  static renderPixels(source: string): Promise<PixelOutput>;

  /**
   * Render markup to 1-bit packed pixels (thresholded black/white).
   *
   * Output is MSB-first, `ceil(width/8)` bytes per row.
   * Referenced images and fonts are fetched automatically.
   */
  static renderRaster(source: string): Promise<PixelOutput>;

  /**
   * Render markup to ESC/POS binary commands for thermal printers.
   *
   * Referenced images and fonts are fetched automatically.
   */
  static renderEscpos(source: string): Promise<Uint8Array>;
}
