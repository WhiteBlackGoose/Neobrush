//! Adjustments and effects.

use super::geom::Rect;
use super::surface::{luminance, unpremul, Px, Surface};
use rayon::prelude::*;

#[derive(Clone, Debug)]
pub enum ParamKind {
    Slider { min: f32, max: f32, step: f32, decimals: i32 },
    Check,
    Choice(Vec<&'static str>),
}

#[derive(Clone, Debug)]
pub struct ParamDef {
    pub label: &'static str,
    pub kind: ParamKind,
    pub default: f32,
}

fn sl(label: &'static str, min: f32, max: f32, default: f32) -> ParamDef {
    ParamDef { label, kind: ParamKind::Slider { min, max, step: 1.0, decimals: 0 }, default }
}
fn slf(label: &'static str, min: f32, max: f32, default: f32, decimals: i32) -> ParamDef {
    let step = 10f32.powi(-decimals);
    ParamDef { label, kind: ParamKind::Slider { min, max, step, decimals }, default }
}
fn ck(label: &'static str, default: bool) -> ParamDef {
    ParamDef { label, kind: ParamKind::Check, default: if default { 1.0 } else { 0.0 } }
}
fn ch(label: &'static str, choices: &[&'static str], default: usize) -> ParamDef {
    ParamDef { label, kind: ParamKind::Choice(choices.to_vec()), default: default as f32 }
}

pub struct Ctx {
    pub primary: Px,
    pub secondary: Px,
    /// Bounds of the selection (or the whole image).
    pub bounds: Rect,
    /// Curves LUTs (r, g, b) for the curves adjustment.
    pub curves: Option<[[u8; 256]; 3]>,
    /// Image position of the surface's top-left pixel (non-zero when previewing a crop).
    pub ox: i32,
    pub oy: i32,
}

impl Ctx {
    fn plain() -> Ctx {
        Ctx { primary: [0; 4], secondary: [0; 4], bounds: Rect::EMPTY, curves: None, ox: 0, oy: 0 }
    }
}

/// Margin around the visible area used when previewing on a crop.
pub const PREVIEW_MARGIN: i32 = 256;

/// Filters that must see the whole image (they can't be previewed on a crop).
pub fn needs_full_image(id: &str) -> bool {
    matches!(id, "auto-level" | "rotate-zoom" | "polar-inversion" | "twist" | "bulge" | "radial-blur" | "zoom-blur" | "drop-shadow")
}

pub type RunFn = fn(&Surface, &[f32], &Ctx) -> Surface;

pub struct FilterDef {
    pub id: &'static str,
    pub name: &'static str,
    pub params: Vec<ParamDef>,
    pub run: RunFn,
}

pub fn find(id: &str) -> Option<FilterDef> {
    all().into_iter().find(|f| f.id == id)
}

