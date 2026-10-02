//! L'aperçu des pages : on y voit le PDF, on y désigne les blocs.

use std::collections::HashSet;

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, StrokeKind};
use nectar_core::BlockId;
use nectar_core::model::Node;
use nectar_typst::BlockPosition;

use crate::app::NectarApp;
use crate::panels::actions::{self, Quick};
use crate::theme;

pub struct PageView {
    /// Image de la page, éventuellement à une autre résolution (zoom en cours),
    /// d'une version précédente le temps du rendu, ou absente.
    pub texture: Option<egui::TextureHandle>,
    /// L'image correspond bien à la mise en page actuelle.
    pub fresh: bool,
    pub size_pt: egui::Vec2,
}

/// Où en est le défilement des pages (image précédente).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Viewport {
    pub offset: f32,
    pub height: f32,
}

/// Ce que l'atelier demande au défilement des pages.
#[derive(Default)]
pub struct Navigation {
    /// Amener ce bloc à l'écran ; `true` : seulement s'il n'y est pas déjà.
    pub scroll_to: Option<(BlockId, bool)>,
    /// Défilement au clavier, en points d'écran.
    pub delta: f32,
    /// Garder ce bloc à cette hauteur de l'écran (après une nouvelle mise en page).
    pub anchor: Option<(BlockId, f32)>,
    pub reset_horizontal: bool,
    pub viewport: Viewport,
}

#[derive(Default)]
pub struct PageAction {
    /// Défilement après cette image.
    pub viewport: Viewport,
    /// Premier bloc visible et sa hauteur à l'écran : la vue s'y accroche
    /// quand la mise en page change.
    pub anchor: Option<(BlockId, f32)>,
    /// `Some(None)` : clic dans le vide (désélection).
    pub clicked: Option<Option<BlockId>>,
    /// Pages au moins en partie visibles.
    pub visible: Vec<usize>,
    /// Nouvelle largeur (en %) donnée à l'image sélectionnée avec la poignée.
    pub resize: Option<(BlockId, f32)>,
    /// Bloc déplacé à la souris : décalage vertical en millimètres, et le
    /// fantôme à garder affiché jusqu'à la nouvelle mise en page.
    pub nudge: Option<(BlockId, f32, Ghost)>,
    /// Action rapide choisie sur le bloc (barre ou clic droit).
    pub quick: Option<(BlockId, Quick)>,
    /// Bloc glissé au-delà du bas de sa page : il passe à la page suivante.
    pub next_page: Option<BlockId>,
    /// Bloc remonté au-dessus de son saut de page : le saut est retiré.
    pub unbreak: Option<BlockId>,
    /// « Plus… » : ouvrir toutes les retouches du bloc.
    pub more: bool,
    /// Page sélectionnée (numéro, marge ou blanc de la page).
    pub page_clicked: Option<usize>,
    /// Clic dans le vide, en dehors des pages.
    pub background_clicked: bool,
}

/// Aperçu d'un déplacement : la partie de la page sous le haut du bloc,
/// décalée de `dy` points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ghost {
    pub page: usize,
    pub top: f32,
    pub dy: f32,
    /// Mise en page attendue (le fantôme disparaît quand elle est affichée).
    pub generation: u64,
}

/// Bloc en cours de déplacement.
#[derive(Clone, Debug)]
struct Drag {
    id: BlockId,
    origin: Pos2,
    /// Page et haut du bloc au début du geste (points).
    page: usize,
    top: f32,
    /// Place libre au-dessus du bloc : il ne remonte jamais plus haut (il
    /// passerait sur le texte d'avant).
    up: f32,
    /// Le bloc commence par un saut de page.
    breaks: bool,
}

/// Ce que fera le geste en cours, une fois lâché.
#[derive(Clone, Copy, Debug, PartialEq)]
enum DragMode {
    /// Rapprocher ou éloigner, de `dy` points.
    Nudge,
    /// Glissé au-delà du bas de la page : page suivante.
    NextPage,
    /// Remonté au-dessus d'un saut de page : le saut est retiré.
    Unbreak,
}

/// Place libre (points) entre le haut d'un bloc et ce qui le précède sur sa
/// page : de combien il peut remonter sans recouvrir le texte d'avant.
pub fn up_room(rendered: &crate::worker::Layouted, id: &BlockId, margin_top: f32) -> Option<f32> {
    let first = rendered
        .boxes
        .iter()
        .filter(|b| &b.id == id)
        .min_by(|a, b| a.page.cmp(&b.page).then(a.rect[1].total_cmp(&b.rect[1])))?;
    let top = first.rect[1] as f32;
    let above = rendered
        .boxes
        .iter()
        .filter(|b| b.page == first.page && &b.id != id && (b.rect[3] as f32) <= top + 2.0)
        .map(|b| b.rect[3] as f32)
        .fold(margin_top, f32::max);
    Some((top - above - 1.5).max(0.0))
}

/// En dessous, un glisser est un clic qui a tremblé : rien ne bouge.
const DRAG_DEAD_ZONE: f32 = 10.0;

/// Points typographiques → points d'écran à 100 % (96 ppp).
const PT_TO_SCREEN: f32 = 96.0 / 72.0;
const TOP_GAP: f32 = 24.0;
/// Espace entre deux pages, en points d'écran.
const PAGE_GAP: f32 = 20.0;
const MM: f32 = 72.0 / 25.4;

