//! Retouches écrites dans la note elle-même (le « repli » quand on ne veut
//! pas de fichier à part) :
//!
//! ```markdown
//! <!-- nectar: break-before, page=a3-paysage -->
//! ![[schema.png]]
//! ```
//!
//! Le commentaire s'applique au bloc qui le suit. `<!-- pagebreak -->`
//! (syntaxe de l'ancien Nectar Render) vaut `break-before`.

use crate::layout::{BlockOps, BlockStyle, DefaultPage, HAlign, ImageOps, PageChange, PageSpec, Placement, TextAlign};

#[derive(Debug, PartialEq)]
pub enum DirectiveLine {
    /// Ce n'est pas une directive Nectar.
    None,
    Ops(Box<BlockOps>),
    Invalid(String),
}

/// Analyse un bloc HTML : renvoie les retouches s'il s'agit d'une directive.
pub fn parse_html(html: &str) -> DirectiveLine {
    let Some(inner) = html.trim().strip_prefix("<!--").and_then(|s| s.trim_end().strip_suffix("-->")) else {
        return DirectiveLine::None;
    };
    let inner = inner.trim();
    if inner.eq_ignore_ascii_case("pagebreak") || inner.eq_ignore_ascii_case("page-break") {
        return DirectiveLine::Ops(Box::new(BlockOps { break_before: true, ..BlockOps::default() }));
    }
    let Some(body) = inner.strip_prefix("nectar:") else { return DirectiveLine::None };
    match parse_ops(body) {
        Ok(ops) => DirectiveLine::Ops(Box::new(ops)),
        Err(message) => DirectiveLine::Invalid(message),
    }
}

/// `break-before, page=a3-paysage, width=60, placement=full-page`
pub fn parse_ops(body: &str) -> Result<BlockOps, String> {
    let mut ops = BlockOps::default();
    for item in body.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let (key, value) = match item.split_once('=') {
            Some((k, v)) => (k.trim().to_lowercase(), Some(v.trim())),
            None => (item.to_lowercase(), None),
        };
        let number = |v: Option<&str>| {
            v.and_then(|v| v.trim_end_matches(['%', ' ']).trim_end_matches("mm").parse::<f32>().ok())
                .ok_or_else(|| format!("« {key} » attend un nombre"))
        };
        match (key.as_str(), value) {
            ("break-before" | "saut-avant", None) => ops.break_before = true,
            ("break-after" | "saut-apres" | "saut-après", None) => ops.break_after = true,
            ("keep-with-next" | "garder-avec-suivant", None) => ops.keep_with_next = true,
            ("push-to-bottom" | "bas-de-page", None) => ops.push_to_bottom = true,
            ("keep-together" | "insecable" | "insécable", None) => ops.keep_together = Some(true),
            ("allow-break" | "secable" | "sécable", None) => ops.keep_together = Some(false),
            ("hidden" | "masquer", None) => ops.hidden = true,
            ("space-before" | "espace-avant", v) => ops.space_before_mm = Some(number(v)?),
            ("page", Some(v)) if v.eq_ignore_ascii_case("default") || v.eq_ignore_ascii_case("défaut") => {
                ops.page = Some(PageChange::Default(DefaultPage::Default));
            }
            ("suite" | "et-suivantes" | "onward", None) => ops.page_onward = true,
            ("page", Some(v)) => {
                let spec = PageSpec::parse(v).ok_or_else(|| format!("format de page inconnu : {v}"))?;
                ops.page = Some(PageChange::Set(spec));
            }
            ("width" | "largeur", v) => image(&mut ops).width_percent = Some(number(v)?),
            ("align" | "alignement", Some(v)) => {
                image(&mut ops).align = Some(match v.to_lowercase().as_str() {
                    "left" | "gauche" => HAlign::Left,
                    "center" | "centre" => HAlign::Center,
                    "right" | "droite" => HAlign::Right,
                    _ => return Err(format!("alignement inconnu : {v}")),
                });
            }
            ("placement", Some(v)) => {
                image(&mut ops).placement = match v.to_lowercase().as_str() {
                    "inline" | "texte" => Placement::Inline,
                    "top" | "haut" => Placement::Top,
                    "bottom" | "bas" => Placement::Bottom,
                    "full-page" | "pleine-page" => Placement::FullPage,
                    "landscape" | "paysage" => Placement::Landscape,
                    _ => return Err(format!("placement inconnu : {v}")),
                };
            }
            ("center" | "centre" | "centrer", None) => style(&mut ops).align = Some(TextAlign::Center),
            ("right" | "droite", None) => style(&mut ops).align = Some(TextAlign::Right),
            ("columns" | "colonnes", v) => style(&mut ops).columns = Some(number(v)? as u8),
            ("size" | "taille", v) => style(&mut ops).size_percent = Some(number(v)?),
            ("caption" | "legende" | "légende", Some(v)) => {
                image(&mut ops).caption = Some(v.trim_matches('"').to_string());
            }
            _ => return Err(format!("directive inconnue : {item}")),
        }
    }
    Ok(ops)
}

fn style(ops: &mut BlockOps) -> &mut BlockStyle {
    ops.style.get_or_insert_with(BlockStyle::default)
}

fn image(ops: &mut BlockOps) -> &mut ImageOps {
    ops.image.get_or_insert_with(ImageOps::default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_french_and_english_keys() {
        let DirectiveLine::Ops(ops) =
            parse_html("<!-- nectar: saut-avant, page=a3-paysage, largeur=60%, placement=pleine-page -->")
        else {
            panic!()
        };
        assert!(ops.break_before);
        assert_eq!(ops.page, Some(PageChange::Set(PageSpec::paper("a3", true))));
        let image = ops.image.unwrap();
        assert_eq!((image.width_percent, image.placement), (Some(60.0), Placement::FullPage));
    }

    #[test]
    fn ignores_other_comments_and_reports_errors() {
        assert_eq!(parse_html("<!-- un commentaire -->"), DirectiveLine::None);
        assert_eq!(parse_html("<div>x</div>"), DirectiveLine::None);
        assert!(matches!(parse_html("<!-- nectar: voler -->"), DirectiveLine::Invalid(_)));
    }
}
