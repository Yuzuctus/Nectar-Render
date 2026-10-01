//! Cœur de Nectar Render : lecture des notes Obsidian, modèle de document,
//! retouches de mise en page et génération de la source Typst.
//!
//! Ce crate ne dépend pas de Typst : il produit du texte, que `nectar-typst`
//! compile.

pub mod code_themes;
pub mod codegen;
pub mod directives;
pub mod frontmatter;
pub mod ids;
pub mod layout;
pub mod model;
pub mod parse;
pub mod style;
mod typst_style;
pub mod vault;

pub use codegen::{Asset, Generated, generate};
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
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("lecture de {path} impossible : {source}")]
    Read { path: PathBuf, source: std::io::Error },
    #[error(transparent)]
    Layout(#[from] layout::LayoutError),
}

impl Project {
    /// Ouvre une note et charge ses retouches si elles existent.
    pub fn open(note: &Path) -> Result<Self, ProjectError> {
        let vault = Vault::discover(note);
        let layout_path = vault.layout_path(note);
        let layout = Layout::load(&layout_path)?;
        let presets = PresetStore::load(style::default_user_dir());
        let mut project =
            Self { note: note.to_path_buf(), vault, document: Document::default(), layout, layout_path, presets };
        project.reload()?;
        Ok(project)
    }

    /// Relit la note (après une modification dans Obsidian).
    pub fn reload(&mut self) -> Result<(), ProjectError> {
        let text = std::fs::read_to_string(&self.note)
            .map_err(|source| ProjectError::Read { path: self.note.clone(), source })?;
        let note_dir = self.note.parent().map(Path::to_path_buf).unwrap_or_default();
        let options = ParseOptions { vault: Some(&self.vault), note_dir: Some(&note_dir) };
        self.document = parse(&text, &options);
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
        self.layout.prune();
        self.layout.save(&self.layout_path)?;
        Ok(())
    }
}
