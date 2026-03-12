use std::collections::{HashMap, HashSet};

use fontdue::Font;
use rip_parser::ast::*;
use rip_parser::text_util;
use rip_resources::{ImageData, RenderResources};

use crate::barcode;
use crate::canvas::Canvas;
use crate::layout;
use crate::text;
use crate::RenderError;

/// Default body font (Inter Medium, OFL-licensed).
const DEFAULT_FONT: &[u8] = include_bytes!("../assets/inter_medium.ttf");

/// Default bold body font (Inter Bold, OFL-licensed).
const DEFAULT_BOLD_FONT: &[u8] = include_bytes!("../assets/inter_bold.ttf");

/// Default title font (Inter Display Bold, OFL-licensed).
const DEFAULT_TITLE_FONT: &[u8] = include_bytes!("../assets/inter_display_bold.ttf");

/// Per-size font configuration collected from @style directives.
struct FontStyle {
    url: String,
    points: f64,
}

/// Internal rendering context.
pub struct RenderContext<'a> {
    images: &'a HashMap<String, ImageData>,
    dpi: f64,
    paper_width_px: u32,
    font_cache: text::FontCache,
    /// Maps Size level → (font URL, pixel size).
    size_map: HashMap<Size, (String, f32)>,
    /// Maps Size level → (bold font URL, pixel size) for real bold rendering.
    bold_size_map: HashMap<Size, (String, f32)>,
    /// Vertical padding in pixels around dividers, images, etc.
    padding: u32,
}

impl<'a> RenderContext<'a> {
    pub fn new(nodes: &[Node], resources: &'a RenderResources) -> Self {
        let mut paper_width_mm = 80.0;
        let base_size_pt = 12.0;
        let mut dpi: f64 = 203.0;
        let mut styles: HashMap<Size, FontStyle> = HashMap::new();
        let mut bold_styles: HashMap<Size, FontStyle> = HashMap::new();
        // Track which levels had an explicit regular @style (to clear default bold pairing)
        let mut explicit_regular: HashSet<Size> = HashSet::new();

        // Pass 1: collect config
        for node in nodes {
            match node {
                Node::PrinterWidth { mm } => {
                    paper_width_mm = *mm;
                }
                Node::PrinterDpi { dpi: d } => {
                    dpi = *d as f64;
                }
                Node::Style {
                    level,
                    font,
                    points,
                    bold,
                } => {
                    if *bold {
                        bold_styles.insert(
                            *level,
                            FontStyle {
                                url: font.clone(),
                                points: *points,
                            },
                        );
                    } else {
                        styles.insert(
                            *level,
                            FontStyle {
                                url: font.clone(),
                                points: *points,
                            },
                        );
                        explicit_regular.insert(*level);
                        // Setting a regular style clears the bold pairing
                        bold_styles.remove(level);
                    }
                }
                _ => {}
            }
        }

        let paper_width_px = layout::printable_px(paper_width_mm, dpi);
        let padding = ((4.0 * dpi / 96.0).round() as u32).max(1);

        let mut font_cache = text::FontCache::new();
        let mut size_map = HashMap::new();
        let mut bold_size_map = HashMap::new();

        // Resolve fonts for each defined style level
        for (level, style) in &styles {
            let url = &style.url;
            let px = layout::font_pt_to_px(style.points, dpi);

            if font_cache.get(url).is_none() {
                if let Some(bytes) = resources.fonts.get(url.as_str()) {
                    font_cache.load(url, bytes);
                }
            }
            size_map.insert(*level, (url.clone(), px));
        }

        // Resolve bold fonts
        for (level, style) in &bold_styles {
            let url = &style.url;
            let px = layout::font_pt_to_px(style.points, dpi);

            if font_cache.get(url).is_none() {
                if let Some(bytes) = resources.fonts.get(url.as_str()) {
                    font_cache.load(url, bytes);
                }
            }
            bold_size_map.insert(*level, (url.clone(), px));
        }

        // Always load the built-in default fonts (embedded in binary)
        font_cache.load("default", DEFAULT_FONT);
        font_cache.load("default-bold", DEFAULT_BOLD_FONT);
        font_cache.load("default-title", DEFAULT_TITLE_FONT);

        // Populate default size entries for any levels not explicitly styled.
        let default_sizes: [(Size, &str, f64); 6] = [
            (Size::Text,   "default",       base_size_pt),
            (Size::TextM,  "default",       base_size_pt * 1.33),
            (Size::TextL,  "default",       base_size_pt * 1.67),
            (Size::Title,  "default-title", base_size_pt * 2.0),
            (Size::TitleM, "default-title", base_size_pt * 2.5),
            (Size::TitleL, "default-title", base_size_pt * 3.0),
        ];
        for (level, font_id, pt) in default_sizes {
            if !size_map.contains_key(&level) {
                size_map.insert(level, (font_id.to_string(), layout::font_pt_to_px(pt, dpi)));
            }
        }

        // Populate default bold entries for body sizes (not titles) unless
        // the user set a custom regular style without a matching bold.
        let default_bold_sizes: [(Size, f64); 3] = [
            (Size::Text,  base_size_pt),
            (Size::TextM, base_size_pt * 1.33),
            (Size::TextL, base_size_pt * 1.67),
        ];
        for (level, pt) in default_bold_sizes {
            if !bold_size_map.contains_key(&level) && !explicit_regular.contains(&level) {
                bold_size_map.insert(level, ("default-bold".to_string(), layout::font_pt_to_px(pt, dpi)));
            }
        }

        Self {
            images: &resources.images,
            dpi,
            paper_width_px,
            font_cache,
            size_map,
            bold_size_map,
            padding,
        }
    }

