//! Raster painting primitives: coverage buffers, strokes, shapes, fills and gradients.

use super::geom::{dist_to_segment, Pt, Rect};
use super::selection::Selection;
use super::surface::{color_distance, lerp_px, over, Px};
use super::tiled::Tiled;

const CT: i32 = 64;

/// A sparse, tiled coverage buffer over a region of the document.
/// Tiles are only allocated once something is drawn into them.
#[derive(Clone)]
pub struct Cov {
    pub rect: Rect,
    tw: i32,
    tiles: Vec<Option<Box<[u8; (CT * CT) as usize]>>>,
    /// Area that has been touched so far.
    pub touched: Rect,
}

impl Cov {
    pub fn new(rect: Rect) -> Cov {
        let tw = (rect.width() + CT - 1) / CT;
        let th = (rect.height() + CT - 1) / CT;
        Cov { rect, tw, tiles: vec![None; (tw * th).max(0) as usize], touched: Rect::EMPTY }
    }

    /// Builds a coverage buffer from row-major data covering `rect`.
    pub fn from_data(rect: Rect, data: &[u8]) -> Cov {
        let mut c = Cov::new(rect);
        let rw = rect.width();
        for y in 0..rect.height() {
            for x in 0..rw {
                let v = data[(y * rw + x) as usize];
                if v != 0 {
                    c.set(rect.x0 + x, rect.y0 + y, v);
                }
            }
        }
        c.touched = rect;
        c
    }

    #[inline]
    fn locate(&self, x: i32, y: i32) -> Option<(usize, usize)> {
        if !self.rect.contains(x, y) {
            return None;
        }
        let lx = x - self.rect.x0;
        let ly = y - self.rect.y0;
        Some((((ly / CT) * self.tw + lx / CT) as usize, ((ly % CT) * CT + lx % CT) as usize))
    }

    #[inline]
    pub fn get(&self, x: i32, y: i32) -> u8 {
        match self.locate(x, y) {
            Some((t, i)) => match &self.tiles[t] {
                Some(tile) => tile[i],
                None => 0,
            },
            None => 0,
        }
    }

    #[inline]
    pub fn max(&mut self, x: i32, y: i32, v: u8) {
        if v == 0 {
            return;
        }
        if let Some((t, i)) = self.locate(x, y) {
            let tile = self.tiles[t].get_or_insert_with(|| Box::new([0; (CT * CT) as usize]));
            if tile[i] < v {
                tile[i] = v;
            }
        }
    }

    #[inline]
    pub fn set(&mut self, x: i32, y: i32, v: u8) {
        if let Some((t, i)) = self.locate(x, y) {
            if v == 0 && self.tiles[t].is_none() {
                return;
            }
            let tile = self.tiles[t].get_or_insert_with(|| Box::new([0; (CT * CT) as usize]));
            tile[i] = v;
        }
    }

    pub fn touch(&mut self, r: Rect) {
        self.touched = self.touched.union(&r.intersect(&self.rect));
    }

