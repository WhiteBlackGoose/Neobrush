//! Menu actions, dialogs, file handling and effects.

use super::editor::{px_to_color, with_editor, Editor};
use super::session::Session;
use crate::core::blend::BlendMode;
use crate::core::document::{DocState, Document, Layer};
use crate::core::filters::{self, Ctx, ParamKind};
use crate::core::geom::Rect;
use crate::core::io;
use crate::core::selection::{Mask, Selection};
use crate::core::surface::{lerp_px, Surface};
use crate::core::tiled::Tiled;
use crate::core::transform;
use crate::{App, CurvePoint, ParamItem, Tool};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct FxSession {
    pub id: String,
    pub name: String,
    pub params: Vec<filters::ParamDef>,
    pub values: Vec<f32>,
    pub base: Tiled,
    pub base_surface: Arc<Surface>,
    pub layer: usize,
    pub gen: u64,
    pub applied_gen: u64,
    pub running: bool,
    pub curves: Option<[Vec<(f32, f32)>; 4]>,
    pub curve_drag: Option<usize>,
}

pub struct LayerPropsSession {
    pub layer: usize,
    pub orig: (String, bool, f32, BlendMode),
}

#[derive(Clone, Copy, PartialEq)]
pub enum AfterSave {
    CloseDoc(u64),
    ContinueExit,
}

fn default_curves() -> [Vec<(f32, f32)>; 4] {
    std::array::from_fn(|_| vec![(0.0, 0.0), (255.0, 255.0)])
}

fn curves_lut(c: &[Vec<(f32, f32)>; 4]) -> [[u8; 256]; 3] {
    let all = filters::curve_lut(&c[0]);
    let mut out = [[0u8; 256]; 3];
    for ch in 0..3 {
        let l = filters::curve_lut(&c[ch + 1]);
        for i in 0..256 {
            out[ch][i] = l[all[i] as usize];
        }
    }
    out
}