pub fn show(app: &NectarApp, ui: &mut egui::Ui, views: &[PageView], zoom: f32, nav: Navigation) -> PageAction {
    let t = theme::tokens(ui.ctx());
    let mut action = PageAction::default();
    let Some(rendered) = &app.rendered else { return action };
    let positions = &rendered.positions;
    let scale = zoom * PT_TO_SCREEN;
    let margins = Margins::from(&app.style);
    let children = list_children(app);
    let paper = crate::panels::widgets::parse_hex(&app.style.page.background)
        .map(|[r, g, b]| Color32::from_rgb(r, g, b))
        .unwrap_or(Color32::WHITE);
    let drag_id = ui.id().with("glisser-bloc");
    let menu_id = ui.id().with("menu-bloc");

    // Hauteur d'un bloc dans le contenu défilé (les pages sont empilées sans
    // autre espace que TOP_GAP et PAGE_GAP).
    let block_y = |id: &BlockId| {
        let position = positions.iter().find(|p| &p.id == id)?;
        let above: f32 = views.iter().take(position.page).map(|v| v.size_pt.y * scale + PAGE_GAP).sum();
        Some(TOP_GAP + above + position.y as f32 * scale)
    };
    let vp = nav.viewport;
    let mut target: Option<f32> = None;
    if let Some((id, reveal)) = &nav.scroll_to
        && let Some(y) = block_y(id)
    {
        let visible = y >= vp.offset + 8.0 && y <= vp.offset + vp.height - 80.0;
        if !(*reveal && visible && vp.height > 0.0) {
            target = Some(y - 40.0 * scale);
        }
    } else if let Some((id, dy)) = &nav.anchor
        && let Some(y) = block_y(id)
    {
        target = Some(y - dy);
    }
    if nav.delta != 0.0 {
        target = Some(target.unwrap_or(vp.offset) + nav.delta);
    }

    let mut area = egui::ScrollArea::both()
        .auto_shrink(false)
        .scroll_source(egui::scroll_area::ScrollSource::SCROLL_BAR | egui::scroll_area::ScrollSource::MOUSE_WHEEL);
    if nav.reset_horizontal {
        area = area.horizontal_scroll_offset(0.0);
    }
    if let Some(offset) = target {
        area = area.vertical_scroll_offset(offset.max(0.0));
    }
    let output = area.show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.add_space(TOP_GAP);
        ui.vertical_centered(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let mut hovered: Option<BlockId> = None;
            let mut page_rects = Vec::with_capacity(views.len());
            let mut drag: Option<Drag> = ui.data(|d| d.get_temp(drag_id));
            let mut drag_dy = 0.0f32;
            let mut drag_mode = DragMode::Nudge;
            for (index, view) in views.iter().enumerate() {
                let size = view.size_pt * scale;
                let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
                page_rects.push(rect);
                let painter = ui.painter_at(rect.expand(2.0));
                painter.rect_stroke(rect, 0.0, Stroke::new(1.0, t.rule_strong), StrokeKind::Outside);
                if ui.is_rect_visible(rect) {
                    action.visible.push(index);
                }
                match &view.texture {
                    Some(texture) => {
                        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
                        painter.image(texture.id(), rect, uv, Color32::WHITE);
                    }
                    // Pas encore rendue : une page vierge le temps du rendu.
                    None => {
                        painter.rect_filled(rect, 0.0, paper);
                    }
                }

                let to_pt = |p: Pos2| ((p.x - rect.left()) / scale, (p.y - rect.top()) / scale);
                if let Some(pointer) = response.hover_pos()
                    && drag.is_none()
                {
                    let (_, y) = to_pt(pointer);
                    hovered = hit(positions, index, y);
                    if hovered.is_some() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                    }
                }
                if response.clicked()
                    && let Some(pointer) = response.interact_pointer_pos()
                {
                    let (x, y) = to_pt(pointer);
                    // Dans la marge, ou dans le blanc sous le texte : la page.
                    let content_bottom = rendered
                        .boxes
                        .iter()
                        .filter(|b| b.page == index)
                        .map(|b| b.rect[3] as f32)
                        .fold(margins.top, f32::max);
                    let in_margin = x < margins.left - 6.0 || x > view.size_pt.x - margins.right + 6.0;
                    if in_margin || y > content_bottom + 8.0 || y < margins.top - 6.0 {
                        action.page_clicked = Some(index);
                        action.clicked = Some(None);
                    } else {
                        action.clicked = Some(hit(positions, index, y));
                    }
                }
                // Clic droit : on sélectionne le bloc et on ouvre ses actions.
                if response.secondary_clicked()
                    && let Some(pointer) = response.interact_pointer_pos()
                {
                    let (_, y) = to_pt(pointer);
                    let target = hit(positions, index, y);
                    action.clicked = Some(target.clone());
                    ui.data_mut(|d| d.insert_temp(menu_id, target));
                }
                response.context_menu(|ui| {
                    let target: Option<BlockId> = ui.data(|d| d.get_temp(menu_id)).flatten();
                    let Some(id) = target else {
                        ui.close();
                        return;
                    };
                    for offer in actions::offers(app, &id) {
                        let label = if offer.active { format!("✔ {}", offer.label) } else { offer.label.to_string() };
                        let button = ui.button(label).on_hover_text(offer.hint);
                        if button.hovered() {
                            hover_action(ui.ctx(), &id, offer.quick);
                        }
                        if button.clicked() {
                            action.quick = Some((id.clone(), offer.quick));
                            ui.close();
                        }
                    }
                    ui.separator();
                    if ui.button("Toutes les retouches…").clicked() {
                        action.more = true;
                        ui.close();
                    }
                });

                // Glisser un bloc : il descend ou remonte, et tout ce qui suit avec lui.
                if response.drag_started()
                    && let Some(origin) = ui.input(|i| i.pointer.press_origin())
                    && let Some(id) = hit(positions, index, to_pt(origin).1)
                    && let Some((page, [_, top, _, _])) =
                        bands(rendered, &id, &children, views, &margins).into_iter().next()
                {
                    action.clicked = Some(Some(id.clone()));
                    let up = up_room(rendered, &id, margins.top).unwrap_or(0.0);
                    let breaks = app.resolved.get(&id).is_some_and(|o| o.break_before);
                    drag = Some(Drag { id, origin, page, top, up, breaks });
                }
                if let Some(d) = &drag
                    && (response.dragged() || response.drag_stopped())
                    && let Some(pointer) = ui.input(|i| i.pointer.latest_pos())
                {
                    let delta = pointer - d.origin;
                    // Un geste surtout horizontal, ou trop court, ne déplace rien.
                    let vertical = delta.y.abs() >= DRAG_DEAD_ZONE && delta.y.abs() >= delta.x.abs() * 0.5;
                    let raw = if vertical { delta.y / scale } else { 0.0 };
                    let page_bottom = views.get(d.page).map(|v| v.size_pt.y - margins.bottom).unwrap_or(f32::MAX);
                    drag_mode = if d.top + raw > page_bottom - 18.0 {
                        DragMode::NextPage
                    } else if raw < -d.up - 18.0 && d.breaks {
                        DragMode::Unbreak
                    } else {
                        DragMode::Nudge
                    };
                    // Jamais par-dessus le texte d'avant.
                    drag_dy = raw.max(-d.up);
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                    if response.drag_stopped() {
                        match drag_mode {
                            DragMode::NextPage => action.next_page = Some(d.id.clone()),
                            DragMode::Unbreak => action.unbreak = Some(d.id.clone()),
                            DragMode::Nudge => {
                                let mm = (drag_dy / MM * 2.0).round() / 2.0;
                                if mm.abs() >= 0.5 {
                                    let ghost = Ghost { page: d.page, top: d.top, dy: mm * MM, generation: 0 };
                                    action.nudge = Some((d.id.clone(), mm, ghost));
                                }
                            }
                        }
                        drag = None;
                    }
                }

                // Numéro de page : un clic sélectionne la page.
                let selected_page = app.selected_page == Some(index);
                let tag = tag_rect(rect);
                let tag_response = ui.interact(tag, ui.id().with(("page", index)), Sense::click());
                let tag_painter = ui.painter_at(tag.expand(1.0));
                if selected_page || tag_response.hovered() {
                    tag_painter.rect_filled(tag, 0.0, if selected_page { t.accent } else { t.surface });
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                tag_painter.text(
                    tag.center(),
                    egui::Align2::CENTER_CENTER,
                    format!("{}", index + 1),
                    egui::FontId::new(10.5, theme::mono()),
                    if selected_page { t.accent_ink } else { t.faint },
                );
                if tag_response.on_hover_text(format!("Sélectionner la page {}", index + 1)).clicked() {
                    action.page_clicked = Some(index);
                    action.clicked = Some(None);
                }
                if selected_page {
                    painter.rect_stroke(rect, 0.0, Stroke::new(2.5, t.identity), StrokeKind::Outside);
                }
                ui.add_space(PAGE_GAP);
            }
            if !ui.input(|i| i.pointer.any_down()) && action.nudge.is_none() {
                drag = None;
            }
            ui.data_mut(|d| match &drag {
                Some(value) => {
                    d.insert_temp(drag_id, value.clone());
                }
                None => d.remove::<Drag>(drag_id),
            });

            // Fantôme du déplacement en cours, ou de celui qui attend sa mise en page.
            let live = drag.as_ref().filter(|_| drag_mode == DragMode::Nudge).map(|d| Ghost {
                page: d.page,
                top: d.top,
                dy: drag_dy,
                generation: 0,
            });
            // Glissé hors de la page, ou au-dessus d'un saut : on montre ce qui
            // se passera au lâcher, sans dessiner le bloc n'importe où.
            if let Some(d) = drag.as_ref().filter(|_| drag_mode != DragMode::Nudge)
                && let (Some(page_rect), Some(view)) = (page_rects.get(d.page), views.get(d.page))
            {
                let bottom = rendered
                    .boxes
                    .iter()
                    .filter(|b| b.page == d.page)
                    .map(|b| b.rect[3] as f32)
                    .fold(d.top + 14.0, f32::max);
                let text = if drag_mode == DragMode::NextPage {
                    format!("↓ Lâcher : tout ceci passe en haut de la page {}", d.page + 2)
                } else {
                    "↑ Lâcher : revient à la suite de la page précédente".to_string()
                };
                paint_region(ui, *page_rect, view, d.top, bottom, &text, scale, &margins, t);
            }
            let pending =
                app.ghost.filter(|g| g.generation > rendered.generation || views.get(g.page).is_some_and(|v| !v.fresh));
            if let Some(ghost) = live.or(pending)
                && let (Some(view), Some(page_rect)) = (views.get(ghost.page), page_rects.get(ghost.page))
                && let Some(texture) = &view.texture
            {
                paint_ghost(ui, &ghost, view, *page_rect, texture, scale, &margins, paper, t, live.is_some());
            }
            let shift = live.map(|g| g.dy).unwrap_or(0.0);

            let paint = |id: &BlockId, fill: Color32, stroke: Stroke| {
                for (page, [x0, y0, x1, y1]) in bands(rendered, id, &children, views, &margins) {
                    let Some(page_rect) = page_rects.get(page) else { continue };
                    let dy = if live.is_some_and(|g| g.page == page) { shift } else { 0.0 };
                    let r = Rect::from_min_max(
                        page_rect.min + egui::vec2((x0 - 5.0) * scale, (y0 - 4.0 + dy) * scale),
                        page_rect.min + egui::vec2((x1 + 5.0) * scale, (y1 + 3.0 + dy) * scale),
                    );
                    let painter = ui.painter_at(*page_rect);
                    painter.rect_filled(r, 0.0, fill);
                    painter.rect_stroke(r, 0.0, stroke, StrokeKind::Outside);
                }
            };
            if let Some(id) = &hovered
                && Some(id) != app.selected.as_ref()
            {
                paint(id, Color32::TRANSPARENT, Stroke::new(1.0, t.rule_strong));
            }
            if let Some(id) = &app.selected {
                paint(id, t.marker.gamma_multiply(0.12), Stroke::new(2.0, t.identity));
            }

            // Aperçu de l'action survolée : ce qui va bouger, avant le clic.
            if live.is_none()
                && let Some((id, quick)) = hovered_action(ui.ctx())
                && let Some((page, top, bottom, text)) = preview(app, rendered, &id, quick, &children, views, &margins)
                && let (Some(page_rect), Some(view)) = (page_rects.get(page), views.get(page))
            {
                paint_region(ui, *page_rect, view, top, bottom, &text, scale, &margins, t);
            }

            // Repères des retouches dans la marge gauche.
            {
                let ops = &app.resolved;
                for position in positions {
                    let Some(block_ops) = ops.get(&position.id) else { continue };
                    let summary = summary(block_ops);
                    if summary.is_empty() {
                        continue;
                    }
                    let Some(page_rect) = page_rects.get(position.page) else { continue };
                    let at = page_rect.min + egui::vec2((margins.left - 16.0) * scale, position.y as f32 * scale);
                    let tag = Rect::from_min_size(at, egui::vec2(5.0 * zoom.max(0.6), 12.0 * zoom.max(0.6)));
                    ui.painter_at(*page_rect).rect_filled(tag, 0.0, t.accent);
                    ui.painter_at(*page_rect).rect_stroke(tag, 0.0, Stroke::new(1.0, t.accent_ink), StrokeKind::Inside);
                    let response = ui.interact(tag.expand(3.0), ui.id().with(("repere", &position.id)), Sense::hover());
                    response.on_hover_text(summary.join("\n"));
                }
            }

            // Poignée de largeur de l'image sélectionnée.
            if let Some(id) = &app.selected
                && live.is_none()
                && let Some(project) = &app.project
                && project
                    .document
                    .blocks
                    .iter()
                    .any(|b| &b.id == id && matches!(b.node, Node::Figure(_) | Node::Diagram { .. }))
                && let Some((page, [_, top, image_right, bottom])) =
                    bands(rendered, id, &children, views, &margins).into_iter().next()
                && let Some(page_rect) = page_rects.get(page)
            {
                let ops = project.layout.ops_for(&project.document, id);
                let width_pt = views[page].size_pt.x - margins.left - margins.right;
                let center = margins.left + width_pt / 2.0;
                // Largeur actuelle lue sur la page (l'image peut être en taille d'origine).
                let percent = ops
                    .image
                    .as_ref()
                    .and_then(|i| i.width_percent)
                    .unwrap_or_else(|| (((image_right - center) * 200.0 / width_pt).clamp(10.0, 100.0)).round());
                let right = image_right;
                let handle = Rect::from_center_size(
                    page_rect.min + egui::vec2(right * scale, ((top + bottom) / 2.0) * scale),
                    egui::vec2(10.0, 22.0),
                );
                let response = ui.interact(handle.expand(4.0), ui.id().with("poignee-image"), Sense::drag());
                let painter = ui.painter_at(*page_rect);
                painter.rect_filled(handle, 0.0, if response.dragged() { t.accent } else { t.raised });
                painter.rect_stroke(handle, 0.0, Stroke::new(1.5, t.identity), StrokeKind::Inside);
                if response.hovered() || response.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                    painter.text(
                        handle.right_top() + egui::vec2(6.0, -2.0),
                        egui::Align2::LEFT_BOTTOM,
                        format!("{percent:.0} %"),
                        egui::FontId::new(11.0, theme::mono()),
                        t.identity,
                    );
                }
                if response.dragged()
                    && let Some(pointer) = response.interact_pointer_pos()
                {
                    let x = (pointer.x - page_rect.left()) / scale;
                    let new = ((x - center).abs() * 200.0 / width_pt).clamp(10.0, 100.0).round();
                    if (new - percent).abs() >= 1.0 {
                        action.resize = Some((id.clone(), new));
                    }
                }
            }

            // Clic en dehors des pages (et de leurs numéros) : plus rien de sélectionné.
            if let Some(pointer) = ui.input(|i| i.pointer.interact_pos())
                && ui.input(|i| i.pointer.primary_clicked())
                && ui.clip_rect().contains(pointer)
                && !page_rects.iter().any(|r| r.contains(pointer) || tag_rect(*r).contains(pointer))
                && ui.ctx().layer_id_at(pointer).is_none_or(|layer| layer.order == egui::Order::Background)
            {
                action.background_clicked = true;
            }

            // Barre d'actions posée sur le bloc sélectionné (pas pendant un geste).
            if let Some(id) = &app.selected
                && live.is_none()
                && drag.is_none()
                && let Some((page, [_, top, x1, bottom])) =
                    bands(rendered, id, &children, views, &margins).into_iter().next()
                && let Some(page_rect) = page_rects.get(page)
            {
                // La barre ne recouvre jamais un autre bloc (un clic destiné au
                // voisin tomberait sur un de ses boutons) : elle se pose dans le
                // bloc lui-même s'il est assez haut, sinon à côté de la page.
                let clip = ui.clip_rect();
                let right = (page_rect.left() + (x1 + 5.0).max(margins.left + 120.0) * scale).min(clip.right() - 4.0);
                let band_top = page_rect.top() + (top - 4.0) * scale;
                let band_bottom = page_rect.top() + (bottom + 3.0) * scale;
                let visible_top = band_top.max(clip.top() + 4.0);
                // Sans place sûre, pas de barre : le panneau et le clic droit restent.
                let placed = if band_bottom - visible_top >= 44.0 {
                    Some((egui::pos2(right - 4.0, visible_top + 4.0), egui::Align2::RIGHT_TOP))
                } else if clip.right() - page_rect.right() >= 380.0 {
                    Some((egui::pos2(page_rect.right() + 40.0, visible_top), egui::Align2::LEFT_TOP))
                } else {
                    None
                };
                if let Some((pos, pivot)) = placed
                    && clip.contains(pos)
                {
                    toolbar(app, ui.ctx(), id, pos, pivot, &mut action);
                }
            }
        });
        ui.add_space(40.0);
    });
    let offset = output.state.offset.y;
    action.viewport = Viewport { offset, height: output.inner_rect.height() };
    // Premier bloc dont le haut est à l'écran : la vue s'y accrochera.
    action.anchor = positions.iter().find_map(|p| {
        let y = block_y(&p.id)?;
        (y >= offset).then(|| (p.id.clone(), y - offset))
    });
    action
}