pub fn all() -> Vec<FilterDef> {
    macro_rules! f {
        ($id:expr, $name:expr, [$($p:expr),*], $run:expr) => {
            FilterDef { id: $id, name: $name, params: vec![$($p),*], run: $run }
        };
    }
    vec![
        // Adjustments
        f!("auto-level", "Auto-Level", [], auto_level),
        f!("black-and-white", "Black and White", [], black_white),
        f!("brightness-contrast", "Brightness / Contrast", [sl("Brightness", -100.0, 100.0, 0.0), sl("Contrast", -100.0, 100.0, 0.0)], brightness_contrast),
        f!("curves", "Curves", [], curves),
        f!("hue-saturation", "Hue / Saturation", [sl("Hue", -180.0, 180.0, 0.0), sl("Saturation", 0.0, 200.0, 100.0), sl("Lightness", -100.0, 100.0, 0.0)], hue_saturation),
        f!("invert-colors", "Invert Colors", [], invert),
        f!("levels", "Levels", [sl("Input black", 0.0, 254.0, 0.0), sl("Input white", 1.0, 255.0, 255.0), slf("Gamma", 0.1, 5.0, 1.0, 2), sl("Output black", 0.0, 254.0, 0.0), sl("Output white", 1.0, 255.0, 255.0)], levels),
        f!("posterize", "Posterize", [sl("Red", 2.0, 64.0, 16.0), sl("Green", 2.0, 64.0, 16.0), sl("Blue", 2.0, 64.0, 16.0)], posterize),
        f!("sepia", "Sepia", [], sepia),
        f!("vibrance", "Vibrance", [sl("Vibrance", -100.0, 100.0, 30.0)], vibrance),
        f!("temperature", "Temperature / Tint", [sl("Temperature", -100.0, 100.0, 0.0), sl("Tint", -100.0, 100.0, 0.0)], temperature),
        // Artistic
        f!("ink-sketch", "Ink Sketch", [sl("Ink outline", 0.0, 99.0, 50.0), sl("Coloring", 0.0, 100.0, 50.0)], ink_sketch),
        f!("oil-painting", "Oil Painting", [sl("Brush size", 1.0, 8.0, 3.0), sl("Coarseness", 3.0, 255.0, 50.0)], oil_painting),
        f!("pencil-sketch", "Pencil Sketch", [sl("Pencil tip size", 1.0, 20.0, 2.0), sl("Color range", -20.0, 20.0, 0.0)], pencil_sketch),
        // Blurs
        f!("fragment", "Fragment", [sl("Fragments", 2.0, 50.0, 4.0), sl("Distance", 0.0, 100.0, 8.0), sl("Rotation", 0.0, 360.0, 0.0)], fragment),
        f!("gaussian-blur", "Gaussian Blur", [sl("Radius", 0.0, 200.0, 2.0)], gaussian_blur),
        f!("motion-blur", "Motion Blur", [sl("Angle", -180.0, 180.0, 25.0), sl("Distance", 1.0, 200.0, 10.0), ck("Centered", true)], motion_blur),
        f!("radial-blur", "Radial Blur", [sl("Angle", 0.0, 360.0, 2.0), slf("Center X", -1.0, 1.0, 0.0, 2), slf("Center Y", -1.0, 1.0, 0.0, 2), sl("Quality", 1.0, 5.0, 2.0)], radial_blur),
        f!("surface-blur", "Surface Blur", [sl("Radius", 1.0, 30.0, 6.0), sl("Threshold", 1.0, 100.0, 15.0)], surface_blur),
        f!("unfocus", "Unfocus", [sl("Radius", 1.0, 100.0, 4.0)], unfocus),
        f!("zoom-blur", "Zoom Blur", [sl("Amount", 0.0, 100.0, 10.0), slf("Center X", -1.0, 1.0, 0.0, 2), slf("Center Y", -1.0, 1.0, 0.0, 2)], zoom_blur),
        // Distort
        f!("bulge", "Bulge", [sl("Amount", -200.0, 100.0, 45.0), slf("Center X", -1.0, 1.0, 0.0, 2), slf("Center Y", -1.0, 1.0, 0.0, 2)], bulge),
        f!("dents", "Dents", [sl("Scale", 1.0, 200.0, 25.0), sl("Refraction", 0.0, 200.0, 50.0), sl("Roughness", 0.0, 100.0, 10.0), sl("Seed", 0.0, 1000.0, 0.0)], dents),
        f!("frosted-glass", "Frosted Glass", [sl("Amount", 1.0, 20.0, 4.0), sl("Seed", 0.0, 1000.0, 0.0)], frosted_glass),
        f!("pixelate", "Pixelate", [sl("Cell size", 2.0, 100.0, 8.0)], pixelate),
        f!("polar-inversion", "Polar Inversion", [slf("Amount", -4.0, 4.0, 1.0, 2), ck("Tile", true)], polar_inversion),
        f!("tile-reflection", "Tile Reflection", [sl("Tile size", 2.0, 200.0, 40.0), sl("Curvature", -100.0, 100.0, 8.0), sl("Angle", -180.0, 180.0, 30.0)], tile_reflection),
        f!("twist", "Twist", [sl("Amount", -100.0, 100.0, 30.0), sl("Size", 1.0, 200.0, 100.0)], twist),
        // Noise
        f!("add-noise", "Add Noise", [sl("Intensity", 0.0, 100.0, 64.0), sl("Color saturation", 0.0, 400.0, 100.0), sl("Coverage", 0.0, 100.0, 100.0), sl("Seed", 0.0, 1000.0, 0.0)], add_noise),
        f!("median", "Median", [sl("Radius", 1.0, 50.0, 10.0), sl("Percentile", 0.0, 100.0, 50.0)], median),
        f!("reduce-noise", "Reduce Noise", [sl("Radius", 1.0, 50.0, 6.0), slf("Strength", 0.0, 1.0, 0.4, 2)], reduce_noise),
        // Photo
        f!("glow", "Glow", [sl("Radius", 1.0, 50.0, 6.0), sl("Brightness", -100.0, 100.0, 10.0), sl("Contrast", -100.0, 100.0, 10.0)], glow),
        f!("red-eye", "Red Eye Removal", [sl("Tolerance", 0.0, 100.0, 70.0), sl("Saturation", 0.0, 100.0, 90.0)], red_eye),
        f!("sharpen", "Sharpen", [sl("Amount", 1.0, 20.0, 2.0)], sharpen),
        f!("soft-portrait", "Soft Portrait", [sl("Softness", 0.0, 10.0, 5.0), sl("Lighting", -20.0, 20.0, 0.0), sl("Warmth", 0.0, 20.0, 10.0)], soft_portrait),
        f!("vignette", "Vignette", [slf("Center X", -1.0, 1.0, 0.0, 2), slf("Center Y", -1.0, 1.0, 0.0, 2), slf("Radius", 0.1, 4.0, 1.0, 2), sl("Strength", 0.0, 100.0, 70.0)], vignette),
        // Render
        f!("clouds", "Clouds", [sl("Scale", 2.0, 1000.0, 250.0), slf("Roughness", 0.0, 1.0, 0.5, 2), sl("Seed", 0.0, 1000.0, 0.0), ch("Blend", &["Normal", "Multiply", "Overlay", "Screen"], 0)], clouds),
        f!("julia", "Julia Fractal", [slf("Factor", 1.0, 10.0, 4.0, 1), sl("Quality", 1.0, 5.0, 2.0), slf("Zoom", 0.1, 50.0, 1.0, 1), sl("Angle", -180.0, 180.0, 0.0)], julia),
        f!("mandelbrot", "Mandelbrot Fractal", [slf("Factor", 1.0, 10.0, 1.0, 1), sl("Quality", 1.0, 5.0, 2.0), slf("Zoom", 0.1, 50.0, 1.0, 1), sl("Angle", -180.0, 180.0, 0.0), ck("Invert colors", false)], mandelbrot),
        // Stylize
        f!("edge-detect", "Edge Detect", [sl("Angle", -180.0, 180.0, 45.0)], edge_detect),
        f!("emboss", "Emboss", [sl("Angle", -180.0, 180.0, 0.0)], emboss),
        f!("outline", "Outline", [sl("Thickness", 1.0, 200.0, 3.0), sl("Intensity", 0.0, 100.0, 50.0)], outline_fx),
        f!("relief", "Relief", [sl("Angle", -180.0, 180.0, 45.0)], relief),
        // Object
        f!("drop-shadow", "Drop Shadow", [sl("Offset X", -100.0, 100.0, 6.0), sl("Offset Y", -100.0, 100.0, 6.0), sl("Blur", 0.0, 100.0, 8.0), sl("Opacity", 0.0, 100.0, 60.0)], drop_shadow),
        // Layer
        f!("rotate-zoom", "Layer Rotate / Zoom", [sl("Angle", -180.0, 180.0, 0.0), slf("Zoom", 0.1, 8.0, 1.0, 2), slf("Pan X", -1.0, 1.0, 0.0, 2), slf("Pan Y", -1.0, 1.0, 0.0, 2), ck("Tiling", false)], rotate_zoom_fx),
    ]
}

// ---------------------------------------------------------------------------------------------
// Helpers

fn map_px(s: &Surface, f: impl Fn(Px) -> Px + Sync) -> Surface {
    Surface { w: s.w, h: s.h, data: s.data.par_iter().map(|p| f(*p)).collect() }
}

fn map_xy(s: &Surface, f: impl Fn(i32, i32) -> Px + Sync) -> Surface {
    let mut out = Surface::new(s.w, s.h);
    let w = s.w as usize;
    out.data.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        for (x, o) in row.iter_mut().enumerate() {
            *o = f(x as i32, y as i32);
        }
    });
    out
}

#[inline]
fn c8(v: f32) -> u8 {
    v.round().clamp(0.0, 255.0) as u8
}

fn lut_map(s: &Surface, lut: &[[u8; 256]; 3]) -> Surface {
    map_px(s, |p| [lut[0][p[0] as usize], lut[1][p[1] as usize], lut[2][p[2] as usize], p[3]])
}

pub fn rgb_to_hsl(p: Px) -> (f32, f32, f32) {
    let r = p[0] as f32 / 255.0;
    let g = p[1] as f32 / 255.0;
    let b = p[2] as f32 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if (max - min).abs() < 1e-6 {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if max == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 60.0, s, l)
}

