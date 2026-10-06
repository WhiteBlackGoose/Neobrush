//! Raster painting primitives: coverage buffers, strokes, shapes, fills and gradients.

use super::geom::{dist_to_segment, Pt, Rect};
use super::selection::Selection;
use super::surface::{color_distance, lerp_px, over, Px, Surface};
use super::tiled::Tiled;

/// A coverage buffer over a region of the document.
#[derive(Clone)]
pub struct Cov {
    pub rect: Rect,
    pub data: Vec<u8>,
    /// Area that has been touched so far.
    pub touched: Rect,
}

impl Cov {
    pub fn new(rect: Rect) -> Cov {
        Cov { rect, data: vec![0; (rect.width() * rect.height()).max(0) as usize], touched: Rect::EMPTY }
    }
    #[inline]
    pub fn get(&self, x: i32, y: i32) -> u8 {
        if !self.rect.contains(x, y) {
            return 0;
        }
        self.data[((y - self.rect.y0) * self.rect.width() + (x - self.rect.x0)) as usize]
    }
    #[inline]
    pub fn max(&mut self, x: i32, y: i32, v: u8) {
        if !self.rect.contains(x, y) || v == 0 {
            return;
        }
        let i = ((y - self.rect.y0) * self.rect.width() + (x - self.rect.x0)) as usize;
        if self.data[i] < v {
            self.data[i] = v;
        }
    }
    #[inline]
    pub fn set(&mut self, x: i32, y: i32, v: u8) {
        if !self.rect.contains(x, y) {
            return;
        }
        let i = ((y - self.rect.y0) * self.rect.width() + (x - self.rect.x0)) as usize;
        self.data[i] = v;
    }
    pub fn touch(&mut self, r: Rect) {
        self.touched = self.touched.union(&r.intersect(&self.rect));
    }

    /// Fills coverage for every pixel in `r` from a function returning 0..1.
    pub fn fill_with<F: Fn(f32, f32) -> f32 + Sync>(&mut self, r: Rect, f: F) {
        let r = r.intersect(&self.rect);
        if r.is_empty() {
            return;
        }
        use rayon::prelude::*;
        let rw = self.rect.width() as usize;
        let (rx0, ry0) = (self.rect.x0, self.rect.y0);
        self.data.par_chunks_mut(rw).enumerate().for_each(|(row_i, row)| {
            let y = ry0 + row_i as i32;
            if y < r.y0 || y >= r.y1 {
                return;
            }
            for x in r.x0..r.x1 {
                let v = (f(x as f32 + 0.5, y as f32 + 0.5).clamp(0.0, 1.0) * 255.0).round() as u8;
                let i = (x - rx0) as usize;
                if row[i] < v {
                    row[i] = v;
                }
            }
        });
        self.touch(r);
    }
}

#[derive(Clone)]
pub enum PaintOp {
    /// Paint a color. `overwrite` replaces pixels (including alpha) instead of blending.
    Color { c: Px, overwrite: bool },
    Erase,
    /// Copies pixels from `src` offset by (dx, dy).
    Clone { src: Tiled, dx: i32, dy: i32 },
    /// Replaces colors similar to `from` with `to`.
    Recolor { from: Px, to: Px, tol: f32 },
}

impl PaintOp {
    #[inline]
    fn apply(&self, x: i32, y: i32, b: Px, cov: f32) -> Px {
        if cov <= 0.0 {
            return b;
        }
        match self {
            PaintOp::Color { c, overwrite } => {
                if *overwrite {
                    lerp_px(b, *c, cov)
                } else {
                    over(b, *c, cov)
                }
            }
            PaintOp::Erase => {
                let a = (b[3] as f32 * (1.0 - cov)).round() as u8;
                if a == 0 {
                    [0, 0, 0, 0]
                } else {
                    [b[0], b[1], b[2], a]
                }
            }
            PaintOp::Clone { src, dx, dy } => {
                let s = src.get(x + dx, y + dy);
                lerp_px(b, s, cov)
            }
            PaintOp::Recolor { from, to, tol } => {
                let d = color_distance([b[0], b[1], b[2], 255], [from[0], from[1], from[2], 255]);
                if d > *tol {
                    return b;
                }
                let k = if *tol <= 0.0 { 1.0 } else { (1.0 - d / tol).clamp(0.0, 1.0).sqrt() };
                let target = [to[0], to[1], to[2], b[3]];
                lerp_px(b, target, cov * k)
            }
        }
    }
}

