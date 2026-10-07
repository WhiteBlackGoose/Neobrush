//! Ongoing tool interactions: short-lived drags and longer editing sessions
//! (shapes, lines, text and floating pixels) that stay editable until finished.

use super::editor::Editor;
use super::render::Overlay;
use crate::core::document::Document;
use crate::core::geom::{flatten_cubic, Pt, Rect};
use crate::core::paint::{self, Cov, PaintOp, Shape, ShapeKind};
use crate::core::selection::{Mask, Selection, SelectionMode};
use crate::core::surface::{over, unpremul, Px, Surface};
use crate::core::text::{FontLib, TextStyle};
use crate::core::tiled::Tiled;
use std::sync::Arc;

/// Tool options snapshot taken from the UI.
#[derive(Clone)]
pub struct Opts {
    pub primary: Px,
    pub secondary: Px,
    pub width: f32,
    pub hardness: f32,
    pub aa: bool,
    pub overwrite: bool,
    pub tolerance: f32,
    pub global: bool,
    pub sample_image: bool,
    pub sel_mode: SelectionMode,
    pub gradient: paint::GradientKind,
    pub gradient_alpha: bool,
    pub shape: ShapeKind,
    pub fill_style: i32,
    pub radius: f32,
    pub dash: i32,
    pub text: TextStyle,
}

pub struct StrokeState {
    pub base: Tiled,
    pub cov: Cov,
    pub last: Pt,
    pub op: PaintOp,
    pub width: f32,
    pub hardness: f32,
    pub aa: bool,
    pub pencil: bool,
    pub name: &'static str,
    pub icon: &'static str,
    pub clone_dx: i32,
    pub clone_dy: i32,
    pub color: Px,
}

pub enum Drag {
    None,
    Pan { sx: f32, sy: f32, ox: f32, oy: f32 },
    Stroke(Box<StrokeState>),
    Select { start: Pt, base: Selection, mode: SelectionMode, ellipse: bool, moved: bool },
    Lasso { pts: Vec<Pt>, base: Selection, mode: SelectionMode },
    Gradient { a: Pt, b: Pt, base: Tiled, swap: bool },
    ZoomRect { a: Pt, b: Pt, button: i32 },
    Picker { button: i32 },
}