    /// Get the font and pixel size for a given Size level.
    ///
    /// Fallback chain:
    /// 1. The exact size level and its configured font
    /// 2. The size level's pixel size with the appropriate default font
    ///    (default-title for title sizes, default for body sizes)
    /// 3. The Text level as a last resort
    fn resolve_font(&self, size: Size) -> Option<(&Font, f32)> {
        // Try the requested size level
        if let Some((url, px)) = self.size_map.get(&size) {
            if let Some(font) = self.font_cache.get(url) {
                return Some((font, *px));
            }
            // Font failed to load — try the appropriate default at this size's px
            let fallback_id = if matches!(size, Size::Title | Size::TitleM | Size::TitleL) {
                "default-title"
            } else {
                "default"
            };
            if let Some(font) = self.font_cache.get(fallback_id) {
                return Some((font, *px));
            }
            // Last-ditch: try the other default
            if let Some(font) = self.font_cache.get("default") {
                return Some((font, *px));
            }
        }
        // Fall back to Text level entirely
        if size != Size::Text {
            if let Some((url, px)) = self.size_map.get(&Size::Text) {
                if let Some(font) = self.font_cache.get(url) {
                    return Some((font, *px));
                }
            }
        }
        None
    }

    /// Get the bold font for a given Size level, if one is configured.
    ///
    /// Returns `None` if no bold font is available (caller should faux-bold).
    fn resolve_bold_font(&self, size: Size) -> Option<(&Font, f32)> {
        if let Some((url, px)) = self.bold_size_map.get(&size) {
            if let Some(font) = self.font_cache.get(url) {
                return Some((font, *px));
            }
        }
        None
    }

    /// Get the default line height (for Blank nodes, etc.)
    fn default_line_height(&self) -> u32 {
        self.resolve_font(Size::Text)
            .map(|(font, px)| text::line_height(font, px).ceil() as u32)
            .unwrap_or(20) // fallback: ~12pt at 203 DPI
    }