/// Une action survolée (barre, clic droit, panneau) : on montre ce qui va
/// bouger avant le clic.
pub fn hover_action(ctx: &egui::Context, id: &BlockId, quick: Quick) {
    let key = egui::Id::new("apercu-action");
    let frame = ctx.cumulative_frame_nr();
    let previous: Option<(BlockId, Quick, u64)> = ctx.data(|d| d.get_temp(key));
    if previous.as_ref().is_none_or(|(i, q, _)| i != id || *q != quick) {
        ctx.request_repaint();
    }
    ctx.data_mut(|d| d.insert_temp(key, (id.clone(), quick, frame)));
}

/// L'action survolée à l'image précédente (ou à celle-ci).
fn hovered_action(ctx: &egui::Context) -> Option<(BlockId, Quick)> {
    let (id, quick, frame): (BlockId, Quick, u64) = ctx.data(|d| d.get_temp(egui::Id::new("apercu-action")))?;
    // On revérifie bientôt : l'aperçu s'efface quand le pointeur s'en va.
    ctx.request_repaint_after(std::time::Duration::from_millis(120));
    (frame + 1 >= ctx.cumulative_frame_nr()).then_some((id, quick))
}

/// Ce que l'action va changer : la page, la zone touchée (haut, bas, en
/// points) et une phrase.
fn preview(
    app: &NectarApp,
    rendered: &crate::worker::Layouted,
    id: &BlockId,
    quick: Quick,
    children: &HashSet<BlockId>,
    views: &[PageView],
    margins: &Margins,
) -> Option<(usize, f32, f32, String)> {
    let project = app.project.as_ref()?;
    let ops = project.layout.ops_for(&project.document, id);
    let positions = &rendered.positions;
    let (page, [_, top, _, bottom]) = bands(rendered, id, children, views, margins).into_iter().next()?;
    let content_bottom = |page: usize| {
        rendered.boxes.iter().filter(|b| b.page == page).map(|b| b.rect[3] as f32).fold(margins.top + 14.0, f32::max)
    };
    let auto_landscape =
        rendered.choices.iter().any(|c| &c.block == id && c.kind == nectar_core::auto::ChoiceKind::Landscape);
    Some(match quick {
        Quick::BreakBefore if ops.break_before => (page, top, bottom, "↑ Le bloc revient à sa place".into()),
        Quick::BreakBefore => {
            // Les titres qui le précèdent sur la page partent avec lui.
            let blocks = &project.document.blocks;
            let mut start = (page, top);
            if let Some(index) = blocks.iter().position(|b| &b.id == id) {
                for previous in blocks[..index].iter().rev() {
                    let Some(p) = positions.iter().find(|p| p.id == previous.id) else { break };
                    if !matches!(previous.node, Node::Heading { .. }) || p.page != page {
                        break;
                    }
                    start = (p.page, p.y as f32 - 2.0);
                }
            }
            if start.1 <= margins.top + 4.0 {
                (page, top, bottom, "Déjà en haut de la page : rien ne bouge".into())
            } else {
                (page, start.1, content_bottom(page), format!("↓ Tout ceci passe en haut de la page {}", page + 2))
            }
        }
        Quick::BreakAfter if ops.break_after => (page, top, bottom, "La suite remonte sur cette page".into()),
        Quick::BreakAfter => {
            let next = positions
                .iter()
                .skip_while(|p| &p.id != id)
                .skip(1)
                .find(|p| !children.contains(&p.id) && (p.page > page || p.y as f32 > bottom - 2.0))?;
            if next.page != page {
                (page, top, bottom, "Déjà en bas de page : rien ne bouge".into())
            } else {
                let y = next.y as f32 - 2.0;
                (
                    page,
                    y,
                    content_bottom(page),
                    format!("↓ La suite passe page {} ; ce bas de page reste vide", page + 2),
                )
            }
        }
        Quick::Landscape
            if auto_landscape
                || ops.image.as_ref().is_some_and(|i| i.placement == nectar_core::layout::Placement::Landscape) =>
        {
            (page, top, bottom, "↺ Revient dans le texte, en portrait".into())
        }
        Quick::Landscape => (page, top, bottom, "→ Seul sur une page paysage, en grand".into()),
        Quick::AsIs if auto_landscape => (page, top, bottom, "↺ Revient dans le texte, en portrait".into()),
        Quick::AsIs => (page, top, bottom, "Le placement automatique n'y touchera plus".into()),
        Quick::FullPage => (page, top, bottom, "→ Seul sur sa page, aussi grand que possible".into()),
        Quick::KeepWithNext => {
            let next_bottom = positions
                .iter()
                .skip_while(|p| &p.id != id)
                .skip(1)
                .find(|p| !children.contains(&p.id))
                .and_then(|n| bands(rendered, &n.id, children, views, margins).into_iter().find(|(p, _)| *p == page))
                .map(|(_, r)| r[3])
                .unwrap_or(bottom);
            (page, top, next_bottom, "Ces deux blocs resteront sur la même page".into())
        }
        Quick::KeepTogether => (page, top, bottom, "Restera entier, sur une seule page".into()),
        Quick::Clear => (page, top, bottom, "Toutes les retouches de ce bloc seront retirées".into()),
    })
}

