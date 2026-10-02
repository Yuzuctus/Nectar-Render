//! Le placement automatique : ce que Nectar décide seul, sans retouche.
//!
//! Les décisions sont recalculées à chaque mise en page (elles ne sont pas
//! écrites dans les retouches) et listées pour l'atelier, qui les montre et
//! permet de les refuser bloc par bloc (« laisser tel quel »).

use std::collections::HashMap;

use crate::layout::{BlockOps, PageSpec};
use crate::model::{BlockId, Document, Node, Table, plain_text};
use crate::style::Style;

/// Une décision du placement automatique.
#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    pub block: BlockId,
    pub kind: ChoiceKind,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChoiceKind {
    /// Mis sur une page paysage (schéma ou tableau large).
    Landscape,
    /// Image réduite pour tenir dans la place restante (pourcentage gardé).
    Fitted(u8),
    /// Bloc autorisé à se couper entre deux pages pour ne pas laisser de trou.
    Split,
    /// Page paysage placée un peu plus loin : le texte qui suit remplit
    /// d'abord la page d'avant.
    Deferred,
    /// Espacements resserrés pour éviter une dernière page presque vide.
    Tightened,
    /// Tableau un peu resserré pour ne pas déborder de quelques lignes.
    Compacted,
    /// Page paysage sur un papier plus grand (A3), pour tenir sur une page.
    LargerPaper,
}

impl Choice {
    /// Ce qui a été fait, en une phrase.
    pub fn describe(&self) -> String {
        match self.kind {
            ChoiceKind::Landscape => "Mis sur une page paysage pour être lu en grand".into(),
            ChoiceKind::Fitted(percent) => {
                format!("Image réduite à {percent} % pour tenir dans la place restante (pas de trou en bas de page)")
            }
            ChoiceKind::Split => "Coupé entre deux pages pour ne pas laisser de demi-page vide".into(),
            ChoiceKind::Deferred => "Page paysage placée après le texte qui suit, pour remplir la page d'avant".into(),
            ChoiceKind::Tightened => {
                "Espacements légèrement resserrés pour éviter une dernière page presque vide".into()
            }
            ChoiceKind::LargerPaper => "Sur une page A3 paysage, pour tenir en entier sur une seule page".into(),
            ChoiceKind::Compacted => {
                "Tableau légèrement resserré pour ne pas déborder de quelques lignes sur une page de plus".into()
            }
        }
    }
}

/// Le placement automatique peut-il toucher à ce bloc ?
pub fn allowed(ops: Option<&BlockOps>) -> bool {
    ops.is_none_or(|o| !o.manual && !o.hidden && o.page.is_none() && o.image.is_none())
}

const PT_PER_MM: f32 = 72.0 / 25.4;
/// Largeur moyenne d'un caractère, en fraction de la taille du texte.
const CHAR_EM: f32 = 0.52;

/// Largeur de texte d'une page, en points.
pub fn text_width_pt(page: &PageSpec, style: &Style) -> f32 {
    let (width, _) = page.size_mm();
    let margins = match page.margin_mm {
        Some(m) => 2.0 * m,
        None => style.page.margin_left_mm + style.page.margin_right_mm,
    };
    (width - margins).max(40.0) * PT_PER_MM
}

/// La même page tournée d'un quart de tour.
pub fn flipped(page: &PageSpec) -> PageSpec {
    let (w, h) = page.size_mm();
    PageSpec { width_mm: Some(h), height_mm: Some(w), landscape: false, ..page.clone() }
}

/// Comment un tableau tient dans une largeur.
#[derive(Debug, Clone, PartialEq)]
pub struct TableFit {
    /// Largeurs relatives des colonnes ; `None` : tout tient à la largeur
    /// naturelle (aucune cellule ne passe à la ligne).
    pub widths: Option<Vec<f32>>,
    /// Largeur naturelle de chaque colonne (en caractères) : une colonne
    /// plus étroite que sa largeur naturelle passe à la ligne.
    pub natural: Vec<f32>,
    /// Nombre de lignes de texte estimé, pour tout le tableau.
    pub lines: f32,
    /// Même en passant à la ligne, un mot ne tient pas : le tableau déborde.
    pub overflow: bool,
}

