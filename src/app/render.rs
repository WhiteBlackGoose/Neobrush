//! Renders the visible part of a document into a viewport-sized pixel buffer.

use crate::core::document::Document;
use crate::core::geom::{Pt, Rect};
use rayon::prelude::*;
use slint::{Rgba8Pixel, SharedPixelBuffer};

#[derive(Default)]
pub struct Overlay {
    /// Dashed lines in document space.
    pub lines: Vec<(Pt, Pt)>,
    /// Handle positions in document space.
    pub handles: Vec<Pt>,
    /// Brush outline: center and radius in document space.
    pub circle: Option<(Pt, f32)>,
    /// Crosshair marker in document space.
    pub cross: Option<Pt>,
    /// Text caret in document space.
    pub caret: Option<Rect>,
}

pub const ACCENT: [u8; 3] = [124, 92, 255];

pub struct Canvas<'a> {
    pub px: &'a mut [Rgba8Pixel],
    pub w: i32,
    pub h: i32,
}

impl Canvas<'_> {
    #[inline]
    fn put(&mut self, x: i32, y: i32, c: [u8; 3]) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            self.px[(y * self.w + x) as usize] = Rgba8Pixel { r: c[0], g: c[1], b: c[2], a: 255 };
        }
    }
    #[inline]
    fn blend(&mut self, x: i32, y: i32, c: [u8; 3], a: f32) {
        if a <= 0.0 || x < 0 || y < 0 || x >= self.w || y >= self.h {
            return;
        }
        let p = &mut self.px[(y * self.w + x) as usize];
        let a = a.min(1.0);
        p.r = (p.r as f32 * (1.0 - a) + c[0] as f32 * a) as u8;
        p.g = (p.g as f32 * (1.0 - a) + c[1] as f32 * a) as u8;
        p.b = (p.b as f32 * (1.0 - a) + c[2] as f32 * a) as u8;
    }

    fn dashed_line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, phase: u32) {
        let len = ((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt();
        let n = len.ceil().max(1.0) as i32;
        for i in 0..=n {
            let t = i as f32 / n as f32;
            let x = (x0 + (x1 - x0) * t).floor() as i32;
            let y = (y0 + (y1 - y0) * t).floor() as i32;
            let on = ((i as u32 + phase) / 4) % 2 == 0;
            self.put(x, y, if on { [0, 0, 0] } else { [255, 255, 255] });
        }
    }

    fn circle(&mut self, cx: f32, cy: f32, r: f32) {
        let ri = r.ceil() as i32 + 2;
        let (icx, icy) = (cx.floor() as i32, cy.floor() as i32);
        for y in icy - ri..=icy + ri {
            if y < 0 || y >= self.h {
                continue;
            }
            for x in icx - ri..=icx + ri {
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                let outer = 1.0 - (d - r).abs();
                let inner = 1.0 - (d - (r - 1.0)).abs();
                if outer > 0.0 {
                    self.blend(x, y, [0, 0, 0], outer * 0.8);
                }
                if inner > 0.0 && r > 2.0 {
                    self.blend(x, y, [255, 255, 255], inner * 0.9);
                }
            }
        }
    }

    fn handle(&mut self, cx: f32, cy: f32) {
        let r = 5.5f32;
        let ri = 8;
        let (icx, icy) = (cx.floor() as i32, cy.floor() as i32);
        for y in icy - ri..=icy + ri {
            for x in icx - ri..=icx + ri {
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                // Shadow
                let sh = (r + 2.0 - d).clamp(0.0, 1.0) * 0.25;
                self.blend(x, y, [0, 0, 0], sh);
                let fill = (r - d + 0.5).clamp(0.0, 1.0);
                self.blend(x, y, ACCENT, fill);
                let inner = (r - 2.0 - d + 0.5).clamp(0.0, 1.0);
                self.blend(x, y, [255, 255, 255], inner);
            }
        }
    }
}

pub struct RenderParams<'a> {
    pub dark: bool,
    pub grid: bool,
    pub phase: u32,
    pub rulers: Option<&'a Glyphs>,
    pub hover: Option<Pt>,
    pub scale: f32,
}

/// Pre-rasterized digits for ruler labels.
pub struct Glyphs {
    pub px: f32,
    /// (advance, width, height, x offset, y offset (from baseline), alpha)
    pub glyphs: Vec<(f32, i32, i32, i32, i32, Vec<u8>)>,
    pub ascent: f32,
}

