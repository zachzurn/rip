use fontdue::{Font, FontSettings, Metrics};
use rip_parser::ast::*;
use std::collections::{HashMap, HashSet};

use crate::canvas::Canvas;

/// Cache of loaded fontdue fonts, keyed by URL to avoid duplicate loads.
pub struct FontCache {
    fonts: HashMap<String, Font>,
}

impl FontCache {
    pub fn new() -> Self {
        Self {
            fonts: HashMap::new(),
        }
    }

    /// Load a font from raw TTF or OTF bytes and cache it under the given URL key.
    ///
    /// Returns true if the font was loaded (or already cached).
    pub fn load(&mut self, url: &str, bytes: &[u8]) -> bool {
        if self.fonts.contains_key(url) {
            return true;
        }
        match Font::from_bytes(bytes, FontSettings::default()) {
            Ok(font) => {
                self.fonts.insert(url.to_string(), font);
                true
            }
            Err(_) => false,
        }
    }

    pub fn get(&self, url: &str) -> Option<&Font> {
        self.fonts.get(url)
    }
}

/// Key for the glyph cache: (character, font pointer identity, size in f32 bits).
#[derive(Hash, Eq, PartialEq)]
struct GlyphKey {
    ch: char,
    font_id: usize, // pointer as identity
    size_bits: u32,  // f32::to_bits()
}

/// Pre-rasterized glyph: metrics + raw coverage bitmap.
pub struct CachedGlyph {
    pub metrics: Metrics,
    pub bitmap: Vec<u8>,
}

/// Cache of pre-rasterized glyphs. Built once from the document's character set,
/// then used for all rendering lookups.
pub struct GlyphCache {
    glyphs: HashMap<GlyphKey, CachedGlyph>,
}

impl GlyphCache {
    /// Build a glyph cache by pre-rasterizing all unique (char, font, size) combos.
    /// The `u32` in each tuple is `f32::to_bits()` of size_px.
    pub fn build(chars: &HashSet<(char, *const Font, u32)>) -> Self {
        let mut glyphs = HashMap::with_capacity(chars.len());
        for &(ch, font_ptr, size_bits) in chars {
            let font = unsafe { &*font_ptr };
            let size_px = f32::from_bits(size_bits);
            let (metrics, bitmap) = font.rasterize(ch, size_px);
            let key = GlyphKey {
                ch,
                font_id: font_ptr as usize,
                size_bits,
            };
            glyphs.insert(key, CachedGlyph { metrics, bitmap });
        }
        Self { glyphs }
    }

    /// Look up a cached glyph. Returns None only if the char wasn't in the
    /// original character set (shouldn't happen in normal use).
    #[inline]
    pub fn get(&self, ch: char, font: &Font, size_px: f32) -> Option<&CachedGlyph> {
        let key = GlyphKey {
            ch,
            font_id: font as *const Font as usize,
            size_bits: size_px.to_bits(),
        };
        self.glyphs.get(&key)
    }
}

/// Measure the pixel width of a text string.
pub fn measure_text(font: &Font, size_px: f32, text: &str) -> f32 {
    text.chars()
        .map(|ch| font.metrics(ch, size_px).advance_width)
        .sum()
}

/// Get the line height for a font at a given pixel size.
pub fn line_height(font: &Font, size_px: f32) -> f32 {
    font.horizontal_line_metrics(size_px)
        .map(|m| m.new_line_size)
        .unwrap_or(size_px * 1.2)
}

/// Get the ascent (baseline offset from top) for a font at a given pixel size.
pub fn ascent(font: &Font, size_px: f32) -> f32 {
    font.horizontal_line_metrics(size_px)
        .map(|m| m.ascent)
        .unwrap_or(size_px * 0.8)
}