/// Recomputes `layer` = `base` + ops within `r`, masking by selection.
pub fn apply_ops(layer: &mut Tiled, base: &Tiled, r: Rect, sel: &Selection, ops: &[(&Cov, &PaintOp)]) {
    layer.map_rect(r, |x, y, _| {
        let mut p = base.get(x, y);
        let s = sel.coverage(x, y) as f32 / 255.0;
        if s <= 0.0 {
            return p;
        }
        for (cov, op) in ops {
            let c = cov.get(x, y) as f32 / 255.0 * s;
            p = op.apply(x, y, p, c);
        }
        p
    });
}

/// Coverage profile of a round brush at distance d from its center line.
#[inline]
pub fn brush_profile(d: f32, radius: f32, hardness: f32, aa: bool) -> f32 {
    if !aa {
        return if d <= radius.max(0.5) { 1.0 } else { 0.0 };
    }
    let edge = (radius - d + 0.5).clamp(0.0, 1.0);
    if hardness >= 0.999 || radius < 1.5 {
        return edge;
    }
    let inner = radius * hardness;
    if d <= inner {
        edge
    } else {
        let t = ((d - inner) / (radius - inner).max(0.001)).clamp(0.0, 1.0);
        // Smoothstep falloff.
        let f = 1.0 - t * t * (3.0 - 2.0 * t);
        f * edge
    }
}

/// Adds a round brush segment (capsule) to the coverage buffer. Returns affected rect.
pub fn stroke_segment(cov: &mut Cov, a: Pt, b: Pt, width: f32, hardness: f32, aa: bool) -> Rect {
    let r = (width / 2.0).max(0.5);
    let rect = Rect::from_points(a.x, a.y, b.x, b.y).inflate(r.ceil() as i32 + 2);
    if !aa {
        // Aliased: pixel centers within the radius.
        cov.fill_with(rect, |x, y| if dist_to_segment(Pt::new(x, y), a, b) <= r { 1.0 } else { 0.0 });
    } else {
        cov.fill_with(rect, |x, y| brush_profile(dist_to_segment(Pt::new(x, y), a, b), r, hardness, aa));
    }
    rect.intersect(&cov.rect)
}

/// 1px aliased line (pencil).
pub fn pencil_line(cov: &mut Cov, a: Pt, b: Pt) -> Rect {
    let (mut x0, mut y0) = (a.x.floor() as i32, a.y.floor() as i32);
    let (x1, y1) = (b.x.floor() as i32, b.y.floor() as i32);
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        cov.set(x0, y0, 255);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
    let r = Rect::new(a.x.min(b.x).floor() as i32, a.y.min(b.y).floor() as i32, a.x.max(b.x).floor() as i32 + 1, a.y.max(b.y).floor() as i32 + 1);
    cov.touch(r);
    r
}

// ---------------------------------------------------------------------------------------------
// Shapes (signed distance based)

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    Rectangle,
    RoundedRectangle,
    Ellipse,
    Triangle,
    RightTriangle,
    Diamond,
    Pentagon,
    Hexagon,
    Star,
    ArrowRight,
    Heart,
}

pub const SHAPES: [ShapeKind; 11] = [
    ShapeKind::Rectangle,
    ShapeKind::RoundedRectangle,
    ShapeKind::Ellipse,
    ShapeKind::Triangle,
    ShapeKind::RightTriangle,
    ShapeKind::Diamond,
    ShapeKind::Pentagon,
    ShapeKind::Hexagon,
    ShapeKind::Star,
    ShapeKind::ArrowRight,
    ShapeKind::Heart,
];