pub fn hsl_to_rgb(h: f32, s: f32, l: f32, a: u8) -> Px {
    if s <= 0.0 {
        let v = c8(l * 255.0);
        return [v, v, v, a];
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let hk = h.rem_euclid(360.0) / 360.0;
    let f = |t: f32| {
        let t = t.rem_euclid(1.0);
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    [c8(f(hk + 1.0 / 3.0) * 255.0), c8(f(hk) * 255.0), c8(f(hk - 1.0 / 3.0) * 255.0), a]
}

/// Deterministic hash based random in 0..1.
#[inline]
fn hash(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374761393) ^ (y as u32).wrapping_mul(668265263) ^ seed.wrapping_mul(2246822519);
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    h ^= h >> 16;
    (h & 0xFFFFFF) as f32 / 16777216.0
}

/// Box blur on premultiplied float data, horizontal then vertical.
fn box_blur_premul(data: &mut [[f32; 4]], w: usize, h: usize, r: usize) {
    if r == 0 {
        return;
    }
    let mut tmp = vec![[0f32; 4]; w * h];
    let norm = 1.0 / (2 * r + 1) as f32;
    tmp.par_chunks_mut(w).enumerate().for_each(|(y, out)| {
        let row = &data[y * w..(y + 1) * w];
        let mut acc = [0f32; 4];
        for i in 0..=2 * r {
            let x = (i as isize - r as isize).clamp(0, w as isize - 1) as usize;
            for c in 0..4 {
                acc[c] += row[x][c];
            }
        }
        for x in 0..w {
            for c in 0..4 {
                out[x][c] = acc[c] * norm;
            }
            let add = (x + r + 1).min(w - 1);
            let sub = (x as isize - r as isize).max(0) as usize;
            for c in 0..4 {
                acc[c] += row[add][c] - row[sub][c];
            }
        }
    });
    // Vertical pass, processing columns in parallel chunks.
    let cols: Vec<Vec<[f32; 4]>> = (0..w)
        .into_par_iter()
        .map(|x| {
            let mut col = vec![[0f32; 4]; h];
            let mut acc = [0f32; 4];
            for i in 0..=2 * r {
                let y = (i as isize - r as isize).clamp(0, h as isize - 1) as usize;
                for c in 0..4 {
                    acc[c] += tmp[y * w + x][c];
                }
            }
            for y in 0..h {
                for c in 0..4 {
                    col[y][c] = acc[c] * norm;
                }
                let add = (y + r + 1).min(h - 1);
                let sub = (y as isize - r as isize).max(0) as usize;
                for c in 0..4 {
                    acc[c] += tmp[add * w + x][c] - tmp[sub * w + x][c];
                }
            }
            col
        })
        .collect();
    data.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            row[x] = cols[x][y];
        }
    });
}

fn to_premul(s: &Surface) -> Vec<[f32; 4]> {
    s.data
        .par_iter()
        .map(|p| {
            let a = p[3] as f32 / 255.0;
            [p[0] as f32 * a, p[1] as f32 * a, p[2] as f32 * a, p[3] as f32]
        })
        .collect()
}

fn from_premul(w: u32, h: u32, d: &[[f32; 4]]) -> Surface {
    Surface { w, h, data: d.par_iter().map(|p| unpremul(*p)).collect() }
}

pub fn blur(s: &Surface, radius: f32) -> Surface {
    if radius < 0.5 || s.w == 0 || s.h == 0 {
        return s.clone();
    }
    let mut d = to_premul(s);
    // Three box passes approximate a gaussian with sigma ~ radius/2.
    let sigma = radius / 2.0;
    let r = ((12.0 * sigma * sigma / 3.0 + 1.0).sqrt() - 1.0) / 2.0;
    let r = r.round().max(1.0) as usize;
    for _ in 0..3 {
        box_blur_premul(&mut d, s.w as usize, s.h as usize, r);
    }
    from_premul(s.w, s.h, &d)
}

fn convolve3(s: &Surface, k: [f32; 9], bias: f32, keep_alpha: bool) -> Surface {
    map_xy(s, |x, y| {
        let mut acc = [0f32; 3];
        for j in 0..3 {
            for i in 0..3 {
                let p = s.get_clamped(x + i - 1, y + j - 1);
                let kk = k[(j * 3 + i) as usize];
                for c in 0..3 {
                    acc[c] += p[c] as f32 * kk;
                }
            }
        }
        let a = if keep_alpha { s.get(x, y)[3] } else { 255 };
        [c8(acc[0] + bias), c8(acc[1] + bias), c8(acc[2] + bias), a]
    })
}

fn center(ctx: &Ctx, ox: f32, oy: f32) -> (f32, f32, f32, f32) {
    let b = ctx.bounds;
    let hw = b.width() as f32 / 2.0;
    let hh = b.height() as f32 / 2.0;
    (b.x0 as f32 + hw * (1.0 + ox), b.y0 as f32 + hh * (1.0 + oy), hw.max(1.0), hh.max(1.0))
}

// ---------------------------------------------------------------------------------------------
// Adjustments

fn auto_level(s: &Surface, _: &[f32], _: &Ctx) -> Surface {
    let mut hist = [[0u32; 256]; 3];
    let mut n = 0u32;
    for p in &s.data {
        if p[3] == 0 {
            continue;
        }
        n += 1;
        for c in 0..3 {
            hist[c][p[c] as usize] += 1;
        }
    }
    if n == 0 {
        return s.clone();
    }
    let clip = (n as f32 * 0.005) as u32;
    let mut lut = [[0u8; 256]; 3];
    for c in 0..3 {
        let mut lo = 0;
        let mut acc = 0;
        for i in 0..256 {
            acc += hist[c][i];
            if acc > clip {
                lo = i;
                break;
            }
        }
        let mut hi = 255;
        acc = 0;
        for i in (0..256).rev() {
            acc += hist[c][i];
            if acc > clip {
                hi = i;
                break;
            }
        }
        let range = (hi as f32 - lo as f32).max(1.0);
        for i in 0..256 {
            lut[c][i] = c8((i as f32 - lo as f32) / range * 255.0);
        }
    }
    lut_map(s, &lut)
}

fn black_white(s: &Surface, _: &[f32], _: &Ctx) -> Surface {
    map_px(s, |p| {
        let l = c8(luminance(p));
        [l, l, l, p[3]]
    })
}

fn brightness_contrast(s: &Surface, p: &[f32], _: &Ctx) -> Surface {
    let b = p[0] / 100.0 * 255.0 * 0.5;
    let c = p[1] / 100.0;
    let k = if c >= 0.0 { 1.0 / (1.0 - c * 0.99) } else { 1.0 + c };
    let mut lut = [0u8; 256];
    for (i, v) in lut.iter_mut().enumerate() {
        *v = c8((i as f32 - 127.5) * k + 127.5 + b);
    }
    lut_map(s, &[lut, lut, lut])
}

fn curves(s: &Surface, _: &[f32], ctx: &Ctx) -> Surface {
    match &ctx.curves {
        Some(l) => lut_map(s, l),
        None => s.clone(),
    }
}

fn hue_saturation(s: &Surface, p: &[f32], _: &Ctx) -> Surface {
    let (dh, sat, light) = (p[0], p[1] / 100.0, p[2] / 100.0);
    map_px(s, |px| {
        let (h, s0, l) = rgb_to_hsl(px);
        let mut l2 = l;
        if light > 0.0 {
            l2 = l + (1.0 - l) * light;
        } else if light < 0.0 {
            l2 = l * (1.0 + light);
        }
        hsl_to_rgb(h + dh, (s0 * sat).clamp(0.0, 1.0), l2, px[3])
    })
}

fn invert(s: &Surface, _: &[f32], _: &Ctx) -> Surface {
    map_px(s, |p| [255 - p[0], 255 - p[1], 255 - p[2], p[3]])
}

