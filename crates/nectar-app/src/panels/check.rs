//! L'onglet « Vérifier » : ce que l'assistant a repéré dans les pages.

use eframe::egui::{self, FontId, RichText};
use nectar_core::assistant::{FixAction, Issue, Severity};
use nectar_core::layout::Placement;

use crate::app::NectarApp;
use crate::theme::{self, kicker};

pub fn show(app: &mut NectarApp, ui: &mut egui::Ui) {
    let t = theme::tokens(ui.ctx());
    let ctx = ui.ctx().clone();
    let issues: Vec<Issue> = app.rendered.as_ref().map(|r| r.issues.clone()).unwrap_or_default();
    kicker(ui, "Assistant de mise en page");
    ui.add_space(4.0);
    if issues.is_empty() {
        ui.label(RichText::new("Rien à signaler : les pages sont propres.").color(t.identity));
        return;
    }
    ui.label(
        RichText::new("Ce que Nectar a repéré dans les pages. « Voir » sélectionne le bloc ; les boutons appliquent une retouche (annulable).")
            .small()
            .color(t.faint),
    );
    ui.add_space(8.0);
    // Plusieurs schémas à lire en grand : tous d'un coup.
    let landscapes: Vec<nectar_core::assistant::Fix> = issues
        .iter()
        .flat_map(|i| &i.fixes)
        .filter(|f| f.action == FixAction::ImagePlacement(Placement::Landscape))
        .cloned()
        .collect();
    if landscapes.len() > 1
        && ui
            .button(format!("Mettre les {} schémas en paysage", landscapes.len()))
            .on_hover_text("Chacun sur sa page paysage, en grand ; le texte continue autour")
            .clicked()
    {
        for fix in &landscapes {
            app.edit_block(&ctx, &fix.block, |ops| fix.apply(ops));
        }
        app.notify(format!("{} schémas mis en paysage", landscapes.len()), false);
        return;
    }
    let mut select = None;
    let mut apply = None;
    for (index, issue) in issues.iter().enumerate() {
        let (mark, color) = match issue.severity {
            Severity::Problem => ("✗", t.danger),
            Severity::Warning => ("!", ui.visuals().warn_fg_color),
            Severity::Info => ("·", t.faint),
        };
        egui::Frame::new().inner_margin(egui::Margin::symmetric(0, 6)).show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.label(RichText::new(mark).font(FontId::new(15.0, theme::strong())).color(color));
                ui.vertical(|ui| {
                    ui.label(RichText::new(&issue.title).font(FontId::new(13.5, theme::strong())));
                    if !issue.detail.is_empty() {
                        ui.label(RichText::new(&issue.detail).color(t.muted));
                    }
                    ui.horizontal_wrapped(|ui| {
                        if let Some(block) = &issue.block
                            && ui.small_button("Voir").clicked()
                        {
                            select = Some(block.clone());
                        }
                        for (k, fix) in issue.fixes.iter().enumerate() {
                            if ui.small_button(&fix.label).clicked() {
                                apply = Some((index, k));
                            }
                        }
                    });
                });
            });
        });
        theme::rule(ui, 1.0, false);
    }
    if let Some(block) = select {
        app.select(Some(block), true);
    }
    if let Some((index, k)) = apply {
        let fix = issues[index].fixes[k].clone();
        app.edit_block(&ctx, &fix.block, |ops| fix.apply(ops));
        app.notify(format!("Retouche appliquée : {}", fix.label), false);
    }
}
