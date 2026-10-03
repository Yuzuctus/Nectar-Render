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
use crate::worker::{CheckInputs, Layouted, Request, Response, Worker};

const LAST_NOTE: &str = "derniere-note";
const RECENTS: &str = "notes-recentes";
const UI_SCALE: &str = "taille-interface";
const ZOOMS: &[f32] = &[0.5, 0.67, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Block,
    Style,
    Check,
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
    pub rendered: Option<Layouted>,
    /// Image de chaque page déjà rendue (par empreinte), avec sa résolution.
    textures: HashMap<u128, (f32, egui::TextureHandle)>,
    /// Dernière demande d'images envoyée au moteur.
    requested: Vec<(u128, u32)>,
    zoom: f32,
    fit_pending: bool,
    reset_horizontal: bool,
    pub selected: Option<BlockId>,
    /// Page sélectionnée (clic sur son numéro ou dans sa marge).
    pub selected_page: Option<usize>,
    pub tab: Tab,
    undo: Vec<Layout>,
    redo: Vec<Layout>,
    watcher: Option<(notify::RecommendedWatcher, Receiver<Changed>)>,
    /// Retouches modifiées mais pas encore écrites (pendant un glisser).
    dirty_since: Option<Instant>,
    /// Dernière modification, pour regrouper un glisser en une seule annulation.
    last_edit: Option<Instant>,
    /// Message du moment (bulle en bas des pages).
    status: Option<Toast>,
    /// Bloc à amener à l'écran ; `true` : seulement s'il n'y est pas déjà.
    scroll_to: Option<(BlockId, bool)>,
    /// Défilement demandé au clavier (points d'écran).
    scroll_delta: f32,
    /// Défilement à l'image précédente.
    viewport: pages::Viewport,
    /// Premier bloc visible : la vue s'y accroche quand la mise en page change.
    view_anchor: Option<(BlockId, f32)>,
    pending_anchor: Option<(BlockId, f32)>,
    pub save_preset_dialog: Option<String>,
    show_warnings: bool,
    /// La fenêtre d'aide (F1).
    show_help: bool,
    /// Résultat du dernier « Connecter à Claude Desktop ».
    ai_status: Option<String>,
    /// Taille de l'interface (zoom de l'atelier, pas du PDF).
    ui_scale: f32,
    launch_select: Option<String>,
    /// Notes ouvertes récemment (la plus récente d'abord).
    recents: Vec<PathBuf>,
    /// Dernière image affichée de chaque page : montrée le temps que la
    /// nouvelle arrive, plutôt qu'une page blanche.
    shown: HashMap<usize, (egui::Vec2, egui::TextureHandle)>,
    /// Déplacement à la souris qui attend sa nouvelle mise en page.
    pub ghost: Option<pages::Ghost>,
    /// Détecteur de lenteur : étapes de l'image en cours, dernière alerte.
    timings: Vec<(&'static str, Duration)>,
    frame_start: Instant,
    last_slow: Option<Instant>,
    /// Relecture de la note prévue (moment, tentatives).
    reload_due: Option<(Instant, u32)>,
    /// Retouches ancrées sur la note, recalculées à chaque changement.
    pub resolved: HashMap<BlockId, nectar_core::BlockOps>,
    /// Images de pages reçues du moteur, envoyées à la carte graphique peu à
    /// peu (une grande page pèse plus de 10 Mo : tout d'un coup, ça fige).
    uploads: std::collections::VecDeque<crate::worker::PageImage>,
}

impl NectarApp {
    pub fn new(cc: &eframe::CreationContext<'_>, launch: Launch) -> Self {
        theme::install(&cc.egui_ctx);
        // Ctrl + / Ctrl − zooment les pages, pas toute l'interface.
        cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
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
            requested: Vec::new(),
            zoom: 1.0,
            fit_pending: true,
            reset_horizontal: true,
            selected: None,
            selected_page: None,
            tab: launch.tab.unwrap_or(Tab::Block),
            undo: Vec::new(),
            redo: Vec::new(),
            watcher: None,
            dirty_since: None,
            last_edit: None,
            status: None,
            scroll_to: None,
            scroll_delta: 0.0,
            viewport: pages::Viewport::default(),
            view_anchor: None,
            pending_anchor: None,
            save_preset_dialog: None,
            show_warnings: false,
            show_help: false,
            ai_status: None,
            ui_scale: cc.storage.and_then(|s| s.get_string(UI_SCALE)).and_then(|v| v.parse().ok()).unwrap_or(1.0),
            launch_select: launch.select,
            recents: cc
                .storage
                .and_then(|s| s.get_string(RECENTS))
                .map(|list| list.lines().map(PathBuf::from).filter(|p| p.is_file()).collect())
                .unwrap_or_default(),
            shown: HashMap::new(),
            ghost: None,
            reload_due: None,
            timings: Vec::new(),
            frame_start: Instant::now(),
            last_slow: None,
            resolved: HashMap::new(),
            uploads: std::collections::VecDeque::new(),
        };
        let remembered = cc.storage.and_then(|s| s.get_string(LAST_NOTE)).map(PathBuf::from).filter(|p| p.is_file());
        cc.egui_ctx.set_zoom_factor(app.ui_scale);
        if let Some(note) = launch.note.or(remembered) {
            app.open(&note, &cc.egui_ctx);
        }
        app
    }

    // ------------------------------------------------------------ actions

    pub fn open(&mut self, note: &Path, ctx: &egui::Context) {
        self.flush();
        match Project::open(note) {
            Ok(mut project) => {
                for notice in std::mem::take(&mut project.notices) {
                    crate::journal::write(&notice);
                    self.notify(notice, true);
                }
                self.reload_due = None;
                self.watch(&project, ctx);
                self.recents.retain(|p| p != &project.note);
                self.recents.insert(0, project.note.clone());
                self.recents.truncate(10);
                self.project = Some(project);
                self.undo.clear();
                self.redo.clear();
                self.selected = self.launch_select.take().map(BlockId);
                self.selected_page = None;
                self.scroll_to = self.selected.clone().map(|id| (id, false));
                self.rendered = None;
                self.textures.clear();
                self.shown.clear();
                self.ghost = None;
                self.requested.clear();
                self.fit_pending = true;
                self.restyle();
                self.regenerate(ctx);
            }
            Err(e) => self.notify(format!("Ouverture impossible : {e}"), true),
        }
    }

    fn watch(&mut self, project: &Project, ctx: &egui::Context) {
        let (tx, rx) = channel();
        let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        let note = canon(&project.note);
        let layout = canon(&project.layout_path);
        let layout_name = project.layout_path.file_name().map(|n| n.to_os_string());
        let ctx = ctx.clone();
        let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            let Ok(event) = event else { return };
            if !(event.kind.is_modify() || event.kind.is_create()) {
                return;
            }
            for path in &event.paths {
                let changed = if canon(path) == note {
                    Some(Changed::Note)
                } else if canon(path) == layout || (path.file_name().map(|n| n.to_os_string()) == layout_name) {
                    Some(Changed::Layout)
                } else {
                    None
                };
                if let Some(changed) = changed {
                    let _ = tx.send(changed);
                    ctx.request_repaint();
                }
            }
        });
        self.watcher = watcher
            .and_then(|mut w| {
                w.watch(project.note.parent().unwrap_or(Path::new(".")), notify::RecursiveMode::NonRecursive)?;
                if let Some(dir) = project.layout_path.parent()
                    && dir.is_dir()
                {
                    w.watch(dir, notify::RecursiveMode::NonRecursive)?;
                }
                Ok((w, rx))
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
        self.resolved = project.layout.resolve(&project.document).ops;
        let check = Box::new(CheckInputs {
            document: project.document.clone(),
            layout: project.layout.clone(),
            style: self.style.clone(),
        });
        self.worker.send(Request::Layout { generation: self.generation, check });
        ctx.request_repaint();
    }

    /// Applique une modification des retouches, avec annulation possible.
    ///
    /// Les modifications rapprochées (un curseur qu'on fait glisser, un champ
    /// qu'on tape) forment une seule étape d'annulation, et le fichier n'est
    /// écrit qu'une fois le geste terminé.
    pub fn edit_layout(&mut self, ctx: &egui::Context, edit: impl FnOnce(&mut Layout)) {
        let Some(project) = &mut self.project else { return };
        let before = project.layout.clone();
        edit(&mut project.layout);
        project.layout.prune(&project.document);
        if project.layout == before {
            return;
        }
        // Un geste continu (glisser, curseur, saisie, Alt + flèches) ne fait
        // qu'une étape d'annulation ; deux clics distincts en font deux.
        let continuous = ctx.input(|i| i.pointer.any_down() || i.modifiers.alt) || ctx.egui_wants_keyboard_input();
        let gesture = continuous && self.last_edit.is_some_and(|t| t.elapsed() < Duration::from_millis(700));
        if !gesture || self.undo.is_empty() {
            self.undo.push(before);
        }
        self.last_edit = Some(Instant::now());
        self.redo.clear();
        self.dirty_since.get_or_insert_with(Instant::now);
        self.restyle();
        self.regenerate(ctx);
    }

    /// Modifie les retouches d'un bloc (ou d'une puce).
    pub fn edit_block(&mut self, ctx: &egui::Context, id: &BlockId, edit: impl FnOnce(&mut nectar_core::BlockOps)) {
        let Some(project) = &self.project else { return };
        let Some(anchor) = project.document.anchors().into_iter().find(|a| a.id == id) else { return };
        let (kind, line, excerpt) = (anchor.kind, anchor.line, anchor.excerpt.to_string());
        let before = project.layout.ops_for(&project.document, id);
        let mut ops = before.clone();
        edit(&mut ops);
        if ops == before {
            return;
        }
        let told = panels::actions::change(&before, &ops);
        let id = id.clone();
        self.edit_layout(ctx, move |layout| {
            *layout.ops_mut(nectar_core::model::AnchorInfo { id: &id, kind, line, excerpt: &excerpt }) = ops;
        });
        self.notify_done(told);
    }

    /// Modifie plusieurs blocs d'un coup : une seule étape d'annulation.
    pub fn edit_blocks(
        &mut self,
        ctx: &egui::Context,
        ids: &[BlockId],
        edit: impl Fn(&BlockId, &mut nectar_core::BlockOps),
    ) {
        let Some(project) = &self.project else { return };
        let anchors: Vec<(BlockId, nectar_core::model::BlockKind, usize, String)> = project
            .document
            .anchors()
            .into_iter()
            .filter(|a| ids.contains(a.id))
            .map(|a| (a.id.clone(), a.kind, a.line, a.excerpt.to_string()))
            .collect();
        let edited: Vec<_> = anchors
            .into_iter()
            .map(|(id, kind, line, excerpt)| {
                let mut ops = project.layout.ops_for(&project.document, &id);
                edit(&id, &mut ops);
                (id, kind, line, excerpt, ops)
            })
            .collect();
        self.edit_layout(ctx, move |layout| {
            for (id, kind, line, excerpt, ops) in &edited {
                *layout.ops_mut(nectar_core::model::AnchorInfo { id, kind: *kind, line: *line, excerpt }) = ops.clone();
            }
        });
    }

    /// Décale un bloc verticalement (espace avant, en millimètres). Il ne
    /// remonte jamais plus haut que le bas de ce qui le précède.
    pub fn nudge(&mut self, ctx: &egui::Context, id: &BlockId, mm: f32) {
        let room = self
            .rendered
            .as_ref()
            .and_then(|r| pages::up_room(r, id, self.style.page.margin_top_mm * 72.0 / 25.4))
            .map(|pt| pt * 25.4 / 72.0);
        let mm = match room {
            Some(room) if mm < 0.0 => mm.max(-room),
            _ => mm,
        };
        if mm.abs() < 0.25 {
            self.notify("Le bloc touche déjà ce qui le précède", false);
            return;
        }
        self.edit_block(ctx, id, |ops| {
            let value = ((ops.space_before_mm.unwrap_or(0.0) + mm) * 2.0).round() / 2.0;
            ops.space_before_mm = (value.abs() >= 0.25).then_some(value.clamp(-50.0, 200.0));
        });
    }

    /// Écrit les retouches en attente.
    fn flush(&mut self) {
        if self.dirty_since.take().is_none() {
            return;
        }
        if let Some(project) = &mut self.project
            && let Err(e) = project.save_layout()
        {
            self.notify(format!("Enregistrement des retouches impossible : {e}"), true);
        }
    }

    fn save_and_refresh(&mut self, ctx: &egui::Context) {
        self.dirty_since = Some(Instant::now());
        self.flush();
        self.last_edit = None;
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
            self.status =
                Some(Toast { text: "Annulé".into(), error: false, at: Instant::now(), offer: Some(Undo::Redo) });
        }
    }

    fn redo(&mut self, ctx: &egui::Context) {
        let Some(project) = &mut self.project else { return };
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut project.layout, next));
            self.save_and_refresh(ctx);
            self.status =
                Some(Toast { text: "Rétabli".into(), error: false, at: Instant::now(), offer: Some(Undo::Undo) });
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

    pub fn export(&mut self) {
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
            self.worker.send(Request::Export {
                path,
                ident: project.note.display().to_string(),
                pdf_a: self.style.export.pdf_a,
            });
        }
    }

    pub fn notify(&mut self, message: impl Into<String>, error: bool) {
        self.status = Some(Toast { text: message.into(), error, at: Instant::now(), offer: None });
    }

    /// Une retouche faite : la bulle la décrit et propose de l'annuler.
    pub fn notify_done(&mut self, message: impl Into<String>) {
        self.status = Some(Toast { text: message.into(), error: false, at: Instant::now(), offer: Some(Undo::Undo) });
    }

    pub fn select(&mut self, id: Option<BlockId>, scroll: bool) {
        if scroll {
            self.scroll_to = id.clone().map(|id| (id, false));
        }
        if id.is_some() {
            self.tab = Tab::Block;
            self.selected_page = None;
        }
        self.selected = id;
    }

    pub fn select_page(&mut self, page: Option<usize>) {
        if page.is_some() {
            self.tab = Tab::Block;
            self.selected = None;
        }
        self.selected_page = page;
    }

    /// Le bloc qui porte le format d'une page : celui qui l'a changé, sinon
    /// le premier qui y commence.
    pub fn page_owner(&self, page: usize) -> Option<BlockId> {
        let rendered = self.rendered.as_ref()?;
        let ops = &self.resolved;
        let on_page: Vec<&BlockId> = rendered.positions.iter().filter(|p| p.page == page).map(|p| &p.id).collect();
        on_page
            .iter()
            .find(|id| ops.get(**id).is_some_and(|o| o.page.is_some()))
            .or_else(|| on_page.first())
            .map(|id| (*id).clone())
    }

    /// Le format réel d'une page, tel qu'on le montre : « Normal » quand elle
    /// est au format du document.
    pub fn page_format(&self, page: usize) -> Option<panels::page::Format> {
        let size = self.rendered.as_ref()?.pages.get(page)?.size_pt;
        Some(if self.page_differs(page) { panels::page::format_of_size(size) } else { panels::page::Format::Document })
    }

    /// Donne un format à une page. Un schéma ou un tableau posé seul sur une
    /// page paysage (retouche ou décision automatique) porte ce format : il
    /// revient dans le texte pour « Normal ».
    pub fn set_page_format(&mut self, ctx: &egui::Context, page: usize, format: &panels::page::Format) {
        use nectar_core::layout::Placement;
        let Some(rendered) = &self.rendered else { return };
        let ops = &self.resolved;
        let landscape_image = rendered
            .positions
            .iter()
            .filter(|p| p.page == page)
            .map(|p| &p.id)
            .find(|id| {
                ops.get(*id).and_then(|o| o.image.as_ref()).is_some_and(|i| i.placement == Placement::Landscape)
                    || rendered
                        .choices
                        .iter()
                        .any(|c| &c.block == *id && c.kind == nectar_core::auto::ChoiceKind::Landscape)
            })
            .cloned();
        let differs = self.page_differs(page);
        if let Some(image) = landscape_image {
            let format = format.clone();
            let auto =
                !ops.get(&image).and_then(|o| o.image.as_ref()).is_some_and(|i| i.placement == Placement::Landscape);
            self.edit_block(ctx, &image, move |ops| {
                // Mis en paysage d'office : on refuse la décision automatique.
                ops.manual |= auto;
                if let Some(i) = &mut ops.image {
                    i.placement = Placement::Inline;
                }
                if ops.image.as_ref().is_some_and(|i| *i == Default::default()) {
                    ops.image = None;
                }
                if format != panels::page::Format::Document {
                    panels::page::apply(&mut ops.page, &mut ops.page_onward, &format, false);
                }
            });
            return;
        }
        let Some(owner) = self.page_owner(page) else { return };
        self.edit_block(ctx, &owner, |ops| panels::page::apply(&mut ops.page, &mut ops.page_onward, format, differs));
    }

    /// La page n'est pas au format du document.
    pub fn page_differs(&self, page: usize) -> bool {
        let Some(size) = self.rendered.as_ref().and_then(|r| r.pages.get(page)).map(|p| p.size_pt) else {
            return false;
        };
        let Some(project) = &self.project else { return false };
        panels::page::size_of(&project.layout.page).is_some_and(|doc| (doc - size).length() > 2.0)
    }

    // ---------------------------------------------------------- événements

    fn poll(&mut self, ctx: &egui::Context) {
        loop {
            let response = match self.worker.rx.try_recv() {
                Ok(response) => response,
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                // Le moteur s'est arrêté : on en relance un, sans perdre la note.
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.worker = Worker::spawn(ctx.clone());
                    self.textures.clear();
                    self.requested.clear();
                    self.notify("Le moteur a redémarré", true);
                    self.regenerate(ctx);
                    break;
                }
            };
            match response {
                Response::Ready { families } => self.families = families,
                Response::Laid(laid) => {
                    if laid.generation == self.generation || self.rendered.is_none() {
                        if let Some(error) = &laid.error {
                            self.notify(first_line(error), true);
                        }
                        // La vue reste sur ce qu'on regardait, même si des pages
                        // plus haut ont changé de hauteur.
                        if self.rendered.as_ref().is_some_and(|r| r.positions != laid.positions) {
                            self.pending_anchor = self.view_anchor.clone();
                        }
                        // Sauf si le bloc retouché change de page : la vue le suit.
                        if let Some(id) = &self.selected {
                            let page =
                                |r: &crate::worker::Layouted| r.positions.iter().find(|p| &p.id == id).map(|p| p.page);
                            let before = self.rendered.as_ref().and_then(page);
                            if before.is_some() && page(&laid) != before {
                                self.scroll_to = Some((id.clone(), true));
                            }
                        }
                        self.rendered = Some(*laid);
                        self.requested.clear();
                    }
                }
                Response::Images(images) => self.uploads.extend(images),
                Response::Exported(Ok(path)) => self.notify(format!("PDF exporté : {}", path.display()), false),
                Response::Exported(Err(e)) => self.notify(format!("Export impossible : {e}"), true),
                Response::Failed(message) => {
                    // La mise en page précédente reste affichée ; l'attente s'arrête.
                    if let Some(rendered) = &mut self.rendered {
                        rendered.generation = self.generation;
                        rendered.provisional = false;
                    }
                    self.requested.clear();
                    crate::journal::write(&format!("erreur du moteur : {message}"));
                    self.notify(format!("Erreur du moteur, retouche non appliquée : {}", first_line(&message)), true);
                }
            }
        }
        // Envoi des images à la carte graphique : au plus ~10 ms par image affichée.
        let started = Instant::now();
        let target = self.ppi(ctx);
        while let Some(page) = self.uploads.pop_front() {
            // Un aperçu rapide ne remplace jamais une image déjà nette.
            if self.textures.get(&page.hash).is_some_and(|(ppi, _)| *ppi > page.ppi && (*ppi - target).abs() < 0.5) {
                continue;
            }
            let texture = ctx.load_texture(
                format!("page-{:x}", page.hash),
                egui::ImageData::Color(page.image),
                egui::TextureOptions::LINEAR,
            );
            self.textures.insert(page.hash, (page.ppi, texture));
            if started.elapsed() > Duration::from_millis(10) {
                break;
            }
        }
        if !self.uploads.is_empty() {
            ctx.request_repaint();
        }
        let changes: Vec<Changed> = self.watcher.as_ref().map(|(_, rx)| rx.try_iter().collect()).unwrap_or_default();
        // La note a changé : on attend qu'Obsidian ait fini de l'écrire
        // (plusieurs enregistrements rapprochés n'en font qu'un).
        if changes.contains(&Changed::Note) {
            self.reload_due = Some((Instant::now() + Duration::from_millis(250), 0));
        }
        if let Some((due, attempts)) = self.reload_due {
            if Instant::now() < due {
                ctx.request_repaint_after(due - Instant::now());
            } else if let Some(project) = &mut self.project {
                // Quelques tentatives si la note paraît tronquée, puis on la prend telle quelle.
                match project.reload_checked(attempts >= 6) {
                    Ok(()) => {
                        self.reload_due = None;
                        self.regenerate(ctx);
                    }
                    Err(nectar_core::ProjectError::Incomplete) => {
                        self.reload_due = Some((Instant::now() + Duration::from_millis(200), attempts + 1));
                        ctx.request_repaint_after(Duration::from_millis(200));
                    }
                    Err(e) => {
                        self.reload_due = None;
                        self.notify(format!("Relecture impossible : {e}"), true);
                    }
                }
            }
        }
        // Retouches modifiées hors de l'atelier (à la main, autre fenêtre).
        if changes.contains(&Changed::Layout)
            && self.dirty_since.is_none()
            && let Some(project) = &mut self.project
            && let Ok(on_disk) = nectar_core::Layout::load(&project.layout_path)
            && on_disk != project.layout
        {
            let before = project.layout.clone();
            match project.reload_layout() {
                Ok(()) => {
                    self.undo.push(before);
                    self.restyle();
                    self.regenerate(ctx);
                    self.notify("Retouches rechargées depuis le disque", false);
                }
                Err(e) => self.notify(format!("Retouches illisibles : {e}"), true),
            }
        }
        // Écrire les retouches une fois le geste terminé.
        if let Some(since) = self.dirty_since {
            let pointer_down = ctx.input(|i| i.pointer.any_down());
            if !pointer_down && since.elapsed() > Duration::from_millis(400) {
                self.flush();
            } else {
                ctx.request_repaint_after(Duration::from_millis(200));
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
        if pressed(Modifiers::NONE, Key::F1) {
            self.show_help = !self.show_help;
        }
        if pressed(Modifiers::COMMAND, Key::O) {
            self.pick_and_open(ctx);
        }
        // Sur le bloc sélectionné : Alt+flèches le déplace (Maj : 5 mm),
        // Ctrl+Entrée lui donne une nouvelle page, Suppr efface ses retouches.
        if !ctx.egui_wants_keyboard_input()
            && let Some(id) = self.selected.clone()
        {
            let mut mm = 0.0;
            for (modifiers, key, step) in [
                (Modifiers::ALT | Modifiers::SHIFT, Key::ArrowUp, -5.0),
                (Modifiers::ALT | Modifiers::SHIFT, Key::ArrowDown, 5.0),
                (Modifiers::ALT, Key::ArrowUp, -1.0),
                (Modifiers::ALT, Key::ArrowDown, 1.0),
            ] {
                if pressed(modifiers, key) {
                    mm += step;
                }
            }
            if mm != 0.0 {
                self.nudge(ctx, &id, mm);
            }
            if pressed(Modifiers::COMMAND, Key::Enter) {
                self.edit_block(ctx, &id, |ops| panels::actions::apply(ops, panels::actions::Quick::BreakBefore));
            }
            if pressed(Modifiers::NONE, Key::Delete) {
                self.edit_block(ctx, &id, |ops| panels::actions::apply(ops, panels::actions::Quick::Clear));
            }
        }
        // Sans bloc sélectionné, les flèches et Page préc./suiv. font défiler.
        if !ctx.egui_wants_keyboard_input() && self.project.is_some() {
            let page = (self.viewport.height * 0.85).max(200.0);
            for (key, delta) in [(Key::PageDown, page), (Key::PageUp, -page), (Key::Home, -1.0e7), (Key::End, 1.0e7)] {
                if pressed(Modifiers::NONE, key) {
                    self.scroll_delta += delta;
                }
            }
            if self.selected.is_none() {
                if pressed(Modifiers::NONE, Key::ArrowDown) {
                    self.scroll_delta += 60.0;
                }
                if pressed(Modifiers::NONE, Key::ArrowUp) {
                    self.scroll_delta -= 60.0;
                }
            }
        }
        // Flèches haut/bas, un bloc sélectionné : bloc précédent ou suivant
        // (la vue ne suit que s'il sort de l'écran).
        if !ctx.egui_wants_keyboard_input() && self.project.is_some() && self.selected.is_some() {
            let step = if pressed(Modifiers::NONE, Key::ArrowDown) {
                1
            } else if pressed(Modifiers::NONE, Key::ArrowUp) {
                -1
            } else {
                0
            };
            if step != 0
                && let Some(project) = &self.project
            {
                let ids: Vec<BlockId> = project.document.anchors().into_iter().map(|a| a.id.clone()).collect();
                let current = self.selected.as_ref().and_then(|s| ids.iter().position(|i| i == s));
                let next = match current {
                    Some(i) => (i as isize + step).clamp(0, ids.len() as isize - 1) as usize,
                    None => 0,
                };
                if let Some(id) = ids.get(next).cloned() {
                    self.select(Some(id.clone()), false);
                    self.scroll_to = Some((id, true));
                }
            }
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
        // Ctrl + molette : zoom.
        let zoom_delta = ctx.input(|i| i.zoom_delta());
        if (zoom_delta - 1.0).abs() > 0.001 && self.project.is_some() {
            self.set_zoom(ctx, self.zoom * zoom_delta);
        }
        if pressed(Modifiers::COMMAND, Key::Plus) || pressed(Modifiers::COMMAND, Key::Equals) {
            self.step_zoom(ctx, 1);
        }
        if pressed(Modifiers::COMMAND, Key::Minus) {
            self.step_zoom(ctx, -1);
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) && !ctx.egui_wants_keyboard_input() {
            self.selected = None;
            self.selected_page = None;
        }
    }

    fn set_zoom(&mut self, ctx: &egui::Context, zoom: f32) {
        let zoom = zoom.clamp(0.25, 4.0);
        if (zoom - self.zoom).abs() > 0.001 {
            self.zoom = zoom;
            // Le bloc en haut de l'écran y reste : on zoome sur ce qu'on lit.
            if self.scroll_to.is_none() {
                self.pending_anchor = self.view_anchor.clone();
            }
            ctx.request_repaint();
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
                    let recents = self.recents.clone();
                    ui.add_enabled_ui(!recents.is_empty(), |ui| {
                        ui.menu_button("Récents", |ui| {
                            for path in &recents {
                                let name =
                                    path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                                if ui.button(name).on_hover_text(path.display().to_string()).clicked() {
                                    self.open(path, &ctx);
                                    ui.close();
                                }
                            }
                        });
                    });
                    self.plan_menu(ui);
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
                        if ui.button("Aide").on_hover_text("Gestes, raccourcis et explications (F1)").clicked() {
                            self.show_help = !self.show_help;
                        }
                        ui.menu_button("Affichage", |ui| {
                            kicker(ui, "Thème");
                            let mut dark = ctx.theme() == egui::Theme::Dark;
                            if panels::widgets::segmented(ui, &mut dark, &[(false, "Clair"), (true, "Sombre")]) {
                                ctx.set_theme(if dark {
                                    egui::ThemePreference::Dark
                                } else {
                                    egui::ThemePreference::Light
                                });
                            }
                            ui.add_space(6.0);
                            kicker(ui, "Taille de l'interface");
                            let mut scale = self.ui_scale;
                            if panels::widgets::segmented(
                                ui,
                                &mut scale,
                                &[(0.9, "90 %"), (1.0, "100 %"), (1.15, "115 %"), (1.3, "130 %"), (1.5, "150 %")],
                            ) {
                                self.ui_scale = scale;
                                ctx.set_zoom_factor(scale);
                            }
                            panels::widgets::help(ui, "Agrandit les boutons et les textes de l'atelier (pas le PDF).");
                        });
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

    /// Le plan de la note : aller directement à un titre.
    fn plan_menu(&mut self, ui: &mut egui::Ui) {
        let headings: Vec<(BlockId, u8, String)> = self
            .project
            .as_ref()
            .map(|p| {
                p.document
                    .blocks
                    .iter()
                    .filter_map(|b| match &b.node {
                        nectar_core::model::Node::Heading { level, .. } if *level <= 3 => {
                            Some((b.id.clone(), *level, b.excerpt.clone()))
                        }
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        ui.add_enabled_ui(!headings.is_empty(), |ui| {
            ui.menu_button("Plan", |ui| {
                egui::ScrollArea::vertical().max_height(480.0).show(ui, |ui| {
                    for (id, level, text) in &headings {
                        let label = format!("{}{text}", "    ".repeat(usize::from(level.saturating_sub(1))));
                        if ui.selectable_label(self.selected.as_ref() == Some(id), label).clicked() {
                            self.select(Some(id.clone()), true);
                            ui.close();
                        }
                    }
                });
            });
        });
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
                    let link = |s: String, color: egui::Color32| {
                        egui::Button::new(RichText::new(s).font(FontId::new(11.0, theme::mono())).color(color))
                            .frame(false)
                    };
                    match &self.rendered {
                        Some(r) => {
                            let pages = r.pages.len();
                            ui.label(mono(format!("{pages} PAGE{}", if pages > 1 { "S" } else { "" })))
                                .on_hover_text(format!("Mise en page en {} ms", r.millis));
                            ui.label(mono("·".into()));
                            // L'état de la mise en page, en un coup d'œil ; un clic ouvre Vérifier.
                            let remarks =
                                r.issues.iter().filter(|i| i.severity > nectar_core::assistant::Severity::Info).count();
                            let (text, color) = if remarks == 0 {
                                ("✔ PAGES PROPRES".to_string(), t.identity)
                            } else {
                                (
                                    format!("{remarks} POINT{} À VOIR", if remarks > 1 { "S" } else { "" }),
                                    ui.visuals().warn_fg_color,
                                )
                            };
                            if ui.add(link(text, color)).on_hover_text("Ouvrir Vérifier").clicked() {
                                self.tab = Tab::Check;
                            }
                            let automatic = r.choices.iter().filter(|c| !c.kind.global()).count();
                            if automatic > 0 {
                                ui.label(mono("·".into()));
                                let text = format!(
                                    "{automatic} AJUSTEMENT{} AUTOMATIQUE{}",
                                    if automatic > 1 { "S" } else { "" },
                                    if automatic > 1 { "S" } else { "" }
                                );
                                if ui
                                    .add(link(text, t.faint))
                                    .on_hover_text("Ce que Nectar a placé tout seul (dans Vérifier)")
                                    .clicked()
                                {
                                    self.tab = Tab::Check;
                                }
                            }
                            let warnings = r.warnings.len() + usize::from(r.error.is_some());
                            if warnings > 0 {
                                ui.label(mono("·".into()));
                                let text = format!("⚠ {warnings} AVERTISSEMENT{}", if warnings > 1 { "S" } else { "" });
                                if ui.add(link(text, ui.visuals().warn_fg_color)).clicked() {
                                    self.show_warnings = !self.show_warnings;
                                }
                            }
                            if !r.missing_fonts.is_empty() {
                                ui.label(mono(format!(
                                    "· POLICES ABSENTES : {}",
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
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        ui.label(mono("RETOUCHES ENREGISTRÉES AUTOMATIQUEMENT".into())).on_hover_text(
                            "À côté de la note, dans le dossier .nectar ; la note n'est jamais modifiée.",
                        );
                    });
                });
            });
    }

    /// La bulle du moment : ce qui vient d'être fait, et « Annuler ».
    fn toast(&mut self, ctx: &egui::Context, area: egui::Rect) {
        let t = theme::tokens(ctx);
        let Some(toast) = &self.status else { return };
        let hovered_key = egui::Id::new("bulle-survolee");
        let hovered = ctx.data(|d| d.get_temp::<bool>(hovered_key)).unwrap_or(false);
        let life = Duration::from_secs(if toast.error { 10 } else { 6 });
        if toast.at.elapsed() > life && !hovered {
            self.status = None;
            return;
        }
        ctx.request_repaint_after(Duration::from_millis(250));
        let offer = toast.offer.filter(|o| match o {
            Undo::Undo => !self.undo.is_empty(),
            Undo::Redo => !self.redo.is_empty(),
        });
        let (text, error) = (toast.text.clone(), toast.error);
        let mut clicked = None;
        let mut close = false;
        let response = egui::Area::new(egui::Id::new("bulle"))
            .fixed_pos(egui::pos2(area.center().x, area.bottom() - 18.0))
            .pivot(egui::Align2::CENTER_BOTTOM)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(if error { t.danger } else { t.ink })
                    .inner_margin(egui::Margin { left: 14, right: 6, top: 6, bottom: 6 })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&text).color(t.paper));
                            if let Some(offer) = offer {
                                ui.add_space(8.0);
                                let label = match offer {
                                    Undo::Undo => "Annuler",
                                    Undo::Redo => "Rétablir",
                                };
                                let button =
                                    egui::Button::new(RichText::new(label).strong().color(t.ink)).fill(t.accent);
                                if ui.add(button).clicked() {
                                    clicked = Some(offer);
                                }
                            }
                            let cross = egui::Button::new(RichText::new("×").size(16.0).color(t.paper)).frame(false);
                            if ui.add(cross).on_hover_text("Fermer").clicked() {
                                close = true;
                            }
                        });
                    });
            })
            .response;
        ctx.data_mut(|d| d.insert_temp(hovered_key, response.hovered() || response.contains_pointer()));
        match clicked {
            Some(Undo::Undo) => self.undo(ctx),
            Some(Undo::Redo) => self.redo(ctx),
            None if close => self.status = None,
            None => {}
        }
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

    /// L'aide : ce qu'on peut faire, en mots simples.
    fn help_window(&mut self, ctx: &egui::Context) {
        if !self.show_help {
            return;
        }
        let mut open = true;
        egui::Window::new("Aide de Nectar Render")
            .open(&mut open)
            .default_size([520.0, 560.0])
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.content_rect().center())
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().max_height(620.0).show(ui, |ui| {
                    for (title, lines) in HELP {
                        kicker(ui, title);
                        ui.add_space(2.0);
                        for (what, how) in *lines {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(RichText::new(*what).strong());
                                ui.label(*how);
                            });
                        }
                        ui.add_space(10.0);
                    }
                    self.ai_section(ui);
                });
            });
        self.show_help = open;
    }

    /// Brancher une IA : un clic pour Claude Desktop, une commande à copier
    /// pour Claude Code.
    fn ai_section(&mut self, ui: &mut egui::Ui) {
        let t = theme::tokens(ui.ctx());
        kicker(ui, "Avec une IA");
        ui.add_space(2.0);
        ui.label(
            "Une IA (Claude Desktop, Claude Code…) peut mettre en page une note pour vous : formats de page, \
             style, retouches, export. Il faut la brancher une seule fois.",
        );
        ui.add_space(4.0);
        let Some(nectar) = nectar_core::connect::nectar_beside_current_exe() else {
            ui.label(
                RichText::new("nectar.exe est introuvable à côté de l'atelier : réinstallez Nectar Render.")
                    .color(t.muted),
            );
            return;
        };
        let connected = nectar_core::connect::claude_desktop_connected(&nectar);
        ui.horizontal(|ui| {
            let label = if connected { "Rebrancher à Claude Desktop" } else { "Connecter à Claude Desktop" };
            if ui.button(label).clicked() {
                self.ai_status = Some(match nectar_core::connect::connect_claude_desktop(&nectar) {
                    Ok(_) => "C'est fait. Quittez complètement Claude Desktop (icône près de l'horloge → Quitter), \
                              puis rouvrez-le : Nectar apparaît dans ses outils."
                        .into(),
                    Err(error) => format!("Impossible : {error}"),
                });
            }
            if connected {
                ui.label(RichText::new("✔ déjà branché").color(t.muted));
            }
        });
        if let Some(status) = &self.ai_status {
            ui.label(RichText::new(status).small());
        }
        ui.add_space(6.0);
        ui.label("Claude Code : lancer une fois cette commande dans un terminal.");
        let command = nectar_core::connect::claude_code_command(&nectar);
        ui.horizontal(|ui| {
            ui.add(egui::Label::new(RichText::new(&command).monospace().small()).truncate());
            if ui.small_button("Copier").clicked() {
                ui.ctx().copy_text(command.clone());
            }
        });
        ui.add_space(4.0);
        ui.label(
            RichText::new(
                "Ensuite, demandez par exemple : « Mets en page ma note TP WIFI : style Académique, \
                 aucune page à moitié vide, puis exporte le PDF ».",
            )
            .small()
            .color(t.faint),
        );
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

    /// Les pages à afficher, avec l'image la plus proche disponible : la
    /// bonne, ou à défaut la précédente de la même page (pas de clignotement).
    fn views(&mut self) -> Vec<PageView> {
        let Some(rendered) = &self.rendered else { return Vec::new() };
        let mut views = Vec::with_capacity(rendered.pages.len());
        for (index, page) in rendered.pages.iter().enumerate() {
            let view = match self.textures.get(&page.hash) {
                Some((_, texture)) => {
                    self.shown.insert(index, (page.size_pt, texture.clone()));
                    PageView { texture: Some(texture.clone()), fresh: true, size_pt: page.size_pt }
                }
                // L'ancienne image, seulement si la page garde sa taille (sinon elle serait déformée).
                None => PageView {
                    texture: self
                        .shown
                        .get(&index)
                        .filter(|(size, _)| (*size - page.size_pt).length() < 1.0)
                        .map(|(_, t)| t.clone()),
                    fresh: false,
                    size_pt: page.size_pt,
                },
            };
            views.push(view);
        }
        self.shown.retain(|index, _| *index < rendered.pages.len());
        views
    }

    /// Demande au moteur les pages visibles (et leurs voisines) qui manquent
    /// à la bonne résolution, et oublie les images trop éloignées.
    fn request_pages(&mut self, ctx: &egui::Context, visible: &[usize]) {
        let Some(rendered) = &self.rendered else { return };
        let ppi = self.ppi(ctx);
        let count = rendered.pages.len();
        let mut wanted: Vec<usize> = Vec::new();
        for &index in visible {
            for i in index.saturating_sub(1)..=(index + 1).min(count.saturating_sub(1)) {
                if !wanted.contains(&i) {
                    wanted.push(i);
                }
            }
        }
        // Les pages visibles d'abord, leurs voisines ensuite.
        wanted.sort_by_key(|i| !visible.contains(i));
        let missing: Vec<usize> = wanted
            .iter()
            .copied()
            .filter(|&i| self.textures.get(&rendered.pages[i].hash).is_none_or(|(p, _)| (p - ppi).abs() > 0.5))
            .collect();
        let key: Vec<(u128, u32)> = missing.iter().map(|&i| (rendered.pages[i].hash, ppi.to_bits())).collect();
        if !missing.is_empty() && key != self.requested {
            self.worker.send(Request::Pages { pages: missing, ppi });
            self.requested = key;
        }
        // Mémoire bornée : on garde les images des pages proches de la vue.
        if let (Some(&first), Some(&last)) = (visible.iter().min(), visible.iter().max()) {
            let keep: std::collections::HashSet<u128> = rendered.pages
                [first.saturating_sub(4)..=(last + 4).min(count.saturating_sub(1))]
                .iter()
                .map(|p| p.hash)
                .collect();
            self.textures.retain(|hash, _| keep.contains(hash));
            // Les images de secours aussi : seulement autour de la vue.
            let range = first.saturating_sub(6)..=last + 6;
            self.shown.retain(|index, _| range.contains(index));
        }
    }
}