fn levels(s: &Surface, p: &[f32], _: &Ctx) -> Surface {
    let (ib, iw, g, ob, ow) = (p[0], p[1].max(p[0] + 1.0), p[2].max(0.01), p[3], p[4]);
    let mut lut = [0u8; 256];
    for (i, v) in lut.iter_mut().enumerate() {
        let t = ((i as f32 - ib) / (iw - ib)).clamp(0.0, 1.0).powf(1.0 / g);
        *v = c8(ob + t * (ow - ob));
    }
    lut_map(s, &[lut, lut, lut])
}

fn posterize(s: &Surface, p: &[f32], _: &Ctx) -> Surface {
    let mut lut = [[0u8; 256]; 3];
    for c in 0..3 {
        let n = p[c].max(2.0);
        for i in 0..256 {
            let t = (i as f32 / 255.0 * (n - 1.0)).round() / (n - 1.0);
            lut[c][i] = c8(t * 255.0);
        }
    }
    lut_map(s, &lut)
}

fn sepia(s: &Surface, _: &[f32], _: &Ctx) -> Surface {
    map_px(s, |p| {
        let l = luminance(p) / 255.0;
        // Tone mapping toward a warm brown.
        let r = l * 255.0 * 1.07 + 10.0;
        let g = l * 255.0 * 0.95 + 4.0;
        let b = l * 255.0 * 0.76;
        [c8(r), c8(g), c8(b), p[3]]
    })
}

fn vibrance(s: &Surface, p: &[f32], _: &Ctx) -> Surface {
    let amt = p[0] / 100.0;
    map_px(s, |px| {
        let (h, sat, l) = rgb_to_hsl(px);
        let k = if amt > 0.0 { sat + (1.0 - sat) * amt * (1.0 - sat) } else { sat * (1.0 + amt) };
        hsl_to_rgb(h, k.clamp(0.0, 1.0), l, px[3])
    })
}

fn temperature(s: &Surface, p: &[f32], _: &Ctx) -> Surface {
    let t = p[0] / 100.0 * 40.0;
    let tint = p[1] / 100.0 * 40.0;
    map_px(s, |px| [c8(px[0] as f32 + t), c8(px[1] as f32 + tint), c8(px[2] as f32 - t), px[3]])
}

// ---------------------------------------------------------------------------------------------
// Artistic

fn ink_sketch(s: &Surface, p: &[f32], ctx: &Ctx) -> Surface {
    let ink = p[0] / 100.0;
    let coloring = p[1] / 100.0;
    let edges = edge_magnitude(s);
    let blurred = blur(s, 2.0);
    let _ = ctx;
    map_xy(s, |x, y| {
        let i = (y as u32 * s.w + x as u32) as usize;
        let e = (edges[i] * (0.5 + ink * 2.0)).clamp(0.0, 1.0);
        let b = blurred.data[i];
        let l = luminance(b);
        let mut col = [0f32; 3];
        for c in 0..3 {
            let grey = l;
            col[c] = grey + (b[c] as f32 - grey) * coloring;
            col[c] = col[c] * (1.0 - e);
        }
        [c8(col[0]), c8(col[1]), c8(col[2]), s.data[i][3]]
    })
}

fn edge_magnitude(s: &Surface) -> Vec<f32> {
    let w = s.w as i32;
    let h = s.h as i32;
    (0..(w * h))
        .into_par_iter()
        .map(|i| {
            let x = i % w;
            let y = i / w;
            let l = |dx: i32, dy: i32| luminance(s.get_clamped(x + dx, y + dy)) / 255.0;
            let gx = -l(-1, -1) - 2.0 * l(-1, 0) - l(-1, 1) + l(1, -1) + 2.0 * l(1, 0) + l(1, 1);
            let gy = -l(-1, -1) - 2.0 * l(0, -1) - l(1, -1) + l(-1, 1) + 2.0 * l(0, 1) + l(1, 1);
            (gx * gx + gy * gy).sqrt()
        })
        .collect()
}

fn oil_painting(s: &Surface, p: &[f32], _: &Ctx) -> Surface {
    let r = p[0] as i32;
    let levels = p[1].max(3.0) as usize;
    map_xy(s, |x, y| {
        let mut count = vec![0u32; levels];
        let mut sum = vec![[0u32; 3]; levels];
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy > r * r {
                    continue;
                }
                let q = s.get_clamped(x + dx, y + dy);
                let bin = ((luminance(q) / 256.0) * levels as f32) as usize;
                let bin = bin.min(levels - 1);
                count[bin] += 1;
                for c in 0..3 {
                    sum[bin][c] += q[c] as u32;
                }
            }
        }
        let (bi, n) = count.iter().enumerate().max_by_key(|(_, c)| **c).unwrap();
        let n = (*n).max(1);
        [(sum[bi][0] / n) as u8, (sum[bi][1] / n) as u8, (sum[bi][2] / n) as u8, s.get(x, y)[3]]
    })
}

fn pencil_sketch(s: &Surface, p: &[f32], _: &Ctx) -> Surface {
    let tip = p[0];
    let range = p[1] / 20.0;
    let grey = black_white(s, &[], &Ctx::plain());
    let inv = invert(&grey, &[], &Ctx::plain());
    let b = blur(&inv, tip * 2.0);
    map_xy(s, |x, y| {
        let i = (y as u32 * s.w + x as u32) as usize;
        let base = grey.data[i][0] as f32;
        let top = b.data[i][0] as f32;
        // Color dodge.
        let mut v = if top >= 255.0 { 255.0 } else { (base * 255.0 / (255.0 - top)).min(255.0) };
        v = (v - 128.0) * (1.0 + range) + 128.0 + range * 40.0;
        let v = c8(v);
        [v, v, v, s.data[i][3]]
    })
}

// ---------------------------------------------------------------------------------------------
// Blurs

fn fragment(s: &Surface, p: &[f32], _: &Ctx) -> Surface {
    let n = p[0] as i32;
    let dist = p[1];
    let rot = p[2].to_radians();
    let offs: Vec<(f32, f32)> = (0..n)
        .map(|i| {
            let a = rot + i as f32 * std::f32::consts::TAU / n as f32;
            (a.cos() * dist, a.sin() * dist)
        })
        .collect();
    map_xy(s, |x, y| {
        let mut acc = [0f32; 4];
        for (ox, oy) in &offs {
            let q = s.get_clamped(x + *ox as i32, y + *oy as i32);
            let a = q[3] as f32 / 255.0;
            for c in 0..3 {
                acc[c] += q[c] as f32 * a;
            }
            acc[3] += q[3] as f32;
        }
        for v in acc.iter_mut() {
            *v /= n as f32;
        }
        unpremul(acc)
    })
}

fn gaussian_blur(s: &Surface, p: &[f32], _: &Ctx) -> Surface {
    blur(s, p[0])
}

