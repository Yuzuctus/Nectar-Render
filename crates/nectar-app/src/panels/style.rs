//! Le style du PDF : un preset, puis tout réglable à la main.

use eframe::egui::{self, FontId, RichText};
use nectar_core::Style;
use nectar_core::code_themes::THEMES;
use nectar_core::style::{CalloutVariant, CodeHeader, CoverMode, QuoteVariant, StyleRef, TableVariant};

use super::widgets::{choice, color, font, grid, label, number, optional_color, section};
use crate::app::NectarApp;
use crate::theme::{self, kicker};

pub fn show(app: &mut NectarApp, ui: &mut egui::Ui) {
    let t = theme::tokens(ui.ctx());
    let ctx = ui.ctx().clone();
    let Some(project) = &app.project else { return };

    // Preset.
    kicker(ui, "Preset");
    ui.add_space(2.0);
    let presets: Vec<(String, String, bool)> =
        project.presets.presets().iter().map(|p| (p.id.clone(), p.label.clone(), p.builtin)).collect();
    let mut preset = project.layout.style.preset.clone();
    let has_overrides = !project.layout.style.overrides.is_empty();
    let current_label = presets.iter().find(|p| p.0 == preset).map(|p| p.1.clone()).unwrap_or(preset.clone());
    let current_builtin = presets.iter().find(|p| p.0 == preset).is_none_or(|p| p.2);
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("preset").selected_text(&current_label).width(200.0).show_ui(ui, |ui| {
            for (id, label, builtin) in &presets {
                let text = if *builtin { label.clone() } else { format!("{label}  ·  perso") };
                ui.selectable_value(&mut preset, id.clone(), text);
            }
        });
        if has_overrides {
            ui.label(RichText::new("modifié").font(FontId::new(10.5, theme::mono())).color(t.identity));
        }
    });
    let mut deleted = false;
    ui.horizontal_wrapped(|ui| {
        if ui.add_enabled(has_overrides, egui::Button::new("Rétablir le preset")).clicked() {
            app.edit_layout(&ctx, |layout| layout.style.overrides.clear());
        }
        if ui.button("Enregistrer comme preset…").clicked() {
            app.save_preset_dialog = Some(String::new());
        }
        if !current_builtin && ui.button("Supprimer").clicked() {
            let id = preset.clone();
            if let Some(project) = &mut app.project {
                let _ = project.presets.delete_user(&id);
            }
            app.edit_layout(&ctx, |layout| layout.style = StyleRef::default());
            deleted = true;
        }
    });
    if deleted {
        return;
    }
    if app.project.as_ref().is_some_and(|p| p.layout.style.preset != preset) {
        app.edit_layout(&ctx, move |layout| layout.style = StyleRef { preset, overrides: Default::default() });
        return;
    }
    ui.add_space(8.0);
    theme::rule(ui, 1.0, false);

    let families = app.families.clone();
    let mut s = app.style.clone();
    fields(ui, &mut s, &families);
    if s != app.style {
        app.edit_style(&ctx, &s);
    }
}

