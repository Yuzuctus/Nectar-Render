//! Les retouches de mise en page.
//!
//! Elles vivent hors de la note, dans `<coffre>/.nectar/<note>.md.json`, et
//! désignent les blocs par leur id. Si le texte d'un bloc change (et donc son
//! id), on le retrouve par son type, son extrait et sa ligne.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::ids::normalize;
use crate::model::{AnchorInfo, BlockId, BlockKind, Document};
use crate::style::StyleRef;

pub const LAYOUT_VERSION: u32 = 1;

/// Seuil de ressemblance pour rattacher une retouche à un bloc modifié.
const FUZZY_THRESHOLD: f64 = 0.72;

/// Le fichier de retouches d'une note.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    #[serde(default = "layout_version")]
    pub version: u32,
    #[serde(default)]
    pub style: StyleRef,
    /// Format de page par défaut du document.
    #[serde(default)]
    pub page: PageSpec,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<Directive>,
}

fn layout_version() -> u32 {
    LAYOUT_VERSION
}

impl Default for Layout {
    fn default() -> Self {
        Self { version: LAYOUT_VERSION, style: StyleRef::default(), page: PageSpec::default(), blocks: Vec::new() }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LayoutError {
    #[error("lecture de {path} impossible : {source}")]
    Io { path: String, source: std::io::Error },
    #[error("{path} n'est pas un fichier de retouches valide : {source}")]
    Json { path: String, source: serde_json::Error },
}

impl Layout {
    /// Charge un fichier de retouches ; un fichier absent donne des retouches vides.
    pub fn load(path: &Path) -> Result<Self, LayoutError> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(source) => return Err(LayoutError::Io { path: path.display().to_string(), source }),
        };
        serde_json::from_str(&text).map_err(|source| LayoutError::Json { path: path.display().to_string(), source })
    }

