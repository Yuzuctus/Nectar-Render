//! Les retouches du bloc sélectionné : sauts, format de page, image.

use eframe::egui::{self, FontId, RichText};
use nectar_core::layout::{
    BlockOps, BlockStyle, HAlign, ImageOps, PageChange, PageSpec, Placement, TableOps, TextAlign,
};
use nectar_core::model::{AnchorInfo, BlockKind, Node};

use super::actions::{self, Quick};
use super::widgets::{self, choice, grid, label, section};
use crate::app::NectarApp;
use crate::theme::{self, kicker};

const PAPERS: &[(&str, &str)] =
    &[("a4", "A4"), ("a3", "A3"), ("a5", "A5"), ("us-letter", "Letter"), ("us-legal", "Legal"), ("a2", "A2")];

pub fn show(app: &mut NectarApp, ui: &mut egui::Ui) {
    let t = theme::tokens(ui.ctx());
    let ctx = ui.ctx().clone();
    let Some(project) = &app.project else { return };
    let Some(id) = app.selected.clone() else {
        kicker(ui, "Aucun bloc sélectionné");
        ui.add_space(4.0);
        ui.label(RichText::new("Clique sur un bloc dans les pages pour le retoucher.").color(t.muted));
        ui.add_space(6.0);
        for tip in [
            "Glisse un bloc vers le haut ou le bas pour le déplacer",
            "Clic sur le numéro d'une page (ou dans sa marge) : son format",
            "Alt + ↑ / ↓ : déplacer d'1 mm (Maj : 5 mm)",
            "Clic droit sur un bloc : actions rapides",
            "Ctrl + Entrée : nouvelle page avant le bloc",
            "↑ / ↓ : bloc précédent ou suivant",
        ] {
            ui.label(RichText::new(format!("·  {tip}")).small().color(t.faint));
        }
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
    let block = project.document.blocks.iter().find(|b| b.id == id);
    let is_figure = block.is_some_and(|b| matches!(b.node, Node::Figure(_) | Node::Diagram { .. }));
    let inline_ops = block.and_then(|b| b.inline_ops.clone());
    let table_columns = block.and_then(|b| match &b.node {
        Node::Table(t) => Some(
            (0..t.align.len().max(t.header.len()))
                .map(|i| t.header.get(i).map(|c| nectar_core::model::plain_text(c)).unwrap_or_default())
                .collect::<Vec<String>>(),
        ),
        _ => None,
    });
    let heading_level = block.and_then(|b| match &b.node {
        Node::Heading { level, .. } => Some(*level),
        _ => None,
    });
    let current: BlockOps = project.layout.ops_for(&project.document, &id);
    let position = app.rendered.as_ref().and_then(|r| r.positions.iter().find(|p| p.id == id)).map(|p| p.page + 1);
    let offers = actions::offers(app, &id);
    // Format réel de la page du bloc (il peut venir d'un changement plus haut).
    let page_state = position.and_then(|p| Some((app.page_format(p - 1)?, app.page_differs(p - 1))));

    // En-tête du bloc.
    let mut head = kind.label_fr().to_string();
    if let Some(page) = position {
        head.push_str(&format!(" · page {page}"));
    }
    kicker(ui, &head);
    ui.add_space(2.0);
    ui.add(
        egui::Label::new(
            RichText::new(if excerpt.is_empty() { "—".into() } else { excerpt.clone() })
                .font(FontId::new(15.0, theme::strong())),
        )
        .truncate(),
    )
    .on_hover_text(format!("ligne {line} · {id}"));
    if inline_ops.is_some() {
        ui.label(
            RichText::new("Retouches écrites dans la note (<!-- nectar: … -->) : les modifier ici les remplace.")
                .color(t.muted)
                .small(),
        );
    }
    ui.add_space(10.0);

    let mut ops = current.clone();

    // L'essentiel, à un clic.
    kicker(ui, "Sur la page");
    ui.add_space(2.0);
    ui.horizontal_wrapped(|ui| {
        for offer in &offers {
            if offer.quick == Quick::Clear || (is_figure && matches!(offer.quick, Quick::Landscape | Quick::FullPage)) {
                continue;
            }
            if widgets::toggle(ui, offer.active, offer.label, offer.hint) {
                actions::apply(&mut ops, offer.quick);
            }
        }
    });
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        label(ui, "Décalage");
        let mut mm = ops.space_before_mm.unwrap_or(0.0);
        if widgets::number(ui, &mut mm, -50.0..=200.0, 0.5, " mm") {
            ops.space_before_mm = (mm.abs() >= 0.25).then_some(mm);
        }
        if ops.space_before_mm.is_some() && ui.small_button("0").on_hover_text("Remettre à sa place").clicked() {
            ops.space_before_mm = None;
        }
    });
    ui.label(RichText::new("Ou glisse le bloc dans la page · Alt + ↑ / ↓").small().color(t.faint));
    ui.add_space(10.0);

    if is_figure {
        kicker(ui, "Image");
        ui.add_space(2.0);
        let image = ops.image.get_or_insert_with(ImageOps::default);
        widgets::segmented(
            ui,
            &mut image.placement,
            &[(Placement::Inline, "Dans le texte"), (Placement::Top, "En haut"), (Placement::Bottom, "En bas")],
        );
        widgets::segmented(
            ui,
            &mut image.placement,
            &[(Placement::FullPage, "Seule sur sa page"), (Placement::Landscape, "Page paysage")],
        );
        ui.add_space(4.0);
        grid(ui, "image-grille", |ui| {
            label(ui, "Largeur");
            ui.horizontal(|ui| {
                match &mut image.width_percent {
                    Some(w) => {
                        ui.add(egui::Slider::new(w, 10.0..=100.0).suffix(" %").integer());
                    }
                    None => {
                        ui.label(RichText::new("taille d'origine").color(t.faint));
                    }
                }
                let auto = image.width_percent.is_none();
                if ui.small_button(if auto { "Régler" } else { "Auto" }).clicked() {
                    image.width_percent = if auto { Some(100.0) } else { None };
                }
            });
            ui.end_row();
            label(ui, "Alignement");
            let mut align = image.align.unwrap_or(HAlign::Center);
            ui.horizontal(|ui| {
                if widgets::segmented(
                    ui,
                    &mut align,
                    &[(HAlign::Left, "Gauche"), (HAlign::Center, "Centre"), (HAlign::Right, "Droite")],
                ) {
                    image.align = Some(align);
                }
            });
            ui.end_row();
            label(ui, "Légende");
            let mut caption = image.caption.clone().unwrap_or_default();
            if ui
                .add(egui::TextEdit::singleline(&mut caption).hint_text("celle de la note").desired_width(180.0))
                .changed()
            {
                image.caption = (!caption.is_empty()).then_some(caption);
            }
            ui.end_row();
        });
        if image == &ImageOps::default() {
            ops.image = None;
        }
        ui.label(RichText::new("La poignée à droite de l'image règle aussi sa largeur.").small().color(t.faint));
        ui.add_space(10.0);
    }

    if let Some(headers) = &table_columns {
        kicker(ui, "Colonnes du tableau");
        ui.add_space(2.0);
        let table = ops.table.get_or_insert_with(TableOps::default);
        table.widths.resize(headers.len(), 0.0);
        table.align.resize(headers.len(), None);
        grid(ui, "colonnes-tableau", |ui| {
            for (i, header) in headers.iter().enumerate() {
                let name = if header.is_empty() { format!("Colonne {}", i + 1) } else { header.clone() };
                label(ui, &name);
                ui.horizontal(|ui| {
                    let mut fixed = table.widths[i] > 0.0;
                    if ui.checkbox(&mut fixed, "").on_hover_text("Largeur relative (sinon automatique)").changed() {
                        table.widths[i] = if fixed { 1.0 } else { 0.0 };
                    }
                    if fixed {
                        ui.add(
                            egui::DragValue::new(&mut table.widths[i]).range(0.2..=10.0).speed(0.05).suffix(" part"),
                        );
                    } else {
                        ui.label(RichText::new("auto").color(t.faint));
                    }
                    let mut align = table.align[i];
                    choice(
                        ui,
                        &format!("align-col-{i}"),
                        &mut align,
                        &[
                            (None, "—"),
                            (Some(HAlign::Left), "Gauche"),
                            (Some(HAlign::Center), "Centre"),
                            (Some(HAlign::Right), "Droite"),
                        ],
                    );
                    table.align[i] = align;
                });
                ui.end_row();
            }
        });
        if table.widths.iter().all(|w| *w <= 0.0) && table.align.iter().all(Option::is_none) {
            ops.table = None;
        }
        ui.add_space(10.0);
    }

    theme::rule(ui, 1.0, false);
    let mut apply_to_headings = false;
    section(ui, "Plus d'options", false, |ui| {
        if !is_figure {
            ui.checkbox(&mut ops.break_after, "Laisser le reste de la page vide après");
        }
        ui.checkbox(&mut ops.push_to_bottom, "Pousser en bas de la page");
        ui.checkbox(&mut ops.hidden, "Masquer ce bloc dans le PDF");
        ui.add_space(6.0);
        page_format(ui, &mut ops, page_state, t);
        if kind != BlockKind::ListItem && !is_figure {
            ui.add_space(6.0);
            appearance(ui, &mut ops, kind, heading_level.is_some());
        }
        if let Some(level) = heading_level {
            ui.add_space(6.0);
            apply_to_headings = ui
                .button(format!("Appliquer à tous les titres de niveau {level}"))
                .on_hover_text("Sauts, décalage, format de page et apparence")
                .clicked();
        }
        ui.add_space(6.0);
        if ui.button("Ouvrir la note dans Obsidian").clicked()
            && let Some(project) = &app.project
        {
            let path = project.note.display().to_string().replace('\\', "/");
            let encoded: String = path
                .bytes()
                .map(|b| match b {
                    b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                        (b as char).to_string()
                    }
                    _ => format!("%{b:02X}"),
                })
                .collect();
            ctx.open_url(egui::OpenUrl::new_tab(format!("obsidian://open?path={encoded}")));
        }
    });

    ui.add_space(8.0);
    if ui.add_enabled(!current.is_empty(), egui::Button::new("Effacer les retouches de ce bloc")).clicked() {
        ops = BlockOps::default();
    }

    if apply_to_headings && let Some(level) = heading_level {
        let model = ops.clone();
        let targets: Vec<(nectar_core::BlockId, usize, String)> = app
            .project
            .as_ref()
            .map(|p| {
                p.document
                    .blocks
                    .iter()
                    .filter(|b| matches!(b.node, Node::Heading { level: l, .. } if l == level))
                    .map(|b| (b.id.clone(), b.line, b.excerpt.clone()))
                    .collect()
            })
            .unwrap_or_default();
        let count = targets.len();
        app.edit_layout(&ctx, move |layout| {
            for (id, line, excerpt) in &targets {
                let ops = layout.ops_mut(AnchorInfo { id, kind: BlockKind::Heading, line: *line, excerpt });
                ops.break_before = model.break_before;
                ops.break_after = model.break_after;
                ops.space_before_mm = model.space_before_mm;
                ops.keep_with_next = model.keep_with_next;
                ops.style = model.style.clone();
            }
        });
        app.notify(format!("Retouches appliquées à {count} titres"), false);
        return;
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

/// Format de page à partir de ce bloc.
fn page_format(ui: &mut egui::Ui, ops: &mut BlockOps, page: Option<(super::page::Format, bool)>, t: theme::Tokens) {
    use super::page::{self, Format};
    label(ui, "Format de sa page");
    // Le format propre au bloc s'il en a un, sinon celui de sa page.
    let (actual, differs) = page.unwrap_or((Format::Document, false));
    let current = if ops.page.is_some() { page::format_of(&ops.page) } else { actual };
    if let Some(format) = page::picker(ui, "format-bloc", &current) {
        let mut inherited = differs && ops.page.is_none();
        // Une image seule sur sa page paysage : ce format-là vient de son placement.
        if let Some(image) = &mut ops.image
            && image.placement == Placement::Landscape
        {
            image.placement = Placement::Inline;
            inherited = false;
        }
        page::apply(&mut ops.page, &mut ops.page_onward, &format, inherited);
    }
    if let Some(PageChange::Set(spec)) = &mut ops.page {
        if current == Format::Custom {
            ui.horizontal(|ui| {
                label(ui, "Largeur");
                widgets::number(ui, spec.width_mm.get_or_insert(297.0), 50.0..=2000.0, 1.0, " mm");
                label(ui, "hauteur");
                widgets::number(ui, spec.height_mm.get_or_insert(210.0), 50.0..=2000.0, 1.0, " mm");
            });
        }
        ui.checkbox(&mut ops.page_onward, "Et les pages suivantes");
    } else if ui.small_button("Format libre…").clicked() {
        ops.page =
            Some(PageChange::Set(PageSpec { width_mm: Some(297.0), height_mm: Some(210.0), ..PageSpec::default() }));
    }
    ui.label(
        RichText::new(
            "Le bloc ouvre une page à ce format, qui se remplit avec la suite ; puis le document reprend son format.",
        )
        .small()
        .color(t.faint),
    );
}

/// Apparence propre au bloc.
fn appearance(ui: &mut egui::Ui, ops: &mut BlockOps, kind: BlockKind, heading: bool) {
    let style = ops.style.get_or_insert_with(BlockStyle::default);
    grid(ui, "apparence", |ui| {
        label(ui, "Alignement");
        let mut align = style.align;
        choice(
            ui,
            "align-bloc",
            &mut align,
            &[
                (None, "Comme le document"),
                (Some(TextAlign::Left), "À gauche"),
                (Some(TextAlign::Center), "Centré"),
                (Some(TextAlign::Right), "À droite"),
                (Some(TextAlign::Justify), "Justifié"),
            ],
        );
        style.align = align;
        ui.end_row();
        if !heading {
            label(ui, "Taille du texte");
            ui.horizontal(|ui| {
                let mut own = style.size_percent.is_some();
                if ui.checkbox(&mut own, "").changed() {
                    style.size_percent = own.then_some(100.0);
                }
                if let Some(size) = &mut style.size_percent {
                    ui.add(egui::Slider::new(size, 50.0..=200.0).suffix(" %").integer());
                }
            });
            ui.end_row();
            label(ui, "Couleur du texte");
            let fallback = "#1f2328".to_string();
            widgets::optional_color(ui, &mut style.color, &fallback);
            ui.end_row();
            label(ui, "Style");
            ui.horizontal(|ui| {
                ui.checkbox(&mut style.bold, "gras");
                ui.checkbox(&mut style.italic, "italique");
            });
            ui.end_row();
        }
        label(ui, "Fond");
        ui.horizontal(|ui| {
            let mut own = style.background.is_some();
            if ui.checkbox(&mut own, "").changed() {
                style.background = own.then(|| "#f6f8fa".to_string());
            }
            if let Some(bg) = &mut style.background {
                widgets::color(ui, bg);
            }
        });
        ui.end_row();
        label(ui, "Cadre");
        ui.checkbox(&mut style.border, "Filet fin autour du bloc");
        ui.end_row();
        label(ui, "Retrait à gauche");
        ui.horizontal(|ui| {
            let mut own = style.indent_mm.is_some();
            if ui.checkbox(&mut own, "").changed() {
                style.indent_mm = own.then_some(10.0);
            }
            if let Some(mm) = &mut style.indent_mm {
                widgets::number(ui, mm, 0.0..=80.0, 0.5, " mm");
            }
        });
        ui.end_row();
        if matches!(kind, BlockKind::Paragraph | BlockKind::List) {
            label(ui, "Colonnes");
            let mut columns = style.columns.unwrap_or(1);
            widgets::segmented(ui, &mut columns, &[(1, "Une"), (2, "Deux"), (3, "Trois")]);
            style.columns = (columns > 1).then_some(columns);
            ui.end_row();
        }
    });
    if style == &BlockStyle::default() {
        ops.style = None;
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