fn fields(ui: &mut egui::Ui, s: &mut Style, families: &[String]) {
    section(ui, "Texte", true, |ui| {
        grid(ui, "texte", |ui| {
            label(ui, "Police");
            font(ui, "police-texte", &mut s.text.font, families);
            ui.end_row();
            label(ui, "Taille");
            number(ui, &mut s.text.size_pt, 6.0..=24.0, 0.1, " pt");
            ui.end_row();
            label(ui, "Couleur");
            color(ui, &mut s.text.color);
            ui.end_row();
            label(ui, "Interligne");
            number(ui, &mut s.text.line_height, 1.0..=2.5, 0.01, "");
            ui.end_row();
            label(ui, "Entre paragraphes");
            number(ui, &mut s.text.paragraph_spacing_em, 0.0..=3.0, 0.02, " em");
            ui.end_row();
            label(ui, "Retrait 1re ligne");
            number(ui, &mut s.text.first_line_indent_em, 0.0..=4.0, 0.05, " em");
            ui.end_row();
            label(ui, "Alignement");
            choice(ui, "justif", &mut s.text.justify, &[(true, "Justifié"), (false, "À gauche")]);
            ui.end_row();
            label(ui, "Césure");
            ui.checkbox(&mut s.text.hyphenate, "Couper les mots en fin de ligne");
            ui.end_row();
            label(ui, "Typographie");
            ui.checkbox(&mut s.text.french_typography, "Espaces insécables françaises");
            ui.end_row();
            label(ui, "Lignes isolées");
            ui.checkbox(&mut s.text.avoid_widows, "Éviter en haut et en bas de page");
            ui.end_row();
            label(ui, "Formules");
            choice(
                ui,
                "police-maths",
                &mut s.text.math_font,
                &[
                    ("New Computer Modern Math".to_string(), "Computer Modern (LaTeX)"),
                    ("Cambria Math".to_string(), "Cambria Math (Windows)"),
                    ("STIX Two Math".to_string(), "STIX Two"),
                    ("Libertinus Math".to_string(), "Libertinus"),
                ],
            );
            ui.end_row();
        });
    });

    section(ui, "Placement automatique", false, |ui| {
        ui.checkbox(
            &mut s.pagination.keep_intro_with_next,
            "Une phrase finissant par « : » reste avec ce qu'elle annonce",
        );
        ui.checkbox(
            &mut s.pagination.keep_small_blocks,
            "Ne jamais couper une liste courte, un code court, un petit tableau, un encadré",
        );
        ui.label(
            RichText::new(
                "Les titres restent toujours avec leur contenu. Une retouche de bloc l'emporte sur ces règles.",
            )
            .small()
            .color(theme::tokens(ui.ctx()).faint),
        );
    });

    section(ui, "Titres", false, |ui| {
        grid(ui, "titres", |ui| {
            label(ui, "Police");
            font(ui, "police-titres", &mut s.headings.font, families);
            ui.end_row();
            label(ui, "Couleur");
            color(ui, &mut s.headings.color);
            ui.end_row();
            label(ui, "Graisse");
            choice(ui, "graisse", &mut s.headings.weight, &[(400, "Normale"), (600, "Demi-gras"), (700, "Gras")]);
            ui.end_row();
            label(ui, "Numérotation");
            ui.checkbox(&mut s.headings.numbering, "1, 1.1, 1.1.1…");
            ui.end_row();
            label(ui, "Titre 1");
            ui.checkbox(&mut s.headings.h1_new_page, "Toujours sur une nouvelle page");
            ui.end_row();
        });
        let base_color = s.headings.color.clone();
        for (i, level) in s.headings.levels.iter_mut().enumerate() {
            egui::CollapsingHeader::new(format!("Titre {}", i + 1)).id_salt(("niveau", i)).show(ui, |ui| {
                grid(ui, &format!("niveau-{i}"), |ui| {
                    label(ui, "Taille");
                    number(ui, &mut level.size_pt, 6.0..=72.0, 0.1, " pt");
                    ui.end_row();
                    label(ui, "Couleur");
                    optional_color(ui, &mut level.color, &base_color);
                    ui.end_row();
                    label(ui, "Police propre");
                    ui.horizontal(|ui| {
                        let mut own = level.font.is_some();
                        if ui.checkbox(&mut own, "").changed() {
                            level.font = own.then(|| "IBM Plex Sans".to_string());
                        }
                        if let Some(f) = &mut level.font {
                            font(ui, &format!("police-niveau-{i}"), f, families);
                        }
                    });
                    ui.end_row();
                    label(ui, "Capitales");
                    ui.checkbox(&mut level.uppercase, "");
                    ui.end_row();
                    label(ui, "Espacement lettres");
                    number(ui, &mut level.tracking_em, 0.0..=0.5, 0.005, " em");
                    ui.end_row();
                    label(ui, "Filet au-dessus");
                    number(ui, &mut level.rule_above_pt, 0.0..=6.0, 0.1, " pt");
                    ui.end_row();
                    label(ui, "Filet dessous");
                    number(ui, &mut level.rule_below_pt, 0.0..=6.0, 0.1, " pt");
                    ui.end_row();
                    label(ui, "Espace avant");
                    number(ui, &mut level.space_above_em, 0.0..=6.0, 0.05, " em");
                    ui.end_row();
                    label(ui, "Espace après");
                    number(ui, &mut level.space_below_em, 0.0..=4.0, 0.05, " em");
                    ui.end_row();
                });
            });
        }
    });

    section(ui, "Code", false, |ui| {
        grid(ui, "code", |ui| {
            label(ui, "Thème");
            let options: Vec<(String, &str)> = THEMES.iter().map(|t| (t.id.to_string(), t.label)).collect();
            choice(ui, "theme-code", &mut s.code.theme, &options);
            ui.end_row();
            label(ui, "Bandeau");
            choice(
                ui,
                "bandeau",
                &mut s.code.header,
                &[
                    (CodeHeader::Tab, "Onglet du langage"),
                    (CodeHeader::Window, "Barre de fenêtre"),
                    (CodeHeader::None, "Aucun"),
                ],
            );
            ui.end_row();
            label(ui, "Numéros de ligne");
            ui.checkbox(&mut s.code.line_numbers, "");
            ui.end_row();
            label(ui, "Police");
            font(ui, "police-code", &mut s.code.font, families);
            ui.end_row();
            label(ui, "Taille");
            number(ui, &mut s.code.size_pt, 5.0..=16.0, 0.1, " pt");
            ui.end_row();
            label(ui, "Interligne");
            number(ui, &mut s.code.line_height, 1.0..=2.2, 0.01, "");
            ui.end_row();
            label(ui, "Arrondi");
            number(ui, &mut s.code.radius_pt, 0.0..=16.0, 0.1, " pt");
            ui.end_row();
            label(ui, "Bordure");
            ui.checkbox(&mut s.code.border, "");
            ui.end_row();
            label(ui, "Code en ligne : fond");
            color(ui, &mut s.code.inline_background);
            ui.end_row();
            label(ui, "Code en ligne : texte");
            color(ui, &mut s.code.inline_color);
            ui.end_row();
        });
    });

    section(ui, "Schémas (Mermaid)", false, |ui| {
        grid(ui, "schemas", |ui| {
            label(ui, "Thème");
            choice(
                ui,
                "theme-mermaid",
                &mut s.diagrams.theme,
                &[
                    ("neutral".to_string(), "Neutre"),
                    ("default".to_string(), "Mermaid"),
                    ("forest".to_string(), "Forêt"),
                    ("base".to_string(), "Sobre"),
                    ("dark".to_string(), "Sombre"),
                ],
            );
            ui.end_row();
            label(ui, "Police du texte");
            ui.checkbox(&mut s.diagrams.document_font, "Utiliser celle du document");
            ui.end_row();
            label(ui, "Taille");
            ui.add(egui::Slider::new(&mut s.diagrams.scale, 0.4..=2.0).fixed_decimals(2));
            ui.end_row();
        });
    });

    section(ui, "Encadrés et citations", false, |ui| {
        grid(ui, "encadres", |ui| {
            label(ui, "Encadrés");
            choice(
                ui,
                "encadre",
                &mut s.callout.variant,
                &[
                    (CalloutVariant::Bordered, "Cadre fin"),
                    (CalloutVariant::Soft, "Fond léger"),
                    (CalloutVariant::Plain, "Sans cadre"),
                ],
            );
            ui.end_row();
            label(ui, "Couleurs par type");
            ui.checkbox(&mut s.callout.colored, "note, astuce, attention…");
            ui.end_row();
            label(ui, "Arrondi");
            number(ui, &mut s.callout.radius_pt, 0.0..=16.0, 0.1, " pt");
            ui.end_row();
            label(ui, "Citations");
            choice(
                ui,
                "citation",
                &mut s.quote.variant,
                &[
                    (QuoteVariant::Indent, "Retrait"),
                    (QuoteVariant::Bar, "Filet fin"),
                    (QuoteVariant::Quotes, "Grand guillemet"),
                ],
            );
            ui.end_row();
            label(ui, "Couleur");
            color(ui, &mut s.quote.color);
            ui.end_row();
            label(ui, "Italique");
            ui.checkbox(&mut s.quote.italic, "");
            ui.end_row();
        });
    });

    section(ui, "Tableaux", false, |ui| {
        grid(ui, "tableaux", |ui| {
            label(ui, "Bordures");
            choice(
                ui,
                "variante-tableau",
                &mut s.table.variant,
                &[
                    (TableVariant::Rules, "Filets horizontaux"),
                    (TableVariant::Grid, "Grille"),
                    (TableVariant::Plain, "Aucune"),
                ],
            );
            ui.end_row();
            label(ui, "Couleur des filets");
            color(ui, &mut s.table.border_color);
            ui.end_row();
            label(ui, "En-tête : fond");
            color(ui, &mut s.table.header_background);
            ui.end_row();
            label(ui, "En-tête : texte");
            color(ui, &mut s.table.header_color);
            ui.end_row();
            label(ui, "Lignes alternées");
            ui.horizontal(|ui| {
                ui.checkbox(&mut s.table.stripes, "");
                if s.table.stripes {
                    color(ui, &mut s.table.stripe_color);
                }
            });
            ui.end_row();
            label(ui, "Marge des cellules");
            ui.horizontal(|ui| {
                number(ui, &mut s.table.cell_padding_x_pt, 0.0..=30.0, 0.1, " pt");
                number(ui, &mut s.table.cell_padding_y_pt, 0.0..=30.0, 0.1, " pt");
            });
            ui.end_row();
        });
    });

    section(ui, "Liens, images, notes", false, |ui| {
        grid(ui, "divers", |ui| {
            label(ui, "Liens");
            color(ui, &mut s.links.color);
            ui.end_row();
            label(ui, "Souligner les liens");
            ui.checkbox(&mut s.links.underline, "");
            ui.end_row();
            label(ui, "Liens [[internes]]");
            color(ui, &mut s.links.wikilink_color);
            ui.end_row();
            label(ui, "Surlignage");
            color(ui, &mut s.highlight);
            ui.end_row();
            label(ui, "Taille des images");
            ui.add(egui::Slider::new(&mut s.images.scale, 0.3..=1.5).fixed_decimals(2));
            ui.end_row();
            label(ui, "Arrondi des images");
            number(ui, &mut s.images.radius_pt, 0.0..=20.0, 0.1, " pt");
            ui.end_row();
            label(ui, "Légendes");
            ui.horizontal(|ui| {
                number(ui, &mut s.images.caption_size_pt, 5.0..=14.0, 0.1, " pt");
                ui.checkbox(&mut s.images.caption_italic, "italique");
            });
            ui.end_row();
            label(ui, "Couleur des légendes");
            color(ui, &mut s.images.caption_color);
            ui.end_row();
            label(ui, "Numéroter les figures");
            ui.checkbox(&mut s.images.numbering, "");
            ui.end_row();
            label(ui, "Notes de bas de page");
            ui.horizontal(|ui| {
                number(ui, &mut s.footnotes.size_pt, 5.0..=14.0, 0.1, " pt");
                color(ui, &mut s.footnotes.color);
            });
            ui.end_row();
            label(ui, "Séparateurs ---");
            ui.checkbox(&mut s.rules, "afficher");
            ui.end_row();
        });
    });

    section(ui, "Fichier PDF", false, |ui| {
        ui.checkbox(&mut s.export.pdf_a, "PDF/A (archivage, dépôt officiel)");
        ui.horizontal(|ui| {
            ui.checkbox(&mut s.export.downscale_images, "Alléger les photos au-delà de");
            ui.add_enabled(
                s.export.downscale_images,
                egui::DragValue::new(&mut s.export.max_image_px).range(800..=8000).speed(20).suffix(" px"),
            );
        });
    });

    section(ui, "Page", false, |ui| {
        grid(ui, "page", |ui| {
            label(ui, "Marges");
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label("haut");
                    number(ui, &mut s.page.margin_top_mm, 0.0..=80.0, 0.5, " mm");
                    ui.label("bas");
                    number(ui, &mut s.page.margin_bottom_mm, 0.0..=80.0, 0.5, " mm");
                });
                ui.horizontal(|ui| {
                    ui.label("gauche");
                    number(ui, &mut s.page.margin_left_mm, 0.0..=80.0, 0.5, " mm");
                    ui.label("droite");
                    number(ui, &mut s.page.margin_right_mm, 0.0..=80.0, 0.5, " mm");
                });
            });
            ui.end_row();
            label(ui, "Fond de page");
            color(ui, &mut s.page.background);
            ui.end_row();
            label(ui, "Pied de page");
            ui.add(egui::TextEdit::singleline(&mut s.footer.text).hint_text("{title}").desired_width(190.0))
                .on_hover_text("{title} est remplacé par le titre de la note");
            ui.end_row();
            label(ui, "Numéros de page");
            ui.horizontal(|ui| {
                ui.checkbox(&mut s.footer.page_numbers, "");
                choice(
                    ui,
                    "align-pied",
                    &mut s.footer.align,
                    &[("left".into(), "À gauche"), ("center".into(), "Au centre"), ("right".into(), "À droite")],
                );
            });
            ui.end_row();
            label(ui, "Couleur du pied");
            color(ui, &mut s.footer.color);
            ui.end_row();
            label(ui, "Page de garde");
            choice(
                ui,
                "garde",
                &mut s.cover.mode,
                &[
                    (CoverMode::Auto, "Si la note a un titre"),
                    (CoverMode::Page, "Page de garde"),
                    (CoverMode::Header, "Titre en haut de page"),
                    (CoverMode::None, "Aucune"),
                ],
            );
            ui.end_row();
            label(ui, "Sommaire");
            ui.checkbox(&mut s.cover.table_of_contents, "");
            ui.end_row();
        });
    });
}

