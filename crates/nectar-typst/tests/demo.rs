//! Compile le coffre de démonstration et vérifie les retouches dans les pages.

use std::path::Path;

use nectar_core::Project;
use nectar_typst::{Engine, FontSources, PdfOptions};

fn demo() -> Project {
    let note = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/coffre-demo/Démo Nectar.md");
    Project::open(&note).expect("la note de démo s'ouvre")
}

#[test]
fn demo_vault_compiles_with_its_retouches() {
    let project = demo();
    assert!(project.document.warnings.is_empty(), "{:?}", project.document.warnings);
    let generated = project.generate();
    assert!(generated.warnings.is_empty(), "{:?}", generated.warnings);

    let engine = Engine::new(FontSources::Bundled);
    let compiled = engine.compile(&generated, "agrume").expect("compilation");
    assert!(compiled.warnings.is_empty(), "{:?}", compiled.warnings);

    let positions = compiled.block_positions();
    let page_of = |id: &str| positions.iter().find(|p| p.id.as_str() == id).map(|p| p.page).unwrap();
    let y_of = |id: &str| positions.iter().find(|p| p.id.as_str() == id).map(|p| p.y).unwrap();

    // La phrase d'introduction et la première puce sont sur deux pages.
    assert_eq!(page_of("li-8f4c06da"), page_of("p-8d8700e1") + 1);
    assert!(y_of("li-8f4c06da") < 80.0, "la puce ouvre sa page");

    // Après l'image, le reste de la page est vide.
    assert_eq!(page_of("p-83167b18"), page_of("fig-ac7fc788") + 1);

    // Le schéma et son titre sont sur une page A3 paysage, puis retour en A4.
    let a3 = page_of("fig-ecdcac58");
    assert_eq!(page_of("h-075ea2df"), a3);
    let (w, h) = compiled.page_size(a3).unwrap();
    assert!((w - 1190.55).abs() < 1.0 && (h - 841.89).abs() < 1.0, "{w}×{h}");
    let (w, h) = compiled.page_size(page_of("h-a58e8b04")).unwrap();
    assert!((w - 595.28).abs() < 1.0 && (h - 841.89).abs() < 1.0, "{w}×{h}");

    let pdf = compiled.pdf(&PdfOptions::default()).expect("export PDF");
    assert!(pdf.starts_with(b"%PDF"));
    assert!(!compiled.png(0, 30.0).unwrap().is_empty());
}
