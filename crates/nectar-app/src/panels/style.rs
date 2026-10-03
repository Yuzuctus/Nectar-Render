//! Le style du PDF : un modèle, puis tout réglable à la main.

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

    // Chercher un réglage : le plus simple quand il y en a des dizaines.
    // Pendant une recherche, seuls les réglages trouvés s'affichent.
    let mut typed = ui.data(|d| d.get_temp::<String>(egui::Id::new("recherche-texte"))).unwrap_or_default();
    ui.horizontal(|ui| {
        let edit = egui::TextEdit::singleline(&mut typed)
            .hint_text("Chercher un réglage : marges, liens, interligne…")
            .desired_width(ui.available_width() - 44.0);
        ui.add(edit);
        if !typed.is_empty() && ui.small_button("×").on_hover_text("Effacer la recherche").clicked() {
            typed.clear();
        }
    });
    let search = plain(typed.trim());
    ui.data_mut(|d| d.insert_temp(egui::Id::new("recherche-texte"), typed));
    ui.add_space(8.0);
    if !search.is_empty() {
        let families = app.families.clone();
        let mut s = app.style.clone();
        set_filter(ui, Filter { needle: search, all: false, count: 0 });
        fields(ui, &mut s, &families);
        if filter(ui).count == 0 {
            ui.label(RichText::new("Aucun réglage ne correspond. Essaie un autre mot.").color(t.muted));
        }
        set_filter(ui, Filter::default());
        if s != app.style {
            app.edit_style(&ctx, &s);
        }
        return;
    }

    // Modèle : des cartes qui disent à quoi il sert, avec ses couleurs.
    kicker(ui, "Modèle");
    ui.add_space(4.0);
    let presets: Vec<(String, String, bool)> =
        project.presets.presets().iter().map(|p| (p.id.clone(), p.label.clone(), p.builtin)).collect();
    let mut preset = project.layout.style.preset.clone();
    let overrides = changed_parts(&project.layout.style.overrides);
    let current_label = presets.iter().find(|p| p.0 == preset).map(|p| p.1.clone()).unwrap_or(preset.clone());
    let current_builtin = presets.iter().find(|p| p.0 == preset).is_none_or(|p| p.2);
    let swatches: Vec<[String; 3]> = presets
        .iter()
        .map(|(id, _, _)| {
            let (style, _) = project.presets.resolve(&StyleRef { preset: id.clone(), overrides: Default::default() });
            [style.page.background, style.headings.color, style.links.color]
        })
        .collect();
    let width = (ui.available_width() - 6.0) / 2.0;
    egui::Grid::new("modeles").num_columns(2).spacing([6.0, 6.0]).show(ui, |ui| {
        for (index, (id, label, builtin)) in presets.iter().enumerate() {
            let about = if *builtin { describe_preset(id) } else { "Ton modèle" };
            if model_card(ui, width, label, about, &swatches[index], *id == preset) {
                preset = id.clone();
            }
            if index % 2 == 1 {
                ui.end_row();
            }
        }
    });
    ui.add_space(6.0);
    // Ce qui a été changé par-dessus le modèle.
    if !overrides.is_empty() {
        egui::Frame::new().fill(t.surface).stroke(egui::Stroke::new(1.0, t.rule)).inner_margin(10).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(format!("{current_label}, changé à la main")).font(FontId::new(13.0, theme::strong())),
            );
            ui.label(RichText::new(overrides.join(" · ")).color(t.muted));
            if ui.button("↺ Revenir au modèle").clicked() {
                app.edit_layout(&ctx, |layout| layout.style.overrides.clear());
                app.notify_done(format!("Style du modèle « {current_label} » rétabli"));
            }
        });
        ui.add_space(4.0);
    }
    let mut deleted = false;
    ui.horizontal_wrapped(|ui| {
        if ui.button("Enregistrer comme modèle…").on_hover_text("Pour le réutiliser dans d'autres notes").clicked()
        {
            app.save_preset_dialog = Some(String::new());
        }
        if !current_builtin && ui.button("Supprimer ce modèle").clicked() {
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
        let name = presets.iter().find(|p| p.0 == preset).map(|p| p.1.clone()).unwrap_or_default();
        app.edit_layout(&ctx, move |layout| layout.style = StyleRef { preset, overrides: Default::default() });
        app.notify_done(format!("Modèle « {name} »"));
        return;
    }
    ui.add_space(8.0);
    theme::rule(ui, 1.0, false);

    let families = app.families.clone();
    let mut s = app.style.clone();
    ui.add_space(6.0);
    essentials(ui, &mut s, &families);
    ui.add_space(4.0);
    theme::rule(ui, 1.0, false);
    ui.add_space(6.0);
    egui::CollapsingHeader::new(
        RichText::new("Réglages détaillés").font(FontId::new(14.0, theme::strong())).color(t.ink),
    )
    .default_open(false)
    .show(ui, |ui| fields(ui, &mut s, &families));
    set_filter(ui, Filter::default());
    if s != app.style {
        app.edit_style(&ctx, &s);
    }
}

/// Les réglages qu'on change vraiment souvent.
fn essentials(ui: &mut egui::Ui, s: &mut Style, families: &[String]) {
    kicker(ui, "L'essentiel");
    super::widgets::help(ui, "Ce qu'on change le plus souvent. Pour le reste : la recherche, en haut.");
    ui.add_space(2.0);
    grid(ui, "essentiel", |ui| {
        label(ui, "Police du texte");
        font(ui, "police-essentiel", &mut s.text.font, families);
        ui.end_row();
        label(ui, "Taille");
        number(ui, &mut s.text.size_pt, 6.0..=24.0, 0.1, " pt");
        ui.end_row();
        label(ui, "Marges");
        let p = &mut s.page;
        let same = [p.margin_bottom_mm, p.margin_left_mm, p.margin_right_mm]
            .iter()
            .all(|m| (m - p.margin_top_mm).abs() < 0.01);
        ui.horizontal(|ui| {
            let mut all = p.margin_top_mm;
            if number(ui, &mut all, 0.0..=80.0, 0.5, " mm") {
                p.margin_top_mm = all;
                p.margin_bottom_mm = all;
                p.margin_left_mm = all;
                p.margin_right_mm = all;
            }
            if !same {
                ui.label(RichText::new("(inégales)").small().color(theme::tokens(ui.ctx()).faint))
                    .on_hover_text("Chaque marge se règle à part : cherche « marge » plus bas");
            }
        });
        ui.end_row();
        label(ui, "Code");
        let options: Vec<(String, &str)> = THEMES.iter().map(|t| (t.id.to_string(), t.label)).collect();
        choice(ui, "theme-code-essentiel", &mut s.code.theme, &options);
        ui.end_row();
        label(ui, "Page de garde");
        choice(
            ui,
            "garde-essentiel",
            &mut s.cover.mode,
            &[
                (CoverMode::Auto, "Si la note a un titre"),
                (CoverMode::Page, "Page de garde"),
                (CoverMode::Header, "Titre en haut de page"),
                (CoverMode::None, "Aucune"),
            ],
        );
        ui.end_row();
        label(ui, "Pages");
        ui.vertical(|ui| {
            ui.checkbox(&mut s.footer.page_numbers, "Numéros de page");
            ui.checkbox(&mut s.cover.table_of_contents, "Sommaire");
            ui.checkbox(&mut s.headings.numbering, "Titres numérotés (1, 1.1…)");
            ui.checkbox(&mut s.headings.h1_new_page, "Chaque titre 1 sur une nouvelle page");
        });
        ui.end_row();
    });
}

fn fields(ui: &mut egui::Ui, s: &mut Style, families: &[String]) {
    find_section(ui, "Texte", true, |ui| {
        grid(ui, "texte", |ui| {
            row(ui, "Police", |ui| {
                font(ui, "police-texte", &mut s.text.font, families);
            });
            row(ui, "Taille", |ui| {
                number(ui, &mut s.text.size_pt, 6.0..=24.0, 0.1, " pt");
            });
            row(ui, "Couleur", |ui| {
                color(ui, &mut s.text.color);
            });
            row(ui, "Interligne", |ui| {
                number(ui, &mut s.text.line_height, 1.0..=2.5, 0.01, "");
            });
            row(ui, "Entre paragraphes", |ui| {
                number(ui, &mut s.text.paragraph_spacing_em, 0.0..=3.0, 0.02, " em");
            });
            row(ui, "Retrait 1re ligne", |ui| {
                number(ui, &mut s.text.first_line_indent_em, 0.0..=4.0, 0.05, " em");
            });
            row(ui, "Alignement", |ui| {
                choice(ui, "justif", &mut s.text.justify, &[(true, "Justifié"), (false, "À gauche")]);
            });
            row(ui, "Césure", |ui| {
                ui.checkbox(&mut s.text.hyphenate, "Couper les mots en fin de ligne");
            });
            row(ui, "Typographie", |ui| {
                ui.checkbox(&mut s.text.french_typography, "Espaces insécables françaises");
            });
            row(ui, "Lignes isolées", |ui| {
                ui.checkbox(&mut s.text.avoid_widows, "Éviter en haut et en bas de page");
            });
            row(ui, "Formules", |ui| {
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
            });
        });
    });

    find_section(ui, "Placement automatique", false, |ui| {
        check(
            ui,
            &mut s.pagination.keep_intro_with_next,
            "Une phrase finissant par « : » reste avec ce qu'elle annonce",
            None,
        );
        check(
            ui,
            &mut s.pagination.keep_small_blocks,
            "Ne jamais couper une liste courte, un code court, un petit tableau, un encadré",
            None,
        );
        check(
            ui,
            &mut s.pagination.auto_landscape,
            "Tableaux et schémas trop larges : page paysage automatique",
            Some("Le texte reprend ensuite en portrait. Une image verticale reste toujours en portrait."),
        );
        check(
            ui,
            &mut s.pagination.lonely_landscape,
            "Image horizontale seule sur sa page : page paysage, en plus grand",
            Some("Avec son titre et sa légende ; jamais si cela ajoute une page."),
        );
        check(
            ui,
            &mut s.pagination.larger_paper,
            "Très grand tableau : page A3 paysage s'il y tient en entier",
            Some("Seulement s'il ne tient pas sur une page A4 paysage."),
        );
        check(
            ui,
            &mut s.pagination.fit_images,
            "Ajuster la taille des images pour éviter un trou en bas de page",
            Some("Jamais en dessous de 55 % de sa taille."),
        );
        check(
            ui,
            &mut s.pagination.avoid_short_last_page,
            "Resserrer un peu les paragraphes si la dernière page n'a que quelques lignes",
            None,
        );
        if visible(ui, "Photo verticale : hauteur maximale") {
            ui.horizontal(|ui| {
                ui.label("Photo verticale : au plus");
                number(ui, &mut s.images.portrait_max_percent, 30.0..=100.0, 1.0, " %");
                ui.label("de la page");
            });
        }
        if filter(ui).needle.is_empty() {
            super::widgets::help(
                ui,
                "Les titres restent toujours avec leur contenu. Une retouche de bloc l'emporte sur ces règles ; « Garder tel quel » les refuse pour un bloc.",
            );
        }
    });

    find_section(ui, "Titres", false, |ui| {
        grid(ui, "titres", |ui| {
            row(ui, "Police", |ui| {
                font(ui, "police-titres", &mut s.headings.font, families);
            });
            row(ui, "Couleur", |ui| {
                color(ui, &mut s.headings.color);
            });
            row(ui, "Graisse", |ui| {
                choice(ui, "graisse", &mut s.headings.weight, &[(400, "Normale"), (600, "Demi-gras"), (700, "Gras")]);
            });
            row(ui, "Numérotation", |ui| {
                ui.checkbox(&mut s.headings.numbering, "1, 1.1, 1.1.1…");
            });
            row(ui, "Titre 1", |ui| {
                ui.checkbox(&mut s.headings.h1_new_page, "Toujours sur une nouvelle page");
            });
        });
        let base_color = s.headings.color.clone();
        for (i, level) in s.headings.levels.iter_mut().enumerate() {
            find_level(ui, &format!("Titre {}", i + 1), |ui| {
                grid(ui, &format!("niveau-{i}"), |ui| {
                    row(ui, "Taille", |ui| {
                        number(ui, &mut level.size_pt, 6.0..=72.0, 0.1, " pt");
                    });
                    row(ui, "Couleur", |ui| {
                        optional_color(ui, &mut level.color, &base_color);
                    });
                    row(ui, "Police propre", |ui| {
                        ui.horizontal(|ui| {
                            let mut own = level.font.is_some();
                            if ui.checkbox(&mut own, "").changed() {
                                level.font = own.then(|| "IBM Plex Sans".to_string());
                            }
                            if let Some(f) = &mut level.font {
                                font(ui, &format!("police-niveau-{i}"), f, families);
                            }
                        });
                    });
                    row(ui, "Capitales", |ui| {
                        ui.checkbox(&mut level.uppercase, "");
                    });
                    row(ui, "Espacement lettres", |ui| {
                        number(ui, &mut level.tracking_em, 0.0..=0.5, 0.005, " em");
                    });
                    row(ui, "Filet au-dessus", |ui| {
                        number(ui, &mut level.rule_above_pt, 0.0..=6.0, 0.1, " pt");
                    });
                    row(ui, "Filet dessous", |ui| {
                        number(ui, &mut level.rule_below_pt, 0.0..=6.0, 0.1, " pt");
                    });
                    row(ui, "Espace avant", |ui| {
                        number(ui, &mut level.space_above_em, 0.0..=6.0, 0.05, " em");
                    });
                    row(ui, "Espace après", |ui| {
                        number(ui, &mut level.space_below_em, 0.0..=4.0, 0.05, " em");
                    });
                });
            });
        }
    });

    find_section(ui, "Code", false, |ui| {
        grid(ui, "code", |ui| {
            row(ui, "Thème", |ui| {
                let options: Vec<(String, &str)> = THEMES.iter().map(|t| (t.id.to_string(), t.label)).collect();
                choice(ui, "theme-code", &mut s.code.theme, &options);
            });
            row(ui, "Bandeau", |ui| {
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
            });
            row(ui, "Numéros de ligne", |ui| {
                ui.checkbox(&mut s.code.line_numbers, "");
            });
            row(ui, "Police", |ui| {
                font(ui, "police-code", &mut s.code.font, families);
            });
            row(ui, "Taille", |ui| {
                number(ui, &mut s.code.size_pt, 5.0..=16.0, 0.1, " pt");
            });
            row(ui, "Interligne", |ui| {
                number(ui, &mut s.code.line_height, 1.0..=2.2, 0.01, "");
            });
            row(ui, "Arrondi", |ui| {
                number(ui, &mut s.code.radius_pt, 0.0..=16.0, 0.1, " pt");
            });
            row(ui, "Bordure", |ui| {
                ui.checkbox(&mut s.code.border, "");
            });
            row(ui, "Code en ligne : fond", |ui| {
                color(ui, &mut s.code.inline_background);
            });
            row(ui, "Code en ligne : texte", |ui| {
                color(ui, &mut s.code.inline_color);
            });
        });
    });

    find_section(ui, "Schémas (Mermaid)", false, |ui| {
        grid(ui, "schemas", |ui| {
            row(ui, "Thème", |ui| {
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
            });
            row(ui, "Police du texte", |ui| {
                ui.checkbox(&mut s.diagrams.document_font, "Utiliser celle du document");
            });
            row(ui, "Taille", |ui| {
                ui.add(egui::Slider::new(&mut s.diagrams.scale, 0.4..=2.0).fixed_decimals(2));
            });
        });
    });

    find_section(ui, "Encadrés et citations", false, |ui| {
        grid(ui, "encadres", |ui| {
            row(ui, "Encadrés", |ui| {
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
            });
            row(ui, "Couleurs par type", |ui| {
                ui.checkbox(&mut s.callout.colored, "note, astuce, attention…");
            });
            row(ui, "Arrondi", |ui| {
                number(ui, &mut s.callout.radius_pt, 0.0..=16.0, 0.1, " pt");
            });
            row(ui, "Citations", |ui| {
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
            });
            row(ui, "Couleur", |ui| {
                color(ui, &mut s.quote.color);
            });
            row(ui, "Italique", |ui| {
                ui.checkbox(&mut s.quote.italic, "");
            });
        });
    });

    find_section(ui, "Tableaux", false, |ui| {
        grid(ui, "tableaux", |ui| {
            row(ui, "Bordures", |ui| {
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
            });
            row(ui, "Couleur des filets", |ui| {
                color(ui, &mut s.table.border_color);
            });
            row(ui, "En-tête : fond", |ui| {
                color(ui, &mut s.table.header_background);
            });
            row(ui, "En-tête : texte", |ui| {
                color(ui, &mut s.table.header_color);
            });
            row(ui, "Lignes alternées", |ui| {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut s.table.stripes, "");
                    if s.table.stripes {
                        color(ui, &mut s.table.stripe_color);
                    }
                });
            });
            row(ui, "Marge des cellules (côtés)", |ui| {
                number(ui, &mut s.table.cell_padding_x_pt, 0.0..=30.0, 0.1, " pt");
            });
            row(ui, "Marge des cellules (haut, bas)", |ui| {
                number(ui, &mut s.table.cell_padding_y_pt, 0.0..=30.0, 0.1, " pt");
            });
        });
    });

    find_section(ui, "Liens, images, notes", false, |ui| {
        grid(ui, "divers", |ui| {
            row(ui, "Liens", |ui| {
                color(ui, &mut s.links.color);
            });
            row(ui, "Souligner les liens", |ui| {
                ui.checkbox(&mut s.links.underline, "");
            });
            row(ui, "Liens [[internes]]", |ui| {
                color(ui, &mut s.links.wikilink_color);
            });
            row(ui, "Surlignage", |ui| {
                color(ui, &mut s.highlight);
            });
            row(ui, "Taille des images", |ui| {
                ui.add(egui::Slider::new(&mut s.images.scale, 0.3..=1.5).fixed_decimals(2));
            });
            row(ui, "Arrondi des images", |ui| {
                number(ui, &mut s.images.radius_pt, 0.0..=20.0, 0.1, " pt");
            });
            row(ui, "Légendes", |ui| {
                ui.horizontal(|ui| {
                    number(ui, &mut s.images.caption_size_pt, 5.0..=14.0, 0.1, " pt");
                    ui.checkbox(&mut s.images.caption_italic, "italique");
                });
            });
            row(ui, "Couleur des légendes", |ui| {
                color(ui, &mut s.images.caption_color);
            });
            row(ui, "Numéroter les figures", |ui| {
                ui.checkbox(&mut s.images.numbering, "");
            });
            row(ui, "Notes de bas de page", |ui| {
                ui.horizontal(|ui| {
                    number(ui, &mut s.footnotes.size_pt, 5.0..=14.0, 0.1, " pt");
                    color(ui, &mut s.footnotes.color);
                });
            });
            row(ui, "Séparateurs ---", |ui| {
                ui.checkbox(&mut s.rules, "afficher");
            });
        });
    });

    find_section(ui, "Fichier PDF", false, |ui| {
        check(ui, &mut s.export.pdf_a, "PDF/A (archivage, dépôt officiel)", None);
        if visible(ui, "Alléger les photos (poids du fichier)") {
            ui.horizontal(|ui| {
                ui.checkbox(&mut s.export.downscale_images, "Alléger les photos au-delà de");
                ui.add_enabled(
                    s.export.downscale_images,
                    egui::DragValue::new(&mut s.export.max_image_px).range(800..=8000).speed(20).suffix(" px"),
                );
            });
        }
    });

    find_section(ui, "Page", false, |ui| {
        grid(ui, "page", |ui| {
            row(ui, "Marge du haut", |ui| {
                number(ui, &mut s.page.margin_top_mm, 0.0..=80.0, 0.5, " mm");
            });
            row(ui, "Marge du bas", |ui| {
                number(ui, &mut s.page.margin_bottom_mm, 0.0..=80.0, 0.5, " mm");
            });
            row(ui, "Marge de gauche", |ui| {
                number(ui, &mut s.page.margin_left_mm, 0.0..=80.0, 0.5, " mm");
            });
            row(ui, "Marge de droite", |ui| {
                number(ui, &mut s.page.margin_right_mm, 0.0..=80.0, 0.5, " mm");
            });
            row(ui, "Fond de page", |ui| {
                color(ui, &mut s.page.background);
            });
            row(ui, "Pied de page", |ui| {
                ui.add(egui::TextEdit::singleline(&mut s.footer.text).hint_text("{title}").desired_width(190.0))
                    .on_hover_text("{title} est remplacé par le titre de la note");
            });
            row(ui, "Numéros de page", |ui| {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut s.footer.page_numbers, "");
                    choice(
                        ui,
                        "align-pied",
                        &mut s.footer.align,
                        &[("left".into(), "À gauche"), ("center".into(), "Au centre"), ("right".into(), "À droite")],
                    );
                });
            });
            row(ui, "Couleur du pied", |ui| {
                color(ui, &mut s.footer.color);
            });
            row(ui, "Page de garde", |ui| {
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
            });
            row(ui, "Sommaire", |ui| {
                ui.checkbox(&mut s.cover.table_of_contents, "");
            });
        });
    });
}