/// Surligne une zone d'une page (du haut `top` au bas `bottom`, en points)
/// et dit, dans une étiquette, ce qui va lui arriver.
#[allow(clippy::too_many_arguments)]
fn paint_region(
    ui: &egui::Ui,
    page_rect: Rect,
    view: &PageView,
    top: f32,
    bottom: f32,
    text: &str,
    scale: f32,
    margins: &Margins,
    t: theme::Tokens,
) {
    let r = Rect::from_min_max(
        page_rect.min + egui::vec2((margins.left - 8.0) * scale, (top - 4.0) * scale),
        page_rect.min + egui::vec2((view.size_pt.x - margins.right + 8.0) * scale, (bottom + 4.0) * scale),
    );
    let painter = ui.painter_at(page_rect);
    painter.rect_filled(r, 0.0, t.accent.gamma_multiply(0.22));
    painter.rect_stroke(r, 0.0, Stroke::new(2.0, t.identity), StrokeKind::Outside);
    let font = egui::FontId::new(12.5, theme::strong());
    let galley = painter.layout_no_wrap(text.to_string(), font, t.accent_ink);
    let pill = Rect::from_min_size(
        egui::pos2(r.right() - galley.size().x - 16.0, (r.top() - galley.size().y - 8.0).max(page_rect.top() + 2.0)),
        galley.size() + egui::vec2(12.0, 6.0),
    );
    painter.rect_filled(pill, 0.0, t.accent);
    painter.rect_stroke(pill, 0.0, Stroke::new(1.0, t.ink), StrokeKind::Inside);
    painter.galley(pill.min + egui::vec2(6.0, 3.0), galley, t.accent_ink);
}

