//! Les retouches les plus courantes, à un clic : dans la barre posée sur le
//! bloc sélectionné, dans le menu du clic droit et en tête du panneau Bloc.

use nectar_core::BlockOps;
use nectar_core::layout::{ImageOps, Placement};
use nectar_core::model::{BlockKind, Node};

use crate::app::NectarApp;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quick {
    BreakBefore,
    BreakAfter,
    KeepWithNext,
    KeepTogether,
    Landscape,
    FullPage,
    /// Refuser les décisions du placement automatique pour ce bloc.
    AsIs,
    Clear,
}

/// Une action rapide telle qu'on la montre : libellé court, état, et ce
/// qu'elle fait, en une phrase.
pub struct Offer {
    pub quick: Quick,
    pub label: &'static str,
    pub active: bool,
    pub hint: &'static str,
}

const NEXT_PAGE: &str = "Une nouvelle page commence à ce bloc (Ctrl + Entrée).";

/// Les actions qui ont du sens pour ce bloc, dans l'ordre d'affichage.
pub fn offers(app: &NectarApp, id: &nectar_core::BlockId) -> Vec<Offer> {
    let Some(project) = &app.project else { return Vec::new() };
    let Some(block) = project.document.blocks.iter().find(|b| &b.id == id) else {
        // Une puce de liste : pas de bloc de premier niveau.
        let ops = project.layout.ops_for(&project.document, id);
        let mut list = vec![offer(Quick::BreakBefore, "↓ Page suivante", ops.break_before, NEXT_PAGE)];
        if !ops.is_empty() {
            list.push(offer(
                Quick::Clear,
                "Tout remettre",
                false,
                "Retire tout ce que tu as changé sur cette puce (Suppr).",
            ));
        }
        return list;
    };
    let ops = project.layout.ops_for(&project.document, id);
    let figure = matches!(block.node, Node::Figure(_) | Node::Diagram { .. });
    let table = matches!(block.node, Node::Table(_));
    let placement = ops.image.as_ref().map(|i| i.placement).unwrap_or_default();
    let auto_landscape = app.rendered.as_ref().is_some_and(|r| {
        r.choices.iter().any(|c| &c.block == id && c.kind == nectar_core::auto::ChoiceKind::Landscape)
    });
    let mut list = vec![offer(Quick::BreakBefore, "↓ Page suivante", ops.break_before, NEXT_PAGE)];
    if (figure || table) && auto_landscape && placement != Placement::Landscape {
        list.push(offer(
            Quick::AsIs,
            "Page paysage (auto)",
            true,
            "Mis en paysage automatiquement pour être lu en grand. Cliquer : le remettre dans le texte.",
        ));
    } else if figure || table {
        list.push(offer(
            Quick::Landscape,
            "Page paysage",
            placement == Placement::Landscape,
            "Seul sur une page tournée, en grand ; le texte reprend ensuite normalement.",
        ));
    }
    if figure {
        list.push(offer(
            Quick::FullPage,
            "Pleine page",
            placement == Placement::FullPage,
            "Seule sur sa page, aussi grande que possible.",
        ));
        list.push(offer(
            Quick::BreakAfter,
            "Finir la page ici",
            ops.break_after,
            "Ce qui suit ce bloc commence sur la page suivante.",
        ));
    } else {
        // Un titre reste déjà toujours avec ce qui le suit.
        if !matches!(block.node, Node::Heading { .. }) {
            list.push(offer(
                Quick::KeepWithNext,
                "Garder avec la suite",
                ops.keep_with_next,
                "Ce bloc et le suivant restent toujours sur la même page.",
            ));
        }
        if matches!(
            block.node.kind(),
            BlockKind::Table | BlockKind::Code | BlockKind::List | BlockKind::Callout | BlockKind::Quote
        ) {
            list.push(offer(
                Quick::KeepTogether,
                "Ne pas couper",
                ops.keep_together == Some(true),
                "Le bloc reste entier, jamais coupé entre deux pages.",
            ));
        }
    }
    if !ops.is_empty() {
        list.push(offer(Quick::Clear, "Tout remettre", false, "Retire tout ce que tu as changé sur ce bloc (Suppr)."));
    }
    list
}

fn offer(quick: Quick, label: &'static str, active: bool, hint: &'static str) -> Offer {
    Offer { quick, label, active, hint }
}

