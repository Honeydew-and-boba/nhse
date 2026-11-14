//! Software rendering: a 1280×720 canvas, rounded rectangles and anti-aliased text.
use std::cell::RefCell;
use std::collections::HashMap;

pub const WIDTH: usize = 1280;
pub const HEIGHT: usize = 720;

/// A color, `0xRRGGBB`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color(pub u32);

impl Color {
    fn r(self) -> u32 {
        (self.0 >> 16) & 0xFF
    }
    fn g(self) -> u32 {
        (self.0 >> 8) & 0xFF
    }
    fn b(self) -> u32 {
        self.0 & 0xFF
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }
    pub fn right(&self) -> i32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }
    pub fn inset(&self, d: i32) -> Rect {
        Rect::new(self.x + d, self.y + d, self.w - 2 * d, self.h - 2 * d)
    }
    fn intersect(&self, o: &Rect) -> Rect {
        let x = self.x.max(o.x);
        let y = self.y.max(o.y);
        let r = self.right().min(o.right());
        let b = self.bottom().min(o.bottom());
        Rect::new(x, y, (r - x).max(0), (b - y).max(0))
    }
}

/// The frame being drawn. Pixels are stored in the framebuffer's byte order (R, G, B, A), so a
/// frame is copied to the screen as is.
pub struct Canvas {
    pub px: Vec<u32>,
    clip: Rect,
}

const FULL: Rect = Rect::new(0, 0, WIDTH as i32, HEIGHT as i32);

#[inline]
fn pack(r: u32, g: u32, b: u32) -> u32 {
    0xFF00_0000 | (b << 16) | (g << 8) | r
}

impl Default for Canvas {
    fn default() -> Self {
        Self::new()
    }
}

impl Canvas {
    pub fn new() -> Self {
        Self {
            px: vec![0xFF00_0000; WIDTH * HEIGHT],
            clip: FULL,
        }
    }

    /// The pixel at (x, y) as `0xRRGGBB` (for screenshots and tests).
    pub fn rgb_at(&self, x: usize, y: usize) -> u32 {
        let p = self.px[y * WIDTH + x];
        ((p & 0xFF) << 16) | (p & 0xFF00) | ((p >> 16) & 0xFF)
    }

    pub fn set_clip(&mut self, r: Option<Rect>) {
        self.clip = r.map(|r| r.intersect(&FULL)).unwrap_or(FULL);
    }

    #[inline]
    fn blend(&mut self, x: i32, y: i32, c: Color, alpha: u32) {
        let cl = &self.clip;
        if alpha == 0 || x < cl.x || y < cl.y || x >= cl.right() || y >= cl.bottom() {
            return;
        }
        let i = y as usize * WIDTH + x as usize;
        if alpha >= 255 {
            self.px[i] = pack(c.r(), c.g(), c.b());
            return;
        }
        let d = self.px[i];
        let (dr, dg, db) = (d & 0xFF, (d >> 8) & 0xFF, (d >> 16) & 0xFF);
        let mix = |s: u32, d: u32| (s * alpha + d * (255 - alpha) + 127) / 255;
        self.px[i] = pack(mix(c.r(), dr), mix(c.g(), dg), mix(c.b(), db));
    }

    pub fn clear(&mut self, c: Color) {
        self.px.fill(pack(c.r(), c.g(), c.b()));
    }

    pub fn fill(&mut self, r: Rect, c: Color) {
        self.fill_alpha(r, c, 255);
    }

    pub fn fill_alpha(&mut self, r: Rect, c: Color, alpha: u32) {
        let r = r.intersect(&self.clip);
        if alpha >= 255 {
            let p = pack(c.r(), c.g(), c.b());
            for y in r.y..r.bottom() {
                let row = y as usize * WIDTH;
                self.px[row + r.x as usize..row + r.right() as usize].fill(p);
            }
            return;
        }
        for y in r.y..r.bottom() {
            for x in r.x..r.right() {
                self.blend(x, y, c, alpha);
            }
        }
    }

