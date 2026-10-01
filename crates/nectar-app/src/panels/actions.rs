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
    Clear,
}

/// Une action rapide telle qu'on la montre : libellé, état, explication.
pub struct Offer {
    pub quick: Quick,
    pub label: &'static str,
    pub active: bool,
    pub hint: &'static str,
}

/// Les actions qui ont du sens pour ce bloc, dans l'ordre d'affichage.
pub fn offers(app: &NectarApp, id: &nectar_core::BlockId) -> Vec<Offer> {
    let Some(project) = &app.project else { return Vec::new() };
    let Some(block) = project.document.blocks.iter().find(|b| &b.id == id) else {
        // Une puce de liste : pas de bloc de premier niveau.
        let ops = project.layout.ops_for(&project.document, id);
        return vec![
            offer(Quick::BreakBefore, "Page avant", ops.break_before, "Commencer une nouvelle page avant cette puce"),
            offer(Quick::Clear, "Effacer", false, "Effacer les retouches de cette puce"),
        ];
    };
    let ops = project.layout.ops_for(&project.document, id);
    let figure = matches!(block.node, Node::Figure(_) | Node::Diagram { .. });
    let placement = ops.image.as_ref().map(|i| i.placement).unwrap_or_default();
    let mut list = vec![offer(
        Quick::BreakBefore,
        "Page avant",
        ops.break_before,
        "Commencer une nouvelle page avant ce bloc (Ctrl+Entrée)",
    )];
    if figure {
        list.push(offer(
            Quick::Landscape,
            "Paysage",
            placement == Placement::Landscape,
            "L'image seule sur une page paysage, en grand",
        ));
        list.push(offer(
            Quick::FullPage,
            "Pleine page",
            placement == Placement::FullPage,
            "L'image seule sur sa page, aussi grande que possible",
        ));
        list.push(offer(Quick::BreakAfter, "Vide après", ops.break_after, "Laisser le reste de la page vide après"));
    } else {
        list.push(offer(
            Quick::KeepWithNext,
            "Avec la suite",
            ops.keep_with_next,
            "Garder ce bloc sur la même page que le suivant",
        ));
        if matches!(
            block.node.kind(),
            BlockKind::Table | BlockKind::Code | BlockKind::List | BlockKind::Callout | BlockKind::Quote
        ) {
            list.push(offer(
                Quick::KeepTogether,
                "D'un seul tenant",
                ops.keep_together == Some(true),
                "Ne jamais couper ce bloc entre deux pages",
            ));
        }
    }
    list.push(offer(Quick::Clear, "Effacer", false, "Effacer les retouches de ce bloc (Suppr)"));
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
        Quick::Clear => *ops = BlockOps::default(),
    }
}