    pub fn save(&self, path: &Path) -> Result<(), LayoutError> {
        let io = |source| LayoutError::Io { path: path.display().to_string(), source };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(io)?;
        }
        let mut text = serde_json::to_string_pretty(self).expect("les retouches se sérialisent toujours");
        text.push('\n');
        std::fs::write(path, text).map_err(io)
    }

    /// Les retouches d'un bloc, créées au besoin (pour l'interface).
    pub fn ops_mut(&mut self, anchor: AnchorInfo<'_>) -> &mut BlockOps {
        let index = match self.blocks.iter().position(|d| &d.anchor.id == anchor.id) {
            Some(index) => index,
            None => {
                self.blocks.push(Directive { anchor: Anchor::from(anchor), ops: BlockOps::default() });
                self.blocks.len() - 1
            }
        };
        let directive = &mut self.blocks[index];
        directive.anchor = Anchor::from(anchor);
        &mut directive.ops
    }

    /// Retire les retouches devenues vides. Une retouche vide reste si le
    /// bloc a des retouches écrites dans la note : elle les annule.
    pub fn prune(&mut self, doc: &Document) {
        let inline: HashSet<&BlockId> = doc.blocks.iter().filter(|b| b.inline_ops.is_some()).map(|b| &b.id).collect();
        self.blocks.retain(|d| !d.ops.is_empty() || inline.contains(&d.anchor.id));
    }

    /// Les retouches qui s'appliquent à un bloc : celles de l'atelier, sinon
    /// celles écrites dans la note.
    pub fn ops_for(&self, doc: &Document, id: &BlockId) -> BlockOps {
        if let Some(directive) = self.blocks.iter().find(|d| &d.anchor.id == id) {
            return directive.ops.clone();
        }
        doc.blocks.iter().find(|b| &b.id == id).and_then(|b| b.inline_ops.clone()).unwrap_or_default()
    }

    /// Recale les ancres sur le document : une retouche rattachée par
    /// ressemblance prend l'id, l'extrait et la ligne de son nouveau bloc, et
    /// deux retouches tombées sur le même bloc fusionnent. Renvoie `true` si
    /// quelque chose a changé (les retouches sont alors à réenregistrer).
    pub fn heal(&mut self, doc: &Document) -> bool {
        let resolution = self.resolve(doc);
        let anchors = doc.anchors();
        let fresh = |id: &BlockId| anchors.iter().find(|a| a.id == id).map(|a| Anchor::from(*a));
        let before = self.blocks.clone();
        for (old, new) in &resolution.relinked {
            if let Some(directive) = self.blocks.iter_mut().find(|d| &d.anchor.id == old)
                && let Some(anchor) = fresh(new)
            {
                directive.anchor = anchor;
            }
        }
        for directive in &mut self.blocks {
            if let Some(anchor) = fresh(&directive.anchor.id) {
                directive.anchor = anchor;
            }
        }
        // Fusion des doublons : la dernière retouche l'emporte champ par champ.
        let mut merged: Vec<Directive> = Vec::with_capacity(self.blocks.len());
        for directive in std::mem::take(&mut self.blocks) {
            match merged.iter_mut().find(|d| d.anchor.id == directive.anchor.id) {
                Some(existing) => existing.ops.merge(&directive.ops),
                None => merged.push(directive),
            }
        }
        self.blocks = merged;
        self.blocks != before
    }

    /// Associe chaque retouche à un bloc du document.
    pub fn resolve(&self, doc: &Document) -> Resolution {
        let anchors = doc.anchors();
        let by_id: HashMap<&BlockId, &AnchorInfo> = anchors.iter().map(|a| (a.id, a)).collect();
        let mut taken: HashSet<BlockId> = HashSet::new();
        let mut resolution = Resolution::default();

        // Les blocs portent aussi les retouches écrites dans la note.
        for block in &doc.blocks {
            if let Some(ops) = &block.inline_ops {
                resolution.ops.insert(block.id.clone(), ops.clone());
            }
        }

        let mut pending = Vec::new();
        for directive in &self.blocks {
            if by_id.contains_key(&directive.anchor.id) && taken.insert(directive.anchor.id.clone()) {
                merge_into(&mut resolution.ops, &directive.anchor.id, &directive.ops);
            } else {
                pending.push(directive);
            }
        }

        // Ancrage approximatif pour les blocs dont le texte a changé.
        for directive in pending {
            let wanted = normalize(&directive.anchor.excerpt);
            let best = anchors
                .iter()
                .filter(|a| a.kind == directive.anchor.kind && !taken.contains(a.id))
                .map(|a| {
                    let score = strsim::normalized_levenshtein(&wanted, &normalize(a.excerpt));
                    (score, a.line.abs_diff(directive.anchor.line), a)
                })
                .filter(|(score, _, _)| *score >= FUZZY_THRESHOLD)
                .max_by(|x, y| x.0.total_cmp(&y.0).then(y.1.cmp(&x.1)));
            match best {
                Some((_, _, anchor)) => {
                    taken.insert(anchor.id.clone());
                    merge_into(&mut resolution.ops, anchor.id, &directive.ops);
                    resolution.relinked.push((directive.anchor.id.clone(), anchor.id.clone()));
                }
                None => resolution.orphans.push(directive.anchor.clone()),
            }
        }
        resolution
    }
}

/// Une retouche de l'atelier remplace entièrement celle écrite dans la note :
/// c'est ce qui permet de décocher dans l'atelier une case cochée par la note.
fn merge_into(map: &mut HashMap<BlockId, BlockOps>, id: &BlockId, ops: &BlockOps) {
    map.insert(id.clone(), ops.clone());
}

/// Résultat de l'ancrage des retouches sur un document.
#[derive(Debug, Default)]
pub struct Resolution {
    pub ops: HashMap<BlockId, BlockOps>,
    /// Retouches rattachées à un bloc dont l'id a changé : (ancien, nouveau).
    pub relinked: Vec<(BlockId, BlockId)>,
    /// Retouches dont le bloc a disparu.
    pub orphans: Vec<Anchor>,
}

/// Une retouche attachée à un bloc.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Directive {
    pub anchor: Anchor,
    #[serde(flatten)]
    pub ops: BlockOps,
}