/// Fenêtre « Enregistrer comme modèle ».
pub fn save_preset_modal(app: &mut NectarApp, ctx: &egui::Context) {
    let Some(mut name) = app.save_preset_dialog.take() else { return };
    let mut keep = true;
    let mut save = false;
    egui::Modal::new(egui::Id::new("enregistrer-preset")).show(ctx, |ui| {
        ui.set_width(340.0);
        kicker(ui, "Nouveau modèle");
        ui.add_space(4.0);
        ui.label("Le style actuel, réglages compris, devient un modèle réutilisable pour toutes tes notes.");
        ui.add_space(8.0);
        let response =
            ui.add(egui::TextEdit::singleline(&mut name).hint_text("Nom du modèle").desired_width(f32::INFINITY));
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
                app.notify(format!("Modèle « {} » enregistré", name.trim()), false);
            }
            Some(Err(e)) => app.notify(format!("Enregistrement du modèle impossible : {e}"), true),
            None => {}
        }
        return;
    }
    if keep && !ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.save_preset_dialog = Some(name);
    }
}

// ------------------------------------------------------------ recherche

/// La recherche dans les réglages détaillés.
#[derive(Clone, Default)]
struct Filter {
    /// Ce qu'on cherche, sans accents ni majuscules ; vide : tout montrer.
    needle: String,
    /// La section entière correspond : toutes ses lignes s'affichent.
    all: bool,
    /// Lignes affichées dans la section en cours.
    count: usize,
}

