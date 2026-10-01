//! La colonne de gauche : tous les blocs de la note, dans l'ordre.

use eframe::egui::{self, FontId, RichText, Sense};
use nectar_core::model::BlockKind;

use crate::app::NectarApp;
use crate::theme::{self, kicker};

pub fn show(app: &mut NectarApp, ui: &mut egui::Ui) {
    let t = theme::tokens(ui.ctx());
    let Some(project) = &app.project else { return };
    kicker(ui, "Blocs");
    ui.add_space(2.0);
    theme::rule(ui, 2.0, true);
    ui.add_space(4.0);

    let retouched: std::collections::HashSet<_> = project.layout.resolve(&project.document).ops.into_keys().collect();
    let anchors: Vec<_> =
        project.document.anchors().into_iter().map(|a| (a.id.clone(), a.kind, a.excerpt.to_string())).collect();
    let mut clicked = None;
    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        for (id, kind, excerpt) in &anchors {
            let selected = app.selected.as_ref() == Some(id);
            let indent = if *kind == BlockKind::ListItem { 14.0 } else { 0.0 };
            let response = egui::Frame::new()
                .fill(if selected { t.surface } else { t.paper })
                .inner_margin(egui::Margin { left: (6.0 + indent) as i8, right: 6, top: 5, bottom: 5 })
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let label = RichText::new(kind.label_fr().to_uppercase())
                            .font(FontId::new(9.5, theme::mono()))
                            .color(if selected { t.identity } else { t.faint });
                        ui.label(label);
                        if retouched.contains(id) {
                            ui.label(RichText::new("•").color(t.identity).strong())
                                .on_hover_text("Ce bloc a des retouches");
                        }
                    });
                    let text = if excerpt.is_empty() { "—".to_string() } else { excerpt.clone() };
                    ui.add(
                        egui::Label::new(RichText::new(text).color(if selected { t.ink } else { t.muted })).truncate(),
                    );
                })
                .response
                .interact(Sense::click());
            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if response.clicked() {
                clicked = Some(id.clone());
            }
            let rect = response.rect;
            ui.painter().hline(rect.x_range(), rect.bottom(), egui::Stroke::new(1.0, t.rule));
        }
    });
    if let Some(id) = clicked {
        app.select(Some(id), true);
    }
}