    /// Collect all unique (char, font pointer, size_px) tuples from text nodes.
    /// Uses u32 bits for size_px to allow hashing.
    fn collect_chars(&self, nodes: &[Node]) -> HashSet<(char, *const Font, u32)> {
        let mut chars = HashSet::new();
        for node in nodes {
            let (spans_list, size) = match node {
                Node::Text { spans, size } => (vec![spans.as_slice()], *size),
                Node::Columns { cells, size } => {
                    let sl: Vec<&[Span]> = cells.iter().filter_map(|c| {
                        if let CellContent::Spans(spans) = &c.content { Some(spans.as_slice()) } else { None }
                    }).collect();
                    (sl, *size)
                }
                _ => continue,
            };

            if let Some((font, px)) = self.resolve_font(size) {
                let ptr = font as *const Font;
                let bits = px.to_bits();
                let bold_info = self.resolve_bold_font(size);

                for spans in &spans_list {
                    for span in *spans {
                        for ch in span.text.chars() {
                            chars.insert((ch, ptr, bits));
                            // Also collect with bold font for bold spans
                            if span.style == SpanStyle::Bold {
                                if let Some((bf, bpx)) = bold_info {
                                    chars.insert((ch, bf as *const Font, bpx.to_bits()));
                                }
                            }
                        }
                    }
                }
            }
        }
        chars
    }

    /// Measure the pixel height of a single node.
    fn measure_node(
        &self,
        node: &Node,
        node_idx: usize,
        col_cache: &HashMap<usize, Vec<(f64, f64)>>,
    ) -> u32 {
        match node {
            Node::Text { spans, size } => {
                self.resolve_font(*size)
                    .map(|(font, px)| {
                        let lh = text::line_height(font, px);
                        let avail = self.paper_width_px as f32;
                        let bold = self.resolve_bold_font(*size);
                        let num_lines = text::count_lines_wrapped(spans, font, px, avail, bold);
                        (lh * num_lines as f32).ceil() as u32
                    })
                    .unwrap_or(self.default_line_height())
            }

            Node::Columns { cells, size } => self
                .resolve_font(*size)
                .map(|(font, px)| {
                    let cols = col_cache.get(&node_idx)
                        .map(|v| v.as_slice())
                        .unwrap_or(&[]);
                    let bold = self.resolve_bold_font(*size);
                    let col = layout::column_layout_wrapped(cols, cells, font, px, bold);
                    col.row_height.max(text::line_height(font, px).ceil() as u32)
                })
                .unwrap_or(self.default_line_height()),

            Node::Line { style } => {
                let thickness = match style {
                    LineStyle::Thin => 2u32,
                    LineStyle::Dotted => 3,
                    LineStyle::Thick => 4,
                };
                thickness + self.padding * 2
            }

            Node::Blank => self.default_line_height(),

            Node::Image { url, width, height, .. } => {
                let max_w = width.map(|w| layout::pt_to_px(w, self.dpi) as u32);
                let max_h = height.map(|h| layout::pt_to_px(h, self.dpi) as u32);
                let img_h = self
                    .image_dimensions(url)
                    .map(|(nw, nh)| self.scale_image_dims(nw, nh, max_w, max_h).1)
                    .unwrap_or(max_h.or(max_w).unwrap_or(50));
                img_h + self.padding * 2
            }

            Node::Qr { size, .. } => {
                let dim = layout::pt_to_px(size.unwrap_or(75.0), self.dpi) as u32;
                dim + self.padding * 2
            }

            Node::Barcode { .. } => {
                let barcode_h = layout::pt_to_px(50.0, self.dpi) as u32;
                barcode_h + self.padding * 2
            }

            Node::Feed { amount, unit } => match unit {
                FeedUnit::Lines => (*amount as f64 * self.default_line_height() as f64).round() as u32,
                FeedUnit::Mm => layout::mm_to_px(*amount, self.dpi),
            },

            // Non-visual nodes
            Node::PrinterWidth { .. } | Node::PrinterDpi { .. }
            | Node::PrinterThreshold { .. } | Node::Style { .. }
            | Node::Cut { .. } | Node::Drawer => 0,
        }
    }

