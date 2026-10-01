//! Le style d'un PDF : tout ce qu'on règle à la main, regroupé par élément.
//!
//! Un style se construit en superposant trois couches JSON :
//! `Style::default()` ← preset (intégré ou personnel) ← retouches de la note.
//! Chaque couche peut être partielle : seuls les champs présents remplacent
//! ceux de la couche du dessous.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Couleur `#rrggbb` (ou `#rrggbbaa`).
pub type Color = String;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Style {
    pub page: PageStyle,
    pub text: TextStyle,
    pub headings: HeadingsStyle,
    pub code: CodeStyle,
    pub links: LinkStyle,
    pub quote: QuoteStyle,
    pub callout: CalloutStyle,
    pub table: TableStyle,
    pub images: ImageStyle,
    pub footnotes: FootnoteStyle,
    pub footer: FooterStyle,
    pub cover: CoverStyle,
    pub diagrams: DiagramStyle,
    pub pagination: PaginationStyle,
    /// Fond du `==surlignage==`.
    pub highlight: Color,
    /// Afficher les séparateurs `---`.
    pub rules: bool,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            page: PageStyle::default(),
            text: TextStyle::default(),
            headings: HeadingsStyle::default(),
            code: CodeStyle::default(),
            links: LinkStyle::default(),
            quote: QuoteStyle::default(),
            callout: CalloutStyle::default(),
            table: TableStyle::default(),
            images: ImageStyle::default(),
            footnotes: FootnoteStyle::default(),
            footer: FooterStyle::default(),
            cover: CoverStyle::default(),
            diagrams: DiagramStyle::default(),
            pagination: PaginationStyle::default(),
            highlight: "#fff3a3".into(),
            rules: true,
        }
    }
}

/// Règles de placement automatiques, avant toute retouche.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PaginationStyle {
    /// Une phrase qui finit par « : » reste sur la page de ce qu'elle annonce.
    pub keep_intro_with_next: bool,
    /// Les petits blocs (liste courte, code court, petit tableau, encadré)
    /// ne sont jamais coupés entre deux pages.
    pub keep_small_blocks: bool,
}