impl Editor {
    pub fn action(&mut self, id: &str) {
        // Most actions end an active editing session first.
        let keeps_session = id.starts_with("view.theme") || matches!(id, "view.zoom-in" | "view.zoom-out" | "view.fit" | "view.actual" | "view.refresh" | "edit.undo" | "color.swap" | "color.reset")
            || id.starts_with("color.hex:");
        if !keeps_session {
            self.finish_session(true);
            if !matches!(id, "edit.erase" | "edit.fill") {
                self.session = Session::None;
            }
        }
        let has_doc = self.doc().is_some();
        match id {
            "file.new" => self.open_new_dialog(),
            "file.open" => self.open_dialog(false),
            s if s.starts_with("file.recent:") => {
                let p = PathBuf::from(s.trim_start_matches("file.recent:"));
                if let Some(i) = self.docs.iter().position(|d| d.path.as_ref().map(|dp| std::fs::canonicalize(dp).ok() == std::fs::canonicalize(&p).ok()).unwrap_or(false)) {
                    self.switch_doc(i);
                } else {
                    self.open_path(&p);
                }
            }
            "file.clear-recent" => {
                self.recent_files.clear();
                self.sync_recent_files();
            }
            "file.save" if has_doc => self.save(false),
            "file.save-as" if has_doc => self.save(true),
            "file.close" if has_doc => self.request_close(self.cur),
            "file.exit" => {
                self.exiting = true;
                self.continue_exit();
            }
            "edit.undo" if has_doc => {
                if self.session.is_editing() {
                    self.finish_session(false);
                } else {
                    self.session = Session::None;
                    self.doc_mut().unwrap().undo();
                }
                self.panels();
            }
            "edit.redo" if has_doc => {
                self.doc_mut().unwrap().redo();
                self.panels();
            }
            "edit.cut" if has_doc => {
                self.copy(false);
                self.erase_selection("Cut");
            }
            "edit.copy" if has_doc => self.copy(false),
            "edit.copy-merged" if has_doc => self.copy(true),
            "edit.paste" => self.paste(0),
            "edit.paste-new-layer" => self.paste(1),
            "edit.paste-new-image" => self.paste(2),
            "edit.select-all" if has_doc => {
                let d = self.doc_mut().unwrap();
                d.state.selection = Selection::from_mask(Mask::rect(d.state.w, d.state.h, d.state.rect()));
                d.commit("Select All", "select");
                self.panels();
            }
            "edit.deselect" if has_doc => {
                let d = self.doc_mut().unwrap();
                if d.state.selection.is_active() {
                    d.state.selection = Selection::none();
                    d.commit("Deselect", "x");
                }
                self.panels();
            }
            "edit.invert-selection" if has_doc => {
                let d = self.doc_mut().unwrap();
                let m = match &d.state.selection.mask {
                    Some(m) => m.invert(),
                    None => Mask::empty(d.state.w, d.state.h).invert(),
                };
                d.state.selection = Selection::from_mask(m);
                d.commit("Invert Selection", "select");
                self.panels();
            }
            "edit.erase" if has_doc => self.erase_selection("Erase Selection"),
            "edit.fill" if has_doc => self.fill_selection(),
            "view.zoom-in" if has_doc => self.zoom_step(1, None),
            "view.zoom-out" if has_doc => self.zoom_step(-1, None),
            "view.fit" if has_doc => self.fit_doc(self.cur, false),
            "view.actual" if has_doc => self.set_zoom_centered(1.0),
            "view.refresh" => self.redraw(),
            "view.theme-system" => self.set_theme(0),
            "view.theme-light" => self.set_theme(1),
            "view.theme-dark" => self.set_theme(2),
            "color.swap" => {
                let ui = self.ui();
                let g = ui.global::<App>();
                let (p, s) = (g.get_primary(), g.get_secondary());
                g.set_primary(s);
                g.set_secondary(p);
                self.options_changed();
            }
            "color.reset" => {
                let ui = self.ui();
                let g = ui.global::<App>();
                g.set_primary(slint::Color::from_rgb_u8(0, 0, 0));
                g.set_secondary(slint::Color::from_rgb_u8(255, 255, 255));
                self.options_changed();
            }
            s if s.starts_with("color.hex:") => {
                let hex = s.trim_start_matches("color.hex:").trim().trim_start_matches('#');
                let parse = |h: &str| u8::from_str_radix(h, 16).ok();
                let c = match hex.len() {
                    3 => {
                        let v: Vec<u8> = hex.chars().filter_map(|c| parse(&format!("{c}{c}"))).collect();
                        if v.len() == 3 { Some([v[0], v[1], v[2], 255]) } else { None }
                    }
                    6 | 8 => {
                        let v: Vec<u8> = (0..hex.len() / 2).filter_map(|i| parse(&hex[i * 2..i * 2 + 2])).collect();
                        if v.len() == hex.len() / 2 { Some([v[0], v[1], v[2], *v.get(3).unwrap_or(&255)]) } else { None }
                    }
                    _ => None,
                };
                if let Some(c) = c {
                    let ui = self.ui();
                    let g = ui.global::<App>();
                    let alpha = if hex.len() == 8 { c[3] } else { if g.get_edit_secondary() { g.get_secondary().alpha() } else { g.get_primary().alpha() } };
                    let col = px_to_color([c[0], c[1], c[2], alpha]);
                    if g.get_edit_secondary() {
                        g.set_secondary(col);
                    } else {
                        g.set_primary(col);
                    }
                    self.options_changed();
                }
            }
            "image.crop" if has_doc => {
                let d = self.doc_mut().unwrap();
                if d.state.selection.is_active() {
                    transform::crop_to_selection(&mut d.state);
                    self.structural("Crop to Selection", "crop");
                }
            }
            "image.autocrop" if has_doc => {
                let d = self.doc_mut().unwrap();
                let r = transform::content_bounds(&d.state);
                if !r.is_empty() && r != d.state.rect() {
                    transform::crop_rect(&mut d.state, r);
                    self.structural("Auto Crop", "crop");
                }
            }
            "image.resize" if has_doc => self.ui().global::<App>().set_dialog("resize".into()),
            "image.canvas" if has_doc => self.ui().global::<App>().set_dialog("canvas".into()),
            "image.flip-h" if has_doc => {
                transform::flip_image(&mut self.doc_mut().unwrap().state, true);
                self.structural("Flip Horizontal", "flip-h");
            }
            "image.flip-v" if has_doc => {
                transform::flip_image(&mut self.doc_mut().unwrap().state, false);
                self.structural("Flip Vertical", "flip-v");
            }
            "image.rot-cw" if has_doc => {
                transform::rotate_image(&mut self.doc_mut().unwrap().state, 1);
                self.structural("Rotate 90° Clockwise", "rotate-cw");
            }
            "image.rot-ccw" if has_doc => {
                transform::rotate_image(&mut self.doc_mut().unwrap().state, 3);
                self.structural("Rotate 90° Counter-clockwise", "rotate-ccw");
            }
            "image.rot-180" if has_doc => {
                transform::rotate_image(&mut self.doc_mut().unwrap().state, 2);
                self.structural("Rotate 180°", "rotate-cw");
            }
            "image.flatten" if has_doc => {
                let d = self.doc_mut().unwrap();
                if d.state.layers.len() > 1 {
                    let flat = d.state.flatten();
                    d.state.layers = vec![Layer::from_surface("Background", &flat)];
                    d.state.active = 0;
                    self.structural("Flatten", "layers-2");
                }
            }
            "layer.add" if has_doc => {
                let d = self.doc_mut().unwrap();
                let n = d.state.layers.len() + 1;
                let l = Layer::new(format!("Layer {n}"), d.state.w, d.state.h);
                let at = d.state.active + 1;
                d.state.layers.insert(at, l);
                d.state.active = at;
                self.structural("Add New Layer", "plus");
            }
            "layer.delete" if has_doc => {
                let d = self.doc_mut().unwrap();
                if d.state.layers.len() > 1 {
                    d.state.layers.remove(d.state.active);
                    d.state.active = d.state.active.saturating_sub(1).min(d.state.layers.len() - 1);
                    self.structural("Delete Layer", "trash");
                }
            }
            "layer.duplicate" if has_doc => {
                let d = self.doc_mut().unwrap();
                let mut l = d.state.layer().clone();
                l.name = format!("{} copy", l.name);
                let at = d.state.active + 1;
                d.state.layers.insert(at, l);
                d.state.active = at;
                self.structural("Duplicate Layer", "copy-plus");
            }
            "layer.merge-down" if has_doc => {
                let d = self.doc_mut().unwrap();
                let a = d.state.active;
                if a > 0 {
                    let top = d.state.layers[a].clone();
                    let bottom = d.state.layers[a - 1].clone();
                    let tmp = DocState {
                        w: d.state.w,
                        h: d.state.h,
                        layers: vec![Layer { visible: true, opacity: 1.0, blend: BlendMode::Normal, ..bottom.clone() }, top],
                        active: 0,
                        selection: Selection::none(),
                    };
                    let merged = tmp.flatten();
                    d.state.layers[a - 1].px = Tiled::from_surface(&merged);
                    d.state.layers.remove(a);
                    d.state.active = a - 1;
                    self.structural("Merge Layer Down", "merge");
                }
            }
            "layer.import" if has_doc => self.open_dialog(true),
            "layer.flip-h" | "layer.flip-v" if has_doc => {
                let h = id == "layer.flip-h";
                let d = self.doc_mut().unwrap();
                let s = d.state.layer().px.to_surface();
                d.state.layer_mut().px = Tiled::from_surface(&transform::flip_surface(&s, h));
                self.structural(if h { "Flip Layer Horizontal" } else { "Flip Layer Vertical" }, if h { "flip-h" } else { "flip-v" });
            }
            "layer.up" if has_doc => {
                let d = self.doc_mut().unwrap();
                let a = d.state.active;
                if a + 1 < d.state.layers.len() {
                    d.state.layers.swap(a, a + 1);
                    d.state.active = a + 1;
                    self.structural("Move Layer Up", "chevron-up");
                }
            }
            "layer.down" if has_doc => {
                let d = self.doc_mut().unwrap();
                let a = d.state.active;
                if a > 0 {
                    d.state.layers.swap(a, a - 1);
                    d.state.active = a - 1;
                    self.structural("Move Layer Down", "chevron-down");
                }
            }
            "layer.properties" if has_doc => self.open_layer_props(),
            "fx.repeat" if has_doc => {
                if let Some((id, values)) = self.last_fx.clone() {
                    self.run_filter_now(&id, &values);
                }
            }
            s if s.starts_with("fx:") && has_doc => {
                let fid = s.trim_start_matches("fx:").to_string();
                self.open_fx(&fid);
            }
            _ => {}
        }
        self.refocus();
    }

