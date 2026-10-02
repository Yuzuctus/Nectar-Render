//! L'assistant de mise en page : il relit les pages produites et signale ce
//! qui gâche un PDF (page à moitié vide, image réduite, contenu qui déborde,
//! image manquante…), avec des corrections en un clic quand c'est possible.

use std::collections::HashMap;

use crate::layout::{BlockOps, ImageOps, Placement};
use crate::model::{BlockId, BlockKind, Document, Node};
use crate::style::Style;

/// Ce que le moteur sait d'une page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageMetrics {
    pub width: f64,
    pub height: f64,
    /// Étendue du contenu (hors pied de page) : gauche, haut, droite, bas, en points.
    pub content: Option<[f64; 4]>,
}

/// Début d'un bloc dans les pages.
#[derive(Debug, Clone, PartialEq)]
pub struct Marker {
    pub id: BlockId,
    pub page: usize,
    pub y: f64,
}

/// Une image telle qu'elle est posée dans les pages.
#[derive(Debug, Clone, PartialEq)]
pub struct FigureView {
    pub id: BlockId,
    pub page: usize,
    /// Taille affichée, en points.
    pub width: f64,
    pub height: f64,
    /// Taille d'origine en pixels d'une photo (rien pour un dessin vectoriel).
    pub pixels: Option<(f64, f64)>,
}

