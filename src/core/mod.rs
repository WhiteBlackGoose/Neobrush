pub mod blend;
pub mod document;
pub mod filters;
pub mod geom;
pub mod io;
pub mod paint;
pub mod selection;
pub mod surface;
pub mod text;
pub mod tiled;
pub mod transform;

#[cfg(test)]
mod tests {
    use super::document::{DocState, Document, Layer};
    use super::filters::{self, Ctx};
    use super::geom::{Pt, Rect};
    use super::paint::{self, Cov, PaintOp, Shape, SHAPES};
    use super::selection::{Mask, Selection, SelectionMode};
    use super::surface::Surface;
    use super::transform;

    fn sample(w: u32, h: u32) -> Surface {
        let mut s = Surface::new(w, h);
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                s.set(x, y, [(x * 7) as u8, (y * 5) as u8, ((x + y) * 3) as u8, if (x / 8 + y / 8) % 3 == 0 { 128 } else { 255 }]);
            }
        }
        s
    }

    #[test]
    fn all_filters_run() {
        let s = sample(67, 41);
        let ctx = Ctx { primary: [0, 0, 0, 255], secondary: [255, 255, 255, 255], bounds: Rect::new(5, 5, 60, 30), curves: Some([[0; 256]; 3]) };
        for f in filters::all() {
            let params: Vec<f32> = f.params.iter().map(|p| p.default).collect();
            let out = (f.run)(&s, &params, &ctx);
            assert_eq!((out.w, out.h), (s.w, s.h), "{}", f.id);
            // Extremes
            let maxp: Vec<f32> = f
                .params
                .iter()
                .map(|p| match &p.kind {
                    filters::ParamKind::Slider { max, .. } => *max,
                    filters::ParamKind::Check => 1.0,
                    filters::ParamKind::Choice(c) => (c.len() - 1) as f32,
                })
                .collect();
            let _ = (f.run)(&s, &maxp, &ctx);
            let minp: Vec<f32> = f
                .params
                .iter()
                .map(|p| match &p.kind {
                    filters::ParamKind::Slider { min, .. } => *min,
                    _ => 0.0,
                })
                .collect();
            let _ = (f.run)(&s, &minp, &ctx);
        }
    }

    #[test]
    fn ora_roundtrip() {
        let mut st = DocState { w: 50, h: 30, layers: vec![Layer::from_surface("A & <b>", &sample(50, 30)), Layer::new("Top", 50, 30)], active: 1, selection: Selection::none() };
        st.layers[1].opacity = 0.5;
        st.layers[1].blend = super::blend::BlendMode::Multiply;
        st.layers[1].visible = false;
        let dir = std::env::temp_dir().join(format!("nb-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("t.ora");
        super::io::save(&p, &st, 90).unwrap();
        let back = super::io::load(&p).unwrap();
        assert_eq!(back.layers.len(), 2);
        assert_eq!(back.layers[0].name, "A & <b>");
        assert_eq!(back.active, 1);
        assert!(!back.layers[1].visible);
        assert_eq!(back.layers[1].blend, super::blend::BlendMode::Multiply);
        assert!((back.layers[1].opacity - 0.5).abs() < 0.01);
        assert_eq!(back.layers[0].px.get(10, 10), st.layers[0].px.get(10, 10));
        for ext in ["png", "jpg", "bmp", "webp", "tiff", "tga", "gif", "qoi", "ico"] {
            let p = dir.join(format!("t.{ext}"));
            super::io::save(&p, &st, 90).unwrap();
            let back = super::io::load(&p).unwrap();
            assert_eq!((back.w, back.h), (50, 30), "{ext}");
        }
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn painting_and_history() {
        let mut d = Document::blank("t".into(), 100, 80, [255, 255, 255, 255]);
        let base = d.state.layer().px.clone();
        let mut cov = Cov::new(d.state.rect());
        let r = paint::stroke_segment(&mut cov, Pt::new(10.0, 10.0), Pt::new(90.0, 70.0), 9.0, 0.8, true);
        let op = PaintOp::Color { c: [255, 0, 0, 255], overwrite: false };
        let sel = d.state.selection.clone();
        paint::apply_ops(&mut d.state.layer_mut().px, &base, r, &sel, &[(&cov, &op)]);
        assert_eq!(d.state.layer().px.get(50, 40)[0], 255);
        assert!(d.state.layer().px.get(50, 40)[1] < 50);
        d.commit("Paint", "brush");
        d.undo();
        assert_eq!(d.state.layer().px.get(50, 40), [255, 255, 255, 255]);
        d.redo();
        assert!(d.state.layer().px.get(50, 40)[1] < 50);
        // Shapes
        for k in SHAPES {
            let s = Shape { kind: k, a: Pt::new(10.0, 10.0), b: Pt::new(60.0, 50.0), radius: 8.0 };
            let f = s.sdf();
            assert!(f(Pt::new(35.0, 32.0)) < 0.0 || k == super::paint::ShapeKind::RightTriangle, "{:?}", k);
            assert!(f(Pt::new(95.0, 75.0)) > 0.0, "{:?}", k);
        }
    }

    #[test]
    fn selection_ops() {
        let a = Mask::rect(40, 40, Rect::new(0, 0, 20, 20));
        let b = Mask::rect(40, 40, Rect::new(10, 10, 30, 30));
        let s = Selection::from_mask(a);
        let u = s.apply(b.clone(), SelectionMode::Union);
        assert_eq!(u.mask.as_ref().unwrap().bounds, Rect::new(0, 0, 30, 30));
        let i = s.apply(b.clone(), SelectionMode::Intersect);
        assert_eq!(i.mask.as_ref().unwrap().bounds, Rect::new(10, 10, 20, 20));
        let e = Mask::ellipse(40, 40, 5.0, 5.0, 35.0, 25.0);
        assert!(e.get(20, 15) == 255 && e.get(1, 1) == 0);
        let poly = Mask::polygon(40, 40, &[Pt::new(2.0, 2.0), Pt::new(30.0, 4.0), Pt::new(15.0, 35.0)]);
        assert!(poly.get(15, 10) > 200);
    }

    #[test]
    fn transforms() {
        let mut st = DocState { w: 30, h: 20, layers: vec![Layer::from_surface("a", &sample(30, 20))], active: 0, selection: Selection::none() };
        let p = st.layers[0].px.get(0, 0);
        transform::rotate_image(&mut st, 1);
        assert_eq!((st.w, st.h), (20, 30));
        assert_eq!(st.layers[0].px.get(19, 0), p);
        transform::resize_image(&mut st, 7, 9, transform::Resample::Supersample);
        assert_eq!((st.w, st.h), (7, 9));
        transform::canvas_size(&mut st, 20, 20, 4, None);
        assert_eq!((st.w, st.h), (20, 20));
        st.selection = Selection::from_mask(Mask::ellipse(20, 20, 2.0, 2.0, 18.0, 12.0));
        transform::crop_to_selection(&mut st);
        assert_eq!((st.w, st.h), (16, 10));
    }
}