    /// A rectangle with anti-aliased round corners.
    pub fn round_rect(&mut self, r: Rect, radius: i32, c: Color) {
        self.round_rect_alpha(r, radius, c, 255);
    }

    pub fn round_rect_alpha(&mut self, r: Rect, radius: i32, c: Color, alpha: u32) {
        let rad = radius.min(r.w / 2).min(r.h / 2).max(0);
        if rad == 0 {
            return self.fill_alpha(r, c, alpha);
        }
        // middle band, then the top and bottom bands with their corners
        self.fill_alpha(Rect::new(r.x, r.y + rad, r.w, r.h - 2 * rad), c, alpha);
        let radf = rad as f32;
        for dy in 0..rad {
            for (y, cy) in [
                (r.y + dy, r.y + rad),
                (r.bottom() - 1 - dy, r.bottom() - rad),
            ] {
                // inner part of the band without AA
                let fy = y as f32 + 0.5 - cy as f32;
                let span = (radf * radf - fy * fy).max(0.0).sqrt();
                let inner = (span - 1.0).floor().max(0.0) as i32;
                self.fill_alpha(
                    Rect::new(r.x + rad - inner, y, r.w - 2 * (rad - inner), 1),
                    c,
                    alpha,
                );
                // anti-aliased edge pixels on both sides
                for k in 0..=(rad - inner) {
                    let lx = r.x + rad - inner - 1 - k;
                    let fx = (r.x + rad) as f32 - (lx as f32 + 0.5);
                    let d = (fx * fx + fy * fy).sqrt();
                    let cov = (radf - d + 0.5).clamp(0.0, 1.0);
                    if cov <= 0.0 {
                        break;
                    }
                    let a = (cov * alpha as f32) as u32;
                    self.blend(lx, y, c, a);
                    self.blend(r.right() - 1 - (lx - r.x), y, c, a);
                }
            }
        }
    }

    /// An anti-aliased filled circle.
    pub fn circle(&mut self, cx: i32, cy: i32, radius: i32, c: Color) {
        self.round_rect(
            Rect::new(cx - radius, cy - radius, radius * 2, radius * 2),
            radius,
            c,
        );
    }
}

/// An 8-bit alpha image, drawn tinted in one color (the logo).
#[derive(Clone)]
pub struct Mask {
    pub w: usize,
    pub h: usize,
    pub alpha: Vec<u8>,
}

impl Mask {
    /// Reads `u16 width, u16 height` (little endian) and the alpha bytes, as written by
    /// switch/tools/make-logo.py.
    pub fn from_bytes(b: &[u8]) -> Self {
        let w = u16::from_le_bytes([b[0], b[1]]) as usize;
        let h = u16::from_le_bytes([b[2], b[3]]) as usize;
        Self {
            w,
            h,
            alpha: b[4..4 + w * h].to_vec(),
        }
    }

    /// The mask scaled down to `height` pixels (area averaging).
    pub fn scaled(&self, height: usize) -> Mask {
        let height = height.clamp(1, self.h);
        let width = (self.w * height).div_ceil(self.h).max(1);
        let mut alpha = vec![0u8; width * height];
        for ty in 0..height {
            let (y0, y1) = (ty * self.h / height, ((ty + 1) * self.h).div_ceil(height));
            for tx in 0..width {
                let (x0, x1) = (
                    tx * self.w / width,
                    ((tx + 1) * self.w).div_ceil(width).min(self.w),
                );
                let mut sum = 0u32;
                for y in y0..y1 {
                    for x in x0..x1 {
                        sum += self.alpha[y * self.w + x] as u32;
                    }
                }
                let n = ((y1 - y0) * (x1 - x0)).max(1) as u32;
                alpha[ty * width + tx] = (sum / n) as u8;
            }
        }
        Mask {
            w: width,
            h: height,
            alpha,
        }
    }
}