const GLYPH_CHARS: &str = "0123456789-";

impl Glyphs {
    pub fn new(px: f32) -> Glyphs {
        use ab_glyph::{Font, FontRef, ScaleFont};
        let font = FontRef::try_from_slice(include_bytes!("../../ui/fonts/Inter-500.ttf")).unwrap();
        let sf = font.as_scaled(px);
        let mut glyphs = Vec::new();
        for ch in GLYPH_CHARS.chars() {
            let id = font.glyph_id(ch);
            let g = id.with_scale_and_position(px, ab_glyph::point(0.0, 0.0));
            let adv = sf.h_advance(id);
            if let Some(o) = font.outline_glyph(g) {
                let b = o.px_bounds();
                let (w, h) = (b.width() as i32, b.height() as i32);
                let mut a = vec![0u8; (w * h).max(0) as usize];
                o.draw(|x, y, c| {
                    let i = (y as i32 * w + x as i32) as usize;
                    if i < a.len() {
                        a[i] = (c * 255.0) as u8;
                    }
                });
                glyphs.push((adv, w, h, b.min.x as i32, b.min.y as i32, a));
            } else {
                glyphs.push((adv, 0, 0, 0, 0, vec![]));
            }
        }
        Glyphs { px, glyphs, ascent: sf.ascent() }
    }

    fn draw(&self, cv: &mut Canvas, text: &str, x: f32, y_top: f32, color: [u8; 3]) {
        let mut pen = x;
        let base = y_top + self.ascent;
        for ch in text.chars() {
            let Some(i) = GLYPH_CHARS.find(ch) else { continue };
            let (adv, w, h, ox, oy, a) = &self.glyphs[i];
            for yy in 0..*h {
                for xx in 0..*w {
                    let v = a[(yy * w + xx) as usize];
                    if v > 0 {
                        cv.blend(pen as i32 + ox + xx, base as i32 + oy + yy, color, v as f32 / 255.0);
                    }
                }
            }
            pen += adv;
        }
    }

    fn draw_vertical(&self, cv: &mut Canvas, text: &str, x_left: f32, y: f32, color: [u8; 3]) {
        // Rotated 90° counter-clockwise, reading bottom to top.
        let mut pen = y;
        let base = x_left + self.ascent;
        for ch in text.chars() {
            let Some(i) = GLYPH_CHARS.find(ch) else { continue };
            let (adv, w, h, ox, oy, a) = &self.glyphs[i];
            for yy in 0..*h {
                for xx in 0..*w {
                    let v = a[(yy * w + xx) as usize];
                    if v > 0 {
                        cv.blend(base as i32 + oy + yy, pen as i32 - ox - xx, color, v as f32 / 255.0);
                    }
                }
            }
            pen -= adv;
        }
    }
}