impl Drag {
    pub fn overlay(&self, ov: &mut Overlay) {
        match self {
            Drag::Lasso { pts, .. } => {
                for w in pts.windows(2) {
                    ov.lines.push((w[0], w[1]));
                }
            }
            Drag::Gradient { a, b, .. } => {
                ov.lines.push((*a, *b));
                ov.handles.push(*a);
                ov.handles.push(*b);
            }
            Drag::ZoomRect { a, b, .. } => {
                let c = [*a, Pt::new(b.x, a.y), *b, Pt::new(a.x, b.y)];
                for i in 0..4 {
                    ov.lines.push((c[i], c[(i + 1) % 4]));
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub struct Xf {
    pub tx: f32,
    pub ty: f32,
    pub sx: f32,
    pub sy: f32,
    pub angle: f32,
}

impl Default for Xf {
    fn default() -> Self {
        Xf { tx: 0.0, ty: 0.0, sx: 1.0, sy: 1.0, angle: 0.0 }
    }
}

impl Xf {
    pub fn is_translation(&self) -> bool {
        (self.sx - 1.0).abs() < 1e-4 && (self.sy - 1.0).abs() < 1e-4 && self.angle.abs() < 1e-4
    }
    pub fn forward(&self, c: Pt, p: Pt) -> Pt {
        let (s, co) = self.angle.sin_cos();
        let lx = (p.x - c.x) * self.sx;
        let ly = (p.y - c.y) * self.sy;
        Pt::new(c.x + self.tx + lx * co - ly * s, c.y + self.ty + lx * s + ly * co)
    }
    pub fn inverse(&self, c: Pt, p: Pt) -> Pt {
        let (s, co) = self.angle.sin_cos();
        let dx = p.x - c.x - self.tx;
        let dy = p.y - c.y - self.ty;
        let lx = dx * co + dy * s;
        let ly = -dx * s + dy * co;
        Pt::new(c.x + lx / self.sx, c.y + ly / self.sy)
    }
}

pub enum MoveDrag {
    Translate { start: Pt, orig: Xf },
    Rotate { start: f32, orig: Xf },
    Scale { corner: usize, orig: Xf },
}

pub struct MoveSession {
    pub pixels: bool,
    pub layer: usize,
    pub cleared: Tiled,
    pub float: Surface,
    pub rect: Rect,
    pub mask: Arc<Mask>,
    pub xf: Xf,
    pub drag: Option<MoveDrag>,
    /// Destination area of the previous apply (to invalidate what moved away).
    pub last_dest: std::cell::Cell<Rect>,
}

impl MoveSession {
    pub fn center(&self) -> Pt {
        Pt::new((self.rect.x0 + self.rect.x1) as f32 / 2.0, (self.rect.y0 + self.rect.y1) as f32 / 2.0)
    }
    pub fn corners(&self) -> [Pt; 4] {
        let r = self.rect;
        let c = self.center();
        [
            self.xf.forward(c, Pt::new(r.x0 as f32, r.y0 as f32)),
            self.xf.forward(c, Pt::new(r.x1 as f32, r.y0 as f32)),
            self.xf.forward(c, Pt::new(r.x1 as f32, r.y1 as f32)),
            self.xf.forward(c, Pt::new(r.x0 as f32, r.y1 as f32)),
        ]
    }
    fn dest_bounds(&self, doc: Rect) -> Rect {
        let mut r = Rect::EMPTY;
        for p in self.corners() {
            r = r.union(&Rect::new(p.x.floor() as i32, p.y.floor() as i32, p.x.ceil() as i32 + 1, p.y.ceil() as i32 + 1));
        }
        r.inflate(1).intersect(&doc)
    }

    /// Writes the transformed pixels/selection into the document. With `region`, pixels are
    /// only updated inside it (used while dragging; the full update follows on release).
    pub fn apply(&self, doc: &mut Document, region: Option<Rect>) {
        let drect = doc.state.rect();
        let clip = region.unwrap_or(drect).intersect(&drect);
        let c = self.center();
        let xf = self.xf;
        let ti = (xf.tx.round() as i32, xf.ty.round() as i32);
        let translate = xf.is_translation();
        let dest = self.dest_bounds(drect);
        if self.pixels {
            let mut px = self.cleared.clone();
            let fr = self.rect;
            let float = &self.float;
            if translate {
                let r = fr.intersect(&Rect::new(-ti.0, -ti.1, drect.x1 - ti.0, drect.y1 - ti.1));
                px.map_rect(Rect::new(r.x0 + ti.0, r.y0 + ti.1, r.x1 + ti.0, r.y1 + ti.1).intersect(&clip), |x, y, b| {
                    let s = float.get_or(x - ti.0 - fr.x0, y - ti.1 - fr.y0, [0; 4]);
                    over(b, s, 1.0)
                });
            } else {
                px.map_rect(dest.intersect(&clip), |x, y, b| {
                    let sp = xf.inverse(c, Pt::new(x as f32 + 0.5, y as f32 + 0.5));
                    let s = unpremul(float.sample_bilinear_premul(sp.x - fr.x0 as f32, sp.y - fr.y0 as f32));
                    over(b, s, 1.0)
                });
            }
            doc.state.layers[self.layer].px = px;
        }
        // Transform the selection mask (only its bounding region is stored).
        let (w, h) = (doc.state.w, doc.state.h);
        let m = &self.mask;
        let d = if translate { Rect::new(m.bounds.x0 + ti.0, m.bounds.y0 + ti.1, m.bounds.x1 + ti.0, m.bounds.y1 + ti.1).intersect(&drect) } else { dest };
        let dw = d.width().max(0);
        let mut data = vec![0u8; (dw * d.height().max(0)) as usize];
        for y in d.y0..d.y1 {
            for x in d.x0..d.x1 {
                let v = if translate {
                    m.get(x - ti.0, y - ti.1)
                } else {
                    let sp = xf.inverse(c, Pt::new(x as f32 + 0.5, y as f32 + 0.5));
                    let (fx, fy) = (sp.x - 0.5, sp.y - 0.5);
                    let (x0, y0) = (fx.floor() as i32, fy.floor() as i32);
                    let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
                    let v = m.get(x0, y0) as f32 * (1.0 - tx) * (1.0 - ty)
                        + m.get(x0 + 1, y0) as f32 * tx * (1.0 - ty)
                        + m.get(x0, y0 + 1) as f32 * (1.0 - tx) * ty
                        + m.get(x0 + 1, y0 + 1) as f32 * tx * ty;
                    v.round() as u8
                };
                data[((y - d.y0) * dw + (x - d.x0)) as usize] = v;
            }
        }
        doc.state.selection = Selection::from_mask(Mask::from_region(w, h, d, data));
        let old = self.last_dest.get();
        doc.invalidate(old.union(&dest).union(&self.rect));
        self.last_dest.set(dest);
    }

    /// Returns the handle index under the screen-space point, if any.
    pub fn hit_corner(&self, p: Pt, tol: f32) -> Option<usize> {
        self.corners().iter().position(|c| c.dist(p) <= tol)
    }

    pub fn contains(&self, p: Pt) -> bool {
        let q = self.xf.inverse(self.center(), p);
        let r = self.rect;
        q.x >= r.x0 as f32 && q.x <= r.x1 as f32 && q.y >= r.y0 as f32 && q.y <= r.y1 as f32
    }

    pub fn drag_to(&mut self, p: Pt, shift: bool) {
        let c = self.center();
        match &self.drag {
            Some(MoveDrag::Translate { start, orig }) => {
                let mut dx = p.x - start.x;
                let mut dy = p.y - start.y;
                if shift {
                    if dx.abs() > dy.abs() {
                        dy = 0.0;
                    } else {
                        dx = 0.0;
                    }
                }
                self.xf = Xf { tx: orig.tx + dx, ty: orig.ty + dy, ..*orig };
            }
            Some(MoveDrag::Rotate { start, orig }) => {
                let cc = Pt::new(c.x + orig.tx, c.y + orig.ty);
                let a = (p.y - cc.y).atan2(p.x - cc.x);
                let mut ang = orig.angle + (a - start);
                if shift {
                    let step = 15f32.to_radians();
                    ang = (ang / step).round() * step;
                }
                self.xf = Xf { angle: ang, ..*orig };
            }
            Some(MoveDrag::Scale { corner, orig }) => {
                let r = self.rect;
                let hw = r.width() as f32 / 2.0;
                let hh = r.height() as f32 / 2.0;
                let k = [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)][*corner];
                let o = (-k.0, -k.1);
                let (s, co) = orig.angle.sin_cos();
                // World position of the opposite corner, which stays fixed.
                let ox = o.0 * orig.sx;
                let oy = o.1 * orig.sy;
                let wo = Pt::new(c.x + orig.tx + ox * co - oy * s, c.y + orig.ty + ox * s + oy * co);
                let dx = (p.x - wo.x) / 2.0;
                let dy = (p.y - wo.y) / 2.0;
                let qx = dx * co + dy * s;
                let qy = -dx * s + dy * co;
                let mut sx = if k.0.abs() > 0.0 { qx / k.0 } else { 1.0 };
                let mut sy = if k.1.abs() > 0.0 { qy / k.1 } else { 1.0 };
                if shift {
                    let u = (sx.abs() + sy.abs()) / 2.0;
                    sx = u * sx.signum();
                    sy = u * sy.signum();
                }
                if sx.abs() < 0.01 {
                    sx = 0.01 * sx.signum().max(0.0).max(1.0);
                }
                if sy.abs() < 0.01 {
                    sy = 0.01;
                }
                // New translation keeping the opposite corner fixed.
                let nox = o.0 * sx;
                let noy = o.1 * sy;
                let tx = wo.x - c.x - (nox * co - noy * s);
                let ty = wo.y - c.y - (nox * s + noy * co);
                self.xf = Xf { tx, ty, sx, sy, angle: orig.angle };
            }
            None => {}
        }
    }
}

// ---------------------------------------------------------------------------------------------

pub enum ShapeDrag {
    Create,
    Corner(usize),
    Move { start: Pt, a0: Pt, b0: Pt },
}

pub struct ShapeSession {
    pub layer: usize,
    pub base: Tiled,
    pub a: Pt,
    pub b: Pt,
    pub drag: Option<ShapeDrag>,
    pub swap: bool,
    pub last: Rect,
}

impl ShapeSession {
    pub fn corners(&self) -> [Pt; 4] {
        [self.a, Pt::new(self.b.x, self.a.y), self.b, Pt::new(self.a.x, self.b.y)]
    }
    pub fn contains(&self, p: Pt) -> bool {
        p.x >= self.a.x.min(self.b.x) && p.x <= self.a.x.max(self.b.x) && p.y >= self.a.y.min(self.b.y) && p.y <= self.a.y.max(self.b.y)
    }
    pub fn render(&mut self, doc: &mut Document, o: &Opts) {
        let (c1, c2) = if self.swap { (o.secondary, o.primary) } else { (o.primary, o.secondary) };
        let shape = Shape { kind: o.shape, a: self.a, b: self.b, radius: o.radius };
        let r = shape.bounds(o.width).intersect(&doc.state.rect());
        let sdf = shape.sdf();
        let mut fill = Cov::new(r);
        let mut line = Cov::new(r);
        let w = o.width;
        let aa = o.aa;
        if o.fill_style >= 1 {
            fill.fill_with(r, |x, y| paint::fill_cov(sdf(Pt::new(x, y)), aa));
        }
        if o.fill_style != 1 {
            line.fill_with(r, |x, y| paint::outline_cov(sdf(Pt::new(x, y)), w, aa));
        }
        let fill_c = if o.fill_style == 2 { c2 } else { c1 };
        let fop = PaintOp::Color { c: fill_c, overwrite: o.overwrite };
        let lop = PaintOp::Color { c: c1, overwrite: o.overwrite };
        let dirty = self.last.union(&r);
        let sel = doc.state.selection.clone();
        paint::apply_ops(&mut doc.state.layers[self.layer].px, &self.base, dirty, &sel, &[(&fill, &fop), (&line, &lop)]);
        doc.invalidate(dirty);
        self.last = r;
    }
}

pub struct LineSession {
    pub layer: usize,
    pub base: Tiled,
    /// Start, control 1, control 2, end.
    pub pts: [Pt; 4],
    pub drag: Option<usize>,
    pub creating: bool,
    pub swap: bool,
    pub last: Rect,
}

impl LineSession {
    pub fn render(&mut self, doc: &mut Document, o: &Opts) {
        let c = if self.swap { o.secondary } else { o.primary };
        let mut poly = Vec::new();
        flatten_cubic(self.pts[0], self.pts[1], self.pts[2], self.pts[3], &mut poly);
        let mut r = Rect::EMPTY;
        for p in &poly {
            r = r.union(&Rect::new(p.x.floor() as i32, p.y.floor() as i32, p.x.ceil() as i32 + 1, p.y.ceil() as i32 + 1));
        }
        let r = r.inflate(o.width.ceil() as i32 + 3).intersect(&doc.state.rect());
        let mut cov = Cov::new(r);
        let dash = match o.dash {
            1 => o.width.max(1.0) * 3.0,
            2 => o.width.max(1.0) * 1.0,
            _ => 0.0,
        };
        paint::polyline_cov(&mut cov, &poly, o.width, o.aa, dash);
        let op = PaintOp::Color { c, overwrite: o.overwrite };
        let dirty = self.last.union(&r);
        let sel = doc.state.selection.clone();
        paint::apply_ops(&mut doc.state.layers[self.layer].px, &self.base, dirty, &sel, &[(&cov, &op)]);
        doc.invalidate(dirty);
        self.last = r;
    }
}

pub struct TextSession {
    pub layer: usize,
    pub base: Tiled,
    pub pos: Pt,
    pub text: String,
    pub last: Rect,
    pub caret: Rect,
    pub bounds: Rect,
    pub drag: Option<(Pt, Pt)>,
}

impl TextSession {
    pub fn render(&mut self, doc: &mut Document, o: &Opts, fonts: &mut FontLib) {
        let lay = fonts.render(&self.text, &o.text, self.pos.x, self.pos.y, doc.state.rect());
        let op = PaintOp::Color { c: o.primary, overwrite: o.overwrite };
        let dirty = self.last.union(&lay.cov.touched).intersect(&doc.state.rect());
        let sel = doc.state.selection.clone();
        paint::apply_ops(&mut doc.state.layers[self.layer].px, &self.base, dirty, &sel, &[(&lay.cov, &op)]);
        doc.invalidate(dirty);
        self.last = lay.cov.touched;
        self.caret = lay.caret;
        self.bounds = lay.bounds.union(&lay.caret);
    }
}

pub enum Session {
    None,
    Move(Box<MoveSession>),
    Shape(Box<ShapeSession>),
    Line(Box<LineSession>),
    Text(Box<TextSession>),
}

impl Session {
    pub fn is_editing(&self) -> bool {
        matches!(self, Session::Shape(_) | Session::Line(_) | Session::Text(_))
    }

    pub fn overlay(&self, ov: &mut Overlay, _doc: &Document, caret_on: bool) {
        match self {
            Session::None => {}
            Session::Move(m) => {
                let c = m.corners();
                for p in c {
                    ov.handles.push(p);
                }
            }
            Session::Shape(s) => {
                let c = s.corners();
                for i in 0..4 {
                    ov.lines.push((c[i], c[(i + 1) % 4]));
                }
                if s.drag.is_none() || !matches!(s.drag, Some(ShapeDrag::Create)) {
                    for p in c {
                        ov.handles.push(p);
                    }
                }
            }
            Session::Line(l) => {
                if !l.creating {
                    ov.lines.push((l.pts[0], l.pts[1]));
                    ov.lines.push((l.pts[3], l.pts[2]));
                    for p in l.pts {
                        ov.handles.push(p);
                    }
                }
            }
            Session::Text(t) => {
                let b = t.bounds.inflate(3);
                let c = [Pt::new(b.x0 as f32, b.y0 as f32), Pt::new(b.x1 as f32, b.y0 as f32), Pt::new(b.x1 as f32, b.y1 as f32), Pt::new(b.x0 as f32, b.y1 as f32)];
                for i in 0..4 {
                    ov.lines.push((c[i], c[(i + 1) % 4]));
                }
                if caret_on {
                    ov.caret = Some(t.caret);
                }
            }
        }
    }

    pub fn cursor_override(&self, ed: &Editor) -> Option<i32> {
        let h = ed.hover?;
        let tol = 8.0 * ed.scale / ed.doc()?.view.zoom;
        match self {
            Session::Move(m) => {
                if m.hit_corner(h, tol).is_some() {
                    Some(6)
                } else if m.contains(h) {
                    Some(2)
                } else {
                    None
                }
            }
            Session::Shape(s) => {
                if s.corners().iter().any(|c| c.dist(h) <= tol) {
                    Some(6)
                } else if s.contains(h) {
                    Some(2)
                } else {
                    None
                }
            }
            Session::Line(l) => {
                if l.pts.iter().any(|c| c.dist(h) <= tol) {
                    Some(6)
                } else {
                    None
                }
            }
            Session::Text(t) => {
                if t.bounds.inflate(3).contains(h.x as i32, h.y as i32) {
                    Some(2)
                } else {
                    None
                }
            }
            Session::None => None,
        }
    }
}
