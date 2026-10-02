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

/// Un tableau large se lit mieux sur une page paysage : il déborderait en
/// portrait, ou ses cellules y passeraient beaucoup à la ligne.
pub fn table_wants_landscape(table: &Table, page: &PageSpec, style: &Style) -> bool {
    let columns = table.align.len().max(table.header.len());
    if columns < 5 || table.rows.len() < 2 {
        return false;
    }
    let portrait = text_width_pt(page, style);
    let turned = text_width_pt(&flipped(page), style);
    if turned < portrait * 1.2 {
        return false;
    }
    let here = fit_table(table, portrait, style);
    if here.widths.is_none() {
        return false;
    }
    let there = fit_table(table, turned, style);
    if there.overflow {
        return false;
    }
    here.overflow || (here.lines >= there.lines * 1.4 && here.lines - there.lines >= 6.0)
}

/// Les tableaux à mettre en paysage, décidés avant toute mise en page.
pub fn wide_tables(doc: &Document, ops: &HashMap<BlockId, BlockOps>, page: &PageSpec, style: &Style) -> Vec<BlockId> {
    doc.blocks
        .iter()
        .filter(|b| allowed(ops.get(&b.id)) && ops.get(&b.id).is_none_or(|o| o.table.is_none()))
        .filter(|b| matches!(&b.node, Node::Table(t) if table_wants_landscape(t, page, style)))
        .map(|b| b.id.clone())
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