impl Canvas {
    /// Draws a mask in `color` with its top left corner at (x, y).
    pub fn mask(&mut self, x: i32, y: i32, m: &Mask, color: Color) {
        for row in 0..m.h {
            for col in 0..m.w {
                let a = m.alpha[row * m.w + col] as u32;
                if a != 0 {
                    self.blend(x + col as i32, y + row as i32, color, a);
                }
            }
        }
    }
}

/// Font weights embedded in the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Weight {
    Medium,
    Bold,
}

struct Glyph {
    xmin: i32,
    ymin: i32,
    w: usize,
    h: usize,
    advance: f32,
    bitmap: Vec<u8>,
}

/// Inter (SIL Open Font License, see assets/Inter-LICENSE.txt), rasterized with fontdue and
/// cached per glyph and size.
pub struct Fonts {
    medium: fontdue::Font,
    bold: fontdue::Font,
    cache: RefCell<HashMap<(Weight, char, u16), Glyph>>,
}

impl Default for Fonts {
    fn default() -> Self {
        Self::new()
    }
}

impl Fonts {
    pub fn new() -> Self {
        let load = |b: &[u8]| {
            fontdue::Font::from_bytes(b, fontdue::FontSettings::default())
                .expect("embedded font is valid")
        };
        Self {
            medium: load(include_bytes!("../assets/Inter-Medium.otf")),
            bold: load(include_bytes!("../assets/Inter-Bold.otf")),
            cache: RefCell::new(HashMap::new()),
        }
    }

    fn font(&self, w: Weight) -> &fontdue::Font {
        match w {
            Weight::Medium => &self.medium,
            Weight::Bold => &self.bold,
        }
    }

    fn with_glyph<T>(&self, w: Weight, ch: char, size: u16, f: impl FnOnce(&Glyph) -> T) -> T {
        let mut cache = self.cache.borrow_mut();
        let g = cache.entry((w, ch, size)).or_insert_with(|| {
            let font = self.font(w);
            // characters the font lacks (CJK names, symbols) fall back to "?"
            let ch = if ch == ' ' || font.lookup_glyph_index(ch) != 0 {
                ch
            } else {
                '?'
            };
            let (m, bitmap) = font.rasterize(ch, size as f32);
            Glyph {
                xmin: m.xmin,
                ymin: m.ymin,
                w: m.width,
                h: m.height,
                advance: m.advance_width,
                bitmap,
            }
        });
        f(g)
    }

    fn ascent(&self, w: Weight, size: u16) -> f32 {
        self.font(w)
            .horizontal_line_metrics(size as f32)
            .map(|m| m.ascent)
            .unwrap_or(size as f32 * 0.8)
    }

    /// Width of `text` in pixels.
    pub fn measure(&self, text: &str, size: u16, w: Weight) -> i32 {
        let mut x = 0.0f32;
        for ch in text.chars() {
            x += self.with_glyph(w, ch, size, |g| g.advance);
        }
        x.ceil() as i32
    }

