//! Rough performance measurements on a large image. Run with:
//! `cargo test --release --lib perf -- --ignored --nocapture`

use super::render::{self, Overlay, RenderParams};
use crate::core::document::{Document, Layer};
use crate::core::filters;
use crate::core::geom::{Pt, Rect};
use crate::core::paint::{self, Cov, PaintOp};
use crate::core::selection::{Mask, SelectionMode};
use std::time::Instant;

fn t<R>(name: &str, n: u32, mut f: impl FnMut() -> R) -> R {
    let start = Instant::now();
    let mut r = None;
    for _ in 0..n {
        r = Some(f());
    }
    let ms = start.elapsed().as_secs_f64() * 1000.0 / n as f64;
    println!("{name:<40} {ms:>9.2} ms");
    r.unwrap()
}

#[test]
#[ignore]
fn perf_8k() {
    let (w, h) = (7680u32, 4320u32);
    let mut doc = t("new 8K document (composite)", 1, || Document::blank("p".into(), w, h, [255, 255, 255, 255]));
    // Add some content so the layer isn't uniform.
    let mut l2 = Layer::new("L2", w, h);
    l2.px.map_rect(Rect::new(0, 0, w as i32, h as i32), |x, y, _| [(x % 256) as u8, (y % 256) as u8, 128, 255]);
    doc.state.layers.push(l2);
    doc.state.active = 1;
    doc.invalidate_all();
    doc.commit("layer", "plus");
    t("full recomposite (2 layers)", 3, || {
        doc.invalidate_all();
        doc.update_composite()
    });
    let rp = RenderParams { dark: true, grid: false, phase: 0, rulers: None, hover: None, scale: 1.0 };
    let ov = Overlay::default();
    doc.view.zoom = 0.2;
    doc.view.ox = 30.0;
    doc.view.oy = 30.0;
    t("render 1600x1000 viewport @ 20%", 10, || render::render(&doc, 1600, 1000, &rp, &ov));
    doc.view.zoom = 1.0;
    t("render 1600x1000 viewport @ 100%", 10, || render::render(&doc, 1600, 1000, &rp, &ov));
    doc.view.zoom = 0.2;
    // What the app does per frame now: copy the cached base and draw overlays on it.
    let key = render::BaseKey::new(&doc, 1600, 1000, true, false);
    let base = render::render(&doc, 1600, 1000, &rp, &ov);
    t("frame from cache (copy + overlays)", 20, || {
        let mut f = base.clone();
        render::draw_overlays(&doc, f.make_mut_slice(), &key, &rp, &ov);
        f
    });
    let mut cached = base.clone();
    t("partial re-render (brush-size dirty rect)", 20, || {
        let region = key.doc_to_screen(Rect::new(3000, 2000, 3080, 2080));
        render::render_base(&doc, cached.make_mut_slice(), &key, region);
    });

    // Brush stroke of 100 segments.
    let base = doc.state.layer().px.clone();
    t("brush stroke, 100 segments (w=40)", 1, || {
        let mut cov = Cov::new(doc.state.rect());
        let op = PaintOp::Color { c: [255, 0, 0, 255], overwrite: false };
        let sel = doc.state.selection.clone();
        let mut last = Pt::new(100.0, 100.0);
        for i in 0..100 {
            let p = Pt::new(100.0 + i as f32 * 30.0, 100.0 + (i as f32 * 0.3).sin() * 300.0 + 1000.0);
            let r = paint::stroke_segment(&mut cov, last, p, 40.0, 0.8, true);
            paint::apply_ops(&mut doc.state.layers[1].px, &base, r, &sel, &[(&cov, &op)]);
            doc.invalidate(r);
            doc.update_composite();
            last = p;
        }
    });
    t("rect select drag, 20 updates", 1, || {
        let base = doc.state.selection.clone();
        for i in 0..20 {
            let m = Mask::rect(w, h, Rect::new(100, 100, 1000 + i * 100, 800 + i * 50));
            doc.state.selection = base.apply(m, SelectionMode::Replace);
        }
    });
    t("ellipse select, 1 update", 3, || Mask::ellipse(w, h, 100.0, 100.0, 3000.0, 2000.0));
    doc.commit("stroke", "brush");
    t("undo + recomposite", 3, || {
        doc.undo();
        doc.update_composite();
        doc.redo();
        doc.update_composite();
    });
    t("flood mask (layer, contiguous)", 1, || {
        let px = &doc.state.layer().px;
        paint::flood_mask(w, h, |x, y| px.get(x, y), 10, 10, 0.1, true)
    });
    let src = doc.state.layer().px.to_surface();
    let ctx = filters::Ctx { primary: [0; 4], secondary: [255; 4], bounds: doc.state.rect(), curves: None, ox: 0, oy: 0 };
    let blur = filters::find("gaussian-blur").unwrap();
    t("gaussian blur r=8 (full 8K)", 1, || (blur.run)(&src, &[8.0], &ctx));
    t("layer to_surface", 3, || doc.state.layer().px.to_surface());
    t("history commit", 10, || doc.commit("x", "brush"));
}

#[test]
#[ignore]
fn perf_ellipse_parts() {
    let (w, h) = (7680u32, 4320u32);
    let r = Rect::new(100, 100, 3000, 2000);
    t("alloc+zero 5.5MB", 5, || vec![0u8; (r.width() * r.height()) as usize]);
    t("ellipse total", 5, || Mask::ellipse(w, h, 100.0, 100.0, 3000.0, 2000.0));
    let data = vec![200u8; (r.width() * r.height()) as usize];
    t("from_region (bounds+outline) on solid data", 5, || Mask::from_region(w, h, r, data.clone()));
}
