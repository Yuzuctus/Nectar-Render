//! Traduction du [`Style`] en module Typst (`/nectar/style.typ`).
//!
//! Chaque police devient un dictionnaire `(font: (choisie, secours…),
//! stretch: …)` : si la police demandée manque sur la machine, Typst prend
//! la suivante, de la même famille de dessin (serif, sans, mono).

use serde_json::Value;

use crate::code_themes;
use crate::codegen::{array, num, string};
use crate::style::Style;

const SERIF: &[&str] = &[
    "georgia",
    "times new roman",
    "cambria",
    "garamond",
    "palatino linotype",
    "book antiqua",
    "constantia",
    "libertinus serif",
    "new computer modern",
    "baskerville",
    "merriweather",
    "ibm plex serif",
];
const MONO: &[&str] = &[
    "consolas",
    "courier new",
    "cascadia code",
    "cascadia mono",
    "jetbrains mono",
    "fira code",
    "ibm plex mono",
    "dejavu sans mono",
    "lucida console",
    "source code pro",
];

/// Source du module de style et liste des polices demandées.
pub(crate) fn style_module(style: &Style) -> (String, Vec<String>) {
    let mut fonts = Vec::new();
    let value = serde_json::to_value(style).expect("style sérialisable");
    let dict = to_typst(&value, "", &mut fonts);
    let theme = code_themes::get(&style.code.theme).unwrap_or(&code_themes::THEMES[0]);
    let code_theme = format!(
        "(background: {}, foreground: {}, gutter: {}, header: {}, border: {}, dark: {})",
        string(theme.background),
        string(theme.foreground),
        string(theme.gutter),
        string(theme.header),
        string(theme.border),
        theme.dark
    );
    fonts.sort();
    fonts.dedup();
    let source = format!(
        "// Généré par Nectar Render à partir du preset et des réglages de la note.\n\
         #let style = {dict}\n\
         #let code-theme = {code_theme}\n"
    );
    (source, fonts)
}

fn to_typst(value: &Value, key: &str, fonts: &mut Vec<String>) -> String {
    match value {
        Value::Null => "none".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => match n.as_f64() {
            Some(f) if n.is_f64() => num(f as f32),
            _ => n.to_string(),
        },
        Value::String(s) if key == "font" => {
            fonts.push(s.clone());
            font_spec(s)
        }
        Value::String(s) => string(s),
        Value::Array(items) => array(&items.iter().map(|v| to_typst(v, "", fonts)).collect::<Vec<_>>()),
        Value::Object(map) => {
            if map.is_empty() {
                return "(:)".into();
            }
            let entries: Vec<String> = map.iter().map(|(k, v)| format!("{}: {}", k, to_typst(v, k, fonts))).collect();
            format!("({})", entries.join(", "))
        }
    }
}

/// `"Georgia"` → `(font: ("Georgia", "Libertinus Serif"), stretch: 100%)`.
pub(crate) fn font_spec(name: &str) -> String {
    let lower = name.trim().to_lowercase();
    let (family, stretch) = match lower.strip_suffix(" condensed") {
        Some(_) => (name.trim()[..name.trim().len() - " condensed".len()].to_string(), "75%"),
        None => (name.trim().to_string(), "100%"),
    };
    let family_lower = family.to_lowercase();
    let fallbacks: &[&str] = if SERIF.contains(&family_lower.as_str()) || family_lower.contains("serif") {
        &["Libertinus Serif"]
    } else if MONO.contains(&family_lower.as_str()) || family_lower.contains("mono") || family_lower.contains("code") {
        &["JetBrains Mono", "IBM Plex Mono", "DejaVu Sans Mono"]
    } else {
        &["IBM Plex Sans"]
    };
    let mut list = vec![string(&family)];
    list.extend(fallbacks.iter().filter(|f| !f.eq_ignore_ascii_case(&family)).map(|f| string(f)));
    format!("(font: {}, stretch: {stretch})", array(&list))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fonts_get_fallbacks_of_the_same_kind() {
        assert_eq!(font_spec("Georgia"), "(font: (\"Georgia\", \"Libertinus Serif\"), stretch: 100%)");
        assert!(font_spec("Consolas").contains("\"JetBrains Mono\""));
        assert_eq!(font_spec("IBM Plex Sans Condensed"), "(font: (\"IBM Plex Sans\",), stretch: 75%)");
    }

    #[test]
    fn module_lists_requested_fonts() {
        let (source, fonts) = style_module(&Style::default());
        assert!(source.starts_with("// Généré"));
        assert!(source.contains("#let code-theme = (background: \"#1e1e1e\""));
        assert!(fonts.contains(&"JetBrains Mono".to_string()));
    }
}