    /// Records a structural change (size/layers) and refreshes everything.
    pub fn structural(&mut self, name: &str, icon: &'static str) {
        let d = self.doc_mut().unwrap();
        d.invalidate_all();
        d.commit(name, icon);
        self.clamp_view();
        self.panels();
    }

    // ------------------------------------------------------------------------------------------
    // Documents

    pub fn add_doc(&mut self, doc: Document) {
        self.finish_session(true);
        self.session = Session::None;
        self.docs.push(doc);
        self.cur = self.docs.len() - 1;
        let i = self.cur;
        self.fit_doc(i, true);
        self.sync_all();
    }

    pub fn switch_doc(&mut self, i: usize) {
        if i < self.docs.len() && i != self.cur {
            self.finish_session(true);
            self.session = Session::None;
            self.drag = super::session::Drag::None;
            self.cur = i;
            self.hover = None;
            self.panels();
        }
    }

    pub fn open_new_dialog(&mut self) {
        let ui = self.ui();
        let g = ui.global::<App>();
        let (cw, ch) = match arboard::Clipboard::new().and_then(|mut c| c.get_image()) {
            Ok(img) => (img.width as i32, img.height as i32),
            Err(_) => (0, 0),
        };
        g.set_clip_w(cw);
        g.set_clip_h(ch);
        if let Some(d) = self.doc() {
            g.set_image_w(d.state.w as i32);
            g.set_image_h(d.state.h as i32);
        } else {
            g.set_image_w(1920);
            g.set_image_h(1080);
        }
        g.set_dialog("new".into());
    }

    fn next_untitled(&self) -> String {
        let mut n = 1;
        loop {
            let t = format!("Untitled {n}");
            if !self.docs.iter().any(|d| d.title == t) {
                return t;
            }
            n += 1;
        }
    }

    pub fn new_image(&mut self, w: i32, h: i32, bg: i32) {
        let bgc = match bg {
            0 => [255, 255, 255, 255],
            1 => [0, 0, 0, 0],
            _ => self.secondary(),
        };
        let doc = Document::blank(self.next_untitled(), w.max(1) as u32, h.max(1) as u32, bgc);
        self.ui().global::<App>().set_dialog("".into());
        self.add_doc(doc);
    }

    pub fn open_path(&mut self, path: &Path) {
        match io::load(path) {
            Ok(st) => {
                let title = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Image".into());
                let mut doc = Document::new(title, st, "Open Image", true);
                doc.path = Some(path.to_path_buf());
                self.add_doc(doc);
                self.add_recent_file(path);
            }
            Err(e) => self.message("Could not open file", &format!("{}\n\n{}", path.display(), e)),
        }
    }

    fn import_layer(&mut self, path: &Path) {
        match io::load_surface(path) {
            Ok(s) => {
                let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Imported".into());
                let d = self.doc_mut().unwrap();
                let mut full = Surface::new(d.state.w, d.state.h);
                for y in 0..s.h.min(d.state.h) as i32 {
                    for x in 0..s.w.min(d.state.w) as i32 {
                        full.set(x, y, s.get(x, y));
                    }
                }
                let at = d.state.active + 1;
                d.state.layers.insert(at, Layer::from_surface(name, &full));
                d.state.active = at;
                self.structural("Import from File", "image-plus");
            }
            Err(e) => self.message("Could not import file", &e),
        }
    }

    pub fn open_dialog(&mut self, as_layer: bool) {
        let dlg = rfd::FileDialog::new()
            .set_title(if as_layer { "Import from File" } else { "Open" })
            .add_filter("Images", io::OPEN_EXTS)
            .add_filter("All files", &["*"]);
        let done = move |files: Vec<PathBuf>| {
            with_editor(move |e| {
                for f in files {
                    if as_layer {
                        e.import_layer(&f);
                    } else {
                        e.open_path(&f);
                    }
                }
            })
        };
        spawn_dialog(move || if as_layer { dlg.pick_file().into_iter().collect() } else { dlg.pick_files().unwrap_or_default() }, done);
    }

