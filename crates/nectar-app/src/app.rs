//! L'atelier : une note ouverte, ses pages, ses retouches et son style.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use eframe::egui::{self, Align, FontId, Key, KeyboardShortcut, Modifiers, RichText};
use nectar_core::layout::Layout;
use nectar_core::{BlockId, Project, Style};
use notify::Watcher;

use crate::pages::{self, PageView};
use crate::panels;
use crate::theme::{self, kicker};
use crate::worker::{Rendered, Request, Response, Worker};

const LAST_NOTE: &str = "derniere-note";
const ZOOMS: &[f32] = &[0.5, 0.67, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Block,
    Style,
}

/// Options de lancement (`nectar-render note.md --bloc ID --onglet style`).
#[derive(Default)]
pub struct Launch {
    pub note: Option<PathBuf>,
    pub select: Option<String>,
    pub tab: Option<Tab>,
    pub theme: Option<egui::ThemePreference>,
}

pub struct NectarApp {
    worker: Worker,
    pub families: Vec<String>,
    pub project: Option<Project>,
    /// Le style complet courant (preset + réglages) et celui du preset seul.
    pub style: Style,
    pub preset_style: Style,
    generation: u64,
    pub rendered: Option<Rendered>,
    textures: HashMap<(u128, u32), egui::TextureHandle>,
    zoom: f32,
    fit_pending: bool,
    reset_horizontal: bool,
    pub selected: Option<BlockId>,
    pub tab: Tab,
    undo: Vec<Layout>,
    redo: Vec<Layout>,
    watcher: Option<(notify::RecommendedWatcher, Receiver<()>)>,
    status: Option<(String, bool, Instant)>,
    scroll_to: Option<BlockId>,
    pub save_preset_dialog: Option<String>,
    show_warnings: bool,
    launch_select: Option<String>,
}

impl NectarApp {
    pub fn new(cc: &eframe::CreationContext<'_>, launch: Launch) -> Self {
        theme::install(&cc.egui_ctx);
        if let Some(preference) = launch.theme {
            cc.egui_ctx.set_theme(preference);
        }
        let mut app = Self {
            worker: Worker::spawn(cc.egui_ctx.clone()),
            families: Vec::new(),
            project: None,
            style: Style::default(),
            preset_style: Style::default(),
            generation: 0,
            rendered: None,
            textures: HashMap::new(),
            zoom: 1.0,
            fit_pending: true,
            reset_horizontal: true,
            selected: None,
            tab: launch.tab.unwrap_or(Tab::Block),
            undo: Vec::new(),
            redo: Vec::new(),
            watcher: None,
            status: None,
            scroll_to: None,
            save_preset_dialog: None,
            show_warnings: false,
            launch_select: launch.select,
        };
        let remembered = cc.storage.and_then(|s| s.get_string(LAST_NOTE)).map(PathBuf::from).filter(|p| p.is_file());
        if let Some(note) = launch.note.or(remembered) {
            app.open(&note, &cc.egui_ctx);
        }
        app
    }

    // ------------------------------------------------------------ actions

    pub fn open(&mut self, note: &Path, ctx: &egui::Context) {
        match Project::open(note) {
            Ok(project) => {
                self.watch(&project.note, ctx);
                self.project = Some(project);
                self.undo.clear();
                self.redo.clear();
                self.selected = self.launch_select.take().map(BlockId);
                self.scroll_to = self.selected.clone();
                self.rendered = None;
                self.fit_pending = true;
                self.restyle();
                self.regenerate(ctx);
            }
            Err(e) => self.notify(format!("Ouverture impossible : {e}"), true),
        }
    }

