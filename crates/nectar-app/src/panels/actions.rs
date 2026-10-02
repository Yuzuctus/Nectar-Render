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

const NEXT_PAGE: &str = "Envoie ce bloc, et tout ce qui le suit, en haut de la page suivante (Ctrl + Entrée).";

/// Les actions qui ont du sens pour ce bloc, dans l'ordre d'affichage.
pub fn offers(app: &NectarApp, id: &nectar_core::BlockId) -> Vec<Offer> {
    let Some(project) = &app.project else { return Vec::new() };
    let Some(block) = project.document.blocks.iter().find(|b| &b.id == id) else {
        // Une puce de liste : pas de bloc de premier niveau.
        let ops = project.layout.ops_for(&project.document, id);
        return vec![
            offer(Quick::BreakBefore, "↓ Page suivante", ops.break_before, NEXT_PAGE),
            offer(Quick::Clear, "Tout annuler", false, "Retire toutes les retouches de cette puce (Suppr)."),
        ];
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
            "Choisi automatiquement pour être lu en grand. Cliquer : le garder dans le texte, en portrait.",
        ));
    } else if figure || table {
        list.push(offer(
            Quick::Landscape,
            "Page paysage",
            placement == Placement::Landscape,
            "Seul sur une page tournée, en grand ; le texte reprend ensuite au format normal.",
        ));
    }
    if figure {
        list.push(offer(
            Quick::FullPage,
            "Pleine page",
            placement == Placement::FullPage,
            "L'image seule sur sa page, aussi grande que possible.",
        ));
        list.push(offer(
            Quick::BreakAfter,
            "Fin de page après",
            ops.break_after,
            "Laisse le reste de la page vide après ce bloc : la suite commence page suivante.",
        ));
    } else {
        // Un titre reste déjà toujours avec ce qui le suit.
        if !matches!(block.node, Node::Heading { .. }) {
            list.push(offer(
                Quick::KeepWithNext,
                "Lier au suivant",
                ops.keep_with_next,
                "Jamais séparé du bloc qui le suit par une fin de page : s'il le faut, les deux passent ensemble à la page suivante.",
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
                "Le bloc reste entier sur une seule page, jamais coupé en deux.",
            ));
        }
    }
    list.push(offer(Quick::Clear, "Tout annuler", false, "Retire toutes les retouches de ce bloc (Suppr)."));
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