/// Remarque émise par le template pendant la mise en page.
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateNote {
    pub kind: String,
    pub page: usize,
    pub y: f64,
    /// Valeur associée (pour `shrunk` : rapport de réduction).
    pub value: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
    Problem,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Issue {
    pub severity: Severity,
    pub page: Option<usize>,
    pub block: Option<BlockId>,
    pub title: String,
    pub detail: String,
    pub fixes: Vec<Fix>,
}

/// Une correction applicable à un bloc.
#[derive(Debug, Clone, PartialEq)]
pub struct Fix {
    pub label: String,
    pub block: BlockId,
    pub action: FixAction,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FixAction {
    BreakBefore,
    ImageWidth(f32),
    ImagePlacement(Placement),
    /// Autoriser (ou interdire) la coupure du bloc entre deux pages.
    KeepTogether(bool),
}

impl Fix {
    /// Applique la correction aux retouches du bloc.
    pub fn apply(&self, ops: &mut BlockOps) {
        match &self.action {
            FixAction::BreakBefore => ops.break_before = true,
            FixAction::ImageWidth(w) => ops.image.get_or_insert_with(ImageOps::default).width_percent = Some(*w),
            FixAction::ImagePlacement(p) => ops.image.get_or_insert_with(ImageOps::default).placement = *p,
            FixAction::KeepTogether(keep) => ops.keep_together = Some(*keep),
        }
    }
}

/// Tout ce que l'analyse lit.
pub struct Inputs<'a> {
    pub document: &'a Document,
    pub ops: &'a HashMap<BlockId, BlockOps>,
    pub style: &'a Style,
    pub pages: &'a [PageMetrics],
    pub markers: &'a [Marker],
    pub notes: &'a [TemplateNote],
    pub figures: &'a [FigureView],
    /// Avertissements de lecture, de génération et de compilation.
    pub warnings: &'a [String],
    pub missing_fonts: &'a [String],
}

const MM: f64 = 72.0 / 25.4;

pub fn analyse(input: &Inputs<'_>) -> Vec<Issue> {
    let mut issues = Vec::new();
    let kinds: HashMap<&BlockId, (BlockKind, &Node)> =
        input.document.blocks.iter().map(|b| (&b.id, (b.node.kind(), &b.node))).collect();
    let margin_top = f64::from(input.style.page.margin_top_mm) * MM;
    let margin_bottom = f64::from(input.style.page.margin_bottom_mm) * MM;
    let margin_right = f64::from(input.style.page.margin_right_mm) * MM;
    let first = |page: usize| input.markers.iter().find(|m| m.page == page);
    let on_page = |page: usize| input.markers.iter().filter(move |m| m.page == page);
    let last_page = input.pages.len().saturating_sub(1);

    for (page, metrics) in input.pages.iter().enumerate() {
        let Some([_, _, right, bottom]) = metrics.content else { continue };
        let body = metrics.height - margin_top - margin_bottom;
        let fill = ((bottom - margin_top) / body).clamp(0.0, 1.0);

        // Contenu qui dépasse la marge droite.
        if right > metrics.width - margin_right + 3.0 {
            let block = on_page(page).next_back().map(|m| m.id.clone());
            issues.push(Issue {
                severity: Severity::Problem,
                page: Some(page),
                block,
                title: format!("Contenu qui dépasse la marge (page {})", page + 1),
                detail: "Un tableau, une image ou une longue ligne sort de la zone de texte.".into(),
                fixes: Vec::new(),
            });
        }

        // Page à moitié vide parce que le bloc suivant n'y tenait pas (une
        // page paysage ne porte que son schéma ou son tableau : rien à dire).
        let landscape = on_page(page).any(|m| {
            input.ops.get(&m.id).and_then(|o| o.image.as_ref()).is_some_and(|i| i.placement == Placement::Landscape)
        });
        if page < last_page
            && !landscape
            && on_page(page).next().is_some()
            && fill < 0.6
            && let Some(next) = first(page + 1)
        {
            // Saut voulu ? On regarde le groupe en tête de la page suivante :
            // titres, liste et sa première puce, phrase d'annonce, puis le
            // premier vrai bloc (un saut posé sur lui remonte devant ses titres).
            let starts_page = |id: &BlockId| {
                input.ops.get(id).is_some_and(|o| {
                    o.break_before
                        || o.page.is_some()
                        || o.image
                            .as_ref()
                            .is_some_and(|i| matches!(i.placement, Placement::Landscape | Placement::FullPage))
                })
            };
            let mut explicit =
                on_page(page).next_back().is_some_and(|m| input.ops.get(&m.id).is_some_and(|o| o.break_after));
            for m in on_page(page + 1) {
                explicit |= starts_page(&m.id);
                let leading = match kinds.get(&m.id) {
                    Some((BlockKind::Heading | BlockKind::List, _)) => true,
                    Some((BlockKind::Paragraph, Node::Paragraph(c))) => {
                        crate::model::plain_text(c).trim_end().ends_with(':')
                    }
                    _ => m.y <= next.y + 0.5,
                };
                if !leading {
                    break;
                }
            }
            // Puces des listes de tête (elles n'ont pas de marqueur de type).
            for block in &input.document.blocks {
                if let Node::List(list) = &block.node
                    && on_page(page + 1).take(3).any(|m| m.id == block.id)
                {
                    explicit |= list.items.first().and_then(|i| i.id.as_ref()).is_some_and(starts_page);
                }
            }
            if !explicit {
                // Le coupable est le premier bloc qui n'est pas un titre (les titres le suivent).
                let culprit = input
                    .markers
                    .iter()
                    .skip_while(|m| m.page <= page)
                    .find(|m| match kinds.get(&m.id) {
                        Some((BlockKind::Heading, _)) => false,
                        // Une phrase qui annonce la suite (« … : ») la suit aussi.
                        Some((BlockKind::Paragraph, Node::Paragraph(c))) => {
                            !crate::model::plain_text(c).trim_end().ends_with(':')
                        }
                        _ => true,
                    })
                    .unwrap_or(next);
                let (kind, _) = kinds.get(&culprit.id).copied().unwrap_or((BlockKind::Paragraph, &Node::Rule));
                let mut fixes = Vec::new();
                let what = match kind {
                    BlockKind::Figure => {
                        fixes.push(fix("Réduire l'image à 70 %", &culprit.id, FixAction::ImageWidth(70.0)));
                        fixes.push(fix(
                            "La faire flotter en haut de page",
                            &culprit.id,
                            FixAction::ImagePlacement(Placement::Top),
                        ));
                        "une image trop haute pour la place restante"
                    }
                    BlockKind::Code | BlockKind::Table | BlockKind::List | BlockKind::Callout | BlockKind::Quote => {
                        fixes.push(fix("Autoriser la coupure de ce bloc", &culprit.id, FixAction::KeepTogether(false)));
                        "un bloc gardé d'un seul tenant"
                    }
                    _ => "un bloc qui ne pouvait pas être coupé",
                };
                issues.push(Issue {
                    severity: Severity::Warning,
                    page: Some(page),
                    block: Some(culprit.id.clone()),
                    title: format!("Page {} remplie à {:.0} %", page + 1, fill * 100.0),
                    detail: format!("La suite commence page {} : {what}.", page + 2),
                    fixes,
                });
            }
        }

        // Titre seul en bas de page.
        if let Some(last) = on_page(page).next_back()
            && matches!(kinds.get(&last.id), Some((BlockKind::Heading, _)))
            && last.y > metrics.height - margin_bottom - 60.0
            && page < last_page
        {
            issues.push(Issue {
                severity: Severity::Warning,
                page: Some(page),
                block: Some(last.id.clone()),
                title: format!("Titre isolé en bas de la page {}", page + 1),
                detail: "Son contenu commence sur la page suivante.".into(),
                fixes: vec![fix("Passer le titre à la page suivante", &last.id, FixAction::BreakBefore)],
            });
        }
    }

    // Dernière page presque vide.
    if let Some(metrics) = input.pages.last()
        && input.pages.len() > 1
        && let Some([_, _, _, bottom]) = metrics.content
    {
        let body = metrics.height - margin_top - margin_bottom;
        let fill = (bottom - margin_top) / body;
        if fill < 0.08 {
            issues.push(Issue {
                severity: Severity::Info,
                page: Some(last_page),
                block: first(last_page).map(|m| m.id.clone()),
                title: "Dernière page presque vide".into(),
                detail: "Quelques lignes seulement : réduire un peu un espacement ou une image peut la supprimer."
                    .into(),
                fixes: Vec::new(),
            });
        }
    }

    // Schémas larges et détaillés, à lire en grand sur une page paysage.
    for figure in input.figures {
        if let Some(issue) = wide_schema(input, figure) {
            issues.push(issue);
        }
    }

    // Images réduites par le template pour tenir dans la page.
    for note in input.notes.iter().filter(|n| n.kind == "shrunk") {
        let block = input.markers.iter().rfind(|m| m.page < note.page || (m.page == note.page && m.y <= note.y + 1.0));
        let mut fixes = Vec::new();
        if let Some(block) = block {
            fixes.push(fix("Lui donner une page entière", &block.id, FixAction::ImagePlacement(Placement::FullPage)));
        }
        issues.push(Issue {
            severity: Severity::Info,
            page: Some(note.page),
            block: block.map(|m| m.id.clone()),
            title: format!("Image réduite à {:.0} % (page {})", note.value * 100.0, note.page + 1),
            detail: "Elle était plus haute que la page.".into(),
            fixes,
        });
    }

    for warning in input.warnings {
        let lower = warning.to_lowercase();
        let severity = if lower.contains("introuvable") || lower.contains("erreur") {
            Severity::Problem
        } else {
            Severity::Warning
        };
        issues.push(Issue {
            severity,
            page: None,
            block: None,
            title: first_sentence(warning),
            detail: String::new(),
            fixes: Vec::new(),
        });
    }
    for font in input.missing_fonts {
        issues.push(Issue {
            severity: Severity::Info,
            page: None,
            block: None,
            title: format!("Police absente : {font}"),
            detail: "Remplacée par une police de secours de la même famille.".into(),
            fixes: Vec::new(),
        });
    }

    issues.sort_by(|a, b| b.severity.cmp(&a.severity).then(a.page.cmp(&b.page)));
    issues
}

/// Mots qui désignent un schéma dans une légende ou un nom de fichier.
/// Volontairement étroits : « réseau » ou « plan » se lisent aussi sous une
/// capture d'écran, qui n'a rien à gagner à passer en paysage.
const SCHEMA_WORDS: &[&str] = &[
    "schéma",
    "schema",
    "diagram",
    "architecture",
    "topolog",
    "organigramme",
    "synoptique",
    "logigramme",
    "excalidraw",
    "mindmap",
    "carte mentale",
    "uml",
];

/// Mots d'une capture d'écran : jamais proposée en paysage.
const SCREENSHOT_WORDS: &[&str] = &["capture", "screenshot", "screen shot", "écran", "ecran", "copie d"];

/// Un schéma large et détaillé, affiché à la largeur du texte d'une page en
/// portrait : sur une page paysage, il serait nettement plus grand.
fn wide_schema(input: &Inputs<'_>, figure: &FigureView) -> Option<Issue> {
    let block = input.document.blocks.iter().find(|b| b.id == figure.id)?;
    let named_schema = |text: &str| {
        let text = text.to_lowercase();
        SCHEMA_WORDS.iter().any(|w| text.contains(w)) && !SCREENSHOT_WORDS.iter().any(|w| text.contains(w))
    };
    let (schema, name) = match &block.node {
        Node::Diagram { .. } => (true, "Le diagramme".to_string()),
        Node::Figure(image) => {
            let both = format!("{} {}", image.alt, image.target);
            let name = if image.alt.trim().is_empty() { image.target.clone() } else { image.alt.clone() };
            (named_schema(&both), format!("« {} »", shorten(&name, 50)))
        }
        _ => (false, String::new()),
    };
    // Une image déjà retouchée : la personne a décidé de sa taille.
    if !schema || input.ops.get(&figure.id).is_some_and(|o| o.image.is_some() || o.page.is_some() || o.manual) {
        return None;
    }
    let page = input.pages.get(figure.page)?;
    if page.width >= page.height || figure.height <= 0.0 {
        return None;
    }
    let p = &input.style.page;
    let (left, right) = (f64::from(p.margin_left_mm) * MM, f64::from(p.margin_right_mm) * MM);
    let (top, bottom) = (f64::from(p.margin_top_mm) * MM, f64::from(p.margin_bottom_mm) * MM);
    let text_width = page.width - left - right;
    let aspect = figure.width / figure.height;
    if aspect < 1.25 || figure.width < text_width * 0.9 {
        return None;
    }
    // Une photo doit avoir des détails à montrer (au moins 200 pixels par pouce affiché).
    if let Some((px, _)) = figure.pixels
        && px / (figure.width / 72.0) < 200.0
    {
        return None;
    }
    // Taille sur la page tournée : largeur et hauteur échangées, place pour la légende.
    let landscape_width = (page.height - left - right).min((page.width - top - bottom - 30.0) * aspect);
    let gain = landscape_width / figure.width;
    if gain < 1.2 {
        return None;
    }
    Some(Issue {
        severity: Severity::Warning,
        page: Some(figure.page),
        block: Some(figure.id.clone()),
        title: format!("Schéma à lire en grand (page {})", figure.page + 1),
        detail: format!(
            "{name} est large et détaillé : sur une page paysage, il serait {} fois plus grand. Le texte reprend ensuite au format normal.",
            format!("{gain:.1}").replace('.', ",")
        ),
        fixes: vec![fix("Le mettre sur une page paysage", &figure.id, FixAction::ImagePlacement(Placement::Landscape))],
    })
}

fn shorten(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        format!("{}…", text.chars().take(max).collect::<String>())
    }
}

