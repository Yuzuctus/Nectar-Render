//! Le placement automatique en deux temps.

use nectar_core::auto::ChoiceKind;
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

    let figure = doc.anchors().into_iter().find(|a| a.id.as_str().starts_with("fig-")).unwrap();
    let figure_id = figure.id.clone();

    // Sans rien demander : la page paysage est choisie d'office, le texte
    // qui suit l'image remplit d'abord la page d'avant, et la section
    // suivante reste après.
    let laid = lay_out(&engine, &doc, &Layout::default(), &style);
    assert!(laid.choices.iter().any(|c| c.block == figure_id && c.kind == ChoiceKind::Landscape));
    let compiled = laid.compiled.unwrap();
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

    // « Tel quel » : la personne refuse, l'image reste dans le texte.
    let mut layout = Layout::default();
    layout.ops_mut(figure).manual = true;
    let compiled = lay_out(&engine, &doc, &layout, &style).compiled.unwrap();
    assert!(compiled.page_size(0).is_some_and(|(w, h)| w < h));
    assert_eq!(compiled.page_count(), 1);

    // Placement automatique coupé dans le style : l'assistant le propose.
    let mut manual_style = style.clone();
    manual_style.pagination.auto_landscape = false;
    let laid = lay_out(&engine, &doc, &Layout::default(), &manual_style);
    let compiled = laid.compiled.unwrap();
    let issues = nectar_typst::inspect(&compiled, &doc, &Layout::default(), &manual_style, &laid.generated, &[]);
    let issue = issues.iter().find(|i| i.title.starts_with("Schéma à lire en grand")).expect("schéma repéré");
    assert_eq!(issue.fixes[0].action, nectar_core::assistant::FixAction::ImagePlacement(Placement::Landscape));
    let _ = ImageOps::default();
}

#[test]
fn wide_tables_go_landscape_with_their_heading_and_fit_one_page() {
    let mut text = String::from("# Rapport\n\nIntroduction courte.\n\n## Scan des réseaux\n\nRésultat du scan :\n\n");
    text.push_str("| SSID | BSSID | Canal | Fréquence | Signal | Sécurité | Débit max | Observation |\n");
    text.push_str("|---|---|---|---|---|---|---|---|\n");
    for i in 0..12 {
        text.push_str(&format!(
            "| TP-WIFI-{i} | a4:2b:b0:{i:02x}:3c:9d | 6 | 2,412 GHz | -60 dBm | WPA2-PSK (AES) | 300 Mbit/s | Signal correct dans la salle, plus faible dans le couloir |\n"
        ));
    }
    text.push_str("\nSuite du rapport.\n");
    let doc = parse(&text, &ParseOptions::default());
    let engine = Engine::new(FontSources::Bundled);
    let laid = lay_out(&engine, &doc, &Layout::default(), &Style::default());
    let table = doc.blocks.iter().find(|b| b.id.as_str().starts_with("table-")).unwrap();
    assert!(laid.choices.iter().any(|c| c.block == table.id && c.kind == ChoiceKind::Landscape));
    let compiled = laid.compiled.unwrap();
    let positions = compiled.block_positions();
    let page = |prefix: &str| positions.iter().find(|p| p.id.as_str().starts_with(prefix)).unwrap().page;
    let (w, h) = compiled.page_size(page("table-")).unwrap();
    assert!(w > h, "tableau en paysage");
    // Le titre de la section et la phrase d'annonce l'accompagnent.
    let heading = doc.blocks.iter().find(|b| b.excerpt.starts_with("Scan")).unwrap();
    assert_eq!(positions.iter().find(|p| p.id == heading.id).unwrap().page, page("table-"));
    // Tout le tableau tient sur sa page, et le texte reprend en portrait.
    let margin = f64::from(Style::default().page.margin_bottom_mm) * 72.0 / 25.4;
    assert_eq!(compiled.block_boxes(margin).iter().filter(|b| b.id == table.id).count(), 1);
    let last = compiled.page_size(compiled.page_count() - 1).unwrap();
    assert!(last.0 < last.1);
}