fn draw_rulers(cv: &mut Canvas, g: &Glyphs, doc_w: u32, doc_h: u32, zoom: f32, ox: f32, oy: f32, dark: bool, hover: Option<Pt>, scale: f32) {
    let t = (20.0 * scale).round() as i32;
    let bg: [u8; 3] = if dark { [27, 27, 33] } else { [255, 255, 255] };
    let line: [u8; 3] = if dark { [58, 58, 70] } else { [207, 210, 218] };
    let fg: [u8; 3] = if dark { [154, 154, 171] } else { [100, 103, 120] };
    let (w, h) = (cv.w, cv.h);
    for y in 0..t.min(h) {
        for x in 0..w {
            cv.put(x, y, bg);
        }
    }
    for y in t..h {
        for x in 0..t.min(w) {
            cv.put(x, y, bg);
        }
    }
    for x in 0..w {
        cv.put(x, t - 1, line);
    }
    for y in 0..h {
        cv.put(t - 1, y, line);
    }
    // Choose a tick step so labels are at least ~64px apart.
    let steps = [1, 2, 5, 10, 20, 25, 50, 100, 200, 250, 500, 1000, 2000, 5000, 10000];
    let step = steps.iter().copied().find(|s| *s as f32 * zoom >= 64.0 * scale).unwrap_or(20000) as f32;
    let minor = step / if step >= 10.0 { 10.0 } else { step.max(1.0) };
    let ext_x = |d: f32| ox + d * zoom;
    let ext_y = |d: f32| oy + d * zoom;
    // Horizontal
    let d0 = ((t as f32 - ox) / zoom / minor).floor() * minor;
    let mut d = d0;
    while ext_x(d) < w as f32 {
        let x = ext_x(d).round() as i32;
        if x >= t {
            let major = (d / step).round() * step == d || ((d / step) - (d / step).round()).abs() < 1e-4;
            let half = ((d / (step / 2.0)) - (d / (step / 2.0)).round()).abs() < 1e-4;
            let len = if major { t } else if half { t / 2 } else { t / 4 };
            for y in t - len..t {
                cv.put(x, y, if major { fg } else { line });
            }
            if major {
                g.draw(cv, &format!("{}", d as i64), x as f32 + 3.0 * scale, 2.0 * scale, fg);
            }
        }
        d += minor;
    }
    // Vertical
    let d0 = ((t as f32 - oy) / zoom / minor).floor() * minor;
    let mut d = d0;
    while ext_y(d) < h as f32 {
        let y = ext_y(d).round() as i32;
        if y >= t {
            let major = ((d / step) - (d / step).round()).abs() < 1e-4;
            let half = ((d / (step / 2.0)) - (d / (step / 2.0)).round()).abs() < 1e-4;
            let len = if major { t } else if half { t / 2 } else { t / 4 };
            for x in t - len..t {
                cv.put(x, y, if major { fg } else { line });
            }
            if major {
                g.draw_vertical(cv, &format!("{}", d as i64), 2.0 * scale, y as f32 - 3.0 * scale, fg);
            }
        }
        d += minor;
    }
    // Image extent and cursor markers.
    let accent = ACCENT;
    let (ix0, ix1) = (ext_x(0.0) as i32, ext_x(doc_w as f32) as i32);
    let (iy0, iy1) = (ext_y(0.0) as i32, ext_y(doc_h as f32) as i32);
    for x in ix0.max(t)..ix1.min(w) {
        cv.blend(x, t - 2, accent, 0.5);
    }
    for y in iy0.max(t)..iy1.min(h) {
        cv.blend(t - 2, y, accent, 0.5);
    }
    if let Some(p) = hover {
        let x = ext_x(p.x).round() as i32;
        let y = ext_y(p.y).round() as i32;
        if x >= t {
            for yy in 0..t {
                cv.put(x, yy, accent);
            }
        }
        if y >= t {
            for xx in 0..t {
                cv.put(xx, y, accent);
            }
        }
    }
    for y in 0..t.min(h) {
        for x in 0..t.min(w) {
            cv.put(x, y, bg);
        }
    }
}