fn line_blur(s: &Surface, x: i32, y: i32, dx: f32, dy: f32, n: i32, centered: bool) -> Px {
    let mut acc = [0f32; 4];
    let start = if centered { -n / 2 } else { 0 };
    let mut cnt = 0.0;
    for i in start..start + n.max(1) {
        let fx = x as f32 + 0.5 + dx * i as f32;
        let fy = y as f32 + 0.5 + dy * i as f32;
        let q = s.sample_bilinear_clamped(fx, fy);
        let a = q[3] as f32 / 255.0;
        for c in 0..3 {
            acc[c] += q[c] as f32 * a;
        }
        acc[3] += q[3] as f32;
        cnt += 1.0;
    }
    for v in acc.iter_mut() {
        *v /= cnt;
    }
    unpremul(acc)
}

fn motion_blur(s: &Surface, p: &[f32], _: &Ctx) -> Surface {
    let a = p[0].to_radians();
    let n = p[1] as i32;
    let centered = p[2] > 0.5;
    map_xy(s, |x, y| line_blur(s, x, y, a.cos(), -a.sin(), n, centered))
}

fn radial_blur(s: &Surface, p: &[f32], ctx: &Ctx) -> Surface {
    let angle = p[0].to_radians();
    let (cx, cy, _, _) = center(ctx, p[1], p[2]);
    let q = p[3] as i32;
    map_xy(s, |x, y| {
        let dx = x as f32 + 0.5 - cx;
        let dy = y as f32 + 0.5 - cy;
        let r = (dx * dx + dy * dy).sqrt();
        let n = ((r * angle) as i32 * q / 2).clamp(1, 64 * q);
        let a0 = dy.atan2(dx);
        let mut acc = [0f32; 4];
        for i in 0..n {
            let t = if n > 1 { i as f32 / (n - 1) as f32 - 0.5 } else { 0.0 };
            let a = a0 + t * angle;
            let qx = cx + r * a.cos();
            let qy = cy + r * a.sin();
            let c = s.sample_bilinear_clamped(qx, qy);
            let al = c[3] as f32 / 255.0;
            for k in 0..3 {
                acc[k] += c[k] as f32 * al;
            }
            acc[3] += c[3] as f32;
        }
        for v in acc.iter_mut() {
            *v /= n as f32;
        }
        unpremul(acc)
    })
}

fn surface_blur(s: &Surface, p: &[f32], _: &Ctx) -> Surface {
    let r = p[0] as i32;
    let thr = p[1] / 100.0 * 255.0;
    let step = (r / 6).max(1);
    map_xy(s, |x, y| {
        let c0 = s.get(x, y);
        let mut acc = [0f32; 4];
        let mut wsum = 0.0;
        let mut dy = -r;
        while dy <= r {
            let mut dx = -r;
            while dx <= r {
                let q = s.get_clamped(x + dx, y + dy);
                let mut diff = 0.0f32;
                for c in 0..3 {
                    diff += (q[c] as f32 - c0[c] as f32).abs();
                }
                let wgt = (1.0 - diff / 3.0 / thr / 2.5).max(0.0);
                if wgt > 0.0 {
                    for c in 0..4 {
                        acc[c] += q[c] as f32 * wgt;
                    }
                    wsum += wgt;
                }
                dx += step;
            }
            dy += step;
        }
        if wsum <= 0.0 {
            return c0;
        }
        [c8(acc[0] / wsum), c8(acc[1] / wsum), c8(acc[2] / wsum), c8(acc[3] / wsum)]
    })
}

fn unfocus(s: &Surface, p: &[f32], _: &Ctx) -> Surface {
    // Disc-shaped blur: approximated by averaging blurred samples on a ring.
    let r = p[0];
    let b = blur(s, r * 0.35);
    let n = 12;
    map_xy(s, |x, y| {
        let mut acc = [0f32; 4];
        for i in 0..n {
            let a = i as f32 * std::f32::consts::TAU / n as f32;
            let q = b.sample_bilinear_clamped(x as f32 + 0.5 + a.cos() * r * 0.7, y as f32 + 0.5 + a.sin() * r * 0.7);
            let al = q[3] as f32 / 255.0;
            for c in 0..3 {
                acc[c] += q[c] as f32 * al;
            }
            acc[3] += q[3] as f32;
        }
        for v in acc.iter_mut() {
            *v /= n as f32;
        }
        unpremul(acc)
    })
}

fn zoom_blur(s: &Surface, p: &[f32], ctx: &Ctx) -> Surface {
    let amt = p[0] / 100.0;
    let (cx, cy, _, _) = center(ctx, p[1], p[2]);
    map_xy(s, |x, y| {
        let dx = x as f32 + 0.5 - cx;
        let dy = y as f32 + 0.5 - cy;
        let len = (dx * dx + dy * dy).sqrt() * amt;
        let n = (len as i32).clamp(1, 64);
        let mut acc = [0f32; 4];
        for i in 0..n {
            let t = 1.0 - amt * i as f32 / n as f32;
            let q = s.sample_bilinear_clamped(cx + dx * t, cy + dy * t);
            let al = q[3] as f32 / 255.0;
            for c in 0..3 {
                acc[c] += q[c] as f32 * al;
            }
            acc[3] += q[3] as f32;
        }
        for v in acc.iter_mut() {
            *v /= n as f32;
        }
        unpremul(acc)
    })
}

// ---------------------------------------------------------------------------------------------
// Distort

fn bulge(s: &Surface, p: &[f32], ctx: &Ctx) -> Surface {
    let amt = p[0] / 100.0;
    let (cx, cy, hw, hh) = center(ctx, p[1], p[2]);
    let r = hw.min(hh);
    map_xy(s, |x, y| {
        let dx = x as f32 + 0.5 - cx;
        let dy = y as f32 + 0.5 - cy;
        let d = (dx * dx + dy * dy).sqrt() / r;
        if d >= 1.0 {
            return s.get(x, y);
        }
        let k = 1.0 - amt * (1.0 - d * d) * 0.75;
        s.sample_bilinear_clamped(cx + dx * k, cy + dy * k)
    })
}

fn value_noise(x: f32, y: f32, seed: u32) -> f32 {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let tx = x - xi as f32;
    let ty = y - yi as f32;
    let sx = tx * tx * (3.0 - 2.0 * tx);
    let sy = ty * ty * (3.0 - 2.0 * ty);
    let a = hash(xi, yi, seed);
    let b = hash(xi + 1, yi, seed);
    let c = hash(xi, yi + 1, seed);
    let d = hash(xi + 1, yi + 1, seed);
    let top = a + (b - a) * sx;
    let bot = c + (d - c) * sx;
    top + (bot - top) * sy
}

fn fbm(x: f32, y: f32, octaves: i32, rough: f32, seed: u32) -> f32 {
    let mut amp = 1.0;
    let mut freq = 1.0;
    let mut sum = 0.0;
    let mut norm = 0.0;
    for o in 0..octaves {
        sum += value_noise(x * freq, y * freq, seed.wrapping_add(o as u32 * 1013)) * amp;
        norm += amp;
        amp *= rough;
        freq *= 2.0;
    }
    sum / norm
}