    fn watch(&mut self, note: &Path, ctx: &egui::Context) {
        let (tx, rx) = channel();
        let target = std::fs::canonicalize(note).unwrap_or_else(|_| note.to_path_buf());
        let ctx = ctx.clone();
        let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            let Ok(event) = event else { return };
            let touches = event.paths.iter().any(|p| std::fs::canonicalize(p).unwrap_or_else(|_| p.clone()) == target);
            if touches && (event.kind.is_modify() || event.kind.is_create()) {
                let _ = tx.send(());
                ctx.request_repaint();
            }
        });
        self.watcher = watcher
            .and_then(|mut w| {
                let dir = note.parent().unwrap_or(Path::new("."));
                w.watch(dir, notify::RecursiveMode::NonRecursive).map(|()| (w, rx))
            })
            .ok();
    }

    /// Recalcule le style complet depuis le preset et les réglages.
    fn restyle(&mut self) {
        let Some(project) = &self.project else { return };
        let (style, warning) = project.style();
        self.style = style;
        let base = nectar_core::StyleRef { preset: project.layout.style.preset.clone(), overrides: Default::default() };
        self.preset_style = project.presets.resolve(&base).0;
        if let Some(w) = warning {
            self.notify(w, true);
        }
    }

    fn ppi(&self, ctx: &egui::Context) -> f32 {
        96.0 * self.zoom * ctx.pixels_per_point()
    }

    pub fn regenerate(&mut self, ctx: &egui::Context) {
        let Some(project) = &self.project else { return };
        self.generation += 1;
        let generated = Box::new(project.generate());
        self.worker.send(Request::Render { generation: self.generation, generated, ppi: self.ppi(ctx) });
    }

    /// Applique une modification des retouches, avec annulation possible.
    pub fn edit_layout(&mut self, ctx: &egui::Context, edit: impl FnOnce(&mut Layout)) {
        let Some(project) = &mut self.project else { return };
        let before = project.layout.clone();
        edit(&mut project.layout);
        project.layout.prune();
        if project.layout == before {
            return;
        }
        self.undo.push(before);
        self.redo.clear();
        self.save_and_refresh(ctx);
    }

    fn save_and_refresh(&mut self, ctx: &egui::Context) {
        if let Some(project) = &mut self.project
            && let Err(e) = project.save_layout()
        {
            self.notify(format!("Enregistrement des retouches impossible : {e}"), true);
        }
        self.restyle();
        self.regenerate(ctx);
    }

    /// Remplace le style par une version modifiée à la main.
    pub fn edit_style(&mut self, ctx: &egui::Context, edited: &Style) {
        let overrides = nectar_core::style::diff(&self.preset_style, edited);
        self.edit_layout(ctx, |layout| layout.style.overrides = overrides);
    }

    fn undo(&mut self, ctx: &egui::Context) {
        let Some(project) = &mut self.project else { return };
        if let Some(previous) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut project.layout, previous));
            self.save_and_refresh(ctx);
        }
    }

    fn redo(&mut self, ctx: &egui::Context) {
        let Some(project) = &mut self.project else { return };
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut project.layout, next));
            self.save_and_refresh(ctx);
        }
    }

    fn pick_and_open(&mut self, ctx: &egui::Context) {
        let mut dialog = rfd::FileDialog::new().add_filter("Note Markdown", &["md", "markdown"]);
        if let Some(dir) = self.project.as_ref().and_then(|p| p.note.parent()) {
            dialog = dialog.set_directory(dir);
        }
        if let Some(path) = dialog.pick_file() {
            self.open(&path, ctx);
        }
    }

    fn export(&mut self) {
        let Some(project) = &self.project else { return };
        let name = project.note.with_extension("pdf");
        let mut dialog = rfd::FileDialog::new().add_filter("PDF", &["pdf"]);
        if let Some(dir) = name.parent() {
            dialog = dialog.set_directory(dir);
        }
        if let Some(file) = name.file_name() {
            dialog = dialog.set_file_name(file.to_string_lossy());
        }
        if let Some(path) = dialog.save_file() {
            self.worker.send(Request::Export { path, ident: project.note.display().to_string() });
        }
    }

    pub fn notify(&mut self, message: impl Into<String>, error: bool) {
        self.status = Some((message.into(), error, Instant::now()));
    }

    pub fn select(&mut self, id: Option<BlockId>, scroll: bool) {
        if scroll {
            self.scroll_to = id.clone();
        }
        if id.is_some() {
            self.tab = Tab::Block;
        }
        self.selected = id;
    }

    // ---------------------------------------------------------- événements

    fn poll(&mut self, ctx: &egui::Context) {
        while let Ok(response) = self.worker.rx.try_recv() {
            match response {
                Response::Ready { families } => self.families = families,
                Response::Rendered(rendered) => {
                    if rendered.generation == self.generation || self.rendered.is_none() {
                        if let Some(error) = &rendered.error {
                            self.notify(first_line(error), true);
                        }
                        self.rendered = Some(*rendered);
                    }
                }
                Response::Exported(Ok(path)) => self.notify(format!("PDF exporté : {}", path.display()), false),
                Response::Exported(Err(e)) => self.notify(format!("Export impossible : {e}"), true),
            }
        }
        let changed = self.watcher.as_ref().is_some_and(|(_, rx)| rx.try_iter().count() > 0);
        if changed && let Some(project) = &mut self.project {
            match project.reload() {
                Ok(()) => self.regenerate(ctx),
                Err(e) => self.notify(format!("Relecture impossible : {e}"), true),
            }
        }
        // Glisser-déposer d'une note.
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect()
        });
        if let Some(path) = dropped.into_iter().find(|p| p.extension().is_some_and(|e| e == "md")) {
            self.open(&path, ctx);
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let pressed = |m: Modifiers, k: Key| ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(m, k)));
        if pressed(Modifiers::COMMAND, Key::O) {
            self.pick_and_open(ctx);
        }
        if pressed(Modifiers::COMMAND, Key::E) {
            self.export();
        }
        if pressed(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z) || pressed(Modifiers::COMMAND, Key::Y) {
            self.redo(ctx);
        }
        if pressed(Modifiers::COMMAND, Key::Z) {
            self.undo(ctx);
        }
        if pressed(Modifiers::COMMAND, Key::Plus) || pressed(Modifiers::COMMAND, Key::Equals) {
            self.step_zoom(ctx, 1);
        }
        if pressed(Modifiers::COMMAND, Key::Minus) {
            self.step_zoom(ctx, -1);
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) && !ctx.egui_wants_keyboard_input() {
            self.selected = None;
        }
    }

    fn set_zoom(&mut self, ctx: &egui::Context, zoom: f32) {
        let zoom = zoom.clamp(0.25, 4.0);
        if (zoom - self.zoom).abs() > 0.001 {
            self.zoom = zoom;
            self.worker.send(Request::Rescale { ppi: self.ppi(ctx) });
        }
    }

    fn step_zoom(&mut self, ctx: &egui::Context, direction: i32) {
        let next = if direction > 0 {
            ZOOMS.iter().copied().find(|z| *z > self.zoom + 0.01)
        } else {
            ZOOMS.iter().rev().copied().find(|z| *z < self.zoom - 0.01)
        };
        if let Some(zoom) = next {
            self.set_zoom(ctx, zoom);
        }
    }

    // ------------------------------------------------------------------ UI

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        let t = theme::tokens(ui.ctx());
        let ctx = ui.ctx().clone();
        egui::Panel::top("barre")
            .frame(egui::Frame::new().fill(t.paper).inner_margin(egui::Margin::symmetric(16, 10)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Nectar").font(FontId::new(22.0, theme::title())).color(t.ink));
                    ui.label(RichText::new("Render").font(FontId::new(22.0, theme::title())).color(t.identity));
                    ui.add_space(18.0);
                    if ui.button("Ouvrir…").on_hover_text("Ctrl+O").clicked() {
                        self.pick_and_open(&ctx);
                    }
                    let can_export = self.rendered.as_ref().is_some_and(|r| !r.pages.is_empty());
                    let export = egui::Button::new(RichText::new("Exporter le PDF").color(t.paper)).fill(t.ink);
                    if ui.add_enabled(can_export, export).on_hover_text("Ctrl+E").clicked() {
                        self.export();
                    }
                    ui.add_space(12.0);
                    if ui
                        .add_enabled(!self.undo.is_empty(), egui::Button::new("↶ Annuler"))
                        .on_hover_text("Ctrl+Z")
                        .clicked()
                    {
                        self.undo(&ctx);
                    }
                    if ui
                        .add_enabled(!self.redo.is_empty(), egui::Button::new("↷ Rétablir"))
                        .on_hover_text("Ctrl+Y")
                        .clicked()
                    {
                        self.redo(&ctx);
                    }

                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        let dark = ctx.theme() == egui::Theme::Dark;
                        if ui
                            .button(if dark { "Clair" } else { "Sombre" })
                            .on_hover_text("Thème de l'atelier")
                            .clicked()
                        {
                            ctx.set_theme(if dark {
                                egui::ThemePreference::Light
                            } else {
                                egui::ThemePreference::Dark
                            });
                        }
                        ui.add_space(8.0);
                        if ui.button("Ajuster").on_hover_text("Ajuster à la largeur").clicked() {
                            self.fit_pending = true;
                        }
                        if ui.button("+").clicked() {
                            self.step_zoom(&ctx, 1);
                        }
                        ui.label(RichText::new(format!("{:.0} %", self.zoom * 100.0)).monospace());
                        if ui.button("−").clicked() {
                            self.step_zoom(&ctx, -1);
                        }
                        ui.add_space(12.0);
                        if let Some(project) = &self.project {
                            let name =
                                project.note.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                            ui.label(RichText::new(name).color(t.muted));
                        }
                    });
                });
            });
        egui::Panel::top("filet").exact_size(2.0).frame(egui::Frame::new().fill(t.ink)).show(ui, |_| {});
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let t = theme::tokens(ui.ctx());
        egui::Panel::bottom("etat")
            .frame(
                egui::Frame::new()
                    .fill(t.paper)
                    .inner_margin(egui::Margin::symmetric(16, 6))
                    .stroke(egui::Stroke::new(1.0, t.rule)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let mono = |s: String| RichText::new(s).font(FontId::new(11.0, theme::mono())).color(t.faint);
                    match &self.rendered {
                        Some(r) => {
                            ui.label(mono(format!("{} PAGES · {} MS", r.pages.len(), r.millis)));
                            let warnings = r.warnings.len() + usize::from(r.error.is_some());
                            if warnings > 0 {
                                let text = RichText::new(format!(
                                    "⚠ {warnings} AVERTISSEMENT{}",
                                    if warnings > 1 { "S" } else { "" }
                                ))
                                .font(FontId::new(11.0, theme::mono()))
                                .color(ui.visuals().warn_fg_color);
                                if ui.add(egui::Button::new(text).frame(false)).clicked() {
                                    self.show_warnings = !self.show_warnings;
                                }
                            }
                            if !r.missing_fonts.is_empty() {
                                ui.label(mono(format!(
                                    "POLICES ABSENTES : {}",
                                    r.missing_fonts.join(", ").to_uppercase()
                                )))
                                .on_hover_text("Remplacées par une police de secours de la même famille.");
                            }
                        }
                        None if self.project.is_some() => {
                            ui.label(mono("MISE EN PAGE…".into()));
                        }
                        None => {}
                    }
                    if let Some((message, error, at)) = &self.status
                        && at.elapsed() < Duration::from_secs(8)
                    {
                        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                            ui.label(RichText::new(message).color(if *error { t.danger } else { t.identity }));
                        });
                        ui.ctx().request_repaint_after(Duration::from_secs(1));
                    }
                });
            });
    }

    fn empty_state(&mut self, ui: &mut egui::Ui) {
        let t = theme::tokens(ui.ctx());
        let ctx = ui.ctx().clone();
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.3);
            kicker(ui, "Atelier de mise en page");
            ui.add_space(6.0);
            ui.label(RichText::new("Ouvre une note Obsidian").font(FontId::new(40.0, theme::title())).color(t.ink));
            ui.add_space(6.0);
            ui.label(
                RichText::new("Le PDF se met en page en direct ; clique sur un bloc pour le retoucher.").color(t.muted),
            );
            ui.add_space(18.0);
            let open = egui::Button::new(RichText::new("Ouvrir une note…").color(t.paper)).fill(t.ink);
            if ui.add(open).clicked() {
                self.pick_and_open(&ctx);
            }
            ui.add_space(8.0);
            ui.label(RichText::new("ou glisse un fichier .md dans la fenêtre").color(t.faint).small());
        });
    }

    fn warnings_window(&mut self, ctx: &egui::Context) {
        if !self.show_warnings {
            return;
        }
        let Some(r) = &self.rendered else { return };
        let lines: Vec<String> = r.error.iter().chain(&r.warnings).cloned().collect();
        let mut open = true;
        egui::Window::new("Avertissements").open(&mut open).default_width(520.0).show(ctx, |ui| {
            egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                for line in &lines {
                    ui.label(line);
                    theme::rule(ui, 1.0, false);
                }
            });
        });
        self.show_warnings = open;
    }

    fn sync_textures(&mut self, ctx: &egui::Context) -> Vec<PageView> {
        let Some(rendered) = &self.rendered else { return Vec::new() };
        let mut keep = HashMap::new();
        let mut views = Vec::with_capacity(rendered.pages.len());
        for page in &rendered.pages {
            let key = (page.hash, page.image.size[0] as u32);
            let texture = match self.textures.remove(&key) {
                Some(texture) => texture,
                None => ctx.load_texture(
                    format!("page-{:x}", page.hash),
                    egui::ImageData::Color(page.image.clone()),
                    egui::TextureOptions::LINEAR,
                ),
            };
            views.push(PageView { texture: texture.clone(), size_pt: page.size_pt });
            keep.insert(key, texture);
        }
        self.textures = keep;
        views
    }
}