    pub fn save(&mut self, save_as: bool) {
        let Some(doc) = self.doc() else { return };
        let path = doc.path.clone();
        match (save_as, path) {
            (false, Some(p)) => self.save_to(p),
            (_, p) => {
                let stem = doc.title.rsplit_once('.').map(|(a, _)| a.to_string()).unwrap_or(doc.title.clone());
                let default_ext = if doc.state.layers.len() > 1 { "ora" } else { "png" };
                let name = p.as_ref().and_then(|p| p.file_name()).map(|s| s.to_string_lossy().to_string()).unwrap_or(format!("{stem}.{default_ext}"));
                let mut dlg = rfd::FileDialog::new().set_title("Save As").set_file_name(&name);
                dlg = dlg
                    .add_filter("PNG", &["png"])
                    .add_filter("OpenRaster (layers)", &["ora"])
                    .add_filter("JPEG", &["jpg", "jpeg"])
                    .add_filter("WebP", &["webp"])
                    .add_filter("BMP", &["bmp"])
                    .add_filter("GIF", &["gif"])
                    .add_filter("TIFF", &["tif", "tiff"])
                    .add_filter("TGA", &["tga"])
                    .add_filter("ICO", &["ico"])
                    .add_filter("QOI", &["qoi"]);
                if let Some(dir) = p.as_ref().and_then(|p| p.parent()) {
                    dlg = dlg.set_directory(dir);
                }
                spawn_dialog(
                    move || dlg.save_file().into_iter().collect::<Vec<_>>(),
                    |files| {
                        with_editor(move |e| match files.into_iter().next() {
                            Some(mut f) => {
                                if f.extension().is_none() {
                                    f.set_extension("png");
                                }
                                e.save_to(f)
                            }
                            None => {
                                e.after_save = None;
                                e.exiting = false;
                            }
                        })
                    },
                );
            }
        }
    }

    fn save_to(&mut self, path: PathBuf) {
        let ext = io::ext_of(&path);
        if ext == "jpg" || ext == "jpeg" {
            self.pending_save_path = Some(path);
            self.ui().global::<App>().set_dialog("jpeg".into());
            return;
        }
        self.write_file(path, 92);
    }

    pub fn jpeg_ok(&mut self) {
        let ui = self.ui();
        let g = ui.global::<App>();
        g.set_dialog("".into());
        if let Some(p) = self.pending_save_path.take() {
            self.write_file(p, g.get_jpeg_quality().clamp(1, 100) as u8);
        }
    }

    fn write_file(&mut self, path: PathBuf, quality: u8) {
        let Some(doc) = self.docs.get_mut(self.cur) else { return };
        match io::save(&path, &doc.state, quality) {
            Ok(()) => {
                doc.path = Some(path.clone());
                doc.title = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                doc.saved_index = Some(doc.history.index);
                self.add_recent_file(&path);
                self.panels();
                match self.after_save.take() {
                    Some(AfterSave::CloseDoc(id)) => {
                        if let Some(i) = self.docs.iter().position(|d| d.id == id) {
                            self.close_doc(i);
                        }
                    }
                    Some(AfterSave::ContinueExit) => self.continue_exit(),
                    None => {}
                }
            }
            Err(e) => {
                self.after_save = None;
                self.exiting = false;
                self.message("Could not save file", &format!("{}\n\n{}", path.display(), e));
            }
        }
    }

    pub fn request_close(&mut self, i: usize) {
        if i >= self.docs.len() {
            return;
        }
        if self.docs[i].is_modified() {
            self.switch_doc(i);
            self.pending_close = Some(AfterSave::CloseDoc(self.docs[i].id));
            let ui = self.ui();
            let g = ui.global::<App>();
            g.set_confirm_doc(self.docs[i].title.as_str().into());
            g.set_dialog("confirm".into());
        } else {
            self.close_doc(i);
        }
    }

    pub fn close_doc(&mut self, i: usize) {
        if i == self.cur {
            self.finish_session(false);
            self.session = Session::None;
        }
        self.docs.remove(i);
        if self.cur >= self.docs.len() {
            self.cur = self.docs.len().saturating_sub(1);
        } else if i < self.cur {
            self.cur -= 1;
        }
        self.hover = None;
        self.sync_all();
    }

    pub fn continue_exit(&mut self) {
        if let Some(i) = self.docs.iter().position(|d| d.is_modified()) {
            self.switch_doc(i);
            self.pending_close = Some(AfterSave::ContinueExit);
            let ui = self.ui();
            let g = ui.global::<App>();
            g.set_confirm_doc(self.docs[i].title.as_str().into());
            g.set_dialog("confirm".into());
        } else {
            let _ = slint::quit_event_loop();
        }
    }

    /// 0 = don't save, 1 = cancel, 2 = save
    pub fn confirm(&mut self, choice: i32) {
        self.ui().global::<App>().set_dialog("".into());
        let Some(pending) = self.pending_close.take() else { return };
        match choice {
            0 => match pending {
                AfterSave::CloseDoc(id) => {
                    if let Some(i) = self.docs.iter().position(|d| d.id == id) {
                        self.close_doc(i);
                    }
                }
                AfterSave::ContinueExit => {
                    let i = self.cur;
                    self.docs[i].saved_index = Some(self.docs[i].history.index);
                    self.close_doc(i);
                    self.continue_exit();
                }
            },
            2 => {
                self.after_save = Some(pending);
                self.save(false);
            }
            _ => self.exiting = false,
        }
    }

