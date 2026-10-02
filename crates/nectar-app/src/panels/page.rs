//! Une page sélectionnée : son format, en un clic.
//!
//! Le format d'une page est porté par le bloc qui l'ouvre (ou par celui qui
//! l'a déjà changée) : la page prend ce format, se remplit avec la suite,
//! puis le document reprend son format, sauf si « et les suivantes ».

use eframe::egui::{self, RichText};
use nectar_core::layout::{DefaultPage, PageChange, PageSpec};

use super::widgets::label;
use crate::app::NectarApp;
use crate::theme::{self, kicker};

/// Un format proposé.
#[derive(Clone, Debug, PartialEq)]
pub enum Format {
    /// Celui du document.
    Document,
    Paper(String, bool),
    /// Format libre, réglé dans le panneau du bloc.
    Custom,
}

/// Les formats proposés en un clic.
pub fn quick() -> Vec<(Format, &'static str)> {
    vec![
        (Format::Document, "Normal"),
        (Format::Paper("a4".into(), true), "A4 paysage"),
        (Format::Paper("a3".into(), false), "A3"),
        (Format::Paper("a3".into(), true), "A3 paysage"),
    ]
}

/// Les autres formats, dans « Autre ».
const OTHERS: &[(&str, &str)] =
    &[("a4", "A4"), ("a5", "A5"), ("a2", "A2"), ("us-letter", "Letter"), ("us-legal", "Legal")];

/// Le format décrit par des retouches.
pub fn format_of(change: &Option<PageChange>) -> Format {
    match change {
        None | Some(PageChange::Default(_)) => Format::Document,
        Some(PageChange::Set(spec)) if spec.width_mm.is_some() => Format::Custom,
        Some(PageChange::Set(spec)) => Format::Paper(spec.paper.clone(), spec.landscape),
    }
}

/// Formats connus : identifiant, nom, petit et grand côté en points.
const PAPERS: &[(&str, &str, f32, f32)] = &[
    ("a4", "A4", 595.3, 841.9),
    ("a3", "A3", 841.9, 1190.6),
    ("a5", "A5", 419.5, 595.3),
    ("a2", "A2", 1190.6, 1683.8),
    ("us-letter", "Letter", 612.0, 792.0),
    ("us-legal", "Legal", 612.0, 1008.0),
];

fn paper_of(size: egui::Vec2) -> Option<&'static (&'static str, &'static str, f32, f32)> {
    let (short, long) = (size.x.min(size.y), size.x.max(size.y));
    PAPERS.iter().find(|(_, _, w, h)| (w - short).abs() < 2.0 && (h - long).abs() < 2.0)
}

/// Nom lisible d'une taille de page (en points).
pub fn describe(size: egui::Vec2) -> String {
    let orientation = if size.x > size.y { "paysage" } else { "portrait" };
    match paper_of(size) {
        Some((_, name, _, _)) => format!("{name} {orientation}"),
        None => format!("{:.0} × {:.0} mm", size.x * 25.4 / 72.0, size.y * 25.4 / 72.0),
    }
}

/// Le format réel d'une page, d'après sa taille.
pub fn format_of_size(size: egui::Vec2) -> Format {
    match paper_of(size) {
        Some((id, _, _, _)) => Format::Paper((*id).to_string(), size.x > size.y),
        None => Format::Custom,
    }
}

/// Taille (en points) d'un format de document.
pub fn size_of(spec: &PageSpec) -> Option<egui::Vec2> {
    let (w, h) = match (spec.width_mm, spec.height_mm) {
        (Some(w), Some(h)) => (w * 72.0 / 25.4, h * 72.0 / 25.4),
        _ => PAPERS.iter().find(|(id, ..)| *id == spec.paper).map(|(_, _, w, h)| (*w, *h))?,
    };
    let (short, long) = (w.min(h), w.max(h));
    Some(if spec.landscape { egui::vec2(long, short) } else { egui::vec2(short, long) })
}

/// Barre de choix du format ; renvoie le format choisi.
pub fn picker(ui: &mut egui::Ui, id: &str, current: &Format) -> Option<Format> {
    picker_among(ui, id, current, &quick())
}

/// Le format du document : les mêmes choix, A4 à la place de « Normal ».
fn document_picker(ui: &mut egui::Ui, current: &Format) -> Option<Format> {
    let mut options = quick();
    options[0] = (Format::Paper("a4".into(), false), "A4");
    picker_among(ui, "format-document", current, &options)
}