/// Le numéro cliquable d'une page, à sa droite.
fn tag_rect(page: Rect) -> Rect {
    Rect::from_min_size(page.right_top() + egui::vec2(6.0, 0.0), egui::vec2(26.0, 18.0))
}

/// La barre des actions rapides du bloc sélectionné.
fn toolbar(
    app: &NectarApp,
    ctx: &egui::Context,
    id: &BlockId,
    pos: Pos2,
    pivot: egui::Align2,
    action: &mut PageAction,
) {
    let t = theme::tokens(ctx);
    egui::Area::new(egui::Id::new("barre-bloc")).fixed_pos(pos).pivot(pivot).order(egui::Order::Foreground).show(
        ctx,
        |ui| {
            egui::Frame::new()
                .fill(t.raised)
                .stroke(Stroke::new(1.0, t.ink))
                .inner_margin(egui::Margin::symmetric(4, 3))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    ui.spacing_mut().button_padding = egui::vec2(7.0, 3.0);
                    ui.horizontal(|ui| {
                        for offer in actions::offers(app, id) {
                            if offer.quick == Quick::Clear {
                                continue;
                            }
                            let text = egui::RichText::new(offer.label).size(12.5);
                            let text = if offer.active { text.color(t.accent_ink) } else { text.color(t.ink) };
                            let button = egui::Button::new(text)
                                .fill(if offer.active { t.accent } else { t.raised })
                                .stroke(Stroke::NONE);
                            let button = ui.add(button).on_hover_text(offer.hint);
                            if button.hovered() {
                                hover_action(ui.ctx(), id, offer.quick);
                            }
                            if button.clicked() {
                                action.quick = Some((id.clone(), offer.quick));
                            }
                        }
                        let more = egui::Button::new(egui::RichText::new("Plus…").size(12.5).color(t.identity))
                            .fill(t.raised)
                            .stroke(Stroke::NONE);
                        if ui.add(more).on_hover_text("Toutes les retouches de ce bloc").clicked() {
                            action.more = true;
                        }
                    });
                });
        },
    );
}

