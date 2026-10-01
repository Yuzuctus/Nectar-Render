//! Compilation Typst locale pour Nectar Render.
//!
//! [`Engine`] garde les polices et la bibliothèque standard entre deux
//! compilations : seule la source change, et Typst ne refait que ce qui a
//! bougé (compilation incrémentale), ce qui permet l'aperçu en direct.

mod world;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use nectar_core::{BlockId, Generated};
use typst::diag::{Severity, SourceDiagnostic, Warned};
use typst::foundations::{Bytes, Label, Selector, Smart, Value};
use typst::introspection::{Introspector, MetadataElem};
use typst::text::Font;
use typst::utils::{LazyHash, PicoStr};
use typst::{Library, LibraryExt, World, WorldExt};
use typst_kit::fonts::FontStore;
use typst_layout::PagedDocument;

use world::NectarWorld;

/// Les polices IBM Plex du design Agrume, embarquées dans l'exécutable.
const BUNDLED_FONTS: &[&[u8]] = &[
    include_bytes!("../../../assets/fonts/IBMPlexSans-Regular.ttf"),
    include_bytes!("../../../assets/fonts/IBMPlexSans-Italic.ttf"),
    include_bytes!("../../../assets/fonts/IBMPlexSans-SemiBold.ttf"),
    include_bytes!("../../../assets/fonts/IBMPlexSans-SemiBoldItalic.ttf"),
    include_bytes!("../../../assets/fonts/IBMPlexSansCondensed-SemiBold.ttf"),
    include_bytes!("../../../assets/fonts/IBMPlexMono-Regular.ttf"),
    include_bytes!("../../../assets/fonts/IBMPlexMono-Italic.ttf"),
    include_bytes!("../../../assets/fonts/IBMPlexMono-Medium.ttf"),
    include_bytes!("../../../assets/fonts/IBMPlexMono-SemiBold.ttf"),
];

const LIB_TYP: &str = include_str!("../../../assets/typst/lib.typ");
const MITEX: &[(&str, &str)] = &[
    ("/nectar/mitex/mod.typ", include_str!("../../../assets/typst/mitex/mod.typ")),
    ("/nectar/mitex/prelude.typ", include_str!("../../../assets/typst/mitex/prelude.typ")),
    ("/nectar/mitex/latex/standard.typ", include_str!("../../../assets/typst/mitex/latex/standard.typ")),
];

/// Un thème intégré : son template et ses fichiers d'accompagnement.
pub struct Theme {
    pub name: &'static str,
    pub label: &'static str,
    source: &'static str,
    files: &'static [(&'static str, &'static [u8])],
}

pub const THEMES: &[Theme] = &[Theme {
    name: "agrume",
    label: "Agrume",
    source: include_str!("../../../assets/themes/agrume.typ"),
    files: &[("/nectar/agrume.tmTheme", include_bytes!("../../../assets/themes/agrume.tmTheme"))],
}];

pub fn theme(name: &str) -> Option<&'static Theme> {
    THEMES.iter().find(|t| t.name.eq_ignore_ascii_case(name))
}

/// Où chercher les polices en plus de celles embarquées.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontSources {
    /// Plex + polices de Typst seulement : rendu identique partout.
    Bundled,
    /// Ajoute les polices installées sur la machine.
    WithSystem,
}

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("thème inconnu : {0}")]
    UnknownTheme(String),
    #[error("la compilation a échoué :\n{}", .0.join("\n"))]
    Compile(Vec<String>),
    #[error("l'export PDF a échoué :\n{}", .0.join("\n"))]
    Pdf(Vec<String>),
    #[error("page {0} inexistante")]
    NoSuchPage(usize),
    #[error("encodage PNG impossible : {0}")]
    Png(String),
}

/// Le moteur de rendu, à garder ouvert pendant toute la session.
pub struct Engine {
    library: Arc<LazyHash<Library>>,
    fonts: Arc<FontStore>,
    images: Arc<Mutex<HashMap<PathBuf, (std::time::SystemTime, Bytes)>>>,
}

impl Engine {
    pub fn new(sources: FontSources) -> Self {
        let mut fonts = FontStore::new();
        for data in BUNDLED_FONTS {
            for font in Font::iter(Bytes::new(*data)) {
                let info = font.info().clone();
                fonts.push((font, info));
            }
        }
        fonts.extend(typst_kit::fonts::embedded());
        if sources == FontSources::WithSystem {
            fonts.extend(typst_kit::fonts::system());
        }
        Self {
            library: Arc::new(LazyHash::new(Library::builder().build())),
            fonts: Arc::new(fonts),
            images: Arc::default(),
        }
    }