fn picker_among(ui: &mut egui::Ui, id: &str, current: &Format, options: &[(Format, &str)]) -> Option<Format> {
    let t = theme::tokens(ui.ctx());
    let mut chosen = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for (format, text) in options {
            let selected = format == current;
            let label = RichText::new(*text).color(if selected { t.accent_ink } else { t.ink });
            if ui.add(egui::Button::new(label).fill(if selected { t.accent } else { t.raised })).clicked() && !selected
            {
                chosen = Some(format.clone());
            }
        }
        let other = !options.iter().any(|(f, _)| f == current);
        let text = match current {
            Format::Paper(paper, landscape) if other => {
                let name = OTHERS.iter().find(|(id, _)| id == paper).map(|(_, l)| *l).unwrap_or(paper.as_str());
                format!("{name}{}", if *landscape { " paysage" } else { "" })
            }
            Format::Custom => "Libre".to_string(),
            _ => "Autre".to_string(),
        };
        let text = RichText::new(text).color(if other { t.accent_ink } else { t.ink });
        egui::ComboBox::from_id_salt(id).selected_text(text).width(90.0).show_ui(ui, |ui| {
            for (paper, name) in OTHERS {
                for landscape in [false, true] {
                    let format = Format::Paper((*paper).to_string(), landscape);
                    let text = format!("{name}{}", if landscape { " paysage" } else { "" });
                    if ui.selectable_label(&format == current, text).clicked() {
                        chosen = Some(format);
                    }
                }
            }
        });
    });
    chosen
}

/// Applique un format aux retouches d'un bloc. `differs` : la page n'est pas
/// au format du document (elle hérite d'un changement « et les suivantes »).
pub fn apply(change: &mut Option<PageChange>, onward: &mut bool, format: &Format, differs: bool) {
    match format {
        Format::Document => {
            if matches!(change, Some(PageChange::Set(_))) || !differs {
                *change = None;
                *onward = false;
            } else {
                *change = Some(PageChange::Default(DefaultPage::Default));
            }
        }
        Format::Paper(paper, landscape) => *change = Some(PageChange::Set(PageSpec::paper(paper, *landscape))),
        Format::Custom => {}
    }
}

/// Le panneau d'une page sélectionnée.
pub fn show(app: &mut NectarApp, ui: &mut egui::Ui, page: usize) {
    let t = theme::tokens(ui.ctx());
    let ctx = ui.ctx().clone();
    let Some(size) = app.rendered.as_ref().and_then(|r| r.pages.get(page)).map(|p| p.size_pt) else {
        app.selected_page = None;
        return;
    };
    let Some(owner) = app.page_owner(page) else { return };
    let Some(current) = app.page_format(page) else { return };
    let Some(project) = &app.project else { return };
    let ops = project.layout.ops_for(&project.document, &owner);
    let excerpt = project.document.anchors().into_iter().find(|a| *a.id == owner).map(|a| a.excerpt.to_string());

    kicker(ui, &format!("Page {}", page + 1));
    ui.add_space(2.0);
    ui.label(RichText::new(describe(size)).font(egui::FontId::new(15.0, theme::strong())));
    ui.add_space(10.0);

    kicker(ui, "Format de cette page");
    ui.add_space(2.0);
    let mut onward = ops.page_onward;
    let chosen = picker(ui, "format-page", &current);
    let onward_changed = matches!(ops.page, Some(PageChange::Set(_)))
        && ui
            .checkbox(&mut onward, "Et les pages suivantes")
            .on_hover_text("Sinon, la page se remplit avec la suite puis le document reprend son format")
            .changed();
    ui.add_space(4.0);
    let mut note = String::from("La page se remplit avec la suite, puis le document reprend son format.");
    if let Some(excerpt) = &excerpt {
        note.push_str(&format!(" Le format commence au bloc « {} ».", truncate(excerpt, 40)));
    }
    ui.label(RichText::new(note).small().color(t.faint));

    if let Some(format) = &chosen {
        app.set_page_format(&ctx, page, format);
    } else if onward_changed {
        app.edit_block(&ctx, &owner, |ops| ops.page_onward = onward);
    }

    ui.add_space(12.0);
    theme::rule(ui, 1.0, false);
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        label(ui, "Premier bloc");
        if ui.button("Le sélectionner").clicked() {
            app.select(Some(owner.clone()), false);
        }
    });
    if let Some(project) = &app.project {
        let mut page_spec = project.layout.page.clone();
        ui.add_space(6.0);
        kicker(ui, "Format du document");
        ui.add_space(2.0);
        let current = Format::Paper(page_spec.paper.clone(), page_spec.landscape);
        if let Some(Format::Paper(paper, landscape)) = document_picker(ui, &current) {
            page_spec.paper = paper;
            page_spec.landscape = landscape;
        }
        if page_spec != project.layout.page {
            app.edit_layout(&ctx, move |layout| layout.page = page_spec);
        }
    }
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        format!("{}…", text.chars().take(max).collect::<String>())
    }
}

/// Barre des formats posée sur la page sélectionnée (dans l'aperçu) : elle
/// montre le format réel de la page.
pub fn bar(app: &NectarApp, ui: &mut egui::Ui, page: usize) -> Option<Format> {
    let current = app.page_format(page)?;
    picker(ui, "format-page-barre", &current)
}