/// Le contenu de l'aide : (rubrique, [(geste, effet)]).
const HELP: &[(&str, &[(&str, &str)])] = &[
    (
        "Sélectionner",
        &[
            ("Clic sur un bloc :", "le sélectionne ; ses réglages s'affichent à droite, dans « Retoucher »."),
            (
                "Clic sur le numéro d'une page",
                "(ou dans sa marge) : sélectionne la page entière, pour changer son format.",
            ),
            ("Clic à côté des pages :", "ne sélectionne plus rien."),
            ("↑ / ↓ :", "bloc précédent ou suivant. Échap : tout désélectionner."),
            ("Plan (en haut) :", "aller directement à un titre."),
        ],
    ),
    (
        "Placer et déplacer",
        &[
            (
                "Glisser un bloc",
                "vers le haut ou le bas : il se rapproche ou s'éloigne de ce qui le précède (jamais par-dessus).",
            ),
            ("Glisser sous le bas de la page :", "le bloc et la suite passent en haut de la page suivante."),
            ("Glisser au-dessus d'un saut de page :", "le saut est retiré, le bloc revient à la suite."),
            ("Défiler :", "molette, ↑ / ↓ (sans bloc sélectionné), Page préc. / Page suiv., Début / Fin."),
            ("Alt + ↑ / ↓ :", "même chose au millimètre (avec Maj : 5 mm)."),
            ("Remonter / Descendre (panneau) :", "décale le bloc de 2 mm ; « Remettre » le replace."),
            (
                "↓ Page suivante (Ctrl + Entrée) :",
                "le bloc et tout ce qui le suit passent en haut de la page suivante.",
            ),
            ("Garder avec la suite :", "le bloc et le suivant restent toujours sur la même page."),
            ("Finir la page ici :", "ce qui suit le bloc commence sur la page suivante."),
            ("Ne pas couper :", "un tableau, une liste ou un code reste entier sur une page."),
            ("Poignée à droite d'une image :", "règle sa largeur."),
            ("Clic droit sur un bloc :", "les mêmes actions, en menu."),
            (
                "Survoler une action",
                "(sans cliquer) : la zone qui va bouger est surlignée dans la page, avec ce qui va se passer.",
            ),
        ],
    ),
    (
        "Ce que Nectar décide seul",
        &[
            (
                "Tableaux et schémas trop larges :",
                "sur une page paysage, avec leur titre ; le texte reprend en portrait.",
            ),
            ("Très grand tableau :", "sur une page A3 paysage s'il y tient en entier, et pas sur une A4 paysage."),
            ("Image horizontale seule sur sa page :", "la page passe en paysage (image plus grande, moins de vide)."),
            (
                "Légende sous une image :",
                "« Capture : … », « Figure … » ou en italique : elle ne quitte jamais son image.",
            ),
            (
                "Titre en bas de page :",
                "s'il ne reste que quelques lignes de son contenu sous lui, il passe page suivante.",
            ),
            (
                "Taille des images :",
                "réduites un peu pour faire tenir la suite, agrandies pour combler un blanc (sauf taille fixée dans la note).",
            ),
            ("Image un peu trop haute :", "légèrement réduite plutôt que de laisser un trou en bas de page."),
            ("Tableau qui déborde de 2 ou 3 lignes :", "un peu resserré pour tenir sur sa page."),
            ("Dernière page de quelques lignes :", "les paragraphes se resserrent un peu pour la supprimer."),
            (
                "Pour refuser :",
                "sélectionner le bloc puis « Garder tel quel » ; tout couper : Style → Placement automatique.",
            ),
        ],
    ),
    (
        "Pages",
        &[
            (
                "Format d'une page :",
                "elle prend ce format, se remplit avec la suite, puis le document reprend le sien.",
            ),
            ("Et les pages suivantes :", "garde ce format jusqu'au prochain changement."),
            ("Page paysage :", "un schéma ou un tableau seul sur une page tournée, en grand."),
        ],
    ),
    (
        "Style et vérification",
        &[
            ("Style :", "un modèle en un clic, puis l'essentiel ; pour le reste, « Chercher un réglage » en haut."),
            (
                "Vérifier :",
                "ce que l'assistant a repéré (page à moitié vide, schéma à agrandir…), corrigeable en un clic.",
            ),
        ],
    ),
    (
        "Annuler, enregistrer, zoomer",
        &[
            ("Ctrl + Z / Ctrl + Y :", "annuler, rétablir. Suppr : tout remettre sur le bloc."),
            ("Après chaque retouche", "une bulle en bas dit ce qui a changé, avec « Annuler »."),
            (
                "« Ce que tu as changé »",
                "(en haut du panneau) : les retouches du bloc, et « Tout remettre comme avant ».",
            ),
            (
                "Les retouches",
                "s'enregistrent toutes seules, à côté de la note (dossier .nectar) ; la note n'est jamais modifiée.",
            ),
            ("Ctrl + molette, Ctrl + / − :", "zoom des pages. « Affichage » : taille de l'interface."),
            ("Ctrl + E :", "exporter le PDF. Ctrl + O : ouvrir une note."),
        ],
    ),
];

