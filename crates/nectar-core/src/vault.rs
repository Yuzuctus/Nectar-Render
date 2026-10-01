//! Le coffre Obsidian : où chercher les images et où ranger les retouches.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Un coffre Obsidian, ou à défaut le dossier de la note.
#[derive(Debug)]
pub struct Vault {
    root: PathBuf,
    obsidian: bool,
    /// Valeur de `attachmentFolderPath` dans `.obsidian/app.json`.
    attachment_folder: Option<String>,
    /// Nom de fichier en minuscules → chemins, construit à la demande.
    index: OnceLock<HashMap<String, Vec<PathBuf>>>,
}

impl Vault {
    /// Remonte depuis la note jusqu'au dossier qui contient `.obsidian`.
    pub fn discover(note: &Path) -> Self {
        let note = absolute(note);
        let start = note.parent().unwrap_or(Path::new(".")).to_path_buf();
        let found = start.ancestors().find(|dir| dir.join(".obsidian").is_dir());
        match found {
            Some(root) => Self {
                attachment_folder: read_attachment_folder(root),
                root: root.to_path_buf(),
                obsidian: true,
                index: OnceLock::new(),
            },
            None => Self::at(start),
        }
    }

    /// Un dossier quelconque utilisé comme racine.
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: absolute(&root.into()), obsidian: false, attachment_folder: None, index: OnceLock::new() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn is_obsidian(&self) -> bool {
        self.obsidian
    }

    /// Fichier de retouches d'une note : `<coffre>/.nectar/<chemin>.json`.
    pub fn layout_path(&self, note: &Path) -> PathBuf {
        let note = absolute(note);
        let relative = note.strip_prefix(&self.root).unwrap_or(note.file_name().map(Path::new).unwrap_or(&note));
        let mut path = self.root.join(".nectar").join(relative);
        let name = format!("{}.json", path.file_name().and_then(|n| n.to_str()).unwrap_or("note"));
        path.set_file_name(name);
        path
    }

    /// Trouve le fichier désigné par une image ou un embed.
    ///
    /// Ordre : chemin absolu, relatif à la note, relatif au coffre, dossier des
    /// pièces jointes, puis recherche par nom dans tout le coffre (comme
    /// Obsidian), en préférant le fichier le plus proche de la note.
    pub fn resolve(&self, target: &str, note_dir: &Path) -> Option<PathBuf> {
        let target = clean_target(target)?;
        let as_path = Path::new(&target);
        if as_path.is_absolute() {
            return as_path.is_file().then(|| as_path.to_path_buf());
        }

        let mut candidates = vec![note_dir.join(as_path), self.root.join(as_path)];
        if let Some(folder) = &self.attachment_folder {
            let base = if let Some(rest) = folder.strip_prefix("./") {
                note_dir.join(rest)
            } else if folder == "/" {
                self.root.clone()
            } else {
                self.root.join(folder)
            };
            candidates.push(base.join(as_path));
        }
        if let Some(found) = candidates.into_iter().find(|p| p.is_file()) {
            return Some(found);
        }

        let name = as_path.file_name()?.to_str()?.to_lowercase();
        let mut matches: Vec<&PathBuf> = self.index().get(&name)?.iter().collect();
        // Si la cible contient un dossier (`img/a.png`), il doit correspondre.
        if target.contains('/') {
            let suffix = target.to_lowercase();
            matches.retain(|p| p.to_string_lossy().replace('\\', "/").to_lowercase().ends_with(&suffix));
        }
        matches.into_iter().min_by_key(|p| (distance(note_dir, p), p.as_os_str().len())).cloned()
    }

    fn index(&self) -> &HashMap<String, Vec<PathBuf>> {
        self.index.get_or_init(|| {
            let mut index: HashMap<String, Vec<PathBuf>> = HashMap::new();
            let walker = walkdir::WalkDir::new(&self.root).into_iter().filter_entry(|entry| {
                let name = entry.file_name().to_string_lossy();
                entry.depth() == 0 || !(name.starts_with('.') || name == "node_modules")
            });
            for entry in walker.flatten() {
                if entry.file_type().is_file() {
                    let name = entry.file_name().to_string_lossy().to_lowercase();
                    index.entry(name).or_default().push(entry.into_path());
                }
            }
            index
        })
    }
}

/// Retire les ancres (`#titre`, `^bloc`) et décode les `%20` d'un lien.
fn clean_target(target: &str) -> Option<String> {
    let target = target.trim().trim_start_matches('<').trim_end_matches('>');
    if target.is_empty() || target.contains("://") || target.starts_with("data:") {
        return None;
    }
    let target = target.split(['#', '^']).next().unwrap_or(target);
    Some(percent_decode(target))
}

pub(crate) fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Some(v) = input.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| input.to_string())
}

/// Nombre de dossiers à remonter puis descendre pour aller de `dir` à `file`.
fn distance(dir: &Path, file: &Path) -> usize {
    let a: Vec<_> = dir.components().collect();
    let b: Vec<_> = file.parent().map(|p| p.components().collect()).unwrap_or_default();
    let common = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    (a.len() - common) + (b.len() - common)
}

fn read_attachment_folder(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join(".obsidian").join("app.json")).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    json.get("attachmentFolderPath")?.as_str().map(str::to_string)
}

fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_attachment_anywhere_in_vault() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join(".obsidian")).unwrap();
        std::fs::create_dir_all(root.join("notes")).unwrap();
        std::fs::create_dir_all(root.join("media/2024")).unwrap();
        std::fs::write(root.join("media/2024/Schéma.PNG"), b"x").unwrap();
        std::fs::write(root.join("notes/n.md"), "").unwrap();

        let vault = Vault::discover(&root.join("notes/n.md"));
        assert!(vault.is_obsidian());
        let found = vault.resolve("schéma.png", &root.join("notes")).unwrap();
        assert!(found.ends_with("media/2024/Schéma.PNG"));
    }

    #[test]
    fn decodes_percent_and_ignores_urls() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("my img.png"), b"x").unwrap();
        let vault = Vault::at(dir.path());
        assert!(vault.resolve("my%20img.png", dir.path()).is_some());
        assert!(vault.resolve("https://example.com/a.png", dir.path()).is_none());
    }

    #[test]
    fn layout_path_mirrors_note() {
        let vault = Vault::at("/coffre");
        let path = vault.layout_path(Path::new("/coffre/cours/maths.md"));
        assert_eq!(path, Path::new("/coffre/.nectar/cours/maths.md.json"));
    }
}