/// Fenêtre « Enregistrer comme preset ».
pub fn save_preset_modal(app: &mut NectarApp, ctx: &egui::Context) {
    let Some(mut name) = app.save_preset_dialog.take() else { return };
    let mut keep = true;
    let mut save = false;
    egui::Modal::new(egui::Id::new("enregistrer-preset")).show(ctx, |ui| {
        ui.set_width(340.0);
        kicker(ui, "Nouveau preset");
        ui.add_space(4.0);
        ui.label("Le style actuel, réglages compris, devient un preset réutilisable pour toutes tes notes.");
        ui.add_space(8.0);
        let response =
            ui.add(egui::TextEdit::singleline(&mut name).hint_text("Nom du preset").desired_width(f32::INFINITY));
        response.request_focus();
        if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            save = true;
        }
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if ui.add_enabled(!name.trim().is_empty(), egui::Button::new("Enregistrer")).clicked() {
                save = true;
            }
            if ui.button("Annuler").clicked() {
                keep = false;
            }
        });
    });
    if save && !name.trim().is_empty() {
        let style = app.style.clone();
        let result = app.project.as_mut().map(|p| p.presets.save_user(name.trim(), &style));
        match result {
            Some(Ok(id)) => {
                app.edit_layout(ctx, |layout| layout.style = StyleRef { preset: id, overrides: Default::default() });
                app.notify(format!("Preset « {} » enregistré", name.trim()), false);
            }
            Some(Err(e)) => app.notify(format!("Enregistrement du preset impossible : {e}"), true),
            None => {}
        }
        return;
    }
    if keep && !ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.save_preset_dialog = Some(name);
    }
}