/// Bascule l'action sur les retouches d'un bloc.
pub fn apply(ops: &mut BlockOps, quick: Quick) {
    match quick {
        Quick::BreakBefore => ops.break_before = !ops.break_before,
        Quick::BreakAfter => ops.break_after = !ops.break_after,
        Quick::KeepWithNext => ops.keep_with_next = !ops.keep_with_next,
        Quick::KeepTogether => {
            ops.keep_together = if ops.keep_together == Some(true) { None } else { Some(true) };
        }
        Quick::Landscape | Quick::FullPage => {
            let wanted = if quick == Quick::Landscape { Placement::Landscape } else { Placement::FullPage };
            let image = ops.image.get_or_insert_with(ImageOps::default);
            image.placement = if image.placement == wanted { Placement::Inline } else { wanted };
            if image == &ImageOps::default() {
                ops.image = None;
            }
        }
        Quick::AsIs => ops.manual = !ops.manual,
        Quick::Clear => *ops = BlockOps::default(),
    }
}

/// Ce qui a été changé sur un bloc, en mots simples (une ligne par retouche).
pub fn summary(ops: &BlockOps) -> Vec<String> {
    use nectar_core::layout::PageChange;
    let mut out = Vec::new();
    if ops.break_before {
        out.push("Commence une nouvelle page".into());
    }
    if ops.break_after {
        out.push("Finit sa page".into());
    }
    if ops.keep_with_next {
        out.push("Gardé avec la suite".into());
    }
    match ops.keep_together {
        Some(true) => out.push("Jamais coupé".into()),
        Some(false) => out.push("Peut être coupé".into()),
        None => {}
    }
    if ops.push_to_bottom {
        out.push("Collé en bas de la page".into());
    }
    if let Some(mm) = ops.space_before_mm {
        let value = format!("{:.1}", mm.abs()).replace(".0", "").replace('.', ",");
        out.push(if mm > 0.0 { format!("Descendu de {value} mm") } else { format!("Remonté de {value} mm") });
    }
    match &ops.page {
        Some(PageChange::Set(spec)) => {
            let name = match (spec.width_mm, spec.height_mm) {
                (Some(w), Some(h)) => format!("{w:.0} × {h:.0} mm"),
                _ => format!("{}{}", spec.paper.to_uppercase(), if spec.landscape { " paysage" } else { "" }),
            };
            out.push(format!("Page en {name}{}", if ops.page_onward { ", et les suivantes" } else { "" }));
        }
        Some(PageChange::Default(_)) => out.push("Retour au format du document".into()),
        None => {}
    }
    if let Some(image) = &ops.image {
        match image.placement {
            Placement::Top => out.push("En haut de la page".into()),
            Placement::Bottom => out.push("En bas de la page".into()),
            Placement::FullPage => out.push("Seule sur sa page".into()),
            Placement::Landscape => out.push("Sur une page paysage".into()),
            Placement::Inline => {}
        }
        if let Some(w) = image.width_percent {
            out.push(format!("Largeur {w:.0} %"));
        }
        if image.align.is_some() {
            out.push("Alignement changé".into());
        }
        if image.caption.is_some() {
            out.push("Légende changée".into());
        }
    }
    if ops.style.is_some() {
        out.push("Apparence changée".into());
    }
    if ops.table.is_some() {
        out.push("Colonnes réglées".into());
    }
    if ops.hidden {
        out.push("Pas imprimé".into());
    }
    if ops.manual {
        out.push("Gardé tel quel".into());
    }
    out
}

/// Ce qu'une retouche vient de changer, en une phrase.
pub fn change(before: &BlockOps, after: &BlockOps) -> String {
    let (old, new) = (summary(before), summary(after));
    let added: Vec<&String> = new.iter().filter(|s| !old.contains(s)).collect();
    let removed: Vec<&String> = old.iter().filter(|s| !new.contains(s)).collect();
    match (added.is_empty(), removed.is_empty()) {
        (false, _) => added.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" · "),
        (true, false) if new.is_empty() => "Bloc remis comme avant".into(),
        (true, false) => {
            format!("Retiré : {}", removed.iter().map(|s| s.to_lowercase()).collect::<Vec<_>>().join(", "))
        }
        (true, true) => "Retouche appliquée".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_change_is_told_in_plain_words() {
        let before = BlockOps::default();
        let mut after = BlockOps { break_before: true, ..BlockOps::default() };
        assert_eq!(change(&before, &after), "Commence une nouvelle page");
        after.space_before_mm = Some(4.5);
        assert_eq!(change(&before, &after), "Commence une nouvelle page · Descendu de 4,5 mm");
        let moved = BlockOps { space_before_mm: Some(-2.0), ..BlockOps::default() };
        assert_eq!(change(&before, &moved), "Remonté de 2 mm");
        assert_eq!(change(&after, &before), "Bloc remis comme avant");
        let kept = BlockOps { break_before: true, ..BlockOps::default() };
        assert_eq!(change(&after, &kept), "Retiré : descendu de 4,5 mm");
    }
}