    pub fn history_goto(&mut self, i: i32) {
        self.finish_session(false);
        self.session = Session::None;
        if let Some(d) = self.doc_mut() {
            d.goto_history(i as usize);
        }
        self.panels();
    }

    pub fn layer_select_ui(&mut self, ui_index: i32) {
        self.finish_session(true);
        self.session = Session::None;
        if let Some(d) = self.doc_mut() {
            let n = d.state.layers.len();
            let idx = n - 1 - (ui_index as usize).min(n - 1);
            d.state.active = idx;
            // Selecting a layer is not a history step, but keep the history copy in sync
            // so reverting doesn't jump back.
            let hi = d.history.index;
            d.history.entries[hi].state.active = idx;
        }
        self.panels();
    }

    pub fn layer_toggle_ui(&mut self, ui_index: i32) {
        self.finish_session(true);
        if let Some(d) = self.doc_mut() {
            let n = d.state.layers.len();
            let idx = n - 1 - (ui_index as usize).min(n - 1);
            d.state.layers[idx].visible = !d.state.layers[idx].visible;
            let name = if d.state.layers[idx].visible { "Show Layer" } else { "Hide Layer" };
            d.invalidate_all();
            d.commit(name, "eye");
        }
        self.panels();
    }

    pub fn layer_opacity(&mut self, v: i32, commit: bool) {
        if let Some(d) = self.doc_mut() {
            let o = v.clamp(0, 100) as f32 / 100.0;
            d.state.layer_mut().opacity = o;
            d.invalidate_all();
            if commit && (d.history.current().layer().opacity - o).abs() > 1e-4 {
                d.commit("Layer Opacity", "sliders");
                self.panels();
            }
            self.redraw();
        }
    }

    pub fn layer_blend(&mut self, i: i32) {
        if let Some(d) = self.doc_mut() {
            let b = BlendMode::from_index(i.max(0) as usize);
            if d.state.layer().blend != b {
                d.state.layer_mut().blend = b;
                d.invalidate_all();
                d.commit("Layer Blend Mode", "sliders");
            }
        }
        self.panels();
    }

    fn open_layer_props(&mut self) {
        let Some(d) = self.doc() else { return };
        let l = d.state.layer().clone();
        let active = d.state.active;
        self.lp = Some(LayerPropsSession { layer: active, orig: (l.name.clone(), l.visible, l.opacity, l.blend) });
        let ui = self.ui();
        let g = ui.global::<App>();
        g.set_lp_name(l.name.as_str().into());
        g.set_lp_visible(l.visible);
        g.set_lp_blend(l.blend.index() as i32);
        g.set_lp_opacity((l.opacity * 255.0).round() as i32);
        g.set_dialog("layer-props".into());
    }

    pub fn layer_props_live(&mut self, name: String, vis: bool, blend: i32, op: i32) {
        let Some(lp) = &self.lp else { return };
        let idx = lp.layer;
        if let Some(d) = self.doc_mut() {
            let l = &mut d.state.layers[idx];
            l.name = name;
            l.visible = vis;
            l.blend = BlendMode::from_index(blend.max(0) as usize);
            l.opacity = op.clamp(0, 255) as f32 / 255.0;
            d.invalidate_all();
        }
        self.panels();
    }

    pub fn layer_props_ok(&mut self) {
        self.ui().global::<App>().set_dialog("".into());
        if let Some(lp) = self.lp.take() {
            if let Some(d) = self.doc_mut() {
                let l = &d.state.layers[lp.layer];
                if (l.name.clone(), l.visible, l.opacity, l.blend) != lp.orig {
                    d.commit("Layer Properties", "sliders");
                }
            }
        }
        self.panels();
        self.refocus();
    }

    pub fn layer_props_cancel(&mut self) {
        self.ui().global::<App>().set_dialog("".into());
        if let Some(lp) = self.lp.take() {
            if let Some(d) = self.doc_mut() {
                let l = &mut d.state.layers[lp.layer];
                (l.name, l.visible, l.opacity, l.blend) = lp.orig;
                d.invalidate_all();
            }
        }
        self.panels();
        self.refocus();
    }

    pub fn resize_image(&mut self, w: i32, h: i32, mode: i32) {
        self.ui().global::<App>().set_dialog("".into());
        if let Some(d) = self.doc_mut() {
            if w as u32 == d.state.w && h as u32 == d.state.h {
                return;
            }
            transform::resize_image(&mut d.state, w.max(1) as u32, h.max(1) as u32, transform::Resample::from_index(mode));
            self.structural("Resize Image", "scaling");
            self.fit_doc(self.cur, false);
        }
    }

    pub fn canvas_resize(&mut self, w: i32, h: i32, anchor: i32) {
        self.ui().global::<App>().set_dialog("".into());
        let sec = self.secondary();
        if let Some(d) = self.doc_mut() {
            if w as u32 == d.state.w && h as u32 == d.state.h {
                return;
            }
            // Fill new background area with the secondary color if the bottom layer is opaque.
            let opaque = d.state.layers[0].px.get(0, 0)[3] == 255;
            transform::canvas_size(&mut d.state, w.max(1) as u32, h.max(1) as u32, anchor, if opaque { Some(sec) } else { None });
            self.structural("Canvas Size", "frame");
            self.fit_doc(self.cur, false);
        }
    }

