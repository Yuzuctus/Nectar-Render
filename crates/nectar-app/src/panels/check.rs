//! L'onglet « Vérifier » : ce que l'assistant a repéré dans les pages.

use eframe::egui::{self, FontId, RichText};
use nectar_core::assistant::{FixAction, Issue, Severity};
use nectar_core::layout::Placement;

use crate::app::NectarApp;
use crate::theme::{self, kicker};

/// Ce que le placement automatique a décidé seul, bloc par bloc.
fn automatic(app: &mut NectarApp, ui: &mut egui::Ui, choices: &[nectar_core::auto::Choice]) {
    let t = theme::tokens(ui.ctx());
    egui::CollapsingHeader::new(
        RichText::new(format!("Fait automatiquement ({})", choices.len())).font(FontId::new(14.0, theme::strong())),
    )
    .id_salt("fait-automatiquement")
    .default_open(false)
    .show(ui, |ui| {
        ui.label(
            RichText::new(
                "Nectar a pris ces décisions seul. Pour en refuser une : « Voir », puis « Garder tel quel » ; pour toutes : Style › Placement automatique.",
            )
            .small()
            .color(t.faint),
        );
        ui.add_space(4.0);
        let anchors: Vec<(nectar_core::BlockId, String)> = app
            .project
            .as_ref()
            .map(|p| p.document.anchors().iter().map(|a| (a.id.clone(), a.excerpt.to_string())).collect())
            .unwrap_or_default();
        let mut select = None;
        for choice in choices {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("✓").color(t.identity));
                if choice.kind.global() {
                    ui.label(choice.describe());
                    return;
                }
                let excerpt = anchors.iter().find(|(id, _)| id == &choice.block).map(|(_, e)| e.as_str()).unwrap_or("");
                let short: String = excerpt.chars().take(40).collect();
                ui.label(format!("{} — « {short}{} »", choice.describe(), if excerpt.chars().count() > 40 { "…" } else { "" }));
                if ui.small_button("Voir").clicked() {
                    select = Some(choice.block.clone());
                }
            });
        }
        if let Some(block) = select {
            app.select(Some(block), true);
        }
    });
    theme::rule(ui, 1.0, false);
}

pub fn show(app: &mut NectarApp, ui: &mut egui::Ui) {
    let t = theme::tokens(ui.ctx());
    let issues: Vec<Issue> = app.rendered.as_ref().map(|r| r.issues.clone()).unwrap_or_default();
    let choices: Vec<nectar_core::auto::Choice> = app.rendered.as_ref().map(|r| r.choices.clone()).unwrap_or_default();
    let remarks = issues.iter().filter(|i| i.severity > Severity::Info).count();
    if remarks == 0 {
        // Rien à corriger : on le dit franchement, et la suite logique est là.
        egui::Frame::new().fill(t.surface).stroke(egui::Stroke::new(1.0, t.identity)).inner_margin(12).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new("✓ Les pages sont propres").font(FontId::new(15.0, theme::strong())).color(t.identity),
            );
            ui.label(RichText::new("Rien à corriger : le PDF est prêt.").color(t.muted));
            ui.add_space(4.0);
            let export = egui::Button::new(RichText::new("Exporter le PDF").color(t.paper)).fill(t.ink);
            if ui.add(export).on_hover_text("Ctrl + E").clicked() {
                app.export();
            }
        });
        ui.add_space(10.0);
    }
    if !issues.is_empty() {
        kicker(ui, &if remarks > 0 { format!("À voir ({remarks})") } else { "Remarques".to_string() });
        ui.add_space(2.0);
        ui.label(RichText::new("« Voir » montre le bloc ; l'autre bouton corrige (annulable).").small().color(t.faint));
        ui.add_space(6.0);
        list(app, ui, &issues);
        ui.add_space(10.0);
    }
    if !choices.is_empty() {
        automatic(app, ui, &choices);
    }
}

/// Les remarques de l'assistant, chacune avec ses corrections.
fn list(app: &mut NectarApp, ui: &mut egui::Ui, issues: &[Issue]) {
    let t = theme::tokens(ui.ctx());
    let ctx = ui.ctx().clone();
    // Plusieurs schémas à lire en grand : tous d'un coup.
    let landscapes: Vec<nectar_core::assistant::Fix> = issues
        .iter()
        .flat_map(|i| &i.fixes)
        .filter(|f| f.action == FixAction::ImagePlacement(Placement::Landscape))
        .cloned()
        .collect();
    if landscapes.len() > 1
        && ui
            .add(
                egui::Button::new(
                    RichText::new(format!("Mettre les {} schémas en paysage", landscapes.len())).color(t.accent_ink),
                )
                .fill(t.accent),
            )
            .on_hover_text("Chacun sur sa page paysage, en grand ; le texte continue autour")
            .clicked()
    {
        let ids: Vec<nectar_core::BlockId> = landscapes.iter().map(|f| f.block.clone()).collect();
        app.edit_blocks(&ctx, &ids, |id, ops| {
            if let Some(fix) = landscapes.iter().find(|f| &f.block == id) {
                fix.apply(ops);
            }
        });
        app.notify_done(format!("{} schémas mis en paysage", landscapes.len()));
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
                            let button =
                                egui::Button::new(RichText::new(&fix.label).color(t.accent_ink)).fill(t.accent);
                            if ui.add(button).clicked() {
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
        app.notify_done(format!("Fait : {}", fix.label));
    }
}