    /// Compile une source générée avec le thème nommé.
    pub fn compile(&self, generated: &Generated, theme_name: &str) -> Result<Compiled, EngineError> {
        let theme = theme(theme_name).ok_or_else(|| EngineError::UnknownTheme(theme_name.to_string()))?;
        let mut texts: Vec<(&str, &str)> = vec![("/nectar/lib.typ", LIB_TYP), ("/nectar/theme.typ", theme.source)];
        texts.extend_from_slice(MITEX);
        let world = NectarWorld::new(
            self.library.clone(),
            self.fonts.clone(),
            self.images.clone(),
            generated.source.clone(),
            &texts,
            theme.files,
            &generated.assets,
        );

        let Warned { output, warnings } = typst::compile::<PagedDocument>(&world);
        let warnings = warnings.iter().map(|d| describe(&world, generated, d)).collect();
        let document = output
            .map_err(|errors| EngineError::Compile(errors.iter().map(|d| describe(&world, generated, d)).collect()))?;

        // Libère de temps en temps le cache incrémental des vieilles versions.
        typst::comemo::evict(30);
        Ok(Compiled { document, warnings })
    }
}

/// Un document mis en page.
pub struct Compiled {
    pub document: PagedDocument,
    pub warnings: Vec<String>,
}

/// Où commence un bloc dans les pages.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockPosition {
    pub id: BlockId,
    /// Page, à partir de 0.
    pub page: usize,
    /// Coordonnées en points depuis le coin haut gauche de la page.
    pub x: f64,
    pub y: f64,
}

/// Métadonnées et options de l'export PDF.
#[derive(Debug, Clone, Default)]
pub struct PdfOptions {
    /// Identifiant stable du document (le chemin de la note, par exemple).
    pub ident: Option<String>,
}

impl Compiled {
    pub fn page_count(&self) -> usize {
        self.document.pages().len()
    }

    /// Taille d'une page en points.
    pub fn page_size(&self, page: usize) -> Option<(f64, f64)> {
        let size = self.document.pages().get(page)?.frame.size();
        Some((size.x.to_pt(), size.y.to_pt()))
    }

    pub fn pdf(&self, options: &PdfOptions) -> Result<Vec<u8>, EngineError> {
        let typst_options = typst_pdf::PdfOptions {
            ident: options.ident.clone().map(Smart::Custom).unwrap_or(Smart::Auto),
            creator: Smart::Custom(Some(format!("Nectar Render {}", env!("CARGO_PKG_VERSION")))),
            ..typst_pdf::PdfOptions::default()
        };
        typst_pdf::pdf(&self.document, &typst_options)
            .map_err(|errors| EngineError::Pdf(errors.iter().map(|d| d.message.to_string()).collect()))
    }

    /// Rend une page en PNG, à `ppi` pixels par pouce.
    pub fn png(&self, page: usize, ppi: f32) -> Result<Vec<u8>, EngineError> {
        self.pixmap(page, ppi)?.encode_png().map_err(|e| EngineError::Png(e.to_string()))
    }

    /// Rend une page en pixels RGBA (prémultipliés), pour l'aperçu.
    pub fn rgba(&self, page: usize, ppi: f32) -> Result<(u32, u32, Vec<u8>), EngineError> {
        let pixmap = self.pixmap(page, ppi)?;
        Ok((pixmap.width(), pixmap.height(), pixmap.take()))
    }

    fn pixmap(&self, page: usize, ppi: f32) -> Result<tiny_skia::Pixmap, EngineError> {
        let page = self.document.pages().get(page).ok_or(EngineError::NoSuchPage(page))?;
        let options =
            typst_render::RenderOptions { pixel_per_pt: (f64::from(ppi) / 72.0).into(), ..Default::default() };
        Ok(typst_render::render(page, &options))
    }

    /// Position de chaque bloc marqué par `#nb(...)`, dans l'ordre du document.
    pub fn block_positions(&self) -> Vec<BlockPosition> {
        let introspector = self.document.introspector();
        let label = Label::new(PicoStr::intern("nectar-block")).expect("étiquette valide");
        introspector
            .query(&Selector::Label(label))
            .iter()
            .filter_map(|content| {
                let location = content.location()?;
                let meta = content.to_packed::<MetadataElem>()?;
                let Value::Str(id) = &meta.value else { return None };
                let position = introspector.position(location)?;
                Some(BlockPosition {
                    id: BlockId(id.to_string()),
                    page: position.page.get() - 1,
                    x: position.point.x.to_pt(),
                    y: position.point.y.to_pt(),
                })
            })
            .collect()
    }
}

/// Message lisible : la ligne et, si possible, le bloc de la note en cause.
fn describe(world: &NectarWorld, generated: &Generated, diag: &SourceDiagnostic) -> String {
    let level = match diag.severity {
        Severity::Error => "erreur",
        Severity::Warning => "avertissement",
    };
    let mut text = format!("{level} : {}", diag.message);
    if let Some(range) = world.range(diag.span)
        && let Some(line) = world.main_source().lines().byte_to_line(range.start)
        && diag.span.id() == Some(World::main(world))
    {
        let line = line + 1;
        text.push_str(&format!(" (source générée, ligne {line}"));
        if let Some(id) = generated.block_at_line(line) {
            text.push_str(&format!(", bloc {id}"));
        }
        text.push(')');
    }
    for hint in &diag.hints {
        text.push_str(&format!("\n  conseil : {}", hint.v));
    }
    text
}
