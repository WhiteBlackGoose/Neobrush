//! Basic geometry helpers.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Rect {
    pub x0: i32,
    pub y0: i32,
    /// Exclusive.
    pub x1: i32,
    /// Exclusive.
    pub y1: i32,
}

impl Rect {
    pub const EMPTY: Rect = Rect { x0: 0, y0: 0, x1: 0, y1: 0 };

    pub fn new(x0: i32, y0: i32, x1: i32, y1: i32) -> Self {
        Rect { x0, y0, x1, y1 }
    }
    pub fn from_size(w: u32, h: u32) -> Self {
        Rect { x0: 0, y0: 0, x1: w as i32, y1: h as i32 }
    }
    /// Rectangle spanned by two arbitrary points (inclusive of both pixels).
    pub fn from_points(ax: f32, ay: f32, bx: f32, by: f32) -> Self {
        Rect {
            x0: ax.min(bx).floor() as i32,
            y0: ay.min(by).floor() as i32,
            x1: ax.max(bx).ceil() as i32,
            y1: ay.max(by).ceil() as i32,
        }
    }
    pub fn width(&self) -> i32 {
        (self.x1 - self.x0).max(0)
    }
    pub fn height(&self) -> i32 {
        (self.y1 - self.y0).max(0)
    }
    pub fn is_empty(&self) -> bool {
        self.x1 <= self.x0 || self.y1 <= self.y0
    }
    pub fn intersect(&self, o: &Rect) -> Rect {
        let r = Rect {
            x0: self.x0.max(o.x0),
            y0: self.y0.max(o.y0),
            x1: self.x1.min(o.x1),
            y1: self.y1.min(o.y1),
        };
        if r.is_empty() {
            Rect::EMPTY
        } else {
            r
        }
    }
    pub fn union(&self, o: &Rect) -> Rect {
        if self.is_empty() {
            return *o;
        }
        if o.is_empty() {
            return *self;
        }
        Rect {
            x0: self.x0.min(o.x0),
            y0: self.y0.min(o.y0),
            x1: self.x1.max(o.x1),
            y1: self.y1.max(o.y1),
        }
    }
    pub fn inflate(&self, d: i32) -> Rect {
        Rect { x0: self.x0 - d, y0: self.y0 - d, x1: self.x1 + d, y1: self.y1 + d }
    }
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x0 && x < self.x1 && y >= self.y0 && y < self.y1
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Pt {
    pub x: f32,
    pub y: f32,
}

impl Pt {
    pub fn new(x: f32, y: f32) -> Self {
        Pt { x, y }
    }
    pub fn dist(&self, o: Pt) -> f32 {
        ((self.x - o.x).powi(2) + (self.y - o.y).powi(2)).sqrt()
    }
    pub fn lerp(&self, o: Pt, t: f32) -> Pt {
        Pt::new(self.x + (o.x - self.x) * t, self.y + (o.y - self.y) * t)
    }
}

/// Distance from point p to segment ab.
pub fn dist_to_segment(p: Pt, a: Pt, b: Pt) -> f32 {
    let (vx, vy) = (b.x - a.x, b.y - a.y);
    let (wx, wy) = (p.x - a.x, p.y - a.y);
    let l2 = vx * vx + vy * vy;
    let t = if l2 <= 1e-9 { 0.0 } else { ((wx * vx + wy * vy) / l2).clamp(0.0, 1.0) };
    let (cx, cy) = (a.x + vx * t, a.y + vy * t);
    ((p.x - cx).powi(2) + (p.y - cy).powi(2)).sqrt()
}

/// Flattens a cubic bezier into a polyline.
pub fn flatten_cubic(p0: Pt, p1: Pt, p2: Pt, p3: Pt, out: &mut Vec<Pt>) {
    let len = p0.dist(p1) + p1.dist(p2) + p2.dist(p3);
    let n = ((len / 3.0).ceil() as usize).clamp(1, 512);
    for i in 0..=n {
        let t = i as f32 / n as f32;
        let mt = 1.0 - t;
        let a = mt * mt * mt;
        let b = 3.0 * mt * mt * t;
        let c = 3.0 * mt * t * t;
        let d = t * t * t;
        out.push(Pt::new(
            a * p0.x + b * p1.x + c * p2.x + d * p3.x,
            a * p0.y + b * p1.y + c * p2.y + d * p3.y,
        ));
    }
}