    // ------------------------------------------------------------------------------------------
    // Clipboard

    fn copy(&mut self, merged: bool) {
        let Some(d) = self.doc() else { return };
        let r = d.state.selection.bounds(d.state.w, d.state.h);
        if r.is_empty() {
            return;
        }
        let mut s = if merged { d.state.flatten().crop(r) } else { d.state.layer().px.read_rect(r) };
        if let Some(m) = &d.state.selection.mask {
            for y in 0..s.h as i32 {
                for x in 0..s.w as i32 {
                    let c = m.get(x + r.x0, y + r.y0);
                    if c < 255 {
                        let mut q = s.get(x, y);
                        q[3] = (q[3] as u16 * c as u16 / 255) as u8;
                        s.set(x, y, q);
                    }
                }
            }
        }
        let bytes: Vec<u8> = s.data.iter().flatten().copied().collect();
        let img = arboard::ImageData { width: s.w as usize, height: s.h as usize, bytes: bytes.into() };
        if let Ok(mut cb) = arboard::Clipboard::new() {
            let _ = cb.set_image(img);
        }
        self.clipboard = Some(s);
    }

    fn clipboard_image(&mut self) -> Option<Surface> {
        if let Ok(mut cb) = arboard::Clipboard::new() {
            if let Ok(img) = cb.get_image() {
                let data = img.bytes.chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect();
                return Some(Surface { w: img.width as u32, h: img.height as u32, data });
            }
            // A file path or URI on the clipboard.
            if let Ok(t) = cb.get_text() {
                let p = t.trim().trim_start_matches("file://");
                let path = Path::new(p);
                if path.is_file() {
                    if let Ok(s) = io::load_surface(path) {
                        return Some(s);
                    }
                }
            }
        }
        self.clipboard.clone()
    }

    /// mode: 0 = into current layer, 1 = new layer, 2 = new image
    fn paste(&mut self, mode: i32) {
        let Some(img) = self.clipboard_image() else {
            self.message("Nothing to paste", "The clipboard does not contain an image.");
            return;
        };
        if mode == 2 || self.docs.is_empty() {
            let st = DocState { w: img.w, h: img.h, layers: vec![Layer::from_surface("Background", &img)], active: 0, selection: Selection::none() };
            let doc = Document::new(self.next_untitled(), st, "Paste into New Image", false);
            self.add_doc(doc);
            return;
        }
        let (cw, ch) = self.canvas_px();
        let d = self.doc_mut().unwrap();
        if img.w > d.state.w || img.h > d.state.h {
            let nw = img.w.max(d.state.w);
            let nh = img.h.max(d.state.h);
            transform::canvas_size(&mut d.state, nw, nh, 0, None);
            d.view.needs_fit = false;
        }
        if mode == 1 {
            let n = d.state.layers.len() + 1;
            let at = d.state.active + 1;
            d.state.layers.insert(at, Layer::new(format!("Layer {n}"), d.state.w, d.state.h));
            d.state.active = at;
        }
        // Place at the top-left of the visible area (clamped into the image).
        let vx = ((-d.view.ox) / d.view.zoom).max(0.0) as i32;
        let vy = ((-d.view.oy) / d.view.zoom).max(0.0) as i32;
        let _ = (cw, ch);
        let x0 = vx.min(d.state.w as i32 - img.w as i32).max(0);
        let y0 = vy.min(d.state.h as i32 - img.h as i32).max(0);
        let rect = Rect::new(x0, y0, x0 + img.w as i32, y0 + img.h as i32);
        let (w, h) = (d.state.w, d.state.h);
        let mask = Arc::new(Mask::rect(w, h, rect));
        let layer = d.state.active;
        let cleared = d.state.layers[layer].px.clone();
        let ms = super::session::MoveSession { pixels: true, layer, cleared, float: img, rect, mask, xf: Default::default(), drag: None };
        ms.apply(d);
        d.commit(if mode == 1 { "Paste into New Layer" } else { "Paste" }, "paste");
        self.session = Session::Move(Box::new(ms));
        let ui = self.ui();
        ui.global::<App>().set_tool(Tool::MovePixels);
        ui.global::<App>().set_status_hint(self.status_hint(Tool::MovePixels).into());
        self.update_cursor();
        self.panels();
    }

    fn erase_selection(&mut self, name: &str) {
        let Some(d) = self.doc_mut() else { return };
        let sel = d.state.selection.clone();
        let r = sel.bounds(d.state.w, d.state.h);
        d.state.layer_mut().px.map_rect(r, |x, y, p| {
            let c = sel.coverage(x, y);
            if c == 0 {
                p
            } else {
                let a = (p[3] as u16 * (255 - c) as u16 / 255) as u8;
                if a == 0 {
                    [0; 4]
                } else {
                    [p[0], p[1], p[2], a]
                }
            }
        });
        if name != "Cut" || sel.is_active() {
            // keep selection
        }
        d.invalidate(r);
        d.commit(name, if name == "Cut" { "scissors" } else { "eraser" });
        self.session = Session::None;
        self.panels();
    }

    fn fill_selection(&mut self) {
        let c = self.primary();
        let Some(d) = self.doc_mut() else { return };
        let sel = d.state.selection.clone();
        let r = sel.bounds(d.state.w, d.state.h);
        d.state.layer_mut().px.map_rect(r, |x, y, p| {
            let k = sel.coverage(x, y) as f32 / 255.0;
            lerp_px(p, c, k)
        });
        d.invalidate(r);
        d.commit("Fill Selection", "bucket");
        self.session = Session::None;
        self.push_recent(c);
        self.panels();
    }