pub fn render(doc: &Document, cw: u32, ch: u32, rp: &RenderParams, ov: &Overlay) -> SharedPixelBuffer<Rgba8Pixel> {
    let cw = cw.max(1);
    let ch = ch.max(1);
    let mut buf = SharedPixelBuffer::<Rgba8Pixel>::new(cw, ch);
    let bg: [u8; 3] = if rp.dark { [13, 13, 16] } else { [227, 229, 234] };
    let (c1, c2): ([u8; 3], [u8; 3]) = if rp.dark { ([58, 58, 66], [74, 74, 84]) } else { ([255, 255, 255], [218, 220, 226]) };
    let v = doc.view;
    let z = v.zoom;
    let comp = &doc.composite;
    let (iw, ih) = (comp.w as i32, comp.h as i32);
    let x0 = v.ox;
    let y0 = v.oy;
    let x1 = x0 + iw as f32 * z;
    let y1 = y0 + ih as f32 * z;
    let grid = rp.grid && z >= 6.0;
    let samples = if z < 1.0 { ((1.0 / z).ceil() as i32).min(4) } else { 1 };

    // Precompute per-column document coordinates for nearest sampling.
    let cols: Vec<i32> = (0..cw).map(|sx| ((sx as f32 + 0.5 - x0) / z).floor() as i32).collect();

    let px = buf.make_mut_slice();
    px.par_chunks_mut(cw as usize).enumerate().for_each(|(sy, row)| {
        let fy = sy as f32 + 0.5;
        let doc_y = ((fy - y0) / z).floor() as i32;
        let gy = grid && {
            let ly = (fy - y0) / z;
            (ly - ly.floor()) * z < 1.0
        };
        for (sx, out) in row.iter_mut().enumerate() {
            let fx = sx as f32 + 0.5;
            let c: [u8; 3];
            if fx >= x0 && fx < x1 && fy >= y0 && fy < y1 {
                let ck = (((fx - x0) / 8.0) as i32 + ((fy - y0) / 8.0) as i32) & 1;
                let chk = if ck == 0 { c1 } else { c2 };
                let p: [f32; 4] = if samples == 1 {
                    let dx = cols[sx].clamp(0, iw - 1);
                    let dy = doc_y.clamp(0, ih - 1);
                    let q = comp.data[(dy * iw + dx) as usize];
                    [q[0] as f32, q[1] as f32, q[2] as f32, q[3] as f32 / 255.0]
                } else {
                    let mut acc = [0f32; 4];
                    let n = samples as f32;
                    for j in 0..samples {
                        for i in 0..samples {
                            let dx = (((fx - 0.5 + (i as f32 + 0.5) / n) - x0) / z).floor() as i32;
                            let dy = (((fy - 0.5 + (j as f32 + 0.5) / n) - y0) / z).floor() as i32;
                            let q = comp.data[(dy.clamp(0, ih - 1) * iw + dx.clamp(0, iw - 1)) as usize];
                            let a = q[3] as f32 / 255.0;
                            acc[0] += q[0] as f32 * a;
                            acc[1] += q[1] as f32 * a;
                            acc[2] += q[2] as f32 * a;
                            acc[3] += a;
                        }
                    }
                    let a = acc[3] / (n * n);
                    if acc[3] > 0.0 {
                        [acc[0] / acc[3], acc[1] / acc[3], acc[2] / acc[3], a]
                    } else {
                        [0.0, 0.0, 0.0, 0.0]
                    }
                };
                let a = p[3];
                let mut cc = [
                    (p[0] * a + chk[0] as f32 * (1.0 - a)) as u8,
                    (p[1] * a + chk[1] as f32 * (1.0 - a)) as u8,
                    (p[2] * a + chk[2] as f32 * (1.0 - a)) as u8,
                ];
                if grid {
                    let lx = (fx - x0) / z;
                    if gy || (lx - lx.floor()) * z < 1.0 {
                        for k in cc.iter_mut() {
                            *k = (*k as f32 * 0.7 + 128.0 * 0.3) as u8;
                        }
                    }
                }
                c = cc;
            } else {
                // Soft drop shadow around the image.
                let dx = (x0 - fx).max(fx - x1).max(0.0);
                let dy = (y0 + 3.0 - fy).max(fy - (y1 + 3.0)).max(0.0);
                let d = (dx * dx + dy * dy).sqrt();
                let r = 18.0;
                if d < r {
                    let k = (1.0 - d / r).powi(2) * if rp.dark { 0.6 } else { 0.22 };
                    c = [(bg[0] as f32 * (1.0 - k)) as u8, (bg[1] as f32 * (1.0 - k)) as u8, (bg[2] as f32 * (1.0 - k)) as u8];
                } else {
                    c = bg;
                }
            }
            *out = Rgba8Pixel { r: c[0], g: c[1], b: c[2], a: 255 };
        }
    });

    let mut cv = Canvas { px, w: cw as i32, h: ch as i32 };
    let to_s = |p: Pt| (x0 + p.x * z, y0 + p.y * z);

    // Selection outline ("marching ants").
    if let Some(m) = &doc.state.selection.mask {
        for s in &m.outline {
            let (ax, ay) = (x0 + s.x0 as f32 * z, y0 + s.y0 as f32 * z);
            let (bx, by) = (x0 + s.x1 as f32 * z, y0 + s.y1 as f32 * z);
            if (ax < -2.0 && bx < -2.0) || (ay < -2.0 && by < -2.0) || (ax > cw as f32 + 2.0 && bx > cw as f32 + 2.0) || (ay > ch as f32 + 2.0 && by > ch as f32 + 2.0) {
                continue;
            }
            if s.y0 == s.y1 {
                let y = (ay.round() as i32).clamp(-1, ch as i32);
                let xa = (ax.round() as i32).max(-1);
                let xb = (bx.round() as i32).min(cw as i32);
                for x in xa..xb {
                    let on = (((x + y) as u32).wrapping_add(rp.phase) / 4) % 2 == 0;
                    cv.put(x, y.min(ch as i32 - 1).max(0), if on { [0, 0, 0] } else { [255, 255, 255] });
                }
            } else {
                let x = (ax.round() as i32).clamp(-1, cw as i32);
                let ya = (ay.round() as i32).max(-1);
                let yb = (by.round() as i32).min(ch as i32);
                for y in ya..yb {
                    let on = (((x + y) as u32).wrapping_add(rp.phase) / 4) % 2 == 0;
                    cv.put(x.min(cw as i32 - 1).max(0), y, if on { [0, 0, 0] } else { [255, 255, 255] });
                }
            }
        }
    }

    for (a, b) in &ov.lines {
        let (ax, ay) = to_s(*a);
        let (bx, by) = to_s(*b);
        cv.dashed_line(ax, ay, bx, by, rp.phase);
    }
    if let Some((c, r)) = ov.circle {
        let (cx, cy) = to_s(c);
        let rr = r * z;
        if rr >= 2.0 {
            cv.circle(cx, cy, rr);
        }
    }
    if let Some(c) = ov.cross {
        let (cx, cy) = to_s(c);
        for d in -8..=8 {
            cv.put(cx as i32 + d, cy as i32, if d % 2 == 0 { [0, 0, 0] } else { [255, 255, 255] });
            cv.put(cx as i32, cy as i32 + d, if d % 2 == 0 { [0, 0, 0] } else { [255, 255, 255] });
        }
        cv.circle(cx, cy, 6.0);
    }
    if let Some(r) = ov.caret {
        let (ax, ay) = to_s(Pt::new(r.x0 as f32, r.y0 as f32));
        let (_, by) = to_s(Pt::new(r.x1 as f32, r.y1 as f32));
        let w = ((r.width() as f32 * z).round() as i32).max(1);
        for y in ay as i32..by as i32 {
            for x in 0..w + 1 {
                cv.put(ax as i32 + x, y, if x == 0 { [255, 255, 255] } else { [20, 20, 20] });
            }
        }
    }
    for h in &ov.handles {
        let (hx, hy) = to_s(*h);
        cv.handle(hx, hy);
    }
    if let Some(g) = rp.rulers {
        draw_rulers(&mut cv, g, comp.w, comp.h, z, x0, y0, rp.dark, rp.hover, rp.scale);
    }
    buf
}