fn dents(s: &Surface, p: &[f32], ctx: &Ctx) -> Surface {
    let scale = p[0];
    let refr = p[1] / 100.0 * scale * 0.5;
    let rough = (p[2] / 100.0).clamp(0.0, 0.95);
    let seed = p[3] as u32;
    map_xy(s, |x, y| {
        let fx = (x + ctx.ox) as f32 / scale;
        let fy = (y + ctx.oy) as f32 / scale;
        let nx = fbm(fx, fy, 4, rough + 0.3, seed) - 0.5;
        let ny = fbm(fx + 31.7, fy + 17.3, 4, rough + 0.3, seed) - 0.5;
        s.sample_bilinear_clamped(x as f32 + 0.5 + nx * refr * 2.0, y as f32 + 0.5 + ny * refr * 2.0)
    })
}

fn frosted_glass(s: &Surface, p: &[f32], ctx: &Ctx) -> Surface {
    let amt = p[0];
    let seed = p[1] as u32;
    map_xy(s, |x, y| {
        let (gx, gy) = (x + ctx.ox, y + ctx.oy);
        let dx = (hash(gx, gy, seed) - 0.5) * 2.0 * amt;
        let dy = (hash(gy, gx, seed + 7) - 0.5) * 2.0 * amt;
        s.get_clamped(x + dx as i32, y + dy as i32)
    })
}

fn pixelate(s: &Surface, p: &[f32], ctx: &Ctx) -> Surface {
    let n = p[0].max(1.0) as i32;
    // Align the cell grid to the image, not to the (possibly cropped) surface.
    let (sx, sy) = (ctx.ox.rem_euclid(n), ctx.oy.rem_euclid(n));
    let w = s.w as i32 + sx;
    let h = s.h as i32 + sy;
    let cw = (w + n - 1) / n;
    let chh = (h + n - 1) / n;
    let cells: Vec<Px> = (0..cw * chh)
        .into_par_iter()
        .map(|i| {
            let cx = (i % cw) * n - sx;
            let cy = (i / cw) * n - sy;
            let mut acc = [0f32; 4];
            let mut cnt = 0.0;
            for y in cy.max(0)..(cy + n).min(s.h as i32) {
                for x in cx.max(0)..(cx + n).min(s.w as i32) {
                    let q = s.get(x, y);
                    let a = q[3] as f32 / 255.0;
                    for c in 0..3 {
                        acc[c] += q[c] as f32 * a;
                    }
                    acc[3] += q[3] as f32;
                    cnt += 1.0;
                }
            }
            for v in acc.iter_mut() {
                *v /= f32::max(cnt, 1.0);
            }
            unpremul(acc)
        })
        .collect();
    map_xy(s, |x, y| cells[(((y + sy) / n) * cw + (x + sx) / n) as usize])
}

fn polar_inversion(s: &Surface, p: &[f32], ctx: &Ctx) -> Surface {
    let amt = p[0];
    let tile = p[1] > 0.5;
    let (cx, cy, hw, hh) = center(ctx, 0.0, 0.0);
    map_xy(s, |x, y| {
        let dx = (x as f32 + 0.5 - cx) / hw;
        let dy = (y as f32 + 0.5 - cy) / hh;
        let r2 = (dx * dx + dy * dy).max(1e-4);
        let k = amt / r2;
        let mut sx = cx + dx * k * hw;
        let mut sy = cy + dy * k * hh;
        if tile {
            sx = sx.rem_euclid(s.w as f32);
            sy = sy.rem_euclid(s.h as f32);
        } else if sx < 0.0 || sy < 0.0 || sx >= s.w as f32 || sy >= s.h as f32 {
            return [0, 0, 0, 0];
        }
        s.sample_bilinear_clamped(sx, sy)
    })
}

fn tile_reflection(s: &Surface, p: &[f32], ctx: &Ctx) -> Surface {
    let size = p[0];
    let curv = p[1] / 100.0;
    let a = p[2].to_radians();
    let (sn, cs) = a.sin_cos();
    map_xy(s, |x, y| {
        let fx = (x + ctx.ox) as f32 + 0.5;
        let fy = (y + ctx.oy) as f32 + 0.5;
        let u = fx * cs - fy * sn;
        let v = fx * sn + fy * cs;
        let tu = (u / size).rem_euclid(1.0) * 2.0 - 1.0;
        let tv = (v / size).rem_euclid(1.0) * 2.0 - 1.0;
        let du = tu * tu.abs() * curv * size * 0.5;
        let dv = tv * tv.abs() * curv * size * 0.5;
        let ou = u - du;
        let ov = v - dv;
        let sx = ou * cs + ov * sn - ctx.ox as f32;
        let sy = -ou * sn + ov * cs - ctx.oy as f32;
        s.sample_bilinear_clamped(sx, sy)
    })
}

fn twist(s: &Surface, p: &[f32], ctx: &Ctx) -> Surface {
    let amt = p[0] / 100.0 * std::f32::consts::TAU;
    let (cx, cy, hw, hh) = center(ctx, 0.0, 0.0);
    let r = hw.min(hh) * p[1] / 100.0;
    map_xy(s, |x, y| {
        let dx = x as f32 + 0.5 - cx;
        let dy = y as f32 + 0.5 - cy;
        let d = (dx * dx + dy * dy).sqrt();
        if d >= r {
            return s.get(x, y);
        }
        let t = 1.0 - d / r;
        let a = amt * t * t;
        let (sn, cs) = a.sin_cos();
        s.sample_bilinear_clamped(cx + dx * cs - dy * sn, cy + dx * sn + dy * cs)
    })
}

// ---------------------------------------------------------------------------------------------
// Noise

fn add_noise(s: &Surface, p: &[f32], ctx: &Ctx) -> Surface {
    let intensity = p[0] / 100.0 * 128.0;
    let sat = p[1] / 100.0;
    let coverage = p[2] / 100.0;
    let seed = p[3] as u32;
    map_xy(s, |lx, ly| {
        let q = s.get(lx, ly);
        let (x, y) = (lx + ctx.ox, ly + ctx.oy);
        if hash(x, y, seed + 99) > coverage {
            return q;
        }
        let n = (hash(x, y, seed) + hash(x, y, seed + 1) + hash(x, y, seed + 2) - 1.5) * intensity;
        let mut out = [0u8; 4];
        for c in 0..3 {
            let cn = (hash(x, y, seed + 10 + c as u32) - 0.5) * intensity * sat;
            out[c] = c8(q[c] as f32 + n + cn);
        }
        out[3] = q[3];
        out
    })
}