fn filter_id() -> egui::Id {
    egui::Id::new("recherche-reglages")
}

fn filter(ui: &egui::Ui) -> Filter {
    ui.data(|d| d.get_temp::<Filter>(filter_id())).unwrap_or_default()
}

fn set_filter(ui: &egui::Ui, f: Filter) {
    ui.data_mut(|d| d.insert_temp(filter_id(), f));
}

/// Minuscules, sans accents : « Écart » se trouve en tapant « ecart ».
fn plain(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            other => other,
        })
        .collect()
}

/// Ce réglage correspond-il à la recherche ? (Le compte au passage.)
fn visible(ui: &egui::Ui, text: &str) -> bool {
    let mut f = filter(ui);
    let shown = f.needle.is_empty() || f.all || plain(text).contains(&f.needle);
    if shown {
        f.count += 1;
        set_filter(ui, f);
    }
    shown
}

/// Une ligne libellé / champ d'une grille, cachée si elle ne correspond pas.
fn row(ui: &mut egui::Ui, text: &str, body: impl FnOnce(&mut egui::Ui)) {
    if !visible(ui, text) {
        return;
    }
    label(ui, text);
    body(ui);
    ui.end_row();
}

/// Une case à cocher seule sur sa ligne.
fn check(ui: &mut egui::Ui, value: &mut bool, text: &str, hover: Option<&str>) {
    if !visible(ui, text) {
        return;
    }
    let response = ui.checkbox(value, text);
    if let Some(hover) = hover {
        response.on_hover_text(hover);
    }
}