/// Peint le déplacement d'un bloc : tout ce qui est sous son haut glisse de
/// `dy`, la place libérée reprend la couleur de la page.
#[allow(clippy::too_many_arguments)]
fn paint_ghost(
    ui: &egui::Ui,
    ghost: &Ghost,
    view: &PageView,
    page_rect: Rect,
    texture: &egui::TextureHandle,
    scale: f32,
    margins: &Margins,
    paper: Color32,
    t: theme::Tokens,
    live: bool,
) {
    let size = view.size_pt;
    let top = (ghost.top - 4.0).max(0.0);
    let body_bottom = size.y - margins.bottom;
    if body_bottom <= top {
        return;
    }
    let screen = |x: f32, y: f32| page_rect.min + egui::vec2(x * scale, y * scale);
    let clip = Rect::from_min_max(screen(0.0, (top + ghost.dy.min(0.0)).max(0.0)), screen(size.x, body_bottom));
    let painter = ui.painter_at(clip.intersect(page_rect));
    painter.rect_filled(clip, 0.0, paper);
    let uv = Rect::from_min_max(Pos2::new(0.0, top / size.y), Pos2::new(1.0, body_bottom / size.y));
    let dest = Rect::from_min_max(screen(0.0, top + ghost.dy), screen(size.x, body_bottom + ghost.dy));
    painter.image(texture.id(), dest, uv, Color32::WHITE);
    if live {
        // Le nouveau haut du bloc et le décalage, au millimètre.
        let y = screen(0.0, top + 4.0 + ghost.dy).y;
        ui.painter_at(page_rect).hline(page_rect.x_range(), y, Stroke::new(1.0, t.identity));
        let mm = (ghost.dy / MM * 2.0).round() / 2.0;
        let text = format!("{}{} mm", if mm > 0.0 { "+" } else { "" }, format!("{mm:.1}").replace('.', ","));
        let galley = ui.painter().layout_no_wrap(text, egui::FontId::new(12.0, theme::mono()), t.accent_ink);
        let at = egui::pos2(page_rect.right() - galley.size().x - 18.0, y - galley.size().y - 8.0);
        let tag = Rect::from_min_size(at, galley.size() + egui::vec2(10.0, 4.0));
        ui.painter().rect_filled(tag, 0.0, t.accent);
        ui.painter().galley(at + egui::vec2(5.0, 2.0), galley, t.accent_ink);
    }
}