#[test]
fn a_page_format_applies_to_its_page_only_unless_asked() {
    use nectar_core::layout::{PageChange, PageSpec};
    let mut text = String::from("# Rapport\n\nIntroduction.\n\n");
    for i in 0..140 {
        text.push_str(&format!("Paragraphe {i} : un texte assez long pour remplir les pages, ligne après ligne.\n\n"));
    }
    let doc = parse(&text, &ParseOptions::default());
    let engine = Engine::new(FontSources::Bundled);
    let target = doc.anchors().into_iter().find(|a| a.excerpt.starts_with("Paragraphe 5 ")).unwrap();
    let target_id = target.id.clone();

    for onward in [false, true] {
        let mut layout = Layout::default();
        let ops = layout.ops_mut(target);
        ops.page = Some(PageChange::Set(PageSpec::paper("a3", false)));
        ops.page_onward = onward;
        let compiled = lay_out(&engine, &doc, &layout, &Style::default()).compiled.unwrap();
        let page = compiled.block_positions().into_iter().find(|p| p.id == target_id).unwrap().page;
        let size = |i: usize| compiled.page_size(i).unwrap();
        assert!((size(page).1 - 1190.55).abs() < 1.0, "la page du bloc est en A3");
        assert!(compiled.page_count() > page + 1, "il reste des pages après");
        let next_is_a3 = (size(page + 1).1 - 1190.55).abs() < 1.0;
        assert_eq!(next_is_a3, onward, "onward = {onward}");
        // La page A3 se remplit avec la suite avant de revenir au format.
        let on_a3 = compiled.block_positions().into_iter().filter(|p| p.page == page).count();
        assert!(on_a3 > 5, "{on_a3} blocs sur la page A3");
    }
}