/// Le nombre de lignes que montrerait `body`, sans rien afficher.
fn matches(ui: &mut egui::Ui, title: &str, body: &mut impl FnMut(&mut egui::Ui)) -> (Filter, usize) {
    let outer = filter(ui);
    let all = outer.all || plain(title).contains(&outer.needle);
    set_filter(ui, Filter { needle: outer.needle.clone(), all, count: 0 });
    let mut dry = ui.new_child(egui::UiBuilder::new().id_salt(("essai", title)).invisible());
    body(&mut dry);
    let count = filter(ui).count;
    (Filter { all, count: 0, ..outer.clone() }, count + usize::from(all))
}

/// Une section des réglages détaillés : ouverte d'office pendant une
/// recherche, absente si rien n'y correspond.
fn find_section(ui: &mut egui::Ui, title: &str, open: bool, mut body: impl FnMut(&mut egui::Ui)) {
    let outer = filter(ui);
    if outer.needle.is_empty() {
        section(ui, title, open, |ui| body(ui));
        return;
    }
    let (inner, count) = matches(ui, title, &mut body);
    if count > 0 {
        set_filter(ui, inner);
        let t = theme::tokens(ui.ctx());
        egui::CollapsingHeader::new(RichText::new(title).font(FontId::new(14.0, theme::strong())).color(t.ink))
            .id_salt(("trouve", title))
            .open(Some(true))
            .show(ui, |ui| body(ui));
        theme::rule(ui, 1.0, false);
    }
    set_filter(ui, Filter { count: outer.count + count, ..outer });
}