fn fix(label: &str, block: &BlockId, action: FixAction) -> Fix {
    Fix { label: label.into(), block: block.clone(), action }
}

fn first_sentence(text: &str) -> String {
    let line = text.lines().next().unwrap_or(text);
    let mut s = line.to_string();
    if let Some(first) = s.get(..1) {
        s = first.to_uppercase() + &s[1..];
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{ParseOptions, parse};

    fn metrics(bottom: f64) -> PageMetrics {
        PageMetrics { width: 595.0, height: 842.0, content: Some([62.0, 62.0, 530.0, bottom]) }
    }

    #[test]
    fn half_empty_page_blames_the_next_figure() {
        let doc = parse("Texte.\n\n## Titre\n\n![[grande.png]]\n", &ParseOptions::default());
        let ids: Vec<BlockId> = doc.blocks.iter().map(|b| b.id.clone()).collect();
        let markers = vec![
            Marker { id: ids[0].clone(), page: 0, y: 70.0 },
            Marker { id: ids[1].clone(), page: 1, y: 70.0 },
            Marker { id: ids[2].clone(), page: 1, y: 100.0 },
        ];
        let style = Style::default();
        let ops = HashMap::new();
        let issues = analyse(&Inputs {
            document: &doc,
            ops: &ops,
            style: &style,
            pages: &[metrics(200.0), metrics(700.0)],
            markers: &markers,
            notes: &[],
            warnings: &[],
            missing_fonts: &[],
            figures: &[],
        });
        let issue = issues.iter().find(|i| i.title.starts_with("Page 1")).expect("page à moitié vide");
        assert_eq!(issue.block.as_ref(), Some(&ids[2]), "le titre suit l'image : c'est l'image qu'on corrige");
        assert_eq!(issue.fixes.len(), 2);
        let mut ops = BlockOps::default();
        issue.fixes[0].apply(&mut ops);
        assert_eq!(ops.image.unwrap().width_percent, Some(70.0));
    }

    #[test]
    fn explicit_break_is_not_reported() {
        let doc = parse("A\n\nB\n", &ParseOptions::default());
        let ids: Vec<BlockId> = doc.blocks.iter().map(|b| b.id.clone()).collect();
        let markers =
            vec![Marker { id: ids[0].clone(), page: 0, y: 70.0 }, Marker { id: ids[1].clone(), page: 1, y: 70.0 }];
        let mut ops = HashMap::new();
        ops.insert(ids[1].clone(), BlockOps { break_before: true, ..Default::default() });
        let style = Style::default();
        let issues = analyse(&Inputs {
            document: &doc,
            ops: &ops,
            style: &style,
            pages: &[metrics(100.0), metrics(700.0)],
            markers: &markers,
            notes: &[],
            warnings: &[],
            missing_fonts: &[],
            figures: &[],
        });
        assert!(issues.iter().all(|i| !i.title.starts_with("Page 1")));
    }
}