/// Répartit la largeur entre les colonnes : une colonne garde sa largeur
/// naturelle si possible ; sinon les plus longues passent à la ligne en
/// premier, sans jamais couper un mot.
pub fn fit_table(table: &Table, width_pt: f32, style: &Style) -> TableFit {
    let columns = table.align.len().max(table.header.len()).max(1);
    let size = style.table.size_pt.unwrap_or(style.text.size_pt);
    let padding = 2.0 * style.table.cell_padding_x_pt;
    let available = ((width_pt - padding * columns as f32) / (size * CHAR_EM)).max(columns as f32);
    let rows: Vec<Vec<String>> = std::iter::once(&table.header)
        .chain(&table.rows)
        .filter(|r| !r.is_empty())
        .map(|r| (0..columns).map(|i| r.get(i).map(|c| plain_text(c)).unwrap_or_default()).collect())
        .collect();
    let mut natural = vec![1f32; columns];
    let mut word = vec![1f32; columns];
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            natural[i] = natural[i].max(cell.chars().count() as f32);
            let longest = cell.split_whitespace().map(|w| w.chars().count()).max().unwrap_or(0);
            word[i] = word[i].max(longest as f32);
        }
    }
    let lines_with = |widths: &[f32]| -> f32 {
        rows.iter()
            .map(|row| row.iter().zip(widths).map(|(cell, w)| wrapped_lines(cell, *w)).fold(1.0f32, f32::max))
            .sum()
    };
    if natural.iter().sum::<f32>() <= available {
        return TableFit { widths: None, natural, lines: rows.len() as f32, overflow: false };
    }
    // Plafond commun : les colonnes plus larges que lui passent à la ligne.
    let width_at = |cap: f32| -> Vec<f32> { (0..columns).map(|i| natural[i].min(cap.max(word[i]))).collect() };
    let (mut low, mut high) = (0.0f32, natural.iter().copied().fold(0.0, f32::max));
    for _ in 0..30 {
        let middle = (low + high) / 2.0;
        if width_at(middle).iter().sum::<f32>() <= available {
            low = middle;
        } else {
            high = middle;
        }
    }
    let widths = width_at(low);
    let overflow = word.iter().sum::<f32>() > available;
    TableFit { lines: lines_with(&widths), widths: Some(widths), natural, overflow }
}

/// Lignes occupées par un texte dans une colonne de `width` caractères.
fn wrapped_lines(text: &str, width: f32) -> f32 {
    let mut lines = 1.0;
    let mut used = 0.0;
    for word in text.split_whitespace() {
        let length = word.chars().count() as f32;
        if used > 0.0 && used + 1.0 + length > width {
            lines += 1.0;
            used = length;
        } else {
            used += if used > 0.0 { 1.0 + length } else { length };
        }
    }
    lines
}

/// Où poser un tableau.
#[derive(Debug, Clone, PartialEq)]
pub enum TablePage {
    /// À sa place, dans le format en cours.
    Inline,
    /// Sur une page paysage du même papier.
    Landscape,
    /// Sur une page paysage d'un papier plus grand (`a3`).
    Larger(String),
}

/// Hauteur de texte d'une page (moins la place d'un titre et d'une phrase).
fn body_height_pt(page: &PageSpec, style: &Style) -> f32 {
    let (_, height) = page.size_mm();
    let margins = match page.margin_mm {
        Some(m) => 2.0 * m,
        None => style.page.margin_top_mm + style.page.margin_bottom_mm,
    };
    (height - margins).max(40.0) * PT_PER_MM - 3.5 * style.text.size_pt
}

/// Hauteur estimée d'un tableau composé ainsi.
fn table_height_pt(table: &Table, fit: &TableFit, style: &Style) -> f32 {
    let size = style.table.size_pt.unwrap_or(style.text.size_pt);
    let rows = (table.rows.len() + 1) as f32;
    fit.lines * size * 1.3 + rows * 2.0 * style.table.cell_padding_y_pt
}

/// Le papier d'un cran plus grand (A4 → A3…), s'il existe.
pub fn larger_paper(paper: &str) -> Option<&'static str> {
    match paper {
        "a5" => Some("a4"),
        "a4" => Some("a3"),
        "a3" => Some("a2"),
        _ => None,
    }
}