    /// Draws `text` with its top at `y` (line box of `size * 1.25`). Returns the width drawn.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        c: &mut Canvas,
        x: i32,
        y: i32,
        text: &str,
        size: u16,
        w: Weight,
        color: Color,
    ) -> i32 {
        let line = size as f32 * 1.25;
        let ascent = self.ascent(w, size);
        let baseline = y as f32 + (line - size as f32 * 1.2) / 2.0 + ascent;
        let mut pen = x as f32;
        for ch in text.chars() {
            pen += self.with_glyph(w, ch, size, |g| {
                let gx = pen.round() as i32 + g.xmin;
                let gy = baseline.round() as i32 - g.ymin - g.h as i32;
                for row in 0..g.h {
                    for col in 0..g.w {
                        let a = g.bitmap[row * g.w + col] as u32;
                        if a != 0 {
                            c.blend(gx + col as i32, gy + row as i32, color, a);
                        }
                    }
                }
                g.advance
            });
        }
        (pen - x as f32).ceil() as i32
    }

    /// Shortens `text` with "…" so it fits in `max` pixels.
    pub fn ellipsize(&self, text: &str, size: u16, w: Weight, max: i32) -> String {
        if self.measure(text, size, w) <= max {
            return text.to_string();
        }
        let mut s: Vec<char> = text.chars().collect();
        while !s.is_empty() {
            s.pop();
            let t: String = s.iter().collect::<String>().trim_end().to_string() + "…";
            if self.measure(&t, size, w) <= max {
                return t;
            }
        }
        "…".into()
    }

    /// Splits `text` into lines no wider than `max` pixels (explicit newlines are kept).
    pub fn wrap(&self, text: &str, size: u16, w: Weight, max: i32) -> Vec<String> {
        let mut out = Vec::new();
        for para in text.split('\n') {
            let mut line = String::new();
            // words wider than a line (paths) are split by character
            let mut words: Vec<String> = Vec::new();
            for word in para.split(' ') {
                if self.measure(word, size, w) <= max {
                    words.push(word.to_string());
                    continue;
                }
                let mut part = String::new();
                for ch in word.chars() {
                    part.push(ch);
                    if self.measure(&part, size, w) > max {
                        part.pop();
                        words.push(std::mem::take(&mut part));
                        part.push(ch);
                    }
                }
                words.push(part);
            }
            for word in &words {
                let candidate = if line.is_empty() {
                    word.to_string()
                } else {
                    format!("{line} {word}")
                };
                if self.measure(&candidate, size, w) <= max || line.is_empty() {
                    line = candidate;
                } else {
                    out.push(std::mem::take(&mut line));
                    line = word.to_string();
                }
            }
            out.push(line);
        }
        out
    }
}

/// The canvas, the fonts and the theme to draw with.
pub struct Gfx {
    pub canvas: Canvas,
    pub fonts: Fonts,
    pub theme: &'static crate::theme::Theme,
}

impl Default for Gfx {
    fn default() -> Self {
        Self::new()
    }
}

impl Gfx {
    pub fn new() -> Self {
        Self {
            canvas: Canvas::new(),
            fonts: Fonts::new(),
            theme: &crate::theme::LIGHT,
        }
    }

    pub fn text(&mut self, x: i32, y: i32, t: &str, size: u16, w: Weight, color: Color) -> i32 {
        self.fonts.draw(&mut self.canvas, x, y, t, size, w, color)
    }

    /// Text that ends at `right`.
    pub fn text_right(&mut self, right: i32, y: i32, t: &str, size: u16, w: Weight, color: Color) {
        let width = self.fonts.measure(t, size, w);
        self.text(right - width, y, t, size, w, color);
    }

    pub fn text_center(&mut self, cx: i32, y: i32, t: &str, size: u16, w: Weight, color: Color) {
        let width = self.fonts.measure(t, size, w);
        self.text(cx - width / 2, y, t, size, w, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_text_and_shapes() {
        let mut g = Gfx::new();
        g.canvas.clear(Color(0xFFFFFF));
        g.canvas
            .round_rect(Rect::new(10, 10, 100, 40), 12, Color(0xFF0000));
        assert_eq!(g.canvas.rgb_at(60, 30), 0xFF0000);
        assert_eq!(g.canvas.rgb_at(10, 10), 0xFFFFFF, "corner stays clear");
        let w = g.text(200, 100, "Hello, island", 24, Weight::Medium, Color(0));
        assert!(w > 100 && w < 220, "{w}");
        assert!(
            g.fonts
                .wrap("one two three four", 24, Weight::Medium, 80)
                .len()
                >= 2
        );
        assert!(g
            .fonts
            .ellipsize("A very long name indeed", 20, Weight::Bold, 90)
            .ends_with('…'));
    }
}