/// Render a list of spans onto the canvas at the given position.
///
/// Returns the total width drawn in pixels.
pub fn render_spans(
    canvas: &mut Canvas,
    spans: &[Span],
    x: f32,
    y: f32,
    font: &Font,
    size_px: f32,
    cache: &GlyphCache,
) -> f32 {
    let baseline_y = y + ascent(font, size_px);
    let mut cursor_x = x;

    for span in spans {
        let span_start_x = cursor_x;

        for ch in span.text.chars() {
            let glyph = match cache.get(ch, font, size_px) {
                Some(g) => g,
                None => { cursor_x += font.metrics(ch, size_px).advance_width; continue; }
            };
            let metrics = &glyph.metrics;

            let glyph_x = cursor_x + metrics.xmin as f32;
            let glyph_y = baseline_y - metrics.height as f32 - metrics.ymin as f32;

            if span.style == SpanStyle::Italic {
                let shear = (baseline_y - glyph_y) * 0.15;
                canvas.blit_glyph(
                    (glyph_x + shear) as i32,
                    glyph_y as i32,
                    metrics.width as u32,
                    metrics.height as u32,
                    &glyph.bitmap,
                );
            } else {
                canvas.blit_glyph(
                    glyph_x as i32,
                    glyph_y as i32,
                    metrics.width as u32,
                    metrics.height as u32,
                    &glyph.bitmap,
                );

                if span.style == SpanStyle::Bold {
                    canvas.blit_glyph(
                        glyph_x as i32 + 1,
                        glyph_y as i32,
                        metrics.width as u32,
                        metrics.height as u32,
                        &glyph.bitmap,
                    );
                }
            }

            cursor_x += metrics.advance_width;
        }

        let span_width = cursor_x - span_start_x;

        if span.style == SpanStyle::Underline {
            let underline_y = (baseline_y + 2.0) as u32;
            canvas.draw_hline(
                span_start_x as u32,
                underline_y,
                span_width as u32,
                3,
                canvas.foreground,
            );
        }

        if span.style == SpanStyle::Strikethrough {
            let strike_y = (y + ascent(font, size_px) * 0.6) as u32;
            canvas.draw_hline(
                span_start_x as u32,
                strike_y,
                span_width as u32,
                3,
                canvas.foreground,
            );
        }
    }

    cursor_x - x
}

/// Measure the total pixel width of a list of spans.
pub fn measure_spans(spans: &[Span], font: &Font, size_px: f32) -> f32 {
    spans
        .iter()
        .map(|span| measure_text(font, size_px, &span.text))
        .sum()
}

/// Flatten spans into a list of styled characters for wrapping.
fn flatten_spans(spans: &[Span]) -> Vec<(char, SpanStyle)> {
    let mut chars = Vec::new();
    for span in spans {
        for ch in span.text.chars() {
            chars.push((ch, span.style));
        }
    }
    chars
}

/// Word-wrap styled characters into lines. Breaks on spaces; falls back to
/// character break if a single word is longer than max_width.
fn wrap_lines(
    chars: &[(char, SpanStyle)],
    font: &Font,
    size_px: f32,
    max_width: f32,
) -> Vec<Vec<(char, SpanStyle)>> {
    if chars.is_empty() {
        return vec![vec![]];
    }

    let mut lines: Vec<Vec<(char, SpanStyle)>> = vec![];
    let mut current_line: Vec<(char, SpanStyle)> = vec![];
    let mut line_width: f32 = 0.0;

    // Current word being accumulated
    let mut word: Vec<(char, SpanStyle)> = vec![];
    let mut word_width: f32 = 0.0;

    for &(ch, style) in chars {
        let advance = font.metrics(ch, size_px).advance_width;

        if ch == ' ' {
            // Space: commit the current word to the line first
            let total = line_width + word_width + advance;
            if total > max_width && !current_line.is_empty() {
                // Word + space doesn't fit → wrap before this word
                // Trim trailing spaces from current line
                while current_line.last().map(|(c, _)| *c == ' ').unwrap_or(false) {
                    current_line.pop();
                }
                lines.push(current_line);
                current_line = word.clone();
                line_width = word_width;
                word.clear();
                word_width = 0.0;
                // Add the space
                current_line.push((ch, style));
                line_width += advance;
            } else {
                // Fits: flush word into line, then add space
                current_line.extend_from_slice(&word);
                line_width += word_width;
                word.clear();
                word_width = 0.0;
                current_line.push((ch, style));
                line_width += advance;
            }
        } else {
            // Non-space: accumulate into current word
            word.push((ch, style));
            word_width += advance;

            // If a single word exceeds max_width, force character-break
            if word_width > max_width && word.len() > 1 {
                if !current_line.is_empty() {
                    // Flush current line first
                    while current_line.last().map(|(c, _)| *c == ' ').unwrap_or(false) {
                        current_line.pop();
                    }
                    lines.push(current_line);
                    current_line = vec![];
                    line_width = 0.0;
                }
                // Put all but the last char of the word on a line
                let last = word.pop().unwrap();
                let last_advance = font.metrics(last.0, size_px).advance_width;
                lines.push(word.clone());
                word.clear();
                word.push(last);
                word_width = last_advance;
            }
        }
    }

    // Flush remaining word and line
    if !word.is_empty() {
        if line_width + word_width > max_width && !current_line.is_empty() {
            while current_line.last().map(|(c, _)| *c == ' ').unwrap_or(false) {
                current_line.pop();
            }
            lines.push(current_line);
            current_line = word;
        } else {
            current_line.extend_from_slice(&word);
        }
    }

    if !current_line.is_empty() {
        // Trim trailing spaces
        while current_line.last().map(|(c, _)| *c == ' ').unwrap_or(false) {
            current_line.pop();
        }
        lines.push(current_line);
    }

    if lines.is_empty() {
        lines.push(vec![]);
    }

    lines
}