/// Ce qui permet de retrouver un bloc.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Anchor {
    pub id: BlockId,
    pub kind: BlockKind,
    pub excerpt: String,
    pub line: usize,
}

impl From<AnchorInfo<'_>> for Anchor {
    fn from(info: AnchorInfo<'_>) -> Self {
        Self { id: info.id.clone(), kind: info.kind, excerpt: info.excerpt.to_string(), line: info.line }
    }
}

/// Les retouches possibles sur un bloc.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BlockOps {
    /// Commencer une nouvelle page avant ce bloc.
    #[serde(skip_serializing_if = "is_false")]
    pub break_before: bool,
    /// Laisser le reste de la page vide après ce bloc.
    #[serde(skip_serializing_if = "is_false")]
    pub break_after: bool,
    /// Ne jamais séparer ce bloc du suivant par une fin de page.
    #[serde(skip_serializing_if = "is_false")]
    pub keep_with_next: bool,
    /// Pousser ce bloc (et la suite de la page) en bas de page.
    #[serde(skip_serializing_if = "is_false")]
    pub push_to_bottom: bool,
    /// Ne pas exporter ce bloc.
    #[serde(skip_serializing_if = "is_false")]
    pub hidden: bool,
    /// Espace supplémentaire avant le bloc, en millimètres.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_before_mm: Option<f32>,
    /// Changer le format de page à partir de ce bloc (nouvelle page).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<PageChange>,
    /// Réglages d'une image.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<ImageOps>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl BlockOps {
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    /// Superpose `other` (prioritaire) sur ces retouches.
    pub fn merge(&mut self, other: &BlockOps) {
        self.break_before |= other.break_before;
        self.break_after |= other.break_after;
        self.keep_with_next |= other.keep_with_next;
        self.push_to_bottom |= other.push_to_bottom;
        self.hidden |= other.hidden;
        if other.space_before_mm.is_some() {
            self.space_before_mm = other.space_before_mm;
        }
        if other.page.is_some() {
            self.page = other.page.clone();
        }
        if let Some(image) = &other.image {
            self.image = Some(image.clone());
        }
    }
}

/// Changement de format de page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PageChange {
    /// Revenir au format du document : `"page": "default"`.
    Default(DefaultPage),
    Set(PageSpec),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DefaultPage {
    Default,
}

/// Un format de page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PageSpec {
    /// Nom de format Typst : `a4`, `a3`, `a5`, `us-letter`…
    pub paper: String,
    #[serde(skip_serializing_if = "is_false")]
    pub landscape: bool,
    /// Format libre (prioritaire sur `paper` si les deux sont donnés).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width_mm: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height_mm: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub margin_mm: Option<f32>,
}

impl Default for PageSpec {
    fn default() -> Self {
        Self { paper: "a4".into(), landscape: false, width_mm: None, height_mm: None, margin_mm: None }
    }
}

impl PageSpec {
    pub fn paper(paper: &str, landscape: bool) -> Self {
        Self { paper: paper.into(), landscape, ..Self::default() }
    }

    /// Lit `a3`, `a3-landscape`, `a4-paysage` ou `210x297` (mm).
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim().to_lowercase();
        if let Some((w, h)) = text.split_once('x')
            && let (Ok(w), Ok(h)) = (w.trim().parse::<f32>(), h.trim().parse::<f32>())
        {
            return Some(Self { width_mm: Some(w), height_mm: Some(h), ..Self::default() });
        }
        let (paper, landscape) = ["-landscape", "-paysage", " landscape", " paysage"]
            .iter()
            .find_map(|suffix| text.strip_suffix(suffix).map(|p| (p.to_string(), true)))
            .unwrap_or((text.clone(), false));
        let paper = paper.trim();
        (!paper.is_empty() && paper.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
            .then(|| Self::paper(paper, landscape))
    }
}