    /// Fills coverage (max-combined) for every pixel in `r` from a function returning 0..1.
    pub fn fill_with<F: Fn(f32, f32) -> f32 + Sync>(&mut self, r: Rect, f: F) {
        let r = r.intersect(&self.rect);
        if r.is_empty() {
            return;
        }
        use rayon::prelude::*;
        let (tw, rx0, ry0) = (self.tw, self.rect.x0, self.rect.y0);
        let (tx0, tx1) = ((r.x0 - rx0) / CT, (r.x1 - 1 - rx0) / CT + 1);
        let (ty0, ty1) = ((r.y0 - ry0) / CT, (r.y1 - 1 - ry0) / CT + 1);
        // Only visit the tiles that intersect `r`, in parallel by tile row.
        self.tiles.par_chunks_mut(tw as usize).enumerate().skip(ty0 as usize).take((ty1 - ty0) as usize).for_each(|(ty, row)| {
          let ty = ty as i32;
          for tx in tx0..tx1 {
            let slot = &mut row[tx as usize];
            let tr = Rect::new(rx0 + tx * CT, ry0 + ty * CT, rx0 + tx * CT + CT, ry0 + ty * CT + CT).intersect(&r);
            if tr.is_empty() {
                continue;
            }
            for y in tr.y0..tr.y1 {
                for x in tr.x0..tr.x1 {
                    let v = (f(x as f32 + 0.5, y as f32 + 0.5).clamp(0.0, 1.0) * 255.0).round() as u8;
                    if v == 0 {
                        continue;
                    }
                    let tile = slot.get_or_insert_with(|| Box::new([0; (CT * CT) as usize]));
                    let i = ((y - ry0 - ty * CT) * CT + (x - rx0 - tx * CT)) as usize;
                    if tile[i] < v {
                        tile[i] = v;
                    }
                }
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

/// Arrowhead length and half-width for a stroke of `width`.
pub fn arrow_size(width: f32) -> (f32, f32) {
    let w = width.max(1.0);
    (w * 3.0 + 8.0, w * 1.6 + 4.0)
}

/// The point `len` back along the polyline from its last point (or the first point).
fn point_back(pts: &[Pt], len: f32) -> Pt {
    let mut left = len;
    for i in (1..pts.len()).rev() {
        let d = pts[i].dist(pts[i - 1]);
        if d >= left && d > 0.0 {
            return pts[i].lerp(pts[i - 1], left / d);
        }
        left -= d;
    }
    pts[0]
}

/// Removes `len` of length from the end of a polyline.
pub fn trim_end(pts: &mut Vec<Pt>, len: f32) {
    let mut left = len;
    while pts.len() >= 2 {
        let n = pts.len();
        let d = pts[n - 1].dist(pts[n - 2]);
        if d > left {
            pts[n - 1] = pts[n - 1].lerp(pts[n - 2], left / d);
            return;
        }
        left -= d;
        pts.pop();
    }
}

/// Filled arrowhead at the end of the polyline, pointing along its last `len` of length.
pub fn arrow_cov(cov: &mut Cov, pts: &[Pt], width: f32, aa: bool) {
    if pts.len() < 2 {
        return;
    }
    let (len, half) = arrow_size(width);
    let tip = *pts.last().unwrap();
    let back = point_back(pts, len);
    let (dx, dy) = (tip.x - back.x, tip.y - back.y);
    let l = (dx * dx + dy * dy).sqrt();
    if l < 1e-3 {
        return;
    }
    let (ux, uy) = (dx / l, dy / l);
    let base = Pt::new(tip.x - ux * len, tip.y - uy * len);
    let a = Pt::new(base.x - uy * half, base.y + ux * half);
    let b = Pt::new(base.x + uy * half, base.y - ux * half);
    let r = Rect::new(
        tip.x.min(a.x).min(b.x).floor() as i32 - 2,
        tip.y.min(a.y).min(b.y).floor() as i32 - 2,
        tip.x.max(a.x).max(b.x).ceil() as i32 + 2,
        tip.y.max(a.y).max(b.y).ceil() as i32 + 2,
    );
    cov.fill_with(r, |x, y| fill_cov(sd_triangle(Pt::new(x, y), tip, a, b), aa));
    cov.touch(r);
}

/// Signed distance to a triangle (negative inside).
fn sd_triangle(p: Pt, p0: Pt, p1: Pt, p2: Pt) -> f32 {
    let sub = |a: Pt, b: Pt| (a.x - b.x, a.y - b.y);
    let dot = |a: (f32, f32), b: (f32, f32)| a.0 * b.0 + a.1 * b.1;
    let (e0, e1, e2) = (sub(p1, p0), sub(p2, p1), sub(p0, p2));
    let (v0, v1, v2) = (sub(p, p0), sub(p, p1), sub(p, p2));
    let pq = |v: (f32, f32), e: (f32, f32)| {
        let t = (dot(v, e) / dot(e, e).max(1e-9)).clamp(0.0, 1.0);
        (v.0 - e.0 * t, v.1 - e.1 * t)
    };
    let (q0, q1, q2) = (pq(v0, e0), pq(v1, e1), pq(v2, e2));
    let s = (e0.0 * e2.1 - e0.1 * e2.0).signum();
    let d0 = (dot(q0, q0), s * (v0.0 * e0.1 - v0.1 * e0.0));
    let d1 = (dot(q1, q1), s * (v1.0 * e1.1 - v1.1 * e1.0));
    let d2 = (dot(q2, q2), s * (v2.0 * e2.1 - v2.1 * e2.0));
    let dist = d0.0.min(d1.0).min(d2.0).sqrt();
    let inside = d0.1 > 0.0 && d1.1 > 0.0 && d2.1 > 0.0 || d0.1 < 0.0 && d1.1 < 0.0 && d2.1 < 0.0;
    if inside { -dist } else { dist }
}

// ---------------------------------------------------------------------------------------------
// Flood fill

/// Produces a full-image 0/255 mask of pixels similar to the seed color, plus its bounding box.
pub fn flood_mask(w: u32, h: u32, get: impl Fn(i32, i32) -> Px + Sync, sx: i32, sy: i32, tol: f32, contiguous: bool) -> (Vec<u8>, Rect) {
    use rayon::prelude::*;
    let (w, h) = (w as i32, h as i32);
    let mut mask = vec![0u8; (w * h) as usize];
    if sx < 0 || sy < 0 || sx >= w || sy >= h {
        return (mask, Rect::EMPTY);
    }
    let seed = get(sx, sy);
    let similar = |p: Px| color_distance(p, seed) <= tol + 1e-6;
    if !contiguous {
        let rows: Vec<Option<(i32, i32)>> = mask
            .par_chunks_mut(w as usize)
            .enumerate()
            .map(|(y, row)| {
                let mut span: Option<(i32, i32)> = None;
                for (x, m) in row.iter_mut().enumerate() {
                    if similar(get(x as i32, y as i32)) {
                        *m = 255;
                        span = Some(match span {
                            None => (x as i32, x as i32),
                            Some((a, _)) => (a, x as i32),
                        });
                    }
                }
                span
            })
            .collect();
        let mut b = Rect::EMPTY;
        for (y, s) in rows.iter().enumerate() {
            if let Some((a, z)) = s {
                b = b.union(&Rect::new(*a, y as i32, z + 1, y as i32 + 1));
            }
        }
        return (mask, b);
    }
    let mut b = Rect::EMPTY;
    let mut stack = vec![(sx, sy)];
    while let Some((x, y)) = stack.pop() {
        let row = (y * w) as usize;
        if mask[row + x as usize] != 0 || !similar(get(x, y)) {
            continue;
        }
        let mut l = x;
        while l > 0 && mask[row + (l - 1) as usize] == 0 && similar(get(l - 1, y)) {
            l -= 1;
        }
        let mut r = x;
        while r < w - 1 && mask[row + (r + 1) as usize] == 0 && similar(get(r + 1, y)) {
            r += 1;
        }
        mask[row + l as usize..=row + r as usize].fill(255);
        b = b.union(&Rect::new(l, y, r + 1, y + 1));
        for ny in [y - 1, y + 1] {
            if ny < 0 || ny >= h {
                continue;
            }
            let nrow = (ny * w) as usize;
            let mut in_run = false;
            for xx in l..=r {
                let ok = mask[nrow + xx as usize] == 0 && similar(get(xx, ny));
                if ok && !in_run {
                    stack.push((xx, ny));
                    in_run = true;
                } else if !ok {
                    in_run = false;
                }
            }
        }
    }
    (mask, b)
}

/// Crops a full-image mask to `r`.
pub fn crop_mask(mask: &[u8], w: u32, r: Rect) -> Vec<u8> {
    let mut out = Vec::with_capacity((r.width() * r.height()).max(0) as usize);
    for y in r.y0..r.y1 {
        let start = (y as u32 * w + r.x0 as u32) as usize;
        out.extend_from_slice(&mask[start..start + r.width() as usize]);
    }
    out
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
    region: Rect,
) {
    let r = sel.bounds(layer.w, layer.h).intersect(&region);
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