/// Count how many wrapped lines the spans would occupy at the given max width.
pub fn count_lines_wrapped(spans: &[Span], font: &Font, size_px: f32, max_width: f32) -> u32 {
    if max_width <= 0.0 {
        return 1;
    }
    let chars = flatten_spans(spans);
    let lines = wrap_lines(&chars, font, size_px, max_width);
    lines.len() as u32
}

/// Render spans with word-level wrapping. Returns total pixel height used.
///
/// If `center` is true, each wrapped line is centered within `max_width`.
pub fn render_spans_wrapped(
    canvas: &mut Canvas,
    spans: &[Span],
    x_start: f32,
    y: f32,
    font: &Font,
    size_px: f32,
    max_width: f32,
    center: bool,
    cache: &GlyphCache,
) -> u32 {
    let lh = line_height(font, size_px);
    let chars = flatten_spans(spans);
    let lines = wrap_lines(&chars, font, size_px, max_width);

    // Render each line
    for (line_idx, line_chars) in lines.iter().enumerate() {
        let line_y = y + line_idx as f32 * lh;
        let baseline_y = line_y + ascent(font, size_px);

        // Calculate line width for centering
        let line_width: f32 = line_chars
            .iter()
            .map(|(ch, _)| font.metrics(*ch, size_px).advance_width)
            .sum();

        let x_offset = if center {
            x_start + (max_width - line_width).max(0.0) / 2.0
        } else {
            x_start
        };

        let mut cursor_x = x_offset;

        // Track contiguous style runs for underline/strikethrough
        let mut run_start_x = cursor_x;
        let mut run_style = line_chars.first().map(|(_, s)| *s).unwrap_or(SpanStyle::Normal);

        for &(ch, style) in line_chars {
            // If style changed, finish the previous run's decorations
            if style != run_style {
                finish_decorations(
                    canvas, run_style, run_start_x, cursor_x, line_y, baseline_y, size_px, font,
                );
                run_start_x = cursor_x;
                run_style = style;
            }

            let glyph = match cache.get(ch, font, size_px) {
                Some(g) => g,
                None => { cursor_x += font.metrics(ch, size_px).advance_width; continue; }
            };
            let metrics = &glyph.metrics;
            let glyph_x = cursor_x + metrics.xmin as f32;
            let glyph_y = baseline_y - metrics.height as f32 - metrics.ymin as f32;

            if style == SpanStyle::Italic {
                let shear = (baseline_y - glyph_y) * 0.15;
                canvas.blit_glyph(
                    (glyph_x + shear) as i32,
                    glyph_y as i32,
                    metrics.width as u32,
                    metrics.height as u32,
                    &glyph.bitmap,
                );
            } else {
                canvas.blit_glyph(
                    glyph_x as i32,
                    glyph_y as i32,
                    metrics.width as u32,
                    metrics.height as u32,
                    &glyph.bitmap,
                );

                if style == SpanStyle::Bold {
                    canvas.blit_glyph(
                        glyph_x as i32 + 1,
                        glyph_y as i32,
                        metrics.width as u32,
                        metrics.height as u32,
                        &glyph.bitmap,
                    );
                }
            }

            cursor_x += metrics.advance_width;
        }

        // Finish decorations for the last run
        finish_decorations(
            canvas, run_style, run_start_x, cursor_x, line_y, baseline_y, size_px, font,
        );
    }

    (lines.len() as f32 * lh).ceil() as u32
}

/// Draw underline/strikethrough decorations for a style run.
fn finish_decorations(
    canvas: &mut Canvas,
    style: SpanStyle,
    start_x: f32,
    end_x: f32,
    line_y: f32,
    baseline_y: f32,
    size_px: f32,
    font: &Font,
) {
    let width = (end_x - start_x) as u32;
    if width == 0 {
        return;
    }

    if style == SpanStyle::Underline {
        let underline_y = (baseline_y + 2.0) as u32;
        canvas.draw_hline(start_x as u32, underline_y, width, 3, canvas.foreground);
    }

    if style == SpanStyle::Strikethrough {
        let strike_y = (line_y + ascent(font, size_px) * 0.6) as u32;
        canvas.draw_hline(start_x as u32, strike_y, width, 3, canvas.foreground);
    }
}