/// Median-like percentile filter using sliding histograms per row.
fn percentile_filter(s: &Surface, r: i32, pct: f32) -> Surface {
    let w = s.w as i32;
    let mut out = Surface::new(s.w, s.h);
    out.data.par_chunks_mut(w as usize).enumerate().for_each(|(y, row)| {
        let y = y as i32;
        let mut hist = vec![[0u32; 256]; 4];
        let mut total = 0u32;
        let add = |hist: &mut Vec<[u32; 256]>, x: i32, sign: i32, total: &mut u32| {
            for dy in -r..=r {
                let q = s.get_clamped(x, y + dy);
                for c in 0..4 {
                    hist[c][q[c] as usize] = (hist[c][q[c] as usize] as i32 + sign) as u32;
                }
                *total = (*total as i32 + sign) as u32;
            }
        };
        for dx in -r..=r {
            add(&mut hist, dx, 1, &mut total);
        }
        for x in 0..w {
            let target = ((total as f32 - 1.0) * pct).round() as u32;
            let mut px = [0u8; 4];
            for c in 0..4 {
                let mut acc = 0;
                for v in 0..256 {
                    acc += hist[c][v];
                    if acc > target {
                        px[c] = v as u8;
                        break;
                    }
                }
            }
            row[x as usize] = px;
            add(&mut hist, x - r, -1, &mut total);
            add(&mut hist, x + r + 1, 1, &mut total);
        }
    });
    out
}

fn median(s: &Surface, p: &[f32], _: &Ctx) -> Surface {
    percentile_filter(s, p[0] as i32, p[1] / 100.0)
}

fn reduce_noise(s: &Surface, p: &[f32], _: &Ctx) -> Surface {
    let m = percentile_filter(s, p[0] as i32, 0.5);
    let k = p[1];
    map_xy(s, |x, y| {
        let a = s.get(x, y);
        let b = m.get(x, y);
        let mut o = [0u8; 4];
        for c in 0..4 {
            o[c] = c8(a[c] as f32 + (b[c] as f32 - a[c] as f32) * k);
        }
        o
    })
}

// ---------------------------------------------------------------------------------------------
// Photo

fn glow(s: &Surface, p: &[f32], _: &Ctx) -> Surface {
    let b = blur(s, p[0]);
    let bc = brightness_contrast(&b, &[p[1], p[2]], &Ctx::plain());
    map_xy(s, |x, y| {
        let a = s.get(x, y);
        let g = bc.get(x, y);
        let mut o = [0u8; 4];
        for c in 0..3 {
            let (av, gv) = (a[c] as f32 / 255.0, g[c] as f32 / 255.0);
            o[c] = c8((av + gv - av * gv) * 255.0);
        }
        o[3] = a[3];
        o
    })
}

fn red_eye(s: &Surface, p: &[f32], _: &Ctx) -> Surface {
    let tol = p[0] / 100.0;
    let sat = p[1] / 100.0;
    map_px(s, |q| {
        let r = q[0] as f32;
        let g = q[1] as f32;
        let b = q[2] as f32;
        let redness = r - (g + b) / 2.0;
        if redness > 60.0 * (1.0 - tol) {
            let nr = (g + b) / 2.0;
            let k = sat;
            [c8(r + (nr - r) * k), q[1], q[2], q[3]]
        } else {
            q
        }
    })
}

fn sharpen(s: &Surface, p: &[f32], _: &Ctx) -> Surface {
    let amt = p[0] / 10.0;
    let b = blur(s, 2.0);
    map_xy(s, |x, y| {
        let a = s.get(x, y);
        let bb = b.get(x, y);
        let mut o = [0u8; 4];
        for c in 0..3 {
            o[c] = c8(a[c] as f32 + (a[c] as f32 - bb[c] as f32) * amt * 2.0);
        }
        o[3] = a[3];
        o
    })
}

fn soft_portrait(s: &Surface, p: &[f32], _: &Ctx) -> Surface {
    let b = blur(s, p[0] * 1.5);
    let light = p[1] * 2.5;
    let warm = p[2] / 20.0;
    map_xy(s, |x, y| {
        let a = s.get(x, y);
        let bb = b.get(x, y);
        let mut o = [0u8; 4];
        for c in 0..3 {
            let v = (a[c] as f32 + bb[c] as f32) / 2.0 + light;
            o[c] = c8(v);
        }
        o[0] = c8(o[0] as f32 + warm * 12.0);
        o[2] = c8(o[2] as f32 - warm * 12.0);
        o[3] = a[3];
        o
    })
}

fn vignette(s: &Surface, p: &[f32], ctx: &Ctx) -> Surface {
    let (cx, cy, hw, hh) = center(ctx, p[0], p[1]);
    let rad = p[2];
    let strength = p[3] / 100.0;
    let diag = (hw * hw + hh * hh).sqrt();
    map_xy(s, |x, y| {
        let q = s.get(x, y);
        let dx = x as f32 + 0.5 - cx;
        let dy = y as f32 + 0.5 - cy;
        let d = (dx * dx + dy * dy).sqrt() / (diag * rad);
        let t = (d.clamp(0.0, 1.0)).powf(2.0);
        let k = 1.0 - strength * t * (3.0 - 2.0 * t.min(1.0));
        let k = k.clamp(0.0, 1.0);
        [c8(q[0] as f32 * k), c8(q[1] as f32 * k), c8(q[2] as f32 * k), q[3]]
    })
}

// ---------------------------------------------------------------------------------------------
// Render

fn clouds(s: &Surface, p: &[f32], ctx: &Ctx) -> Surface {
    let scale = p[0];
    let rough = p[1];
    let seed = p[2] as u32;
    let mode = p[3] as i32;
    let (c0, c1) = (ctx.primary, ctx.secondary);
    map_xy(s, |x, y| {
        let n = fbm((x + ctx.ox) as f32 / scale, (y + ctx.oy) as f32 / scale, 8, rough, seed).clamp(0.0, 1.0);
        let n = ((n - 0.5) * 1.6 + 0.5).clamp(0.0, 1.0);
        let mut cl = [0u8; 4];
        for c in 0..4 {
            cl[c] = c8(c0[c] as f32 * (1.0 - n) + c1[c] as f32 * n);
        }
        let q = s.get(x, y);
        match mode {
            1 => [c8(q[0] as f32 * cl[0] as f32 / 255.0), c8(q[1] as f32 * cl[1] as f32 / 255.0), c8(q[2] as f32 * cl[2] as f32 / 255.0), q[3]],
            2 => {
                let mut o = [0u8; 4];
                for c in 0..3 {
                    let (b, t) = (q[c] as f32 / 255.0, cl[c] as f32 / 255.0);
                    let v = if b < 0.5 { 2.0 * b * t } else { 1.0 - 2.0 * (1.0 - b) * (1.0 - t) };
                    o[c] = c8(v * 255.0);
                }
                o[3] = q[3];
                o
            }
            3 => {
                let mut o = [0u8; 4];
                for c in 0..3 {
                    let (b, t) = (q[c] as f32 / 255.0, cl[c] as f32 / 255.0);
                    o[c] = c8((b + t - b * t) * 255.0);
                }
                o[3] = q[3];
                o
            }
            _ => cl,
        }
    })
}