#[test]
fn a_screenshot_is_never_proposed_in_landscape() {
    let dir = tempfile::tempdir().unwrap();
    let mut svg = String::from(r##"<svg xmlns="http://www.w3.org/2000/svg" width="1920" height="1080">"##);
    for i in 0..30 {
        svg.push_str(&format!(r##"<text x="20" y="{}" font-size="20">Router#show vlan {i}</text>"##, 30 + i * 34));
    }
    svg.push_str("</svg>");
    std::fs::write(dir.path().join("capture.svg"), &svg).unwrap();
    std::fs::write(dir.path().join("plan.svg"), &svg).unwrap();
    let text = "# TP\n\nIntro.\n\n![[capture.svg|Capture de la configuration réseau du switch]]\n\n\
                ![[plan.svg|Plan d'adressage du réseau]]\n\nSuite.\n";
    let options = ParseOptions { note_dir: Some(dir.path()), ..Default::default() };
    let doc = parse(text, &options);
    let engine = Engine::new(FontSources::Bundled);
    let laid = lay_out(&engine, &doc, &Layout::default(), &Style::default());
    let compiled = laid.compiled.unwrap();
    let issues = nectar_typst::inspect(&compiled, &doc, &Layout::default(), &Style::default(), &laid.generated, &[]);
    assert!(!issues.iter().any(|i| i.title.starts_with("Schéma")), "{issues:#?}");
}

#[test]
fn many_page_formats_each_stay_on_their_page() {
    use nectar_core::layout::{PageChange, PageSpec};
    let mut text = String::from("# Long rapport\n\n");
    for i in 0..400 {
        text.push_str(&format!("Paragraphe {i} : du texte pour remplir un long document, ligne après ligne.\n\n"));
    }
    let doc = parse(&text, &ParseOptions::default());
    let engine = Engine::new(FontSources::Bundled);
    // Une page paysage tous les 20 paragraphes : bien plus de 12.
    let mut layout = Layout::default();
    let anchors: Vec<_> = doc.anchors().into_iter().filter(|a| a.excerpt.starts_with("Paragraphe")).collect();
    let owners: Vec<_> = anchors.iter().step_by(20).skip(1).copied().collect();
    for anchor in &owners {
        layout.ops_mut(*anchor).page = Some(PageChange::Set(PageSpec::paper("a4", true)));
    }
    let compiled = lay_out(&engine, &doc, &layout, &Style::default()).compiled.unwrap();
    let positions = compiled.block_positions();
    let owner_pages: std::collections::HashSet<usize> =
        owners.iter().map(|a| positions.iter().find(|p| &p.id == a.id).unwrap().page).collect();
    assert!(owner_pages.len() > 12);
    for page in 0..compiled.page_count() {
        let (w, h) = compiled.page_size(page).unwrap();
        assert_eq!(w > h, owner_pages.contains(&page), "page {} : {w}×{h}", page + 1);
    }
}

#[test]
fn a_slightly_too_tall_image_is_shrunk_to_fill_the_page() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("vue.svg"),
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="1000"><rect width="1200" height="1000" fill="#cde"/></svg>"##,
    )
    .unwrap();
    let options = ParseOptions { note_dir: Some(dir.path()), ..Default::default() };
    let engine = Engine::new(FontSources::Bundled);
    let mut off = Style::default();
    off.pagination.fit_images = false;
    let paragraph = "Un paragraphe de texte ordinaire, assez long pour occuper deux lignes de la page en cours.\n\n";
    let mut fitted_somewhere = false;
    for n in 4..16 {
        let text = format!("# Essai\n\n{}## Vue d'ensemble\n\n![[vue.svg]]\n\nFin.\n", paragraph.repeat(n));
        let doc = parse(&text, &options);
        let figure = doc.blocks.iter().find(|b| b.id.as_str().starts_with("fig-")).unwrap();
        let page =
            |c: &nectar_typst::Compiled| c.block_positions().into_iter().find(|p| p.id == figure.id).unwrap().page;
        if page(&lay_out(&engine, &doc, &Layout::default(), &off).compiled.unwrap()) == 0 {
            continue;
        }
        // Sans réduction, l'image part page suivante et laisse un trou.
        let laid = lay_out(&engine, &doc, &Layout::default(), &Style::default());
        let fitted = laid.choices.iter().find_map(|c| match c.kind {
            ChoiceKind::Fitted(percent) if c.block == figure.id => Some(percent),
            _ => None,
        });
        let landed = page(laid.compiled.as_ref().unwrap());
        match fitted {
            // Réduite (jamais sous 55 %), elle tient sur la première page.
            Some(percent) => {
                assert!((55..100).contains(&percent));
                assert_eq!(landed, 0, "{n} paragraphes");
                fitted_somewhere = true;
            }
            None => assert_eq!(landed, 1),
        }
    }
    assert!(fitted_somewhere);
}

#[test]
fn a_last_page_of_a_few_lines_is_absorbed() {
    let engine = Engine::new(FontSources::Bundled);
    let mut off = Style::default();
    off.pagination.avoid_short_last_page = false;
    let paragraph = "Un paragraphe de texte ordinaire, assez long pour occuper deux lignes de la page en cours.\n\n";
    // Le nombre de paragraphes qui déborde de quelques lignes sur une page 2.
    let (doc, count) = (10..60)
        .find_map(|n| {
            let doc = parse(&format!("# Essai\n\n{}", paragraph.repeat(n)), &ParseOptions::default());
            let compiled = lay_out(&engine, &doc, &Layout::default(), &off).compiled.unwrap();
            (compiled.page_count() == 2).then_some((doc, n))
        })
        .expect("un débordement");
    let laid = lay_out(&engine, &doc, &Layout::default(), &Style::default());
    assert_eq!(laid.compiled.unwrap().page_count(), 1, "{count} paragraphes : une seule page");
    assert!(laid.choices.iter().any(|c| c.kind == ChoiceKind::Tightened));
}

/// Une photo (JPEG) unie de `w` × `h` pixels.
fn photo(dir: &std::path::Path, name: &str, w: u32, h: u32) {
    let img = image::RgbImage::from_pixel(w, h, image::Rgb([90, 120, 160]));
    img.save(dir.join(name)).unwrap();
}

#[test]
fn a_caption_never_leaves_its_image() {
    let dir = tempfile::tempdir().unwrap();
    photo(dir.path(), "vue.jpg", 1600, 1000);
    let options = ParseOptions { note_dir: Some(dir.path()), ..Default::default() };
    let engine = Engine::new(FontSources::Bundled);
    let paragraph = "Un paragraphe de texte ordinaire, assez long pour occuper deux lignes de la page en cours.\n\n";
    let mut style = Style::default();
    style.pagination.fit_images = false;
    for n in 10..30 {
        let text =
            format!("# Essai\n\n{}![[vue.jpg]]\n\n*Capture : la vue d'ensemble.*\n\nSuite.\n", paragraph.repeat(n));
        let doc = parse(&text, &options);
        let compiled = lay_out(&engine, &doc, &Layout::default(), &style).compiled.unwrap();
        let positions = compiled.block_positions();
        let page = |prefix: &str| positions.iter().find(|p| p.id.as_str().starts_with(prefix)).unwrap().page;
        let caption = doc.blocks.iter().find(|b| b.excerpt.starts_with("Capture")).unwrap();
        let caption_page = positions.iter().find(|p| p.id == caption.id).unwrap().page;
        assert_eq!(page("fig-"), caption_page, "{n} paragraphes : la légende suit son image");
    }
}

#[test]
fn a_lonely_wide_photo_gets_a_landscape_page_a_tall_one_does_not() {
    let dir = tempfile::tempdir().unwrap();
    photo(dir.path(), "large.jpg", 3200, 1800);
    photo(dir.path(), "haute.jpg", 1800, 3200);
    let options = ParseOptions { note_dir: Some(dir.path()), ..Default::default() };
    let engine = Engine::new(FontSources::Bundled);
    for (file, landscape) in [("large.jpg", true), ("haute.jpg", false)] {
        let text = format!(
            "# Rapport\n\nIntroduction.\n\n# Annexe\n\n## E.1 Test de débit\n\n![[{file}]]\n\n*Capture : résultat autour de 9,63 Mbit/s.*\n\n# Fin\n\nConclusion.\n"
        );
        let mut layout = Layout::default();
        let doc = parse(&text, &options);
        // Chaque partie commence une page : l'image est seule sur la sienne.
        for anchor in doc.anchors().into_iter().filter(|a| a.excerpt == "Annexe" || a.excerpt == "Fin") {
            layout.ops_mut(anchor).break_before = true;
        }
        let laid = lay_out(&engine, &doc, &layout, &Style::default());
        let compiled = laid.compiled.unwrap();
        let positions = compiled.block_positions();
        let figure = positions.iter().find(|p| p.id.as_str().starts_with("fig-")).unwrap();
        let (w, h) = compiled.page_size(figure.page).unwrap();
        assert_eq!(w > h, landscape, "{file}");
        // Son titre et sa légende sont sur la même page ; rien n'est ajouté.
        let caption = doc.blocks.iter().find(|b| b.excerpt.starts_with("Capture")).unwrap();
        assert_eq!(positions.iter().find(|p| p.id == caption.id).unwrap().page, figure.page);
        assert_eq!(compiled.page_count(), 3, "{file}");
    }
}

#[test]
fn a_long_table_goes_a3_landscape_only_when_a4_landscape_is_not_enough() {
    let row = "| Exigence du sujet assez longue | Mise en œuvre retenue, décrite en une phrase assez longue pour passer à la ligne | Preuve et validation, avec les sections du rapport concernées | Limite éventuelle, elle aussi décrite en une phrase complète |\n";
    let engine = Engine::new(FontSources::Bundled);
    let doc_for = |rows: usize| {
        let text = format!(
            "# Annexe\n\n## D.1 Tableau de couverture\n\n| Exigence | Mise en œuvre | Preuve | Limite |\n|---|---|---|---|\n{}\nSuite.\n",
            row.repeat(rows)
        );
        parse(&text, &ParseOptions::default())
    };
    let margin = f64::from(Style::default().page.margin_bottom_mm) * 72.0 / 25.4;
    let mut seen = std::collections::HashSet::new();
    for rows in [8, 14, 22] {
        let doc = doc_for(rows);
        let laid = lay_out(&engine, &doc, &Layout::default(), &Style::default());
        let table = doc.blocks.iter().find(|b| b.id.as_str().starts_with("table-")).unwrap();
        let larger = laid.choices.iter().any(|c| c.block == table.id && c.kind == ChoiceKind::LargerPaper);
        let landscape = laid.choices.iter().any(|c| c.block == table.id && c.kind == ChoiceKind::Landscape);
        let compiled = laid.compiled.unwrap();
        let parts = compiled.block_boxes(margin).iter().filter(|b| b.id == table.id).count();
        if landscape {
            assert_eq!(parts, 1, "{rows} lignes : en paysage, le tableau tient sur une page");
            let page = compiled.block_positions().into_iter().find(|p| p.id == table.id).unwrap().page;
            let (w, h) = compiled.page_size(page).unwrap();
            assert!(w > h);
            if larger {
                assert!(w > 1100.0, "A3 paysage : {w} pt");
            }
        }
        seen.insert((landscape, larger));
    }
    assert!(seen.contains(&(true, true)), "un tableau a eu besoin de l'A3 : {seen:?}");
}