/// Ce que le surveillant de fichiers a vu changer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Changed {
    Note,
    Layout,
}

impl eframe::App for NectarApp {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.flush();
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        // Détecteur de lenteur : une image qui met trop longtemps à se
        // dessiner est notée au journal, avec l'étape fautive.
        let started = Instant::now();
        self.frame_start = started;
        self.timings.clear();
        self.draw(ui, frame);
        let total = started.elapsed();
        if total > Duration::from_millis(250) && self.last_slow.is_none_or(|t| t.elapsed() > Duration::from_secs(5)) {
            self.last_slow = Some(Instant::now());
            let steps: Vec<String> =
                self.timings.iter().map(|(step, d)| format!("{step} {} ms", d.as_millis())).collect();
            crate::journal::write(&format!("image lente : {} ms ({})", total.as_millis(), steps.join(", ")));
        }
    }
}

impl NectarApp {
    fn draw(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let step = Instant::now();
        self.poll(&ctx);
        self.timings.push(("événements", step.elapsed()));
        self.shortcuts(&ctx);
        let t = theme::tokens(&ctx);

        self.top_bar(ui);
        self.status_bar(ui);

        if self.project.is_none() {
            egui::CentralPanel::default().frame(egui::Frame::new().fill(t.paper)).show(ui, |ui| self.empty_state(ui));
            return;
        }

        // Largeur fixe : le panneau ne saute pas quand on ouvre une section.
        egui::Panel::right("reglages")
            .resizable(false)
            .exact_size(360.0)
            .frame(egui::Frame::new().fill(t.paper).inner_margin(egui::Margin::symmetric(16, 12)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let count = self
                        .rendered
                        .as_ref()
                        .map(|r| {
                            r.issues.iter().filter(|i| i.severity > nectar_core::assistant::Severity::Info).count()
                        })
                        .unwrap_or(0);
                    let check = if count > 0 { format!("Vérifier · {count}") } else { "Vérifier".to_string() };
                    for (tab, label) in
                        [(Tab::Block, "Retoucher".to_string()), (Tab::Style, "Style".to_string()), (Tab::Check, check)]
                    {
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
                    Tab::Block => match self.selected_page {
                        Some(page) => panels::page::show(self, ui, page),
                        None => panels::block::show(self, ui),
                    },
                    Tab::Style => panels::style::show(self, ui),
                    Tab::Check => panels::check::show(self, ui),
                });
            });