/// Un niveau de titre, dans la section Titres.
fn find_level(ui: &mut egui::Ui, title: &str, mut body: impl FnMut(&mut egui::Ui)) {
    let outer = filter(ui);
    if outer.needle.is_empty() {
        egui::CollapsingHeader::new(title).id_salt(("niveau", title)).show(ui, |ui| body(ui));
        return;
    }
    let (inner, count) = matches(ui, title, &mut body);
    if count > 0 {
        set_filter(ui, inner);
        egui::CollapsingHeader::new(title).id_salt(("niveau-trouve", title)).open(Some(true)).show(ui, |ui| body(ui));
    }
    // Les lignes de ce niveau comptent pour la section Titres.
    set_filter(ui, Filter { count: outer.count + count, ..outer });
}

/// À quoi sert un modèle intégré, en quelques mots.
fn describe_preset(id: &str) -> &'static str {
    match id {
        "agrume" => "Moderne et net",
        "academique" => "Mémoire, rapport",
        "magazine" => "Article, titres rouges",
        "entreprise" => "Document pro, sobre",
        "technique" => "Rapport technique",
        "minimal" => "Noir et blanc, épuré",
        "carnet" => "Notes de cours",
        "creatif" => "Original, violet",
        "developpeur" => "Doc de code",
        "elegant" => "Texte long, raffiné",
        _ => "",
    }
}