/// Les retouches d'un bloc, en mots.
fn summary(ops: &nectar_core::BlockOps) -> Vec<String> {
    use nectar_core::layout::{PageChange, Placement};
    let mut out = Vec::new();
    if ops.break_before {
        out.push("Nouvelle page avant".into());
    }
    if ops.break_after {
        out.push("Reste de la page vide après".into());
    }
    if ops.keep_with_next {
        out.push("Gardé avec le suivant".into());
    }
    match ops.keep_together {
        Some(true) => out.push("Insécable".into()),
        Some(false) => out.push("Coupure autorisée".into()),
        None => {}
    }
    if ops.push_to_bottom {
        out.push("Poussé en bas de page".into());
    }
    if let Some(mm) = ops.space_before_mm {
        out.push(format!("Espace avant {mm} mm"));
    }
    match &ops.page {
        Some(PageChange::Set(spec)) => out.push(format!(
            "Format {}{} ({})",
            spec.paper.to_uppercase(),
            if spec.landscape { " paysage" } else { "" },
            if ops.page_onward { "et pages suivantes" } else { "cette page" }
        )),
        Some(PageChange::Default(_)) => out.push("Retour au format du document".into()),
        None => {}
    }
    if let Some(image) = &ops.image {
        if let Some(w) = image.width_percent {
            out.push(format!("Image à {w:.0} %"));
        }
        match image.placement {
            Placement::Top => out.push("Image en haut de page".into()),
            Placement::Bottom => out.push("Image en bas de page".into()),
            Placement::FullPage => out.push("Image pleine page".into()),
            Placement::Landscape => out.push("Image sur une page paysage".into()),
            Placement::Inline => {}
        }
    }
    if ops.style.is_some() {
        out.push("Apparence propre".into());
    }
    if ops.table.is_some() {
        out.push("Colonnes réglées".into());
    }
    if ops.hidden {
        out.push("Masqué".into());
    }
    out
}