        self.timings.push(("jusqu'aux pages", self.frame_start.elapsed()));
        let views = self.views();
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
            let nav = pages::Navigation {
                scroll_to: self.scroll_to.take(),
                delta: std::mem::take(&mut self.scroll_delta),
                anchor: self.pending_anchor.take(),
                reset_horizontal: std::mem::take(&mut self.reset_horizontal),
                viewport: self.viewport,
            };
            let action = pages::show(self, ui, &views, self.zoom, nav);
            self.viewport = action.viewport;
            self.view_anchor = action.anchor.clone();
            self.timings.push(("pages dessinées", self.frame_start.elapsed()));
            if let Some(clicked) = action.clicked {
                self.select(clicked, false);
                if self.selected.is_none() {
                    self.selected_page = None;
                }
            }
            if let Some(page) = action.page_clicked {
                self.select_page(Some(page));
            }
            if action.background_clicked {
                self.select(None, false);
                self.selected_page = None;
            }
            if let Some((id, width)) = action.resize {
                self.edit_block(&ctx, &id, |ops| {
                    ops.image.get_or_insert_with(Default::default).width_percent = Some(width);
                });
            }
            if let Some((id, mm, mut ghost)) = action.nudge {
                self.nudge(&ctx, &id, mm);
                ghost.generation = self.generation;
                self.ghost = Some(ghost);
            }
            if let Some((id, quick)) = action.quick {
                self.edit_block(&ctx, &id, |ops| panels::actions::apply(ops, quick));
            }
            if let Some(id) = action.next_page {
                self.edit_block(&ctx, &id, |ops| {
                    ops.break_before = true;
                    ops.space_before_mm = None;
                });
                self.notify_done("Le bloc et la suite passent en haut de la page suivante");
            }
            if let Some(id) = action.unbreak {
                self.edit_block(&ctx, &id, |ops| ops.break_before = false);
                self.notify_done("Le bloc revient à la suite de la page précédente");
            }
            if action.more {
                self.tab = Tab::Block;
            }
            // Le fantôme d'un déplacement s'efface quand la nouvelle page est là.
            if let (Some(ghost), Some(rendered)) = (self.ghost, &self.rendered)
                && rendered.generation >= ghost.generation
                && views.get(ghost.page).is_none_or(|v| v.fresh)
            {
                self.ghost = None;
            }
            self.request_pages(&ctx, &action.visible);