/// Réglages d'une image.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ImageOps {
    /// Largeur en pourcentage de la largeur de texte.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width_percent: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<HAlign>,
    pub placement: Placement,
    /// Légende affichée sous l'image (remplace le texte alternatif).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Placement {
    /// À sa place dans le texte.
    #[default]
    Inline,
    /// En haut de la page courante (ou suivante).
    Top,
    /// En bas de la page courante (ou suivante).
    Bottom,
    /// Seule sur sa page, aussi grande que possible.
    FullPage,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{ParseOptions, parse};

    fn doc(md: &str) -> Document {
        parse(md, &ParseOptions::default())
    }

    #[test]
    fn page_spec_parsing() {
        assert_eq!(PageSpec::parse("A3-paysage"), Some(PageSpec::paper("a3", true)));
        assert_eq!(PageSpec::parse("a5"), Some(PageSpec::paper("a5", false)));
        let custom = PageSpec::parse("300x200").unwrap();
        assert_eq!((custom.width_mm, custom.height_mm), (Some(300.0), Some(200.0)));
        assert_eq!(PageSpec::parse("a4; rm"), None);
    }

    #[test]
    fn json_round_trip() {
        let original = doc("Intro :\n\n- un\n- deux\n");
        let mut layout = Layout::default();
        let anchors = original.anchors();
        layout.ops_mut(anchors[1]).page = Some(PageChange::Set(PageSpec::paper("a3", true)));
        layout.ops_mut(anchors[2]).break_before = true;
        layout.ops_mut(anchors[0]).page = Some(PageChange::Default(DefaultPage::Default));
        let json = serde_json::to_string_pretty(&layout).unwrap();
        assert!(json.contains("\"page\": \"default\""));
        let back: Layout = serde_json::from_str(&json).unwrap();
        assert_eq!(back, layout);
    }

    #[test]
    fn directive_follows_edited_paragraph() {
        let before = doc("Premier paragraphe.\n\nUne image importante suit ici.\n");
        let mut layout = Layout::default();
        layout.ops_mut(before.anchors()[1]).break_before = true;

        let after = doc("Premier paragraphe.\n\nUne image très importante suit ici.\n");
        let resolution = layout.resolve(&after);
        let target = &after.blocks[1].id;
        assert!(resolution.ops[target].break_before);
        assert_eq!(resolution.relinked.len(), 1);
        assert!(resolution.orphans.is_empty());
    }

    #[test]
    fn healed_directive_can_be_cleared() {
        let before = doc("Premier paragraphe.\n\nUne image importante suit ici.\n");
        let mut layout = Layout::default();
        layout.ops_mut(before.anchors()[1]).break_before = true;

        let after = doc("Premier paragraphe.\n\nUne image très importante suit ici.\n");
        assert!(layout.heal(&after));
        assert_eq!(layout.blocks[0].anchor.id, after.blocks[1].id);
        // L'atelier efface la retouche : elle disparaît pour de bon.
        layout.ops_mut(after.anchors()[1]).break_before = false;
        layout.prune(&after);
        assert!(layout.resolve(&after).ops.is_empty());
        assert!(!layout.heal(&after));
    }

    #[test]
    fn workshop_overrides_inline_directives() {
        let note = doc("A\n\n<!-- nectar: break-before -->\n\nB\n");
        let mut layout = Layout::default();
        assert!(layout.ops_for(&note, &note.blocks[1].id).break_before);
        layout.ops_mut(note.anchors()[1]).break_before = false;
        layout.prune(&note);
        assert_eq!(layout.blocks.len(), 1, "la retouche vide annule celle de la note");
        assert!(!layout.resolve(&note).ops[&note.blocks[1].id].break_before);
    }

    #[test]
    fn unrelated_text_is_orphaned() {
        let before = doc("Un paragraphe.\n");
        let mut layout = Layout::default();
        layout.ops_mut(before.anchors()[0]).break_after = true;
        let after = doc("Tout autre chose, vraiment sans rapport.\n");
        let resolution = layout.resolve(&after);
        assert!(resolution.ops.is_empty());
        assert_eq!(resolution.orphans.len(), 1);
    }
}
