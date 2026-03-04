/// A grayscale pixel buffer for 2D drawing.
pub struct Canvas {
    pub width: u32,
    pub height: u32,
    /// Row-major grayscale pixels. Length = width * height.
    pub pixels: Vec<u8>,
    pub background: u8,
    pub foreground: u8,
    /// Tracks which rows have been drawn on (non-background content).
    pub dirty_rows: Vec<bool>,
}

impl Canvas {
    /// Create a new canvas filled with the background color.
    pub fn new(width: u32, height: u32, background: u8, foreground: u8) -> Self {
        Self {
            width,
            height,
            pixels: vec![background; (width * height) as usize],
            background,
            foreground,
            dirty_rows: vec![false; height as usize],
        }
    }

    /// Mark a range of rows as dirty.
    #[inline]
    fn mark_dirty(&mut self, y_start: u32, y_end: u32) {
        let start = y_start as usize;
        let end = (y_end as usize).min(self.dirty_rows.len());
        for d in &mut self.dirty_rows[start..end] {
            *d = true;
        }
    }

    /// Set a single pixel. No-op if out of bounds.
    #[inline]
    pub fn set_pixel(&mut self, x: u32, y: u32, value: u8) {
        if x < self.width && y < self.height {
            self.pixels[(y * self.width + x) as usize] = value;
            self.dirty_rows[y as usize] = true;
        }
    }

    /// Draw a filled rectangle. Clips to canvas bounds.
    pub fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, value: u8) {
        let x_end = (x + w).min(self.width);
        let y_end = (y + h).min(self.height);
        let row_len = x_end.saturating_sub(x) as usize;
        if row_len == 0 {
            return;
        }
        self.mark_dirty(y, y_end);
        for py in y..y_end {
            let start = (py * self.width + x) as usize;
            self.pixels[start..start + row_len].fill(value);
        }
    }

    /// Draw a solid horizontal line.
    pub fn draw_hline(&mut self, x: u32, y: u32, width: u32, thickness: u32, value: u8) {
        self.fill_rect(x, y, width, thickness, value);
    }

    /// Draw a dotted horizontal line with configurable thickness.
    pub fn draw_dotted_hline(&mut self, x: u32, y: u32, width: u32, thickness: u32, value: u8) {
        let dot_len = 6u32;
        let gap_len = 5u32;
        let cycle = dot_len + gap_len;
        let x_end = (x + width).min(self.width);
        let y_end = (y + thickness).min(self.height);
        self.mark_dirty(y, y_end);
        for py in y..y_end {
            let row_start = (py * self.width) as usize;
            for px in x..x_end {
                if ((px - x) % cycle) < dot_len {
                    self.pixels[row_start + px as usize] = value;
                }
            }
        }
    }

    /// Blit a glyph coverage bitmap onto the canvas as grayscale.
    ///
    /// Each coverage byte (0 = transparent, 255 = fully opaque) is blended
    /// against the background: `pixel = 255 - coverage`. Overlapping draws
    /// (e.g. bold offset) use `min` so the darkest value wins.
    pub fn blit_glyph(
        &mut self,
        x: i32,
        y: i32,
        glyph_w: u32,
        glyph_h: u32,
        coverage: &[u8],
    ) {
        // Pre-clip to canvas bounds
        let gx_start = 0i32.max(-x) as u32;
        let gy_start = 0i32.max(-y) as u32;
        let gx_end = glyph_w.min((self.width as i32 - x).max(0) as u32);
        let gy_end = glyph_h.min((self.height as i32 - y).max(0) as u32);

        if gy_start < gy_end {
            let y_start = (y + gy_start as i32) as u32;
            let y_end = (y + gy_end as i32) as u32;
            self.mark_dirty(y_start, y_end);
        }

        let canvas_w = self.width as usize;

        for gy in gy_start..gy_end {
            let py = (y + gy as i32) as usize;
            let row_offset = py * canvas_w;
            let glyph_row = (gy * glyph_w) as usize;

            for gx in gx_start..gx_end {
                let cov = coverage[glyph_row + gx as usize];
                if cov > 0 {
                    let idx = row_offset + (x + gx as i32) as usize;
                    self.pixels[idx] = self.pixels[idx].min(255 - cov);
                }
            }
        }
    }

    /// Blit a grayscale image onto the canvas. Copies rows directly.
    pub fn blit_image(&mut self, x: u32, y: u32, img_w: u32, img_h: u32, pixels: &[u8]) {
        let x_end = (x + img_w).min(self.width);
        let y_end = (y + img_h).min(self.height);
        let copy_w = (x_end - x) as usize;
        if copy_w == 0 {
            return;
        }
        self.mark_dirty(y, y_end);
        let canvas_w = self.width as usize;
        let src_w = img_w as usize;

        for iy in 0..(y_end - y) {
            let dst_start = ((y + iy) as usize) * canvas_w + x as usize;
            let src_start = iy as usize * src_w;
            self.pixels[dst_start..dst_start + copy_w]
                .copy_from_slice(&pixels[src_start..src_start + copy_w]);
        }
    }

    /// Nearest-neighbor scale + grayscale blit from a luma8 source directly
    /// onto the canvas. Only writes non-white pixels; white pixels are
    /// skipped since the canvas is already background-filled. Rows with
    /// no non-white pixels stay clean for the dirty-row optimization.
    pub fn blit_image_nn(
        &mut self,
        x: u32,
        y: u32,
        dst_w: u32,
        dst_h: u32,
        src: &[u8],
        src_w: u32,
        src_h: u32,
    ) {
        let x_end = (x + dst_w).min(self.width);
        let y_end = (y + dst_h).min(self.height);
        let out_w = x_end - x;
        let out_h = y_end - y;
        if out_w == 0 || out_h == 0 {
            return;
        }
        let canvas_w = self.width as usize;

        for oy in 0..out_h {
            let sy = (oy as u64 * src_h as u64 / dst_h as u64) as usize;
            let dst_row = ((y + oy) as usize) * canvas_w + x as usize;
            let src_row = sy * src_w as usize;
            let mut row_dirty = false;

            for ox in 0..out_w {
                let sx = (ox as u64 * src_w as u64 / dst_w as u64) as usize;
                let val = src[src_row + sx];
                if val < 255 {
                    let idx = dst_row + ox as usize;
                    self.pixels[idx] = self.pixels[idx].min(val);
                    row_dirty = true;
                }
            }

            if row_dirty {
                self.dirty_rows[(y + oy) as usize] = true;
            }
        }
    }

    /// Blit a 1-bit bitmap (e.g., QR code) scaled up by an integer factor.
    ///
    /// `src` is row-major, one byte per module: 0 = dark (foreground), 255 = light (background).
    pub fn blit_bitmap_scaled(
        &mut self,
        x: u32,
        y: u32,
        src_w: u32,
        src_h: u32,
        src: &[u8],
        scale: u32,
        fg: u8,
        bg: u8,
    ) {
        for sy in 0..src_h {
            for sx in 0..src_w {
                let val = src[(sy * src_w + sx) as usize];
                let color = if val == 0 { fg } else { bg };
                let dest_x = x + sx * scale;
                let dest_y = y + sy * scale;
                self.fill_rect(dest_x, dest_y, scale, scale, color);
            }
        }
    }
}