            // Le moteur travaille : on le dit, plutôt que de laisser croire à un gel.
            let laying_out = self.rendered.as_ref().is_some_and(|r| r.generation < self.generation);
            // Retouche déjà visible, placement automatique encore en calcul.
            let adjusting = self.rendered.as_ref().is_some_and(|r| r.provisional);
            let drawing = action.visible.iter().any(|i| views.get(*i).is_some_and(|v| !v.fresh));
            if laying_out || adjusting || drawing {
                let text = if laying_out {
                    "Mise en page…"
                } else if adjusting {
                    "Placement automatique…"
                } else {
                    "Rendu des pages…"
                };
                let at = egui::pos2(ui.max_rect().center().x, ui.max_rect().top() + 12.0);
                egui::Area::new(egui::Id::new("chargement"))
                    .fixed_pos(at)
                    .pivot(egui::Align2::CENTER_TOP)
                    .order(egui::Order::Foreground)
                    .interactable(false)
                    .show(&ctx, |ui| {
                        egui::Frame::new()
                            .fill(t.raised)
                            .stroke(egui::Stroke::new(1.0, t.rule_strong))
                            .inner_margin(egui::Margin::symmetric(10, 5))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.add(egui::Spinner::new().size(14.0).color(t.identity));
                                    ui.label(RichText::new(text).color(t.muted));
                                });
                            });
                    });
                ctx.request_repaint_after(Duration::from_millis(100));
            }
            self.toast(&ctx, ui.max_rect());
        });

        self.warnings_window(&ctx);
        self.help_window(&ctx);
        panels::style::save_preset_modal(self, &ctx);

        if let Some(project) = &self.project
            && let Some(storage) = _frame.storage_mut()
        {
            storage.set_string(LAST_NOTE, project.note.display().to_string());
            let list: Vec<String> = self.recents.iter().map(|p| p.display().to_string()).collect();
            storage.set_string(RECENTS, list.join("\n"));
            storage.set_string(UI_SCALE, self.ui_scale.to_string());
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

/// Ce que la bulle propose.
#[derive(Clone, Copy, PartialEq)]
enum Undo {
    Undo,
    Redo,
}

/// Le message du moment, avec de quoi revenir en arrière.
struct Toast {
    text: String,
    error: bool,
    at: Instant,
    offer: Option<Undo>,
}