    // ------------------------------------------------------------------------------------------
    // Effects & adjustments

    fn fx_ctx(&self, curves: Option<[[u8; 256]; 3]>) -> Ctx {
        let d = self.doc().unwrap();
        Ctx { primary: self.primary(), secondary: self.secondary(), bounds: d.state.selection.bounds(d.state.w, d.state.h), curves }
    }

    fn icon_for_fx(id: &str) -> &'static str {
        match id {
            "auto-level" | "black-and-white" | "brightness-contrast" | "curves" | "hue-saturation" | "invert-colors" | "levels" | "posterize" | "sepia" | "vibrance" | "temperature" => "contrast",
            "rotate-zoom" => "rotate-cw",
            _ => "sparkles",
        }
    }

    /// Applies the filter output to the active layer, respecting the selection.
    fn apply_filter_result(&mut self, layer: usize, base: &Tiled, out: &Surface) {
        let d = self.doc_mut().unwrap();
        let sel = d.state.selection.clone();
        let r = sel.bounds(d.state.w, d.state.h);
        let mut px = base.clone();
        px.map_rect(r, |x, y, b| {
            let k = sel.coverage(x, y);
            if k == 0 {
                b
            } else {
                let o = out.get(x, y);
                if k == 255 {
                    o
                } else {
                    lerp_px(b, o, k as f32 / 255.0)
                }
            }
        });
        d.state.layers[layer].px = px;
        d.invalidate(r);
        self.redraw();
    }

    fn run_filter_now(&mut self, id: &str, values: &[f32]) {
        let Some(def) = filters::find(id) else { return };
        let d = self.doc().unwrap();
        let layer = d.state.active;
        let base = d.state.layer().px.clone();
        let src = base.to_surface();
        let ctx = self.fx_ctx(None);
        let out = (def.run)(&src, values, &ctx);
        self.apply_filter_result(layer, &base, &out);
        self.doc_mut().unwrap().commit(def.name, Self::icon_for_fx(id));
        self.panels();
    }

    fn open_fx(&mut self, id: &str) {
        let Some(def) = filters::find(id) else { return };
        if def.params.is_empty() && id != "curves" {
            self.run_filter_now(id, &[]);
            self.last_fx = Some((id.to_string(), vec![]));
            return;
        }
        let d = self.doc().unwrap();
        let base = d.state.layer().px.clone();
        let base_surface = Arc::new(base.to_surface());
        let values: Vec<f32> = match &self.last_fx {
            Some((lid, v)) if lid == id && v.len() == def.params.len() => v.clone(),
            _ => def.params.iter().map(|p| p.default).collect(),
        };
        self.fx = Some(FxSession {
            id: id.to_string(),
            name: def.name.to_string(),
            params: def.params.clone(),
            values,
            base,
            base_surface,
            layer: d.state.active,
            gen: 0,
            applied_gen: 0,
            running: false,
            curves: if id == "curves" { Some(default_curves()) } else { None },
            curve_drag: None,
        });
        let ui = self.ui();
        let g = ui.global::<App>();
        g.set_fx_title(def.name.into());
        self.sync_fx_params();
        if id == "curves" {
            g.set_curve_channel(0);
            self.curve_sync();
            g.set_dialog("curves".into());
        } else {
            g.set_dialog("fx".into());
        }
        self.fx_preview();
    }

    fn sync_fx_params(&mut self) {
        let Some(fx) = &self.fx else { return };
        let items: Vec<ParamItem> = fx
            .params
            .iter()
            .zip(fx.values.iter())
            .map(|(p, v)| match &p.kind {
                ParamKind::Slider { min, max, step, decimals } => ParamItem {
                    label: p.label.into(),
                    kind: 0,
                    value: *v,
                    min: *min,
                    max: *max,
                    step: *step,
                    decimals: *decimals,
                    choices: ModelRc::default(),
                },
                ParamKind::Check => ParamItem { label: p.label.into(), kind: 1, value: *v, min: 0.0, max: 1.0, step: 1.0, decimals: 0, choices: ModelRc::default() },
                ParamKind::Choice(c) => {
                    let ch: Vec<SharedString> = c.iter().map(|s| (*s).into()).collect();
                    ParamItem { label: p.label.into(), kind: 2, value: *v, min: 0.0, max: c.len() as f32, step: 1.0, decimals: 0, choices: ModelRc::new(VecModel::from(ch)) }
                }
            })
            .collect();
        self.models.params.set_vec(items);
    }

    pub fn fx_param(&mut self, i: usize, v: f32) {
        if let Some(fx) = &mut self.fx {
            if i < fx.values.len() {
                fx.values[i] = v;
            }
        }
        self.fx_preview();
    }

    /// Starts (or schedules) a background preview computation.
    fn fx_preview(&mut self) {
        let curves = self.fx.as_ref().and_then(|f| f.curves.as_ref().map(curves_lut));
        let ctx = self.fx_ctx(curves);
        let Some(fx) = &mut self.fx else { return };
        fx.gen += 1;
        if fx.running {
            return;
        }
        fx.running = true;
        let gen = fx.gen;
        let id = fx.id.clone();
        let values = fx.values.clone();
        let src = fx.base_surface.clone();
        self.ui().global::<App>().set_fx_busy(true);
        std::thread::spawn(move || {
            let out = filters::find(&id).map(|def| (def.run)(&src, &values, &ctx));
            let _ = slint::invoke_from_event_loop(move || {
                with_editor(move |e| e.fx_done(gen, out));
            });
        });
    }

    fn fx_done(&mut self, gen: u64, out: Option<Surface>) {
        let Some(fx) = &mut self.fx else { return };
        fx.running = false;
        let latest = fx.gen;
        let layer = fx.layer;
        let base = fx.base.clone();
        if let Some(out) = out {
            self.apply_filter_result(layer, &base, &out);
            if let Some(fx) = &mut self.fx {
                fx.applied_gen = gen;
            }
        }
        if gen != latest {
            // Parameters changed meanwhile.
            if let Some(fx) = &mut self.fx {
                fx.gen -= 1;
            }
            self.fx_preview();
        } else {
            self.ui().global::<App>().set_fx_busy(false);
        }
    }

    pub fn fx_ok(&mut self) {
        let Some(fx) = self.fx.take() else { return };
        self.ui().global::<App>().set_dialog("".into());
        self.ui().global::<App>().set_fx_busy(false);
        if fx.applied_gen != fx.gen || fx.running {
            // Compute the final result synchronously.
            let curves = fx.curves.as_ref().map(curves_lut);
            let ctx = self.fx_ctx(curves);
            if let Some(def) = filters::find(&fx.id) {
                let out = (def.run)(&fx.base_surface, &fx.values, &ctx);
                self.apply_filter_result(fx.layer, &fx.base, &out);
            }
        }
        self.doc_mut().unwrap().commit(&fx.name, Self::icon_for_fx(&fx.id));
        if fx.curves.is_none() {
            self.last_fx = Some((fx.id.clone(), fx.values.clone()));
        }
        self.panels();
        self.refocus();
    }

    pub fn fx_cancel(&mut self) {
        let ui = self.ui();
        ui.global::<App>().set_dialog("".into());
        ui.global::<App>().set_fx_busy(false);
        if let Some(fx) = self.fx.take() {
            let d = self.doc_mut().unwrap();
            d.state.layers[fx.layer].px = fx.base;
            d.invalidate_all();
        }
        self.redraw();
        self.refocus();
    }

    pub fn fx_reset(&mut self) {
        if let Some(fx) = &mut self.fx {
            fx.values = fx.params.iter().map(|p| p.default).collect();
        }
        self.sync_fx_params();
        self.fx_preview();
    }

    // Curves editor

    fn curve_channel(&self) -> usize {
        self.ui().global::<App>().get_curve_channel().clamp(0, 3) as usize
    }

    pub fn curve_sync(&mut self) {
        let ch = self.curve_channel();
        let Some(fx) = &self.fx else { return };
        let Some(curves) = &fx.curves else { return };
        let pts = &curves[ch];
        let lut = filters::curve_lut(pts);
        let mut path = format!("M 0 {}", 255 - lut[0] as i32);
        for (i, v) in lut.iter().enumerate().skip(1) {
            path += &format!(" L {} {}", i, 255 - *v as i32);
        }
        let ui = self.ui();
        let g = ui.global::<App>();
        g.set_curve_path(path.into());
        g.set_curve_path_faint("M 0 255 L 255 0".into());
        self.models.curve_points.set_vec(pts.iter().map(|(x, y)| CurvePoint { x: *x, y: *y }).collect::<Vec<_>>());
    }

    pub fn curve_pointer(&mut self, kind: i32, button: i32, x: f32, y: f32) {
        let ch = self.curve_channel();
        let x = x.clamp(0.0, 255.0);
        let y = y.clamp(0.0, 255.0);
        let mut changed = false;
        {
            let Some(fx) = &mut self.fx else { return };
            let Some(curves) = &mut fx.curves else { return };
            let pts = &mut curves[ch];
            match kind {
                0 => {
                    let near = pts.iter().position(|(px, py)| ((px - x).powi(2) + (py - y).powi(2)).sqrt() < 9.0);
                    if button == 1 {
                        if let Some(i) = near {
                            if pts.len() > 2 {
                                pts.remove(i);
                                changed = true;
                            }
                        }
                    } else if let Some(i) = near {
                        fx.curve_drag = Some(i);
                    } else {
                        pts.push((x, y));
                        pts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                        fx.curve_drag = pts.iter().position(|p| *p == (x, y));
                        changed = true;
                    }
                }
                2 => {
                    if let Some(i) = fx.curve_drag {
                        let lo = if i > 0 { pts[i - 1].0 + 1.0 } else { 0.0 };
                        let hi = if i + 1 < pts.len() { pts[i + 1].0 - 1.0 } else { 255.0 };
                        pts[i] = (x.clamp(lo, hi.max(lo)), y);
                        changed = true;
                    }
                }
                _ => fx.curve_drag = None,
            }
        }
        if changed {
            self.curve_sync();
            self.fx_preview();
        }
    }

    pub fn curve_reset(&mut self) {
        if let Some(fx) = &mut self.fx {
            fx.curves = Some(default_curves());
        }
        self.curve_sync();
        self.fx_preview();
    }
}

/// Runs a blocking file dialog without freezing the UI where the platform allows it.
fn spawn_dialog<F, D>(f: F, done: D)
where
    F: FnOnce() -> Vec<PathBuf> + Send + 'static,
    D: FnOnce(Vec<PathBuf>) + Send + 'static,
{
    if cfg!(target_os = "macos") {
        // AppKit dialogs must run on the main thread.
        let files = f();
        let _ = slint::invoke_from_event_loop(move || done(files));
    } else {
        std::thread::spawn(move || {
            let files = f();
            let _ = slint::invoke_from_event_loop(move || done(files));
        });
    }
}