    /// Compute scaled (width, height) for an image given its native dimensions.
    ///
    /// The `max_w` and `max_h` parameters are upper bounds — the image is
    /// scaled to fit within them while preserving its aspect ratio.
    fn scale_image_dims(
        &self,
        nat_w: u32,
        nat_h: u32,
        max_w: Option<u32>,
        max_h: Option<u32>,
    ) -> (u32, u32) {
        let (w, h) = match (max_w, max_h) {
            (Some(mw), Some(mh)) => {
                let scale = (mw as f32 / nat_w as f32).min(mh as f32 / nat_h as f32);
                ((nat_w as f32 * scale) as u32, (nat_h as f32 * scale) as u32)
            }
            (Some(mw), None) => {
                let scale = (mw as f32 / nat_w as f32).min(1.0);
                ((nat_w as f32 * scale) as u32, (nat_h as f32 * scale) as u32)
            }
            (None, Some(mh)) => {
                let scale = (mh as f32 / nat_h as f32).min(1.0);
                ((nat_w as f32 * scale) as u32, (nat_h as f32 * scale) as u32)
            }
            (None, None) => {
                let default_max = layout::pt_to_px(125.0, self.dpi) as u32;
                let mw = default_max.min(self.paper_width_px);
                let mh = default_max;
                let scale = (mw as f32 / nat_w as f32).min(mh as f32 / nat_h as f32);
                ((nat_w as f32 * scale) as u32, (nat_h as f32 * scale) as u32)
            }
        };

        (w.max(1), h.max(1))
    }

    /// Get image dimensions from pre-decoded image data.
    fn image_dimensions(&self, url: &str) -> Option<(u32, u32)> {
        let img = self.images.get(url)?;
        Some((img.width, img.height))
    }

    /// Compute the inter-column gap in pixels (2 space characters at default text size).
    fn column_gap_px(&self) -> f64 {
        self.resolve_font(Size::Text)
            .map(|(font, px)| text::measure_text(font, px, "  ") as f64)
            .unwrap_or(10.0)
    }

    /// Compute the minimum column width in pixels (one wide character).
    fn min_column_px(&self) -> f64 {
        self.resolve_font(Size::Text)
            .map(|(font, px)| text::measure_text(font, px, "W") as f64)
            .unwrap_or(8.0)
    }

    /// Pre-compute column widths for all column groups.
    ///
    /// Returns a map from node index to pre-computed `(x, width)` pairs.
    /// Both `measure_node` and `render_node` use these for consistency.
    fn build_column_cache(&self, nodes: &[Node]) -> HashMap<usize, Vec<(f64, f64)>> {
        let groups = text_util::identify_column_groups(nodes);
        let gap = self.column_gap_px();
        let min_col = self.min_column_px();
        let mut cache = HashMap::new();

        for group in &groups {
            let pcts = text_util::group_width_pcts(nodes, &group);

            // Find max natural pixel width per column across the group
            let mut max_natural = vec![0.0f64; group.col_count];
            for idx in group.start..group.end {
                if let Node::Columns { cells, size } = &nodes[idx] {
                    if let Some((font, px)) = self.resolve_font(*size) {
                        let bold = self.resolve_bold_font(*size);
                        for (col, cell) in cells.iter().enumerate() {
                            if col < max_natural.len() {
                                if let CellContent::Spans(spans) = &cell.content {
                                    let w = text::measure_spans(spans, font, px, bold) as f64;
                                    max_natural[col] = max_natural[col].max(w);
                                }
                            }
                        }
                    }
                }
            }

            let cols = text_util::column_widths(
                &pcts, &max_natural, self.paper_width_px as f64, gap, min_col,
            );

            for idx in group.start..group.end {
                cache.insert(idx, cols.clone());
            }
        }

        cache
    }

