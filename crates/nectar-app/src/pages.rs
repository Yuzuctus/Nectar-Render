//! L'aperçu des pages : on y voit le PDF, on y désigne les blocs.

use std::collections::HashSet;

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, StrokeKind};
use nectar_core::BlockId;
use nectar_core::model::Node;
use nectar_typst::BlockPosition;

use crate::app::NectarApp;
use crate::theme;

pub struct PageView {
    /// Image de la page, éventuellement à une autre résolution (zoom en cours)
    /// ou absente (pas encore rendue).
    pub texture: Option<egui::TextureHandle>,
    pub size_pt: egui::Vec2,
}

#[derive(Default)]
pub struct PageAction {
    /// `Some(None)` : clic dans le vide (désélection).
    pub clicked: Option<Option<BlockId>>,
    /// Pages au moins en partie visibles.
    pub visible: Vec<usize>,
    /// Nouvelle largeur (en %) donnée à l'image sélectionnée avec la poignée.
    pub resize: Option<(BlockId, f32)>,
}

/// Points typographiques → points d'écran à 100 % (96 ppp).
const PT_TO_SCREEN: f32 = 96.0 / 72.0;
const TOP_GAP: f32 = 24.0;

pub fn show(
    app: &NectarApp,
    ui: &mut egui::Ui,
    views: &[PageView],
    zoom: f32,
    scroll_to: Option<BlockId>,
    reset_horizontal: bool,
) -> PageAction {
    let t = theme::tokens(ui.ctx());
    let mut action = PageAction::default();
    let Some(rendered) = &app.rendered else { return action };
    let positions = &rendered.positions;
    let scale = zoom * PT_TO_SCREEN;
    let margins = Margins::from(&app.style);
    let children = list_children(app);

    // Défilement vers un bloc : on calcule le décalage vertical d'avance,
    // sans toucher au défilement horizontal.
    let mut area = egui::ScrollArea::both().auto_shrink(false);
    if reset_horizontal {
        area = area.horizontal_scroll_offset(0.0);
    }
    if let Some(target) = &scroll_to
        && let Some(position) = positions.iter().find(|p| &p.id == target)
    {
        let above: f32 = views.iter().take(position.page).map(|v| v.size_pt.y * scale + 20.0).sum();
        area =
            area.vertical_scroll_offset((TOP_GAP + above + (position.y as f32 - 40.0).max(0.0) * scale - 8.0).max(0.0));
    }
    area.show(ui, |ui| {
        ui.add_space(TOP_GAP);
        ui.vertical_centered(|ui| {
            let mut hovered: Option<BlockId> = None;
            let mut page_rects = Vec::with_capacity(views.len());
            for (index, view) in views.iter().enumerate() {
                let size = view.size_pt * scale;
                let (rect, response) = ui.allocate_exact_size(size, Sense::click());
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
                    // Pas encore rendue : une page blanche le temps du rendu.
                    None => {
                        painter.rect_filled(rect, 0.0, Color32::WHITE);
                    }
                }

                let to_pt = |p: Pos2| ((p.x - rect.left()) / scale, (p.y - rect.top()) / scale);
                if let Some(pointer) = response.hover_pos() {
                    let (_, y) = to_pt(pointer);
                    hovered = hit(positions, index, y);
                    if hovered.is_some() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                }
                if response.clicked()
                    && let Some(pointer) = response.interact_pointer_pos()
                {
                    let (_, y) = to_pt(pointer);
                    action.clicked = Some(hit(positions, index, y));
                }

                // Repère de page.
                let label = format!("{}", index + 1);
                painter.text(
                    rect.right_top() + egui::vec2(8.0, 0.0),
                    egui::Align2::LEFT_TOP,
                    label,
                    egui::FontId::new(10.5, theme::mono()),
                    t.faint,
                );
                ui.add_space(20.0);
            }

            let paint = |id: &BlockId, fill: Color32, stroke: Stroke| {
                for (page, [x0, y0, x1, y1]) in bands(rendered, id, &children, views, &margins) {
                    let Some(page_rect) = page_rects.get(page) else { continue };
                    let r = Rect::from_min_max(
                        page_rect.min + egui::vec2((x0 - 5.0) * scale, (y0 - 4.0) * scale),
                        page_rect.min + egui::vec2((x1 + 5.0) * scale, (y1 + 3.0) * scale),
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

            // Repères des retouches dans la marge gauche.
            if let Some(project) = &app.project {
                let ops = project.layout.resolve(&project.document).ops;
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
        });
        ui.add_space(40.0);
    });
    action
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
        Some(PageChange::Set(spec)) => {
            out.push(format!("Format {}{}", spec.paper.to_uppercase(), if spec.landscape { " paysage" } else { "" }))
        }
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
