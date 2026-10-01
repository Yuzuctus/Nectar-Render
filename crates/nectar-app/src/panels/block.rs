//! Les retouches du bloc sélectionné : sauts, format de page, image.

use eframe::egui::{self, FontId, RichText};
use nectar_core::layout::{BlockOps, DefaultPage, HAlign, ImageOps, PageChange, PageSpec, Placement};
use nectar_core::model::{AnchorInfo, BlockKind, Node};

use super::widgets::{self, choice, grid, label, section};
use crate::app::NectarApp;
use crate::theme::{self, kicker};

const PAPERS: &[(&str, &str)] =
    &[("a4", "A4"), ("a3", "A3"), ("a5", "A5"), ("us-letter", "Letter"), ("us-legal", "Legal"), ("a2", "A2")];

#[derive(Clone, PartialEq)]
enum PageChoice {
    Unchanged,
    Default,
    Paper(String),
    Custom,
}

pub fn show(app: &mut NectarApp, ui: &mut egui::Ui) {
    let t = theme::tokens(ui.ctx());
    let ctx = ui.ctx().clone();
    let Some(project) = &app.project else { return };
    let Some(id) = app.selected.clone() else {
        kicker(ui, "Aucun bloc");
        ui.add_space(4.0);
        ui.label(
            RichText::new("Clique sur un bloc dans les pages, ou dans la liste à gauche, pour le retoucher.")
                .color(t.muted),
        );
        ui.add_space(12.0);
        document_section(app, ui);
        return;
    };
    let anchors = project.document.anchors();
    let Some(anchor) = anchors.iter().find(|a| a.id == &id).copied() else {
        // Le bloc a disparu de la note : on revient à « aucune sélection ».
        app.selected = None;
        ui.ctx().request_repaint();
        return;
    };
    let (kind, line, excerpt) = (anchor.kind, anchor.line, anchor.excerpt.to_string());
    let is_figure =
        project.document.blocks.iter().any(|b| b.id == id && matches!(b.node, Node::Figure(_) | Node::Diagram { .. }));
    let inline_ops = project.document.blocks.iter().find(|b| b.id == id).and_then(|b| b.inline_ops.clone());
    let current: BlockOps = project.layout.ops_for(&project.document, &id);
    let position = app.rendered.as_ref().and_then(|r| r.positions.iter().find(|p| p.id == id)).map(|p| p.page + 1);

    // En-tête du bloc.
    kicker(ui, kind.label_fr());
    ui.add_space(2.0);
    ui.label(
        RichText::new(if excerpt.is_empty() { "—".into() } else { excerpt.clone() })
            .font(FontId::new(15.0, theme::strong())),
    );
    let mut meta = format!("ligne {line}");
    if let Some(page) = position {
        meta.push_str(&format!(" · page {page}"));
    }
    meta.push_str(&format!(" · {id}"));
    ui.label(RichText::new(meta).font(FontId::new(10.5, theme::mono())).color(t.faint));
    if inline_ops.is_some() {
        ui.add_space(4.0);
        ui.label(
            RichText::new("Retouches écrites dans la note (<!-- nectar: … -->) : les modifier ici les remplace.")
                .color(t.muted)
                .small(),
        );
    }
    ui.add_space(8.0);

    let mut ops = current.clone();
    section(ui, "Pages", true, |ui| {
        ui.checkbox(&mut ops.break_before, "Commencer une nouvelle page avant");
        ui.checkbox(&mut ops.break_after, "Laisser le reste de la page vide après");
        if kind != BlockKind::ListItem {
            ui.checkbox(&mut ops.keep_with_next, "Garder avec le bloc suivant");
        }
        ui.checkbox(&mut ops.push_to_bottom, "Pousser en bas de la page");
        ui.horizontal(|ui| {
            let mut has_space = ops.space_before_mm.is_some();
            if ui.checkbox(&mut has_space, "Espace avant").changed() {
                ops.space_before_mm = has_space.then_some(10.0);
            }
            if let Some(mm) = &mut ops.space_before_mm {
                widgets::number(ui, mm, -50.0..=200.0, 0.5, " mm");
            }
        });
    });

    section(ui, "Format de page", true, |ui| {
        let mut choice_now = match &ops.page {
            None => PageChoice::Unchanged,
            Some(PageChange::Default(_)) => PageChoice::Default,
            Some(PageChange::Set(spec)) if spec.width_mm.is_some() => PageChoice::Custom,
            Some(PageChange::Set(spec)) => PageChoice::Paper(spec.paper.clone()),
        };
        let mut options: Vec<(PageChoice, &str)> =
            vec![(PageChoice::Unchanged, "Inchangé"), (PageChoice::Default, "Revenir au format du document")];
        options.extend(PAPERS.iter().map(|(id, label)| (PageChoice::Paper((*id).to_string()), *label)));
        options.push((PageChoice::Custom, "Personnalisé…"));
        if choice(ui, "format-bloc", &mut choice_now, &options) {
            let landscape = matches!(&ops.page, Some(PageChange::Set(s)) if s.landscape);
            ops.page = match &choice_now {
                PageChoice::Unchanged => None,
                PageChoice::Default => Some(PageChange::Default(DefaultPage::Default)),
                PageChoice::Paper(paper) => Some(PageChange::Set(PageSpec::paper(paper, landscape))),
                PageChoice::Custom => Some(PageChange::Set(PageSpec {
                    width_mm: Some(297.0),
                    height_mm: Some(210.0),
                    ..PageSpec::default()
                })),
            };
        }
        if let Some(PageChange::Set(spec)) = &mut ops.page {
            if spec.width_mm.is_some() {
                ui.horizontal(|ui| {
                    label(ui, "Largeur");
                    widgets::number(ui, spec.width_mm.get_or_insert(297.0), 50.0..=2000.0, 1.0, " mm");
                    label(ui, "hauteur");
                    widgets::number(ui, spec.height_mm.get_or_insert(210.0), 50.0..=2000.0, 1.0, " mm");
                });
            } else {
                ui.checkbox(&mut spec.landscape, "Paysage");
            }
        }
        ui.label(
            RichText::new(
                "Le format s'applique à partir de ce bloc (sur une nouvelle page) jusqu'au prochain changement. \
                 Si des titres précèdent le bloc, ils le suivent sur la nouvelle page.",
            )
            .small()
            .color(t.faint),
        );
    });

    if is_figure {
        section(ui, "Image", true, |ui| {
            let image = ops.image.get_or_insert_with(ImageOps::default);
            grid(ui, "image-grille", |ui| {
                label(ui, "Largeur");
                ui.horizontal(|ui| {
                    let mut fixed = image.width_percent.is_some();
                    if ui.checkbox(&mut fixed, "").changed() {
                        image.width_percent = fixed.then_some(100.0);
                    }
                    match &mut image.width_percent {
                        Some(w) => {
                            ui.add(egui::Slider::new(w, 10.0..=100.0).suffix(" %").integer());
                        }
                        None => {
                            ui.label(RichText::new("taille d'origine").color(t.faint));
                        }
                    }
                });
                ui.end_row();

                label(ui, "Alignement");
                let mut align = image.align.unwrap_or(HAlign::Center);
                if choice(
                    ui,
                    "align",
                    &mut align,
                    &[(HAlign::Left, "Gauche"), (HAlign::Center, "Centre"), (HAlign::Right, "Droite")],
                ) {
                    image.align = Some(align);
                }
                ui.end_row();

                label(ui, "Placement");
                choice(
                    ui,
                    "placement",
                    &mut image.placement,
                    &[
                        (Placement::Inline, "Dans le texte"),
                        (Placement::Top, "En haut de page"),
                        (Placement::Bottom, "En bas de page"),
                        (Placement::FullPage, "Seule, pleine page"),
                    ],
                );
                ui.end_row();

                label(ui, "Légende");
                let mut caption = image.caption.clone().unwrap_or_default();
                if ui
                    .add(egui::TextEdit::singleline(&mut caption).hint_text("texte alternatif").desired_width(190.0))
                    .changed()
                {
                    image.caption = (!caption.is_empty()).then_some(caption);
                }
                ui.end_row();
            });
            if image == &ImageOps::default() {
                ops.image = None;
            }
        });
    }

    section(ui, "Export", false, |ui| {
        ui.checkbox(&mut ops.hidden, "Masquer ce bloc dans le PDF");
    });

    ui.add_space(8.0);
    let clear = ui.add_enabled(!current.is_empty(), egui::Button::new("Effacer les retouches de ce bloc"));
    if clear.clicked() {
        ops = BlockOps::default();
    }

    if ops != current {
        let owned_id = id.clone();
        let owned_excerpt = excerpt.clone();
        app.edit_layout(&ctx, move |layout| {
            let anchor = AnchorInfo { id: &owned_id, kind, line, excerpt: &owned_excerpt };
            *layout.ops_mut(anchor) = ops;
        });
    }
}

/// Sans sélection : le format par défaut du document.
fn document_section(app: &mut NectarApp, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    let Some(project) = &app.project else { return };
    let mut page = project.layout.page.clone();
    section(ui, "Format du document", true, |ui| {
        let options: Vec<(String, &str)> = PAPERS.iter().map(|(id, label)| ((*id).to_string(), *label)).collect();
        choice(ui, "format-document", &mut page.paper, &options);
        ui.checkbox(&mut page.landscape, "Paysage");
    });
    if page != project.layout.page {
        app.edit_layout(&ctx, move |layout| layout.page = page);
    }
}