    /// Render all nodes and return (width, height, pixels, dirty_rows).
    pub fn render(&self, nodes: &[Node]) -> Result<(u32, u32, Vec<u8>, Vec<bool>), RenderError> {
        // Pre-compute column widths for all groups
        let col_cache = self.build_column_cache(nodes);

        // Pre-compute heights once
        let heights: Vec<u32> = nodes.iter().enumerate()
            .map(|(i, n)| self.measure_node(n, i, &col_cache))
            .collect();
        let total_height: u32 = heights.iter().sum();
        if total_height == 0 {
            return Err(RenderError::EmptyDocument);
        }

        // Pre-rasterize all unique glyphs
        let char_set = self.collect_chars(nodes);
        let glyph_cache = text::GlyphCache::build(&char_set);

        let mut canvas = Canvas::new(self.paper_width_px, total_height, 255, 0);
        let mut y_cursor = 0u32;

        for (i, (node, &height)) in nodes.iter().zip(heights.iter()).enumerate() {
            if height == 0 {
                continue;
            }
            self.render_node(node, i, &col_cache, &mut canvas, &mut y_cursor, &glyph_cache);
        }

        Ok((canvas.width, canvas.height, canvas.pixels, canvas.dirty_rows))
    }

    /// Render a single node onto the canvas.
    fn render_node(
        &self,
        node: &Node,
        node_idx: usize,
        col_cache: &HashMap<usize, Vec<(f64, f64)>>,
        canvas: &mut Canvas,
        y: &mut u32,
        glyph_cache: &text::GlyphCache,
    ) {
        match node {
            Node::Text { spans, size } => {
                if let Some((font, px)) = self.resolve_font(*size) {
                    let avail = self.paper_width_px as f32;
                    let align = if matches!(size, Size::Title | Size::TitleM | Size::TitleL) {
                        Align::Center
                    } else {
                        Align::Left
                    };
                    let bold = self.resolve_bold_font(*size);

                    let height = text::render_spans_wrapped(
                        canvas,
                        spans,
                        0.0,
                        *y as f32,
                        font,
                        px,
                        avail,
                        align,
                        bold,
                        glyph_cache,
                    );
                    *y += height;
                }
            }

            Node::Columns { cells, size } => {
                if let Some((font, px)) = self.resolve_font(*size) {
                    let cols = col_cache.get(&node_idx)
                        .map(|v| v.as_slice())
                        .unwrap_or(&[]);
                    let bold = self.resolve_bold_font(*size);
                    let col_layout = layout::column_layout_wrapped(cols, cells, font, px, bold);
                    let row_height = col_layout.row_height
                        .max(text::line_height(font, px).ceil() as u32);

                    for (i, cell) in cells.iter().enumerate() {
                        if i >= col_layout.cells.len() {
                            break;
                        }
                        let cl = &col_layout.cells[i];

                        match &cell.content {
                            CellContent::Spans(spans) => {
                                text::render_spans_wrapped(
                                    canvas,
                                    spans,
                                    cl.x as f32,
                                    *y as f32,
                                    font,
                                    px,
                                    cl.width as f32,
                                    cell.align,
                                    bold,
                                    glyph_cache,
                                );
                            }
                            CellContent::Divider(line_style) => {
                                // Draw divider at bottom of row (baseline-aligned)
                                let div_y = *y + row_height - 3;
                                match line_style {
                                    LineStyle::Thin => {
                                        canvas.draw_hline(
                                            cl.x, div_y, cl.width, 2, canvas.foreground,
                                        );
                                    }
                                    LineStyle::Thick => {
                                        canvas.draw_hline(
                                            cl.x,
                                            div_y.saturating_sub(3),
                                            cl.width,
                                            4,
                                            canvas.foreground,
                                        );
                                    }
                                    LineStyle::Dotted => {
                                        canvas.draw_dotted_hline(
                                            cl.x, div_y, cl.width, 3, canvas.foreground,
                                        );
                                    }
                                }
                            }
                        }
                    }

                    *y += row_height;
                }
            }

            Node::Line { style } => {
                *y += self.padding;
                let fg = canvas.foreground;
                let w = self.paper_width_px;
                match style {
                    LineStyle::Thin => {
                        canvas.draw_hline(0, *y, w, 2, fg);
                        *y += 2;
                    }
                    LineStyle::Thick => {
                        canvas.draw_hline(0, *y, w, 4, fg);
                        *y += 4;
                    }
                    LineStyle::Dotted => {
                        canvas.draw_dotted_hline(0, *y, w, 3, fg);
                        *y += 3;
                    }
                }
                *y += self.padding;
            }

            Node::Blank => {
                *y += self.default_line_height();
            }

            Node::Image {
                url,
                width,
                height,
                align,
                ..
            } => {
                *y += self.padding;

                let max_w = width.map(|w| layout::pt_to_px(w, self.dpi) as u32);
                let max_h = height.map(|h| layout::pt_to_px(h, self.dpi) as u32);

                if let Some(img) = self.images.get(url.as_str()) {
                    let (scaled_w, scaled_h) =
                        self.scale_image_dims(img.width, img.height, max_w, max_h);

                    let x_offset = match align {
                        Some(Align::Center) => {
                            self.paper_width_px.saturating_sub(scaled_w) / 2
                        }
                        Some(Align::Right) => self.paper_width_px.saturating_sub(scaled_w),
                        _ => 0,
                    };

                    canvas.blit_image_nn(
                        x_offset, *y,
                        scaled_w, scaled_h,
                        &img.pixels,
                        img.width, img.height,
                    );
                    *y += scaled_h;
                }

                *y += self.padding;
            }

            Node::Qr { data, size, align } => {
                *y += self.padding;
                let dim = layout::pt_to_px(size.unwrap_or(75.0), self.dpi) as u32;

                if let Some((actual_size, pixels)) = barcode::render_qr(data, dim) {
                    let x_offset = match align {
                        Some(Align::Center) => {
                            self.paper_width_px.saturating_sub(actual_size) / 2
                        }
                        Some(Align::Right) => self.paper_width_px.saturating_sub(actual_size),
                        _ => 0,
                    };
                    canvas.blit_image(x_offset, *y, actual_size, actual_size, &pixels);
                    *y += actual_size;
                }

                *y += self.padding;
            }

            Node::Barcode { format, data } => {
                *y += self.padding;

                if let Some(encoded) = rip_parser::encode::encode_barcode(format, data) {
                    let pattern_len = encoded.len() as u32;
                    let barcode_h = layout::pt_to_px(50.0, self.dpi) as u32;

                    if pattern_len > 0 {
                        // Scale bars to fill paper width
                        let bar_width_f = self.paper_width_px as f32 / pattern_len as f32;

                        for (i, &bar) in encoded.iter().enumerate() {
                            if bar == 1 {
                                let x_start = (i as f32 * bar_width_f) as u32;
                                let x_end = ((i + 1) as f32 * bar_width_f) as u32;
                                let w = (x_end - x_start).max(1);
                                canvas.fill_rect(x_start, *y, w, barcode_h, canvas.foreground);
                            }
                        }
                        *y += barcode_h;
                    }
                }

                *y += self.padding;
            }

            Node::Feed { amount, unit } => {
                *y += match unit {
                    FeedUnit::Lines => (*amount as f64 * self.default_line_height() as f64).round() as u32,
                    FeedUnit::Mm => layout::mm_to_px(*amount, self.dpi),
                };
            }

            // Non-visual nodes
            Node::PrinterWidth { .. } | Node::PrinterDpi { .. }
            | Node::PrinterThreshold { .. } | Node::Style { .. }
            | Node::Cut { .. } | Node::Drawer => {}
        }
    }
}
