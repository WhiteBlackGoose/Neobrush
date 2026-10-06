//! Flat RGBA8 (straight alpha) pixel buffer used by algorithms.

use super::geom::Rect;

pub type Px = [u8; 4];

#[derive(Clone, Debug)]
pub struct Surface {
    pub w: u32,
    pub h: u32,
    pub data: Vec<Px>,
}

impl Surface {
    pub fn new(w: u32, h: u32) -> Self {
        Surface { w, h, data: vec![[0; 4]; (w as usize) * (h as usize)] }
    }
    pub fn filled(w: u32, h: u32, c: Px) -> Self {
        Surface { w, h, data: vec![c; (w as usize) * (h as usize)] }
    }
    #[inline]
    pub fn idx(&self, x: i32, y: i32) -> usize {
        y as usize * self.w as usize + x as usize
    }
    #[inline]
    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as u32) < self.w && (y as u32) < self.h
    }
    #[inline]
    pub fn get(&self, x: i32, y: i32) -> Px {
        self.data[self.idx(x, y)]
    }
    #[inline]
    pub fn get_or(&self, x: i32, y: i32, d: Px) -> Px {
        if self.in_bounds(x, y) {
            self.get(x, y)
        } else {
            d
        }
    }
    #[inline]
    pub fn get_clamped(&self, x: i32, y: i32) -> Px {
        let x = x.clamp(0, self.w as i32 - 1);
        let y = y.clamp(0, self.h as i32 - 1);
        self.get(x, y)
    }
    #[inline]
    pub fn set(&mut self, x: i32, y: i32, c: Px) {
        let i = self.idx(x, y);
        self.data[i] = c;
    }
    /// Bilinear sample returning premultiplied floats (0..255). Out-of-bounds is transparent.
    pub fn sample_bilinear_premul(&self, fx: f32, fy: f32) -> [f32; 4] {
        let fx = fx - 0.5;
        let fy = fy - 0.5;
        let x0 = fx.floor() as i32;
        let y0 = fy.floor() as i32;
        let tx = fx - x0 as f32;
        let ty = fy - y0 as f32;
        let mut acc = [0f32; 4];
        for (dx, dy, wgt) in [
            (0, 0, (1.0 - tx) * (1.0 - ty)),
            (1, 0, tx * (1.0 - ty)),
            (0, 1, (1.0 - tx) * ty),
            (1, 1, tx * ty),
        ] {
            if wgt <= 0.0 {
                continue;
            }
            let p = self.get_or(x0 + dx, y0 + dy, [0; 4]);
            let a = p[3] as f32 / 255.0;
            acc[0] += p[0] as f32 * a * wgt;
            acc[1] += p[1] as f32 * a * wgt;
            acc[2] += p[2] as f32 * a * wgt;
            acc[3] += p[3] as f32 * wgt;
        }
        acc
    }

    /// Bilinear sample with edge clamping, straight alpha output.
    pub fn sample_bilinear_clamped(&self, fx: f32, fy: f32) -> Px {
        let fx = (fx - 0.5).clamp(0.0, self.w as f32 - 1.0);
        let fy = (fy - 0.5).clamp(0.0, self.h as f32 - 1.0);
        let x0 = fx.floor() as i32;
        let y0 = fy.floor() as i32;
        let tx = fx - x0 as f32;
        let ty = fy - y0 as f32;
        let mut acc = [0f32; 4];
        for (dx, dy, wgt) in [
            (0, 0, (1.0 - tx) * (1.0 - ty)),
            (1, 0, tx * (1.0 - ty)),
            (0, 1, (1.0 - tx) * ty),
            (1, 1, tx * ty),
        ] {
            let p = self.get_clamped(x0 + dx, y0 + dy);
            let a = p[3] as f32 / 255.0;
            acc[0] += p[0] as f32 * a * wgt;
            acc[1] += p[1] as f32 * a * wgt;
            acc[2] += p[2] as f32 * a * wgt;
            acc[3] += p[3] as f32 * wgt;
        }
        unpremul(acc)
    }

    pub fn crop(&self, r: Rect) -> Surface {
        let mut out = Surface::new(r.width() as u32, r.height() as u32);
        for y in 0..r.height() {
            for x in 0..r.width() {
                let c = self.get_or(r.x0 + x, r.y0 + y, [0; 4]);
                out.set(x, y, c);
            }
        }
        out
    }

    pub fn from_image(img: &image::RgbaImage) -> Surface {
        let (w, h) = img.dimensions();
        let data = img.as_raw().chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect();
        Surface { w, h, data }
    }

    pub fn to_image(&self) -> image::RgbaImage {
        let raw: Vec<u8> = self.data.iter().flatten().copied().collect();
        image::RgbaImage::from_raw(self.w, self.h, raw).unwrap()
    }
}

/// Converts premultiplied float (0..255 scale) into straight u8.
#[inline]
pub fn unpremul(p: [f32; 4]) -> Px {
    let a = p[3];
    if a <= 0.001 {
        return [0, 0, 0, 0];
    }
    let k = 255.0 / a;
    [
        (p[0] * k).round().clamp(0.0, 255.0) as u8,
        (p[1] * k).round().clamp(0.0, 255.0) as u8,
        (p[2] * k).round().clamp(0.0, 255.0) as u8,
        a.round().clamp(0.0, 255.0) as u8,
    ]
}

/// Linear interpolation in premultiplied space between two straight colors.
#[inline]
pub fn lerp_px(a: Px, b: Px, t: f32) -> Px {
    if t <= 0.0 {
        return a;
    }
    if t >= 1.0 {
        return b;
    }
    let aa = a[3] as f32 / 255.0;
    let ba = b[3] as f32 / 255.0;
    let mut p = [0f32; 4];
    for i in 0..3 {
        p[i] = a[i] as f32 * aa * (1.0 - t) + b[i] as f32 * ba * t;
    }
    p[3] = a[3] as f32 * (1.0 - t) + b[3] as f32 * t;
    unpremul(p)
}

/// Normal "source over" compositing of src (with extra coverage 0..1) onto dst.
#[inline]
pub fn over(dst: Px, src: Px, cov: f32) -> Px {
    let sa = src[3] as f32 / 255.0 * cov;
    if sa <= 0.0 {
        return dst;
    }
    let da = dst[3] as f32 / 255.0;
    let oa = sa + da * (1.0 - sa);
    if oa <= 0.0 {
        return [0, 0, 0, 0];
    }
    let mut out = [0u8; 4];
    for i in 0..3 {
        let c = (src[i] as f32 * sa + dst[i] as f32 * da * (1.0 - sa)) / oa;
        out[i] = c.round().clamp(0.0, 255.0) as u8;
    }
    out[3] = (oa * 255.0).round().clamp(0.0, 255.0) as u8;
    out
}

pub fn luminance(p: Px) -> f32 {
    0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32
}

/// Paint.NET style color distance in 0..1 (RGBA euclidean).
pub fn color_distance(a: Px, b: Px) -> f32 {
    let mut s = 0.0f32;
    for i in 0..4 {
        let d = a[i] as f32 - b[i] as f32;
        s += d * d;
    }
    (s / (4.0 * 255.0 * 255.0)).sqrt()
}
