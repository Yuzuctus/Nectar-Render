//! Le placement automatique en deux temps.

use nectar_core::{Layout, ParseOptions, Style, parse};
use nectar_typst::{Engine, FontSources, lay_out};

/// Une page remplie à un peu plus de la moitié, puis un titre et un petit
/// tableau (10 lignes) trop haut pour la place restante.
fn note(paragraphs: usize) -> String {
    let mut text = String::from("# Compte rendu\n\n");
    for _ in 0..paragraphs {
        text.push_str(
            "Un paragraphe d'explication assez long pour occuper plusieurs lignes de la page, \
             avec le détail de la configuration réseau mise en place pendant la séance.\n\n",
        );
    }
    text.push_str("## Plan d'adressage\n\n| Ressource | Adresse | Rôle |\n|---|---|---|\n");
    for i in 0..10 {
        text.push_str(&format!(
            "| Machine {i} | `10.0.{i}.1` | Un rôle décrit en plusieurs mots pour que la cellule \
             s'étale sur trois ou quatre lignes dans sa colonne, comme dans un vrai compte rendu |\n"
        ));
    }
    text.push_str("\nSuite du texte.\n");
    text
}

#[test]
fn small_table_is_split_rather_than_leaving_half_a_page_empty() {
    let engine = Engine::new(FontSources::Bundled);
    let doc = parse(&note(9), &ParseOptions::default());
    let laid = lay_out(&engine, &doc, &Layout::default(), &Style::default());
    let compiled = laid.compiled.expect("compilation");
    let positions = compiled.block_positions();
    let page = |prefix: &str| positions.iter().find(|p| p.id.as_str().starts_with(prefix)).unwrap().page;
    assert_eq!(laid.relaxed.len(), 1, "le tableau est autorisé à se couper");
    assert!(laid.relaxed[0].as_str().starts_with("table-"));
    assert_eq!(page("table-"), 0, "le tableau commence sous le texte, sur la première page");
    // Le titre reste avec le début du tableau.
    let heading = positions.iter().find(|p| p.id.as_str().starts_with("h-") && p.page == 0 && p.y > 100.0);
    assert!(heading.is_some(), "{positions:?}");
}

#[test]
fn a_manual_keep_together_is_respected() {
    let engine = Engine::new(FontSources::Bundled);
    let doc = parse(&note(9), &ParseOptions::default());
    let mut layout = Layout::default();
    let table = doc.anchors().into_iter().find(|a| a.id.as_str().starts_with("table-")).unwrap();
    layout.ops_mut(table).keep_together = Some(true);
    let laid = lay_out(&engine, &doc, &layout, &Style::default());
    assert!(laid.relaxed.is_empty());
    let compiled = laid.compiled.expect("compilation");
    let table_page = compiled.block_positions().into_iter().find(|p| p.id.as_str().starts_with("table-")).unwrap().page;
    assert_eq!(table_page, 1, "gardé d'un seul tenant, il passe à la page suivante");
}

#[test]
fn wide_schema_is_detected_then_laid_on_a_landscape_page() {
    use nectar_core::layout::{ImageOps, Placement};
    let dir = tempfile::tempdir().unwrap();
    let mut svg = String::from(r##"<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="900">"##);
    for i in 0..40 {
        svg.push_str(&format!(
            r##"<rect x="{}" y="{}" width="60" height="30" fill="none" stroke="#333"/>"##,
            i * 38,
            i * 20
        ));
    }
    svg.push_str("</svg>");
    std::fs::write(dir.path().join("reseau.svg"), svg).unwrap();
    let mut text = String::from("# Compte rendu\n\n## Architecture\n\nLe schéma suivant montre le réseau.\n\n");
    text.push_str("![[reseau.svg|Schéma du réseau]]\n\n");
    for _ in 0..4 {
        text.push_str("Explication du câblage, assez longue pour occuper quelques lignes de la page courante.\n\n");
    }
    text.push_str("## Suite\n\nUne autre section.\n");
    let options = ParseOptions { note_dir: Some(dir.path()), ..Default::default() };
    let doc = parse(&text, &options);
    let engine = Engine::new(FontSources::Bundled);
    let style = Style::default();

    // Premier temps : l'assistant propose la page paysage.
    let laid = lay_out(&engine, &doc, &Layout::default(), &style);
    let compiled = laid.compiled.unwrap();
    let issues = nectar_typst::inspect(&compiled, &doc, &Layout::default(), &style, &laid.generated, &[]);
    let issue = issues.iter().find(|i| i.title.starts_with("Schéma à lire en grand")).expect("schéma repéré");
    assert_eq!(issue.fixes[0].action, nectar_core::assistant::FixAction::ImagePlacement(Placement::Landscape));

    // Une fois appliquée : l'image a sa page paysage, le texte qui la suit
    // remplit d'abord la page d'avant, et la section suivante reste après.
    let mut layout = Layout::default();
    let figure = doc.anchors().into_iter().find(|a| a.id.as_str().starts_with("fig-")).unwrap();
    let figure_id = figure.id.clone();
    layout.ops_mut(figure).image = Some(ImageOps { placement: Placement::Landscape, ..Default::default() });
    let compiled = lay_out(&engine, &doc, &layout, &style).compiled.unwrap();
    let positions = compiled.block_positions();
    let page = |id: &nectar_core::BlockId| positions.iter().find(|p| &p.id == id).unwrap().page;
    let (w, h) = compiled.page_size(page(&figure_id)).unwrap();
    assert!(w > h, "page paysage");
    assert_eq!(page(&figure_id), 1);
    let explanations: Vec<usize> =
        positions.iter().filter(|p| p.id.as_str().starts_with("p-")).map(|p| p.page).collect();
    assert!(explanations[..5].iter().all(|p| *p == 0), "le texte reste en page 1 : {explanations:?}");
    let suite = doc.blocks.iter().find(|b| b.excerpt.starts_with("Suite")).unwrap();
    assert_eq!(page(&suite.id), 2, "la section suivante vient après le schéma");
}