impl eframe::App for NectarApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll(&ctx);
        self.shortcuts(&ctx);
        let t = theme::tokens(&ctx);

        self.top_bar(ui);
        self.status_bar(ui);

        if self.project.is_none() {
            egui::CentralPanel::default().frame(egui::Frame::new().fill(t.paper)).show(ui, |ui| self.empty_state(ui));
            return;
        }

        egui::Panel::left("blocs")
            .default_size(270.0)
            .size_range(200.0..=420.0)
            .frame(egui::Frame::new().fill(t.paper).inner_margin(egui::Margin::symmetric(14, 12)))
            .show(ui, |ui| panels::outline::show(self, ui));

        egui::Panel::right("reglages")
            .default_size(340.0)
            .size_range(300.0..=520.0)
            .frame(egui::Frame::new().fill(t.paper).inner_margin(egui::Margin::symmetric(16, 12)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (tab, label) in [(Tab::Block, "Bloc"), (Tab::Style, "Style")] {
                        let selected = self.tab == tab;
                        let text = RichText::new(label.to_uppercase())
                            .font(FontId::new(11.5, theme::mono()))
                            .extra_letter_spacing(1.0);
                        let text = if selected { text.color(t.accent_ink) } else { text.color(t.muted) };
                        let button = egui::Button::new(text).fill(if selected { t.accent } else { t.paper });
                        if ui.add(button).clicked() {
                            self.tab = tab;
                        }
                    }
                });
                ui.add_space(4.0);
                theme::rule(ui, 2.0, true);
                ui.add_space(8.0);
                egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| match self.tab {
                    Tab::Block => panels::block::show(self, ui),
                    Tab::Style => panels::style::show(self, ui),
                });
            });

        let views = self.sync_textures(&ctx);
        egui::CentralPanel::default().frame(egui::Frame::new().fill(t.sunken)).show(ui, |ui| {
            if self.fit_pending
                && let Some(width) = usual_width(&views)
            {
                let zoom = (ui.available_width() - 64.0) / (width * 96.0 / 72.0);
                self.fit_pending = false;
                self.reset_horizontal = true;
                self.set_zoom(&ctx, (zoom * 100.0).floor() / 100.0);
            }
            if views.is_empty() {
                ui.centered_and_justified(|ui| {
                    let text = match self.rendered.as_ref().and_then(|r| r.error.as_ref()) {
                        Some(error) => RichText::new(error).color(t.danger),
                        None => RichText::new("Mise en page…").color(t.muted),
                    };
                    ui.label(text);
                });
                return;
            }
            let scroll_to = self.scroll_to.take();
            let reset = std::mem::take(&mut self.reset_horizontal);
            let action = pages::show(self, ui, &views, self.zoom, scroll_to, reset);
            if let Some(clicked) = action.clicked {
                self.select(clicked, false);
            }
        });

        self.warnings_window(&ctx);
        panels::style::save_preset_modal(self, &ctx);

        if let Some(project) = &self.project
            && let Some(storage) = _frame.storage_mut()
        {
            storage.set_string(LAST_NOTE, project.note.display().to_string());
        }
    }
}

/// La largeur de page la plus fréquente (on ajuste sur le format courant,
/// pas sur une page A3 isolée).
fn usual_width(views: &[PageView]) -> Option<f32> {
    let mut counts: Vec<(f32, usize)> = Vec::new();
    for view in views {
        match counts.iter_mut().find(|(w, _)| (*w - view.size_pt.x).abs() < 1.0) {
            Some((_, n)) => *n += 1,
            None => counts.push((view.size_pt.x, 1)),
        }
    }
    counts.into_iter().max_by_key(|(_, n)| *n).map(|(w, _)| w)
}

fn first_line(text: &str) -> String {
    text.lines().take(2).collect::<Vec<_>>().join(" ")
}
