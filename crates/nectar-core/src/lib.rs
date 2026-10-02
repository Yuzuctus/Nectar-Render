//! Cœur de Nectar Render : lecture des notes Obsidian, modèle de document,
//! retouches de mise en page et génération de la source Typst.
//!
//! Ce crate ne dépend pas de Typst : il produit du texte, que `nectar-typst`
//! compile.

pub mod assistant;
pub mod code_themes;
pub mod codegen;
pub mod directives;
pub mod excalidraw;
pub mod frontmatter;
mod html;
pub mod ids;
pub mod layout;
pub mod model;
pub mod parse;
pub mod style;
pub mod typo;
mod typst_style;
pub mod vault;

pub use codegen::{Asset, Generated, Tuning, generate, generate_tuned};
pub use layout::{BlockOps, Layout, PageChange, PageSpec};
pub use model::{Block, BlockId, BlockKind, Document};
pub use parse::{ParseOptions, parse};
pub use style::{PresetStore, Style, StyleRef};
pub use vault::Vault;

use std::path::{Path, PathBuf};

/// Une note ouverte : son coffre, son document et ses retouches.
pub struct Project {
    pub note: PathBuf,
    pub vault: Vault,
    pub document: Document,
    pub layout: Layout,
    /// D'où viennent les retouches (`.nectar/…json`).
    pub layout_path: PathBuf,
    pub presets: PresetStore,
    /// Ce qu'il faut signaler à l'ouverture (retouches illisibles mises de côté…).
    pub notices: Vec<String>,
    /// Longueur de la note à la dernière lecture.
    text_len: usize,
}

/// Écrit un fichier sans jamais laisser une version à moitié écrite : le
/// contenu va d'abord dans un fichier voisin, qui remplace ensuite l'ancien.
pub fn write_atomically(path: &Path, data: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut temp = path.as_os_str().to_owned();
    temp.push(".tmp");
    let temp = PathBuf::from(temp);
    std::fs::write(&temp, data)?;
    std::fs::rename(&temp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&temp);
    })
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("lecture de {path} impossible : {source}")]
    Read { path: PathBuf, source: std::io::Error },
    #[error(transparent)]
    Layout(#[from] layout::LayoutError),
    /// La note semble en cours d'écriture (vide ou tronquée) : à relire un
    /// peu plus tard.
    #[error("la note est en cours d'enregistrement")]
    Incomplete,
}

impl Project {
    /// Ouvre une note et charge ses retouches si elles existent.
    pub fn open(note: &Path) -> Result<Self, ProjectError> {
        let vault = Vault::discover(note);
        let layout_path = vault.layout_path(note);
        let mut notices = Vec::new();
        let layout = match Layout::load(&layout_path) {
            Ok(layout) => layout,
            // Retouches illisibles (fichier abîmé) : mises de côté, la note s'ouvre quand même.
            Err(layout::LayoutError::Json { .. }) => {
                let mut aside = layout_path.as_os_str().to_owned();
                aside.push(".illisible");
                let aside = PathBuf::from(aside);
                let _ = std::fs::rename(&layout_path, &aside);
                notices.push(format!(
                    "Les retouches de cette note étaient illisibles : elles ont été mises de côté dans {}",
                    aside.display()
                ));
                Layout::default()
            }
            Err(e) => return Err(e.into()),
        };
        let presets = PresetStore::load(style::default_user_dir());
        let mut project = Self {
            note: note.to_path_buf(),
            vault,
            document: Document::default(),
            layout,
            layout_path,
            presets,
            notices,
            text_len: 0,
        };
        project.reload()?;
        Ok(project)
    }

    /// Relit les retouches (modifiées hors de l'atelier).
    pub fn reload_layout(&mut self) -> Result<(), ProjectError> {
        self.layout = Layout::load(&self.layout_path)?;
        self.layout.heal(&self.document);
        Ok(())
    }

    /// Relit la note (après une modification dans Obsidian).
    pub fn reload(&mut self) -> Result<(), ProjectError> {
        self.reload_checked(true)
    }

    /// Relit la note ; sans `force`, une note qui semble à moitié écrite
    /// (vide, ou soudain deux fois plus courte) n'est pas prise : les
    /// retouches ne doivent jamais se recaler sur une version tronquée.
    pub fn reload_checked(&mut self, force: bool) -> Result<(), ProjectError> {
        let text = std::fs::read_to_string(&self.note)
            .map_err(|source| ProjectError::Read { path: self.note.clone(), source })?;
        let before = self.document.blocks.len();
        if !force && before > 0 && (text.trim().is_empty() || text.len() * 2 < self.text_len) {
            return Err(ProjectError::Incomplete);
        }
        self.text_len = text.len();
        let note_dir = self.note.parent().map(Path::to_path_buf).unwrap_or_default();
        let options = ParseOptions { vault: Some(&self.vault), note_dir: Some(&note_dir), depth: 0 };
        self.document = parse(&text, &options);
        self.document.name = self.note.file_stem().map(|s| s.to_string_lossy().into_owned());
        // Les retouches suivent le texte : on recale leurs ancres et on les
        // réenregistre si besoin.
        if self.layout.heal(&self.document) && self.layout_path.exists() {
            self.layout.save(&self.layout_path)?;
        }
        Ok(())
    }

    /// Le style complet de la note (preset + réglages faits à la main).
    pub fn style(&self) -> (Style, Option<String>) {
        self.presets.resolve(&self.layout.style)
    }

    pub fn generate(&self) -> Generated {
        let (style, warning) = self.style();
        let mut generated = generate(&self.document, &self.layout, &style);
        generated.warnings.extend(warning);
        generated
    }

    pub fn save_layout(&mut self) -> Result<(), ProjectError> {
        self.layout.prune(&self.document);
        self.layout.save(&self.layout_path)?;
        Ok(())
    }
}
