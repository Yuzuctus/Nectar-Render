//! La note et ses retouches résistent aux accidents.

use nectar_core::{Project, ProjectError};

fn note(dir: &std::path::Path, text: &str) -> std::path::PathBuf {
    let path = dir.join("Note.md");
    std::fs::write(&path, text).unwrap();
    path
}

#[test]
fn unreadable_retouches_are_set_aside_and_the_note_still_opens() {
    let dir = tempfile::tempdir().unwrap();
    let path = note(dir.path(), "# Titre\n\nTexte.\n");
    let layout = dir.path().join(".nectar").join("Note.md.json");
    std::fs::create_dir_all(layout.parent().unwrap()).unwrap();
    std::fs::write(&layout, "{ pas du json").unwrap();
    let project = Project::open(&path).expect("la note s'ouvre quand même");
    assert_eq!(project.notices.len(), 1, "{:?}", project.notices);
    assert!(!layout.exists());
    assert!(dir.path().join(".nectar").join("Note.md.json.illisible").exists());
}

#[test]
fn a_note_caught_mid_write_is_not_taken() {
    let dir = tempfile::tempdir().unwrap();
    let full: String = (0..40).map(|i| format!("Paragraphe {i} avec assez de texte.\n\n")).collect();
    let path = note(dir.path(), &full);
    let mut project = Project::open(&path).unwrap();
    let blocks = project.document.blocks.len();

    // Obsidian vient de vider le fichier, ou n'en a écrit que le début.
    std::fs::write(&path, "").unwrap();
    assert!(matches!(project.reload_checked(false), Err(ProjectError::Incomplete)));
    std::fs::write(&path, &full[..full.len() / 4]).unwrap();
    assert!(matches!(project.reload_checked(false), Err(ProjectError::Incomplete)));
    assert_eq!(project.document.blocks.len(), blocks, "le document n'a pas bougé");

    // Une vraie modification passe ; et après plusieurs essais, on prend ce qui est là.
    std::fs::write(&path, format!("{full}Nouveau paragraphe.\n")).unwrap();
    project.reload_checked(false).unwrap();
    assert_eq!(project.document.blocks.len(), blocks + 1);
    std::fs::write(&path, "Tout a été effacé.\n").unwrap();
    project.reload_checked(true).unwrap();
    assert_eq!(project.document.blocks.len(), 1);
}

#[test]
fn atomic_write_leaves_no_temporary_file() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("sous").join("fichier.json");
    nectar_core::write_atomically(&target, b"un").unwrap();
    nectar_core::write_atomically(&target, b"deux").unwrap();
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "deux");
    assert_eq!(std::fs::read_dir(target.parent().unwrap()).unwrap().count(), 1);
}