/// Une carte de modèle : ses couleurs, son nom, à quoi il sert.
fn model_card(ui: &mut egui::Ui, width: f32, name: &str, about: &str, colors: &[String; 3], selected: bool) -> bool {
    let t = theme::tokens(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 48.0), egui::Sense::click());
    let painter = ui.painter_at(rect);
    let fill = if selected {
        t.accent
    } else if response.hovered() {
        t.surface
    } else {
        t.raised
    };
    painter.rect_filled(rect, 0.0, fill);
    let stroke = if selected { egui::Stroke::new(2.0, t.ink) } else { egui::Stroke::new(1.0, t.rule_strong) };
    painter.rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Inside);
    // Fond de page, couleur des titres, couleur des liens.
    for (i, hex) in colors.iter().enumerate() {
        let [r, g, b] = super::widgets::parse_hex(hex).unwrap_or([255, 255, 255]);
        let swatch =
            egui::Rect::from_min_size(rect.left_top() + egui::vec2(8.0 + i as f32 * 9.0, 10.0), egui::vec2(8.0, 28.0));
        painter.rect_filled(swatch, 0.0, egui::Color32::from_rgb(r, g, b));
        painter.rect_stroke(swatch, 0.0, egui::Stroke::new(1.0, t.rule), egui::StrokeKind::Inside);
    }
    let ink = if selected { t.accent_ink } else { t.ink };
    let x = rect.left() + 42.0;
    painter.text(
        egui::pos2(x, rect.top() + 9.0),
        egui::Align2::LEFT_TOP,
        name,
        FontId::new(13.5, theme::strong()),
        ink,
    );
    painter.text(
        egui::pos2(x, rect.top() + 27.0),
        egui::Align2::LEFT_TOP,
        about,
        FontId::proportional(11.0),
        if selected { t.accent_ink } else { t.muted },
    );
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response.clicked()
}