impl ShapeKind {
    pub fn name(self) -> &'static str {
        match self {
            ShapeKind::Rectangle => "Rectangle",
            ShapeKind::RoundedRectangle => "Rounded Rectangle",
            ShapeKind::Ellipse => "Ellipse",
            ShapeKind::Triangle => "Triangle",
            ShapeKind::RightTriangle => "Right Triangle",
            ShapeKind::Diamond => "Diamond",
            ShapeKind::Pentagon => "Pentagon",
            ShapeKind::Hexagon => "Hexagon",
            ShapeKind::Star => "Star",
            ShapeKind::ArrowRight => "Arrow",
            ShapeKind::Heart => "Heart",
        }
    }

    /// Polygon vertices in unit box [0,1]^2 for polygonal shapes.
    fn polygon(self) -> Option<Vec<Pt>> {
        let reg = |n: usize, rot: f32| -> Vec<Pt> {
            (0..n)
                .map(|i| {
                    let a = rot + i as f32 * std::f32::consts::TAU / n as f32;
                    Pt::new(0.5 + 0.5 * a.cos(), 0.5 + 0.5 * a.sin())
                })
                .collect()
        };
        match self {
            ShapeKind::Triangle => Some(vec![Pt::new(0.5, 0.0), Pt::new(1.0, 1.0), Pt::new(0.0, 1.0)]),
            ShapeKind::RightTriangle => Some(vec![Pt::new(0.0, 0.0), Pt::new(1.0, 1.0), Pt::new(0.0, 1.0)]),
            ShapeKind::Diamond => Some(vec![Pt::new(0.5, 0.0), Pt::new(1.0, 0.5), Pt::new(0.5, 1.0), Pt::new(0.0, 0.5)]),
            ShapeKind::Pentagon => Some(reg(5, -std::f32::consts::FRAC_PI_2)),
            ShapeKind::Hexagon => Some(reg(6, 0.0)),
            ShapeKind::Star => {
                let mut v = Vec::new();
                for i in 0..10 {
                    let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
                    let r = if i % 2 == 0 { 0.5 } else { 0.2 };
                    v.push(Pt::new(0.5 + r * a.cos(), 0.5 + r * a.sin()));
                }
                Some(v)
            }
            ShapeKind::ArrowRight => Some(vec![
                Pt::new(0.0, 0.3),
                Pt::new(0.6, 0.3),
                Pt::new(0.6, 0.0),
                Pt::new(1.0, 0.5),
                Pt::new(0.6, 1.0),
                Pt::new(0.6, 0.7),
                Pt::new(0.0, 0.7),
            ]),
            ShapeKind::Heart => {
                let mut v = Vec::new();
                for i in 0..96 {
                    let t = i as f32 / 96.0 * std::f32::consts::TAU;
                    let x = 16.0 * t.sin().powi(3);
                    let y = 13.0 * t.cos() - 5.0 * (2.0 * t).cos() - 2.0 * (3.0 * t).cos() - (4.0 * t).cos();
                    v.push(Pt::new(0.5 + x / 34.0, 0.45 - y / 34.0));
                }
                Some(v)
            }
            _ => None,
        }
    }
}

fn sd_polygon(p: Pt, v: &[Pt]) -> f32 {
    let mut d = f32::MAX;
    let mut inside = false;
    let n = v.len();
    for i in 0..n {
        let a = v[i];
        let b = v[(i + 1) % n];
        d = d.min(dist_to_segment(p, a, b));
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
    }
    if inside {
        -d
    } else {
        d
    }
}

/// A shape in document space, described by its bounding box.
#[derive(Clone, Copy, Debug)]
pub struct Shape {
    pub kind: ShapeKind,
    pub a: Pt,
    pub b: Pt,
    pub radius: f32,
}

impl Shape {
    /// Signed distance (negative inside) in pixels.
    pub fn sdf(&self) -> Box<dyn Fn(Pt) -> f32 + Sync + '_> {
        let x0 = self.a.x.min(self.b.x);
        let y0 = self.a.y.min(self.b.y);
        let x1 = self.a.x.max(self.b.x);
        let y1 = self.a.y.max(self.b.y);
        let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let (hw, hh) = (((x1 - x0) / 2.0).max(0.01), ((y1 - y0) / 2.0).max(0.01));
        match self.kind {
            ShapeKind::Rectangle | ShapeKind::RoundedRectangle => {
                let r = if self.kind == ShapeKind::Rectangle { 0.0 } else { self.radius.min(hw).min(hh).max(0.0) };
                Box::new(move |p: Pt| {
                    let qx = (p.x - cx).abs() - (hw - r);
                    let qy = (p.y - cy).abs() - (hh - r);
                    let ox = qx.max(0.0);
                    let oy = qy.max(0.0);
                    (ox * ox + oy * oy).sqrt() + qx.max(qy).min(0.0) - r
                })
            }
            ShapeKind::Ellipse => Box::new(move |p: Pt| {
                let dx = p.x - cx;
                let dy = p.y - cy;
                let f = dx * dx / (hw * hw) + dy * dy / (hh * hh) - 1.0;
                let gx = 2.0 * dx / (hw * hw);
                let gy = 2.0 * dy / (hh * hh);
                let g = (gx * gx + gy * gy).sqrt().max(1e-6);
                f / g
            }),
            kind => {
                let poly: Vec<Pt> = kind
                    .polygon()
                    .unwrap()
                    .into_iter()
                    .map(|q| Pt::new(x0 + q.x * (x1 - x0), y0 + q.y * (y1 - y0)))
                    .collect();
                Box::new(move |p: Pt| sd_polygon(p, &poly))
            }
        }
    }

    pub fn bounds(&self, stroke: f32) -> Rect {
        Rect::from_points(self.a.x, self.a.y, self.b.x, self.b.y).inflate(stroke.ceil() as i32 + 2)
    }
}

