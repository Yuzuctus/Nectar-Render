//! Petits champs réutilisés par les panneaux.

use eframe::egui::{self, RichText};

use crate::theme;

/// Champ de couleur `#rrggbb` : pastille + valeur hexadécimale.
pub fn color(ui: &mut egui::Ui, value: &mut String) -> bool {
    let mut rgb = parse_hex(value).unwrap_or([0, 0, 0]);
    let mut changed = false;
    ui.horizontal(|ui| {
        if ui.color_edit_button_srgb(&mut rgb).changed() {
            *value = format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]);
            changed = true;
        }
        let mut text = value.clone();
        let edit = egui::TextEdit::singleline(&mut text).desired_width(78.0).font(egui::TextStyle::Monospace);
        if ui.add(edit).changed() && parse_hex(&text).is_some() {
            *value = text;
            changed = true;
        }
    });
    changed
}

/// Couleur facultative : « par défaut » ou une couleur propre.
pub fn optional_color(ui: &mut egui::Ui, value: &mut Option<String>, fallback: &str) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        let mut own = value.is_some();
        if ui.checkbox(&mut own, "").on_hover_text("Couleur propre à ce niveau").changed() {
            *value = own.then(|| fallback.to_string());
            changed = true;
        }
        match value {
            Some(v) => changed |= color(ui, v),
            None => {
                ui.label(RichText::new("comme les titres").color(theme::tokens(ui.ctx()).faint));
            }
        }
    });
    changed
}

pub fn parse_hex(value: &str) -> Option<[u8; 3]> {
    let hex = value.trim().strip_prefix('#')?;
    if hex.len() != 6 && hex.len() != 8 {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

/// Choix d'une police parmi celles de la machine et de Nectar.
pub fn font(ui: &mut egui::Ui, id: &str, value: &mut String, families: &[String]) -> bool {
    let mut changed = false;
    let mut filter = ui.data_mut(|d| d.get_temp::<String>(egui::Id::new((id, "filtre"))).unwrap_or_default());
    egui::ComboBox::from_id_salt(id).selected_text(value.as_str()).width(165.0).height(320.0).show_ui(ui, |ui| {
        ui.add(egui::TextEdit::singleline(&mut filter).hint_text("Chercher…").desired_width(170.0));
        let needle = filter.to_lowercase();
        let suggested =
            ["IBM Plex Sans", "IBM Plex Sans Condensed", "IBM Plex Mono", "JetBrains Mono", "Libertinus Serif"];
        let mut seen = std::collections::HashSet::new();
        for name in suggested.iter().map(|s| s.to_string()).chain(families.iter().cloned()) {
            if !seen.insert(name.to_lowercase()) || (!needle.is_empty() && !name.to_lowercase().contains(&needle)) {
                continue;
            }
            if ui.selectable_label(*value == name, &name).clicked() {
                *value = name;
                changed = true;
            }
        }
    });
    ui.data_mut(|d| d.insert_temp(egui::Id::new((id, "filtre")), filter));
    changed
}

/// Liste déroulante sur une énumération sérialisée (`"tab"`, `"window"`…).
pub fn choice<T: PartialEq + Clone>(ui: &mut egui::Ui, id: &str, value: &mut T, options: &[(T, &str)]) -> bool {
    let mut changed = false;
    let current = options.iter().find(|(v, _)| v == value).map(|(_, l)| *l).unwrap_or("—");
    egui::ComboBox::from_id_salt(id).selected_text(current).width(165.0).show_ui(ui, |ui| {
        for (option, label) in options {
            if ui.selectable_label(option == value, *label).clicked() && option != value {
                *value = option.clone();
                changed = true;
            }
        }
    });
    changed
}

pub fn number(
    ui: &mut egui::Ui,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    speed: f64,
    suffix: &str,
) -> bool {
    // Virgule décimale, à la française : « 10,5 pt ».
    let drag = egui::DragValue::new(value)
        .range(range)
        .speed(speed)
        .suffix(suffix)
        .max_decimals(2)
        .custom_formatter(|v, decimals| {
            let text = format!("{v:.*}", decimals.end().min(&2).to_owned());
            let text =
                if text.contains('.') { text.trim_end_matches('0').trim_end_matches('.').to_string() } else { text };
            text.replace('.', ",")
        })
        .custom_parser(|text| text.trim().replace(',', ".").parse::<f64>().ok());
    ui.add(drag).changed()
}

/// Un intertitre de section dans un panneau.
pub fn section(ui: &mut egui::Ui, title: &str, open: bool, body: impl FnOnce(&mut egui::Ui)) {
    let t = theme::tokens(ui.ctx());
    egui::CollapsingHeader::new(RichText::new(title).font(egui::FontId::new(14.0, theme::strong())).color(t.ink))
        .default_open(open)
        .show(ui, |ui| {
            ui.add_space(2.0);
            body(ui);
            ui.add_space(4.0);
        });
    theme::rule(ui, 1.0, false);
}

/// Grille libellé / champ.
pub fn grid(ui: &mut egui::Ui, id: &str, body: impl FnOnce(&mut egui::Ui)) {
    egui::Grid::new(id).num_columns(2).spacing([10.0, 7.0]).min_col_width(96.0).show(ui, body);
}

pub fn label(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).color(theme::tokens(ui.ctx()).muted));
}

/// Choix exclusif en boutons côte à côte (plus rapide qu'une liste).
pub fn segmented<T: PartialEq + Clone>(ui: &mut egui::Ui, value: &mut T, options: &[(T, &str)]) -> bool {
    let t = theme::tokens(ui.ctx());
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 4.0);
        for (option, label) in options {
            let selected = option == value;
            let text = RichText::new(*label).color(if selected { t.accent_ink } else { t.ink });
            let button = egui::Button::new(text).fill(if selected { t.accent } else { t.raised });
            if ui.add(button).clicked() && !selected {
                *value = option.clone();
                changed = true;
            }
        }
    });
    changed
}

/// Bouton à bascule : jaune (et coché) quand il est actif.
pub fn toggle(ui: &mut egui::Ui, on: bool, label: &str) -> egui::Response {
    let t = theme::tokens(ui.ctx());
    let text = RichText::new(label).color(if on { t.accent_ink } else { t.ink });
    let text = if on { RichText::new(format!("✔ {label}")).color(t.accent_ink) } else { text };
    ui.add(egui::Button::new(text).fill(if on { t.accent } else { t.raised }))
}

/// Une case à cocher suivie de son explication.
pub fn explained_check(ui: &mut egui::Ui, value: &mut bool, label: &str, explain: &str) -> bool {
    let changed = ui.checkbox(value, label).on_hover_text(explain).changed();
    ui.indent(label, |ui| help(ui, explain));
    changed
}

/// Une phrase d'aide discrète.
pub fn help(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).small().color(theme::tokens(ui.ctx()).faint));
}