/// Builds a small thumbnail image with a checkerboard background.
pub fn thumbnail(w: u32, h: u32, size: u32, get: impl Fn(i32, i32) -> [u8; 4] + Sync) -> slint::Image {
    let (tw, th) = crate::core::io::fit(w, h, size);
    let mut buf = SharedPixelBuffer::<Rgba8Pixel>::new(tw, th);
    let sx = w as f32 / tw as f32;
    let sy = h as f32 / th as f32;
    buf.make_mut_slice().par_chunks_mut(tw as usize).enumerate().for_each(|(y, row)| {
        for (x, o) in row.iter_mut().enumerate() {
            // 2x2 supersampling
            let mut acc = [0f32; 4];
            for j in 0..2 {
                for i in 0..2 {
                    let dx = ((x as f32 + (i as f32 + 0.5) / 2.0) * sx) as i32;
                    let dy = ((y as f32 + (j as f32 + 0.5) / 2.0) * sy) as i32;
                    let q = get(dx.min(w as i32 - 1), dy.min(h as i32 - 1));
                    let a = q[3] as f32 / 255.0;
                    acc[0] += q[0] as f32 * a;
                    acc[1] += q[1] as f32 * a;
                    acc[2] += q[2] as f32 * a;
                    acc[3] += a;
                }
            }
            let a = acc[3] / 4.0;
            let ck = if ((x / 4) + (y / 4)) % 2 == 0 { 255.0 } else { 210.0 };
            let col = |c: f32| if acc[3] > 0.0 { (c / acc[3]) * a + ck * (1.0 - a) } else { ck };
            *o = Rgba8Pixel { r: col(acc[0]) as u8, g: col(acc[1]) as u8, b: col(acc[2]) as u8, a: 255 };
        }
    });
    slint::Image::from_rgba8(buf)
}