/// Fill coverage from a signed distance.
#[inline]
pub fn fill_cov(d: f32, aa: bool) -> f32 {
    if aa {
        (0.5 - d).clamp(0.0, 1.0)
    } else if d <= 0.0 {
        1.0
    } else {
        0.0
    }
}

/// Outline coverage (stroke inside the shape edge) from a signed distance.
#[inline]
pub fn outline_cov(d: f32, w: f32, aa: bool) -> f32 {
    if aa {
        (0.5 - d).clamp(0.0, 1.0) * (0.5 + d + w).clamp(0.0, 1.0)
    } else if d <= 0.0 && d > -w {
        1.0
    } else {
        0.0
    }
}

/// Stroke coverage along a polyline.
pub fn polyline_cov(cov: &mut Cov, pts: &[Pt], width: f32, aa: bool, dash: f32) {
    if pts.len() < 2 {
        return;
    }
    let mut r = Rect::EMPTY;
    for p in pts {
        r = r.union(&Rect::new(p.x.floor() as i32, p.y.floor() as i32, p.x.ceil() as i32 + 1, p.y.ceil() as i32 + 1));
    }
    let r = r.inflate((width / 2.0).ceil() as i32 + 2);
    // Cumulative lengths for dashes.
    let mut cum = vec![0f32];
    for i in 1..pts.len() {
        let l = cum[i - 1] + pts[i - 1].dist(pts[i]);
        cum.push(l);
    }
    let hw = (width / 2.0).max(0.5);
    let pts = pts.to_vec();
    cov.fill_with(r, |x, y| {
        let p = Pt::new(x, y);
        let mut best = f32::MAX;
        let mut along = 0.0;
        for i in 1..pts.len() {
            let a = pts[i - 1];
            let b = pts[i];
            if (p.x < a.x.min(b.x) - hw - 2.0) || (p.x > a.x.max(b.x) + hw + 2.0) || (p.y < a.y.min(b.y) - hw - 2.0) || (p.y > a.y.max(b.y) + hw + 2.0) {
                continue;
            }
            let d = dist_to_segment(p, a, b);
            if d < best {
                best = d;
                let (vx, vy) = (b.x - a.x, b.y - a.y);
                let l2 = vx * vx + vy * vy;
                let t = if l2 <= 1e-9 { 0.0 } else { (((p.x - a.x) * vx + (p.y - a.y) * vy) / l2).clamp(0.0, 1.0) };
                along = cum[i - 1] + t * l2.sqrt();
            }
        }
        if best == f32::MAX {
            return 0.0;
        }
        if dash > 0.0 {
            let period = dash * 2.0;
            if along % period > dash {
                return 0.0;
            }
        }
        brush_profile(best, hw, 1.0, aa)
    });
}

// ---------------------------------------------------------------------------------------------
// Flood fill