/// Un tableau large se lit mieux sur une page paysage : il déborderait en
/// portrait, ses cellules y passeraient beaucoup à la ligne, ou il y
/// prendrait plusieurs pages alors qu'il tient sur une seule page paysage
/// (au besoin d'un papier plus grand, si `larger` le permet).
pub fn table_page(table: &Table, page: &PageSpec, style: &Style, larger: bool) -> TablePage {
    let columns = table.align.len().max(table.header.len());
    if columns < 3 || table.rows.len() < 2 {
        return TablePage::Inline;
    }
    let portrait = text_width_pt(page, style);
    let turned_page = flipped(page);
    let turned = text_width_pt(&turned_page, style);
    if turned < portrait * 1.2 {
        return TablePage::Inline;
    }
    let here = fit_table(table, portrait, style);
    if here.widths.is_none() {
        return TablePage::Inline;
    }
    let there = fit_table(table, turned, style);
    let here_height = table_height_pt(table, &here, style);
    let there_height = table_height_pt(table, &there, style);
    let several_pages = here_height > body_height_pt(page, style);
    let wraps_much = here.lines >= there.lines * 1.3;
    // Plusieurs pages en portrait, une seule en paysage (à 15 % près : un
    // tableau qui déborde de peu est ensuite légèrement resserré).
    if !there.overflow && several_pages && wraps_much && there_height <= body_height_pt(&turned_page, style) * 1.15 {
        return TablePage::Landscape;
    }
    // Trop grand même en paysage : un papier plus grand, s'il y tient en entier.
    if larger
        && several_pages
        && page.width_mm.is_none()
        && let Some(paper) = larger_paper(&page.paper)
    {
        let big = flipped(&PageSpec { paper: paper.into(), landscape: false, ..page.clone() });
        let fit = fit_table(table, text_width_pt(&big, style), style);
        if !fit.overflow
            && here.lines >= fit.lines * 1.3
            && table_height_pt(table, &fit, style) <= body_height_pt(&big, style)
        {
            return TablePage::Larger(paper.into());
        }
    }
    if columns >= 5
        && !there.overflow
        && (here.overflow || (here.lines >= there.lines * 1.4 && here.lines - there.lines >= 6.0))
    {
        return TablePage::Landscape;
    }
    TablePage::Inline
}

/// Le tableau passe-t-il à la ligne dans cette largeur de page ?
pub fn table_wraps(table: &Table, page: &PageSpec, style: &Style) -> bool {
    table.align.len().max(table.header.len()) >= 3
        && fit_table(table, text_width_pt(page, style), style).widths.is_some()
}

/// Compatibilité : le tableau irait-il sur une page paysage ?
pub fn table_wants_landscape(table: &Table, page: &PageSpec, style: &Style) -> bool {
    table_page(table, page, style, true) != TablePage::Inline
}

/// Les tableaux à mettre en paysage, décidés avant toute mise en page, et
/// le papier plus grand de ceux qui en ont besoin.
pub fn wide_tables(
    doc: &Document,
    ops: &HashMap<BlockId, BlockOps>,
    page: &PageSpec,
    style: &Style,
) -> Vec<(BlockId, Option<String>)> {
    doc.blocks
        .iter()
        .filter(|b| allowed(ops.get(&b.id)) && ops.get(&b.id).is_none_or(|o| o.table.is_none()))
        .filter_map(|b| match &b.node {
            // Le papier plus grand se décide sur pièces, une fois les pages composées.
            Node::Table(t) => match table_page(t, page, style, false) {
                TablePage::Inline => None,
                TablePage::Landscape => Some((b.id.clone(), None)),
                TablePage::Larger(paper) => Some((b.id.clone(), Some(paper))),
            },
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{ParseOptions, parse};

    fn table(md: &str) -> Table {
        let doc = parse(md, &ParseOptions::default());
        match &doc.blocks[0].node {
            Node::Table(t) => t.clone(),
            _ => panic!("pas un tableau"),
        }
    }

    #[test]
    fn narrow_tables_keep_their_natural_width() {
        let t = table("| IP | Masque | Passerelle |\n|---|---|---|\n| 192.168.1.10 | 255.255.255.0 | 192.168.1.1 |\n");
        let style = Style::default();
        assert_eq!(fit_table(&t, text_width_pt(&PageSpec::default(), &style), &style).widths, None);
        assert!(!table_wants_landscape(&t, &PageSpec::default(), &style));
    }

    #[test]
    fn wide_tables_go_landscape_and_long_columns_wrap_first() {
        let mut md = String::from(
            "| Lot | Entreprise | Début | Fin prévue | Budget | Engagé | État | Remarque |\n|---|---|---|---|---|---|---|---|\n",
        );
        for i in 0..10 {
            md.push_str(&format!(
                "| Lot {i} | Entreprise Durand et fils | 2026-03-12 | 2026-11-03 | 12 500 € | 11 980 € | En cours de réception | Retard de livraison des menuiseries extérieures |\n"
            ));
        }
        let t = table(&md);
        let style = Style::default();
        let page = PageSpec::default();
        let fit = fit_table(&t, text_width_pt(&page, &style), &style);
        let widths = fit.widths.expect("ne tient pas à sa largeur naturelle");
        // La remarque (la plus longue) passe à la ligne ; les dates jamais.
        assert!(widths[7] < 47.0);
        assert_eq!(widths[2], 10.0);
        assert!(table_wants_landscape(&t, &page, &style));
        // Déjà en paysage : rien à faire.
        assert!(!table_wants_landscape(&t, &PageSpec::paper("a4", true), &style));
    }
}