struct Margins {
    top: f32,
    bottom: f32,
    left: f32,
    right: f32,
}

impl From<&nectar_core::Style> for Margins {
    fn from(style: &nectar_core::Style) -> Self {
        let mm = 72.0 / 25.4;
        Self {
            top: style.page.margin_top_mm * mm,
            bottom: style.page.margin_bottom_mm * mm,
            left: style.page.margin_left_mm * mm,
            right: style.page.margin_right_mm * mm,
        }
    }
}

/// Le bloc sous un point d'une page : le dernier marqueur placé avant lui.
fn hit(positions: &[BlockPosition], page: usize, y: f32) -> Option<BlockId> {
    positions.iter().rfind(|p| p.page < page || (p.page == page && p.y as f32 <= y + 3.0)).map(|p| p.id.clone())
}

/// Ids des puces de chaque liste : une liste s'étend au-delà de ses puces.
fn list_children(app: &NectarApp) -> HashSet<BlockId> {
    let mut set = HashSet::new();
    if let Some(project) = &app.project {
        for block in &project.document.blocks {
            if let Node::List(list) = &block.node {
                set.extend(list.items.iter().filter_map(|i| i.id.clone()));
            }
        }
    }
    set
}

/// Boîtes réelles d'un bloc (et de ses puces pour une liste), par page ;
/// à défaut, la bande entre son marqueur et le suivant.
fn bands(
    rendered: &crate::worker::Layouted,
    id: &BlockId,
    children: &HashSet<BlockId>,
    views: &[PageView],
    margins: &Margins,
) -> Vec<(usize, [f32; 4])> {
    let is_list = id.as_str().starts_with("list-");
    let list_items: HashSet<&BlockId> = if is_list {
        // Les puces de cette liste : les marqueurs qui la suivent tant qu'ils sont des puces.
        let start = rendered.positions.iter().position(|p| &p.id == id).unwrap_or(0);
        rendered.positions[start + 1..].iter().take_while(|p| children.contains(&p.id)).map(|p| &p.id).collect()
    } else {
        HashSet::new()
    };
    let mut pages: Vec<(usize, [f32; 4])> = Vec::new();
    for b in rendered.boxes.iter().filter(|b| &b.id == id || list_items.contains(&b.id)) {
        let r = [b.rect[0] as f32, b.rect[1] as f32, b.rect[2] as f32, b.rect[3] as f32];
        match pages.iter_mut().find(|(p, _)| *p == b.page) {
            Some((_, u)) => *u = [u[0].min(r[0]), u[1].min(r[1]), u[2].max(r[2]), u[3].max(r[3])],
            None => pages.push((b.page, r)),
        }
    }
    if pages.is_empty() {
        return extent(&rendered.positions, id, children, views, margins)
            .into_iter()
            .map(|(page, top, bottom)| {
                let width = views.get(page).map(|v| v.size_pt.x).unwrap_or(595.0);
                (page, [margins.left, top, width - margins.right, bottom])
            })
            .collect();
    }
    pages.sort_by_key(|(p, _)| *p);
    pages
}

/// Les bandes (page, haut, bas) occupées par un bloc, en points.
fn extent(
    positions: &[BlockPosition],
    id: &BlockId,
    children: &HashSet<BlockId>,
    views: &[PageView],
    margins: &Margins,
) -> Vec<(usize, f32, f32)> {
    let Some(start) = positions.iter().position(|p| &p.id == id) else { return Vec::new() };
    let first = &positions[start];
    let is_list = id.as_str().starts_with("list-");
    let end = positions[start + 1..].iter().find(|p| {
        let later = p.page > first.page || p.y > first.y + 0.5;
        later && !(is_list && children.contains(&p.id))
    });
    let page_bottom = |page: usize| views.get(page).map(|v| v.size_pt.y - margins.bottom).unwrap_or(0.0);
    let (end_page, end_y) = match end {
        Some(p) => (p.page, p.y as f32),
        None => {
            let last = views.len().saturating_sub(1);
            (last, page_bottom(last))
        }
    };
    let mut bands = Vec::new();
    for page in first.page..=end_page.min(views.len().saturating_sub(1)) {
        let top = if page == first.page { first.y as f32 } else { margins.top };
        let bottom = if page == end_page { end_y - 2.0 } else { page_bottom(page) };
        if bottom > top + 1.0 {
            bands.push((page, top, bottom));
        }
    }
    if bands.is_empty() {
        bands.push((first.page, first.y as f32, first.y as f32 + 14.0));
    }
    bands
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos(id: &str, page: usize, y: f64) -> BlockPosition {
        BlockPosition { id: BlockId(id.into()), page, x: 0.0, y }
    }

    #[test]
    fn hit_takes_the_last_marker_above_the_pointer() {
        let positions = vec![pos("p-1", 0, 100.0), pos("list-1", 0, 300.0), pos("li-1", 0, 300.0), pos("p-2", 1, 80.0)];
        assert_eq!(hit(&positions, 0, 50.0), None);
        assert_eq!(hit(&positions, 0, 200.0).unwrap().as_str(), "p-1");
        assert_eq!(hit(&positions, 0, 310.0).unwrap().as_str(), "li-1");
        // Haut de la page 2 : le bloc commencé sur la page 1 continue.
        assert_eq!(hit(&positions, 1, 40.0).unwrap().as_str(), "li-1");
    }
}