fn fractal(s: &Surface, ctx: &Ctx, factor: f32, quality: i32, zoom: f32, angle: f32, julia: bool, inv: bool) -> Surface {
    let (cx, cy, hw, hh) = center(ctx, 0.0, 0.0);
    let scale = 2.0 / (hw.min(hh) * zoom);
    let (sn, cs) = angle.to_radians().sin_cos();
    let q = quality.max(1);
    map_xy(s, |x, y| {
        let mut acc = [0f32; 4];
        for sy in 0..q {
            for sx in 0..q {
                let px = (x as f32 + (sx as f32 + 0.5) / q as f32 - cx) * scale;
                let py = (y as f32 + (sy as f32 + 0.5) / q as f32 - cy) * scale;
                let rx = px * cs - py * sn;
                let ry = px * sn + py * cs;
                let (mut zx, mut zy, cr, ci) = if julia { (rx, ry, -0.8, 0.156) } else { (0.0, 0.0, rx - 0.5, ry) };
                let max = 256;
                let mut i = 0;
                while i < max && zx * zx + zy * zy < 16.0 {
                    let t = zx * zx - zy * zy + cr;
                    zy = 2.0 * zx * zy + ci;
                    zx = t;
                    i += 1;
                }
                if i < max {
                    let smooth = i as f32 + 1.0 - ((zx * zx + zy * zy).sqrt().ln().ln() / std::f32::consts::LN_2);
                    let t = (smooth * factor * 0.05).max(0.0);
                    let r = (0.5 + 0.5 * (t * 2.0).sin()) * 255.0;
                    let g = (0.5 + 0.5 * (t * 2.0 + 2.1).sin()) * 255.0;
                    let b = (0.5 + 0.5 * (t * 2.0 + 4.2).sin()) * 255.0;
                    acc[0] += r;
                    acc[1] += g;
                    acc[2] += b;
                    acc[3] += 255.0;
                } else if julia {
                    // inside the set: transparent for julia
                } else {
                    acc[3] += 255.0;
                }
            }
        }
        let n = (q * q) as f32;
        let mut o = [c8(acc[0] / n), c8(acc[1] / n), c8(acc[2] / n), c8(acc[3] / n)];
        if inv {
            o = [255 - o[0], 255 - o[1], 255 - o[2], o[3]];
        }
        o
    })
}

fn julia(s: &Surface, p: &[f32], ctx: &Ctx) -> Surface {
    fractal(s, ctx, p[0], p[1] as i32, p[2], p[3], true, false)
}

fn mandelbrot(s: &Surface, p: &[f32], ctx: &Ctx) -> Surface {
    fractal(s, ctx, p[0], p[1] as i32, p[2], p[3], false, p[4] > 0.5)
}

// ---------------------------------------------------------------------------------------------
// Stylize

fn directional_kernel(angle: f32) -> [f32; 9] {
    let a = angle.to_radians();
    let (dx, dy) = (a.cos(), -a.sin());
    let mut k = [0f32; 9];
    for j in 0..3 {
        for i in 0..3 {
            let (x, y) = (i as f32 - 1.0, j as f32 - 1.0);
            k[j * 3 + i] = -(x * dx + y * dy);
        }
    }
    k
}

fn edge_detect(s: &Surface, p: &[f32], _: &Ctx) -> Surface {
    convolve3(s, directional_kernel(p[0]), 128.0, true)
}

fn emboss(s: &Surface, p: &[f32], _: &Ctx) -> Surface {
    let g = black_white(s, &[], &Ctx::plain());
    convolve3(&g, directional_kernel(p[0]), 128.0, true)
}

fn relief(s: &Surface, p: &[f32], _: &Ctx) -> Surface {
    let mut k = directional_kernel(p[0]);
    k[4] += 1.0;
    convolve3(s, k, 0.0, true)
}

fn outline_fx(s: &Surface, p: &[f32], _: &Ctx) -> Surface {
    let r = p[0] as i32;
    let intensity = p[1] / 100.0;
    let lo = percentile_filter(s, r.min(50), 0.0);
    map_xy(s, |x, y| {
        let a = s.get(x, y);
        let m = lo.get(x, y);
        let mut o = [0u8; 4];
        for c in 0..3 {
            let v = a[c] as f32 + (m[c] as f32 - a[c] as f32) * intensity;
            o[c] = c8(v);
        }
        o[3] = a[3];
        o
    })
}

// ---------------------------------------------------------------------------------------------
// Object / Layer

fn drop_shadow(s: &Surface, p: &[f32], ctx: &Ctx) -> Surface {
    let (ox, oy) = (p[0] as i32, p[1] as i32);
    let opacity = p[3] / 100.0;
    let shadow_color = ctx.secondary;
    let mut sh = Surface::new(s.w, s.h);
    for y in 0..s.h as i32 {
        for x in 0..s.w as i32 {
            let a = s.get_or(x - ox, y - oy, [0; 4])[3];
            sh.set(x, y, [shadow_color[0], shadow_color[1], shadow_color[2], (a as f32 * opacity) as u8]);
        }
    }
    let sh = blur(&sh, p[2]);
    map_xy(s, |x, y| super::surface::over(sh.get(x, y), s.get(x, y), 1.0))
}

fn rotate_zoom_fx(s: &Surface, p: &[f32], _: &Ctx) -> Surface {
    super::transform::rotate_zoom(s, p[0], p[1], p[2], p[3], p[4] > 0.5)
}

/// Builds a LUT from curve control points (x,y in 0..255) using monotone cubic interpolation.
pub fn curve_lut(points: &[(f32, f32)]) -> [u8; 256] {
    let mut pts: Vec<(f32, f32)> = points.to_vec();
    pts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let mut lut = [0u8; 256];
    if pts.is_empty() {
        for (i, v) in lut.iter_mut().enumerate() {
            *v = i as u8;
        }
        return lut;
    }
    if pts.len() == 1 {
        return [c8(pts[0].1); 256];
    }
    let n = pts.len();
    let mut d = vec![0f32; n - 1];
    for i in 0..n - 1 {
        let dx = (pts[i + 1].0 - pts[i].0).max(1e-3);
        d[i] = (pts[i + 1].1 - pts[i].1) / dx;
    }
    let mut m = vec![0f32; n];
    m[0] = d[0];
    m[n - 1] = d[n - 2];
    for i in 1..n - 1 {
        m[i] = if d[i - 1] * d[i] <= 0.0 { 0.0 } else { (d[i - 1] + d[i]) / 2.0 };
    }
    for (i, v) in lut.iter_mut().enumerate() {
        let x = i as f32;
        if x <= pts[0].0 {
            *v = c8(pts[0].1);
            continue;
        }
        if x >= pts[n - 1].0 {
            *v = c8(pts[n - 1].1);
            continue;
        }
        let k = pts.windows(2).position(|w| x >= w[0].0 && x <= w[1].0).unwrap_or(0);
        let (x0, y0) = pts[k];
        let (x1, y1) = pts[k + 1];
        let h = (x1 - x0).max(1e-3);
        let t = (x - x0) / h;
        let h00 = 2.0 * t * t * t - 3.0 * t * t + 1.0;
        let h10 = t * t * t - 2.0 * t * t + t;
        let h01 = -2.0 * t * t * t + 3.0 * t * t;
        let h11 = t * t * t - t * t;
        *v = c8(h00 * y0 + h10 * h * m[k] + h01 * y1 + h11 * h * m[k + 1]);
    }
    lut
}