impl Default for PaginationStyle {
    fn default() -> Self {
        Self { keep_intro_with_next: true, keep_small_blocks: true }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DiagramStyle {
    /// Thème Mermaid : `default`, `neutral`, `forest`, `dark`, `base`.
    pub theme: String,
    /// Écrire les diagrammes avec la police du texte.
    pub document_font: bool,
    /// Taille par rapport à la taille naturelle du diagramme.
    pub scale: f32,
}

impl Default for DiagramStyle {
    fn default() -> Self {
        Self { theme: "neutral".into(), document_font: true, scale: 1.0 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PageStyle {
    pub margin_top_mm: f32,
    pub margin_right_mm: f32,
    pub margin_bottom_mm: f32,
    pub margin_left_mm: f32,
    pub background: Color,
}

impl Default for PageStyle {
    fn default() -> Self {
        Self {
            margin_top_mm: 22.0,
            margin_right_mm: 22.0,
            margin_bottom_mm: 24.0,
            margin_left_mm: 22.0,
            background: "#ffffff".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextStyle {
    pub font: String,
    pub size_pt: f32,
    pub color: Color,
    /// Interligne, comme en CSS (1.5 = une ligne et demie).
    pub line_height: f32,
    pub justify: bool,
    pub hyphenate: bool,
    /// Espace entre paragraphes, en em.
    pub paragraph_spacing_em: f32,
    /// Retrait de première ligne, en em (0 = aucun).
    pub first_line_indent_em: f32,
    /// Espaces insécables françaises (devant `; : ! ?`, dans « »).
    pub french_typography: bool,
    /// Éviter les lignes isolées en haut ou en bas de page (veuves, orphelines).
    pub avoid_widows: bool,
    /// Police des formules (police mathématique OpenType).
    pub math_font: String,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font: "IBM Plex Sans".into(),
            size_pt: 10.5,
            color: "#1f2328".into(),
            line_height: 1.5,
            justify: true,
            hyphenate: true,
            paragraph_spacing_em: 0.9,
            first_line_indent_em: 0.0,
            french_typography: true,
            avoid_widows: true,
            math_font: "New Computer Modern Math".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HeadingsStyle {
    pub font: String,
    pub color: Color,
    pub weight: u16,
    /// Numéroter les titres (1, 1.1, 1.1.1…).
    pub numbering: bool,
    /// Un titre de niveau 1 commence une nouvelle page.
    pub h1_new_page: bool,
    /// H1 à H6.
    pub levels: [HeadingLevel; 6],
}

impl Default for HeadingsStyle {
    fn default() -> Self {
        let level = |size_pt: f32| HeadingLevel { size_pt, ..HeadingLevel::default() };
        Self {
            font: "IBM Plex Sans".into(),
            color: "#1f2328".into(),
            weight: 600,
            numbering: false,
            h1_new_page: false,
            levels: [level(22.0), level(16.5), level(13.0), level(11.5), level(10.5), level(10.0)],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HeadingLevel {
    pub size_pt: f32,
    /// Couleur propre au niveau (sinon celle des titres).
    pub color: Option<Color>,
    /// Police propre au niveau (sinon celle des titres).
    pub font: Option<String>,
    pub uppercase: bool,
    /// Espacement des lettres, en em.
    pub tracking_em: f32,
    /// Filet au-dessus du titre (épaisseur en pt, 0 = aucun).
    pub rule_above_pt: f32,
    /// Filet sous le titre (épaisseur en pt, 0 = aucun).
    pub rule_below_pt: f32,
    pub space_above_em: f32,
    pub space_below_em: f32,
}

impl Default for HeadingLevel {
    fn default() -> Self {
        Self {
            size_pt: 12.0,
            color: None,
            font: None,
            uppercase: false,
            tracking_em: 0.0,
            rule_above_pt: 0.0,
            rule_below_pt: 0.0,
            space_above_em: 1.6,
            space_below_em: 0.7,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CodeStyle {
    pub font: String,
    pub size_pt: f32,
    pub line_height: f32,
    /// Thème de coloration (voir [`crate::code_themes`]).
    pub theme: String,
    pub line_numbers: bool,
    /// Bandeau au-dessus du bloc : `none`, `tab` (onglet avec le langage,
    /// façon VS Code) ou `window` (barre de fenêtre).
    pub header: CodeHeader,
    pub radius_pt: f32,
    /// Bordure fine autour du bloc.
    pub border: bool,
    /// Code en ligne : fond (vide = celui du thème clair par défaut).
    pub inline_background: Color,
    pub inline_color: Color,
}

impl Default for CodeStyle {
    fn default() -> Self {
        Self {
            font: "JetBrains Mono".into(),
            size_pt: 8.8,
            line_height: 1.45,
            theme: "vscode-dark".into(),
            line_numbers: true,
            header: CodeHeader::Tab,
            radius_pt: 6.0,
            border: false,
            inline_background: "#eff1f3".into(),
            inline_color: "#1f2328".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CodeHeader {
    None,
    Tab,
    Window,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LinkStyle {
    pub color: Color,
    pub underline: bool,
    /// Couleur des liens internes `[[note]]` (sans cible dans un PDF).
    pub wikilink_color: Color,
}

impl Default for LinkStyle {
    fn default() -> Self {
        Self { color: "#0969da".into(), underline: false, wikilink_color: "#1f2328".into() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct QuoteStyle {
    pub variant: QuoteVariant,
    pub color: Color,
    pub italic: bool,
}

impl Default for QuoteStyle {
    fn default() -> Self {
        Self { variant: QuoteVariant::Indent, color: "#57606a".into(), italic: true }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum QuoteVariant {
    /// Simple retrait.
    Indent,
    /// Filet fin à gauche.
    Bar,
    /// Grand guillemet ouvrant.
    Quotes,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CalloutStyle {
    pub variant: CalloutVariant,
    pub radius_pt: f32,
    /// Utiliser la couleur propre à chaque type (note, astuce, attention…).
    pub colored: bool,
}

impl Default for CalloutStyle {
    fn default() -> Self {
        Self { variant: CalloutVariant::Bordered, radius_pt: 4.0, colored: true }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CalloutVariant {
    /// Cadre fin, titre en couleur, pas de fond.
    Bordered,
    /// Fond très léger, coins arrondis (comme Obsidian).
    Soft,
    /// Aucun cadre : titre en gras puis texte en retrait.
    Plain,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TableStyle {
    pub variant: TableVariant,
    pub header_background: Color,
    pub header_color: Color,
    pub stripes: bool,
    pub stripe_color: Color,
    pub border_color: Color,
    pub cell_padding_x_pt: f32,
    pub cell_padding_y_pt: f32,
    pub size_pt: Option<f32>,
}

impl Default for TableStyle {
    fn default() -> Self {
        Self {
            variant: TableVariant::Rules,
            header_background: "#ffffff".into(),
            header_color: "#1f2328".into(),
            stripes: false,
            stripe_color: "#f6f8fa".into(),
            border_color: "#d0d7de".into(),
            cell_padding_x_pt: 7.0,
            cell_padding_y_pt: 5.0,
            size_pt: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TableVariant {
    /// Filets horizontaux seulement.
    Rules,
    /// Toutes les bordures.
    Grid,
    /// Aucune bordure (rayures conseillées).
    Plain,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ImageStyle {
    /// Taille par défaut, relative à la taille d'affichage d'Obsidian.
    pub scale: f32,
    pub radius_pt: f32,
    pub caption_size_pt: f32,
    pub caption_color: Color,
    pub caption_italic: bool,
    /// Numéroter les figures légendées.
    pub numbering: bool,
}

impl Default for ImageStyle {
    fn default() -> Self {
        Self {
            scale: 1.0,
            radius_pt: 0.0,
            caption_size_pt: 8.5,
            caption_color: "#57606a".into(),
            caption_italic: false,
            numbering: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FootnoteStyle {
    pub size_pt: f32,
    pub color: Color,
}

impl Default for FootnoteStyle {
    fn default() -> Self {
        Self { size_pt: 8.5, color: "#57606a".into() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FooterStyle {
    /// Texte libre ; `{title}` est remplacé par le titre de la note.
    pub text: String,
    pub page_numbers: bool,
    /// `left`, `center` ou `right` (pour le numéro de page).
    pub align: String,
    pub color: Color,
    pub size_pt: f32,
}

impl Default for FooterStyle {
    fn default() -> Self {
        Self {
            text: "{title}".into(),
            page_numbers: true,
            align: "right".into(),
            color: "#6e7781".into(),
            size_pt: 8.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CoverStyle {
    /// `auto` : page de garde si la note a un titre.
    pub mode: CoverMode,
    pub table_of_contents: bool,
    pub title_size_pt: f32,
}

impl Default for CoverStyle {
    fn default() -> Self {
        Self { mode: CoverMode::Auto, table_of_contents: false, title_size_pt: 34.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CoverMode {
    Auto,
    /// Grande page de garde.
    Page,
    /// Titre en haut de la première page.
    Header,
    /// Pas de titre.
    None,
}

// ------------------------------------------------------------------ presets

/// Référence de style rangée dans les retouches d'une note.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StyleRef {
    pub preset: String,
    /// Réglages faits à la main pour cette note, par-dessus le preset.
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub overrides: serde_json::Map<String, Value>,
}

impl Default for StyleRef {
    fn default() -> Self {
        Self { preset: DEFAULT_PRESET.into(), overrides: serde_json::Map::new() }
    }
}

pub const DEFAULT_PRESET: &str = "agrume";

/// Un preset : un nom et une couche de réglages.
#[derive(Debug, Clone, PartialEq)]
pub struct Preset {
    pub id: String,
    pub label: String,
    pub builtin: bool,
    pub layer: Value,
}

macro_rules! builtin {
    ($($id:literal => $label:literal),* $(,)?) => {
        &[$(($id, $label, include_str!(concat!("../../../assets/presets/", $id, ".json")))),*]
    };
}

const BUILTIN: &[(&str, &str, &str)] = builtin![
    "agrume" => "Agrume",
    "academique" => "Académique",
    "magazine" => "Magazine",
    "entreprise" => "Entreprise",
    "technique" => "Technique",
    "minimal" => "Minimal",
    "carnet" => "Carnet",
    "creatif" => "Créatif",
    "developpeur" => "Développeur",
    "elegant" => "Élégant",
];

/// Les presets intégrés, dans l'ordre d'affichage.
pub fn builtin_presets() -> Vec<Preset> {
    BUILTIN
        .iter()
        .map(|(id, label, json)| Preset {
            id: (*id).into(),
            label: (*label).into(),
            builtin: true,
            layer: serde_json::from_str(json).expect("preset intégré valide"),
        })
        .collect()
}

/// Presets intégrés + presets personnels d'un dossier (`*.json`).
#[derive(Debug, Clone)]
pub struct PresetStore {
    pub user_dir: Option<PathBuf>,
    presets: Vec<Preset>,
}

impl PresetStore {
    pub fn builtin_only() -> Self {
        Self { user_dir: None, presets: builtin_presets() }
    }

    pub fn load(user_dir: Option<PathBuf>) -> Self {
        let mut presets = builtin_presets();
        if let Some(dir) = &user_dir
            && let Ok(entries) = std::fs::read_dir(dir)
        {
            let mut user: Vec<Preset> = entries
                .flatten()
                .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
                .filter_map(|e| {
                    let path = e.path();
                    let layer: Value = serde_json::from_str(&std::fs::read_to_string(&path).ok()?).ok()?;
                    let id = path.file_stem()?.to_string_lossy().into_owned();
                    let label = layer.get("label").and_then(Value::as_str).unwrap_or(&id).to_string();
                    Some(Preset { id, label, builtin: false, layer })
                })
                .collect();
            user.sort_by_key(|a| a.label.to_lowercase());
            presets.extend(user);
        }
        Self { user_dir, presets }
    }

    pub fn presets(&self) -> &[Preset] {
        &self.presets
    }

    pub fn get(&self, id: &str) -> Option<&Preset> {
        self.presets.iter().find(|p| p.id.eq_ignore_ascii_case(id))
    }

    /// Le style complet d'une note.
    pub fn resolve(&self, style: &StyleRef) -> (Style, Option<String>) {
        let (layer, warning) = match self.get(&style.preset) {
            Some(preset) => (preset.layer.clone(), None),
            None => (Value::Null, Some(format!("preset « {} » introuvable, style par défaut utilisé", style.preset))),
        };
        (compose(&[&layer, &Value::Object(style.overrides.clone())]), warning)
    }

    /// Enregistre un style complet comme preset personnel.
    pub fn save_user(&mut self, label: &str, style: &Style) -> std::io::Result<String> {
        let dir = self.user_dir.clone().ok_or_else(|| std::io::Error::other("aucun dossier de presets personnels"))?;
        std::fs::create_dir_all(&dir)?;
        let id = slug(label);
        let mut layer = serde_json::to_value(style).expect("style sérialisable");
        layer["label"] = Value::String(label.to_string());
        let path = dir.join(format!("{id}.json"));
        std::fs::write(&path, serde_json::to_string_pretty(&layer).expect("json") + "\n")?;
        *self = Self::load(Some(dir));
        Ok(id)
    }

    pub fn delete_user(&mut self, id: &str) -> std::io::Result<()> {
        let Some(dir) = self.user_dir.clone() else { return Ok(()) };
        std::fs::remove_file(dir.join(format!("{id}.json")))?;
        *self = Self::load(Some(dir));
        Ok(())
    }
}

/// Superpose des couches JSON sur le style par défaut.
pub fn compose(layers: &[&Value]) -> Style {
    let mut base = serde_json::to_value(Style::default()).expect("style sérialisable");
    for layer in layers {
        merge(&mut base, layer);
    }
    serde_json::from_value(base).unwrap_or_default()
}

/// Fusion profonde : les objets se combinent, le reste remplace.
pub fn merge(base: &mut Value, layer: &Value) {
    match (base, layer) {
        (Value::Object(base), Value::Object(layer)) => {
            for (key, value) in layer {
                if key == "label" {
                    continue;
                }
                match base.get_mut(key) {
                    Some(slot) => merge(slot, value),
                    None => {
                        base.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        // Les tableaux de niveaux de titres se fusionnent case par case.
        (Value::Array(base), Value::Array(layer)) if base.iter().all(Value::is_object) => {
            for (slot, value) in base.iter_mut().zip(layer) {
                merge(slot, value);
            }
        }
        (base, layer) => *base = layer.clone(),
    }
}

/// Différence entre deux styles, sous forme de couche partielle : ce qui
/// change de `base` à `edited`. Sert à ne ranger que les réglages modifiés.
pub fn diff(base: &Style, edited: &Style) -> serde_json::Map<String, Value> {
    let a = serde_json::to_value(base).expect("json");
    let b = serde_json::to_value(edited).expect("json");
    match diff_value(&a, &b) {
        Some(Value::Object(map)) => map,
        _ => serde_json::Map::new(),
    }
}

fn diff_value(a: &Value, b: &Value) -> Option<Value> {
    match (a, b) {
        (Value::Object(a), Value::Object(b)) => {
            let mut out = serde_json::Map::new();
            for (key, vb) in b {
                match a.get(key) {
                    Some(va) => {
                        if let Some(d) = diff_value(va, vb) {
                            out.insert(key.clone(), d);
                        }
                    }
                    None => {
                        out.insert(key.clone(), vb.clone());
                    }
                }
            }
            (!out.is_empty()).then_some(Value::Object(out))
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() && a.iter().all(Value::is_object) => {
            let parts: Vec<Value> = a
                .iter()
                .zip(b)
                .map(|(x, y)| diff_value(x, y).unwrap_or_else(|| Value::Object(Default::default())))
                .collect();
            parts.iter().any(|p| p.as_object().is_none_or(|o| !o.is_empty())).then_some(Value::Array(parts))
        }
        _ => (a != b).then(|| b.clone()),
    }
}

fn slug(label: &str) -> String {
    let mut out = String::new();
    for c in label.to_lowercase().chars() {
        let c = match c {
            'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            c => c,
        };
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() { "preset".into() } else { out }
}

/// Dossier des presets personnels sous Windows : `%APPDATA%\Nectar Render\presets`.
pub fn default_user_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join(".config")))
        .map(|base| base.join("Nectar Render").join("presets"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_preset_is_valid() {
        for preset in builtin_presets() {
            let layer = &preset.layer;
            // Une clé inconnue serait ignorée en silence : on la refuse ici.
            let style = compose(&[layer]);
            let back = serde_json::to_value(&style).unwrap();
            assert_unknown_keys(layer, &back, &preset.id);
            assert!(crate::code_themes::get(&style.code.theme).is_some(), "{}: thème {}", preset.id, style.code.theme);
        }
    }

    fn assert_unknown_keys(layer: &Value, full: &Value, path: &str) {
        match (layer, full) {
            (Value::Object(l), Value::Object(f)) => {
                for (k, v) in l {
                    if k == "label" {
                        continue;
                    }
                    let inner = f.get(k).unwrap_or_else(|| panic!("clé inconnue {path}.{k}"));
                    assert_unknown_keys(v, inner, &format!("{path}.{k}"));
                }
            }
            (Value::Array(l), Value::Array(f)) => {
                for (i, (a, b)) in l.iter().zip(f).enumerate() {
                    assert_unknown_keys(a, b, &format!("{path}[{i}]"));
                }
            }
            _ => {}
        }
    }

    #[test]
    fn layers_merge_deeply() {
        let preset =
            serde_json::json!({ "text": { "size_pt": 12.0 }, "headings": { "levels": [{ "size_pt": 30.0 }] } });
        let overrides = serde_json::json!({ "text": { "color": "#ff0000" } });
        let style = compose(&[&preset, &overrides]);
        assert_eq!(style.text.size_pt, 12.0);
        assert_eq!(style.text.color, "#ff0000");
        assert_eq!(style.text.font, TextStyle::default().font);
        assert_eq!(style.headings.levels[0].size_pt, 30.0);
        assert_eq!(style.headings.levels[1].size_pt, HeadingsStyle::default().levels[1].size_pt);
    }

    #[test]
    fn diff_round_trips() {
        let base = compose(&[&builtin_presets()[1].layer]);
        let mut edited = base.clone();
        edited.text.size_pt = 13.0;
        edited.headings.levels[2].color = Some("#123456".into());
        edited.code.theme = "dracula".into();
        let layer = Value::Object(diff(&base, &edited));
        assert_eq!(layer["text"], serde_json::json!({ "size_pt": 13.0 }));
        let rebuilt = compose(&[&builtin_presets()[1].layer, &layer]);
        assert_eq!(rebuilt, edited);
    }

    #[test]
    fn user_presets_save_and_reload() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = PresetStore::load(Some(dir.path().to_path_buf()));
        let mut style = Style::default();
        style.text.size_pt = 14.0;
        let id = store.save_user("Mon rapport été", &style).unwrap();
        assert_eq!(id, "mon-rapport-ete");
        let preset = store.get(&id).unwrap();
        assert!(!preset.builtin);
        assert_eq!(preset.label, "Mon rapport été");
        let (resolved, warning) = store.resolve(&StyleRef { preset: id, overrides: Default::default() });
        assert!(warning.is_none());
        assert_eq!(resolved.text.size_pt, 14.0);
    }
}
