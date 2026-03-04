/* tslint:disable */
/* eslint-disable */

/**
 * Parse markup and return the external resource URLs it references.
 *
 * Returns `{ fonts: string[], images: string[] }`.
 */
export function get_resources(source: string): any;

/**
 * Render markup to ESC/POS binary commands for thermal printers.
 *
 * Returns a `Uint8Array` of raw ESC/POS bytes.
 */
export function render_escpos(source: string, resources_js: any): any;

/**
 * Render markup to a standalone HTML document.
 *
 * No resources needed — images are `<img>` tags, QR/barcodes are inline SVG.
 */
export function render_html(source: string): string;

/**
 * Render markup to 8-bit grayscale pixels (anti-aliased).
 *
 * Returns `{ width, height, pixels: Uint8Array, dirtyRows: boolean[] }`.
 */
export function render_pixels(source: string, resources_js: any): any;

/**
 * Render markup to 1-bit packed pixels (thresholded black/white).
 *
 * Returns `{ width, height, pixels: Uint8Array, dirtyRows: boolean[] }`.
 * Pixels are MSB-first packed, `ceil(width/8)` bytes per row.
 */
export function render_raster(source: string, resources_js: any): any;

/**
 * Render markup to plain text (monospace ASCII).
 */
export function render_text(source: string): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly get_resources: (a: number, b: number) => any;
    readonly render_escpos: (a: number, b: number, c: any) => any;
    readonly render_html: (a: number, b: number) => [number, number];
    readonly render_pixels: (a: number, b: number, c: any) => [number, number, number];
    readonly render_raster: (a: number, b: number, c: any) => [number, number, number];
    readonly render_text: (a: number, b: number) => [number, number];
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