/// Le nom d'une partie du style, telle qu'on la range dans les réglages.
fn part_name(key: &str) -> &'static str {
    match key {
        "text" => "Texte",
        "headings" => "Titres",
        "code" => "Code",
        "page" => "Page",
        "footer" => "Pied de page",
        "cover" => "Page de garde",
        "table" => "Tableaux",
        "images" => "Images",
        "links" => "Liens",
        "callout" => "Encadrés",
        "quote" => "Citations",
        "diagrams" => "Schémas",
        "pagination" => "Placement automatique",
        "export" => "Fichier PDF",
        "footnotes" => "Notes",
        "highlight" => "Surlignage",
        "rules" => "Séparateurs",
        _ => "Autre réglage",
    }
}

/// Les réglages changés par-dessus le modèle, nommés comme dans le panneau.
fn changed_parts(overrides: &serde_json::Map<String, serde_json::Value>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (section, value) in overrides {
        let fields: Vec<&String> = value.as_object().map(|o| o.keys().collect()).unwrap_or_default();
        // Un réglage sans nom précis est désigné par sa partie (« Texte »).
        let mut unnamed = fields.is_empty();
        for field in fields {
            let name = match (section.as_str(), field.as_str()) {
                ("text", "font") => "Police du texte",
                ("text", "size_pt") => "Taille du texte",
                ("text", "line_height") => "Interligne",
                ("text", "justify") => "Alignement du texte",
                ("text", "color") => "Couleur du texte",
                ("page", f) if f.starts_with("margin") => "Marges",
                ("page", "background") => "Fond de page",
                ("headings", "font") => "Police des titres",
                ("headings", "color") => "Couleur des titres",
                ("headings", "levels") => "Titres (niveaux)",
                ("headings", "numbering") => "Titres numérotés",
                ("headings", "h1_new_page") => "Titre 1 sur une nouvelle page",
                ("code", "theme") => "Thème du code",
                ("cover", "mode") => "Page de garde",
                ("cover", "table_of_contents") => "Sommaire",
                ("footer", "page_numbers") => "Numéros de page",
                ("footer", "text") => "Pied de page",
                ("links", "color") => "Couleur des liens",
                ("table", "variant") => "Bordures des tableaux",
                _ => {
                    unnamed = true;
                    continue;
                }
            };
            if !out.iter().any(|o| o == name) {
                out.push(name.to_string());
            }
        }
        if unnamed {
            let name = part_name(section).to_string();
            if !out.contains(&name) {
                out.push(name);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_settings_are_named_as_in_the_panel() {
        let overrides: serde_json::Map<String, serde_json::Value> = serde_json::from_str(
            r#"{"text": {"size_pt": 11, "paragraph_spacing_em": 1}, "cover": {"table_of_contents": true}, "rules": false}"#,
        )
        .unwrap();
        assert_eq!(changed_parts(&overrides), ["Sommaire", "Séparateurs", "Taille du texte", "Texte"]);
        assert_eq!(plain("Écart des Lignes"), "ecart des lignes");
    }
}