/// Produces a 0/255 mask of pixels similar to the seed color.
pub fn flood_mask(src: &Surface, sx: i32, sy: i32, tol: f32, contiguous: bool) -> Vec<u8> {
    let (w, h) = (src.w as i32, src.h as i32);
    let mut mask = vec![0u8; (w * h) as usize];
    if !src.in_bounds(sx, sy) {
        return mask;
    }
    let seed = src.get(sx, sy);
    let similar = |p: Px| color_distance(p, seed) <= tol + 1e-6;
    if !contiguous {
        for (i, p) in src.data.iter().enumerate() {
            if similar(*p) {
                mask[i] = 255;
            }
        }
        return mask;
    }
    let mut stack = vec![(sx, sy)];
    while let Some((x, y)) = stack.pop() {
        let row = (y * w) as usize;
        if mask[row + x as usize] != 0 || !similar(src.data[row + x as usize]) {
            continue;
        }
        let mut l = x;
        while l > 0 && mask[row + (l - 1) as usize] == 0 && similar(src.data[row + (l - 1) as usize]) {
            l -= 1;
        }
        let mut r = x;
        while r < w - 1 && mask[row + (r + 1) as usize] == 0 && similar(src.data[row + (r + 1) as usize]) {
            r += 1;
        }
        for xx in l..=r {
            mask[row + xx as usize] = 255;
        }
        for ny in [y - 1, y + 1] {
            if ny < 0 || ny >= h {
                continue;
            }
            let nrow = (ny * w) as usize;
            let mut in_run = false;
            for xx in l..=r {
                let ok = mask[nrow + xx as usize] == 0 && similar(src.data[nrow + xx as usize]);
                if ok && !in_run {
                    stack.push((xx, ny));
                    in_run = true;
                } else if !ok {
                    in_run = false;
                }
            }
        }
    }
    mask
}

// ---------------------------------------------------------------------------------------------
// Gradients

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GradientKind {
    Linear,
    Reflected,
    Diamond,
    Radial,
    Conical,
}

impl GradientKind {
    pub fn from_index(i: i32) -> Self {
        match i {
            1 => GradientKind::Reflected,
            2 => GradientKind::Diamond,
            3 => GradientKind::Radial,
            4 => GradientKind::Conical,
            _ => GradientKind::Linear,
        }
    }
    /// Gradient parameter t in 0..1 at point p.
    pub fn t(self, a: Pt, b: Pt, p: Pt) -> f32 {
        let (vx, vy) = (b.x - a.x, b.y - a.y);
        let len2 = (vx * vx + vy * vy).max(1e-6);
        let len = len2.sqrt();
        let (px, py) = (p.x - a.x, p.y - a.y);
        match self {
            GradientKind::Linear => ((px * vx + py * vy) / len2).clamp(0.0, 1.0),
            GradientKind::Reflected => ((px * vx + py * vy) / len2).abs().clamp(0.0, 1.0),
            GradientKind::Diamond => {
                let u = (px * vx + py * vy) / len2;
                let v = (px * -vy + py * vx) / len2;
                (u.abs() + v.abs()).clamp(0.0, 1.0)
            }
            GradientKind::Radial => ((px * px + py * py).sqrt() / len).clamp(0.0, 1.0),
            GradientKind::Conical => {
                let a0 = vy.atan2(vx);
                let a1 = py.atan2(px);
                let mut d = (a1 - a0) / std::f32::consts::TAU;
                d -= d.floor();
                // Symmetric cone.
                (1.0 - (d * 2.0 - 1.0).abs()).clamp(0.0, 1.0)
            }
        }
    }
}

/// Renders a gradient into `layer` from `base`.
#[allow(clippy::too_many_arguments)]
pub fn apply_gradient(
    layer: &mut Tiled,
    base: &Tiled,
    sel: &Selection,
    kind: GradientKind,
    a: Pt,
    b: Pt,
    c0: Px,
    c1: Px,
    transparency: bool,
    overwrite: bool,
) {
    let r = sel.bounds(layer.w, layer.h);
    layer.map_rect(r, |x, y, _| {
        let bp = base.get(x, y);
        let s = sel.coverage(x, y) as f32 / 255.0;
        if s <= 0.0 {
            return bp;
        }
        let t = kind.t(a, b, Pt::new(x as f32 + 0.5, y as f32 + 0.5));
        if transparency {
            let alpha = (c0[3] as f32 * (1.0 - t) + c1[3] as f32 * t) / 255.0;
            let na = (bp[3] as f32 * (1.0 - s + s * alpha)).round() as u8;
            return [bp[0], bp[1], bp[2], na];
        }
        let g = lerp_px(c0, c1, t);
        if overwrite {
            lerp_px(bp, g, s)
        } else {
            over(bp, g, s)
        }
    });
}
