//! L'assistant voit les pages réellement produites.

use nectar_core::{Layout, ParseOptions, Style, generate, parse};
use nectar_typst::{Engine, FontSources, inspect};

#[test]
fn tall_image_is_reported_as_shrunk() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("grand.svg"),
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="600" height="1400"><rect width="600" height="1400" fill="#cde"/></svg>"##,
    )
    .unwrap();
    let options = ParseOptions { note_dir: Some(dir.path()), ..Default::default() };
    let doc = parse("# Essai\n\nOuverture.\n\nVoici le schéma :\n\n![[grand.svg]]\n\nSuite.\n", &options);
    let layout = Layout::default();
    let style = Style::default();
    let generated = generate(&doc, &layout, &style);
    let engine = Engine::new(FontSources::Bundled);
    let compiled = engine.compile(&generated).unwrap();
    assert!(compiled.template_notes().iter().any(|n| n.kind == "shrunk"), "{:?}", compiled.template_notes());
    // La phrase qui annonce l'image tient sur la même page qu'elle.
    let positions = compiled.block_positions();
    let page = |prefix: &str| positions.iter().find(|p| p.id.as_str().starts_with(prefix)).map(|p| p.page);
    assert_eq!(page("p-"), Some(0));
    let issues = inspect(&compiled, &doc, &layout, &style, &generated, &[]);
    assert!(issues.iter().any(|i| i.title.starts_with("Image réduite")), "{issues:#?}");
}
