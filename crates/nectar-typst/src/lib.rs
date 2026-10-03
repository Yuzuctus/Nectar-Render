//! Compilation Typst locale pour Nectar Render.
//!
//! [`Engine`] garde les polices et la bibliothèque standard entre deux
//! compilations : seule la source change, et Typst ne refait que ce qui a
//! bougé (compilation incrémentale), ce qui permet l'aperçu en direct.

mod images;
mod world;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

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
    include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf"),
    include_bytes!("../../../assets/fonts/JetBrainsMono-Bold.ttf"),
    include_bytes!("../../../assets/fonts/JetBrainsMono-Italic.ttf"),
    include_bytes!("../../../assets/fonts/Twemoji.ttf"),
    include_bytes!("../../../assets/fonts/Excalifont-Regular.ttf"),
    include_bytes!("../../../assets/fonts/Virgil-Regular.ttf"),
];

const TEMPLATE: &str = include_str!("../../../assets/typst/nectar.typ");
const MITEX: &[(&str, &str)] = &[
    ("/nectar/mitex/mod.typ", include_str!("../../../assets/typst/mitex/mod.typ")),
    ("/nectar/mitex/prelude.typ", include_str!("../../../assets/typst/mitex/prelude.typ")),
    ("/nectar/mitex/latex/standard.typ", include_str!("../../../assets/typst/mitex/latex/standard.typ")),
];

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
    images: Arc<images::ImageCache>,
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

    /// Garde aussi les photos réduites dans ce dossier, d'une session à l'autre.
    pub fn with_cache_dir(mut self, dir: Option<PathBuf>) -> Self {
        self.images = Arc::new(images::ImageCache::with_disk(dir));
        self
    }

    /// Compile une source générée.
    pub fn compile(&self, generated: &Generated) -> Result<Compiled, EngineError> {
        // Les photos à réduire se préparent en parallèle, avant Typst.
        let photos: Vec<(PathBuf, u32)> = generated
            .assets
            .iter()
            .filter(|a| a.data.is_none())
            .filter_map(|a| a.max_px.map(|max| (a.path.clone(), max)))
            .collect();
        self.images.prepare(&photos);
        let mut texts: Vec<(&str, &str)> = vec![("/nectar/nectar.typ", TEMPLATE)];
        texts.extend_from_slice(MITEX);
        texts.extend(generated.files.iter().map(|(path, text)| (path.as_str(), text.as_str())));
        let world = NectarWorld::new(
            self.library.clone(),
            self.fonts.clone(),
            self.images.clone(),
            generated.source.clone(),
            &texts,
            &[],
            &generated.assets,
        );

        let Warned { output, warnings } = typst::compile::<PagedDocument>(&world);
        // Les polices absentes sont signalées une fois par `missing_fonts` :
        // Typst passe alors à la police de secours sans rien casser.
        let warnings = warnings
            .iter()
            .filter(|d| !d.message.starts_with("unknown font family"))
            .map(|d| describe(&world, generated, d))
            .collect();
        let document = output
            .map_err(|errors| EngineError::Compile(errors.iter().map(|d| describe(&world, generated, d)).collect()))?;

        // Libère de temps en temps le cache incrémental des vieilles versions.
        typst::comemo::evict(30);
        Ok(Compiled { document, warnings })
    }
}

impl Engine {
    /// Les polices demandées qui n'existent ni dans Nectar ni sur la machine.
    pub fn missing_fonts<'a>(&self, names: impl IntoIterator<Item = &'a String>) -> Vec<String> {
        let book = self.fonts.book();
        names
            .into_iter()
            .filter(|name| {
                let lower = name.to_lowercase();
                let family = lower.strip_suffix(" condensed").unwrap_or(&lower);
                !book.contains_family(family)
            })
            .cloned()
            .collect()
    }

    /// Les familles de polices disponibles, triées.
    pub fn font_families(&self) -> Vec<String> {
        let mut names: Vec<String> = self.fonts.book().families().map(|(name, _)| name.to_string()).collect();
        names.dedup();
        names
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

/// Boîte occupée par un bloc sur une page (points : gauche, haut, droite, bas).
#[derive(Debug, Clone, PartialEq)]
pub struct BlockBox {
    pub id: BlockId,
    pub page: usize,
    pub rect: [f64; 4],
}

/// Métadonnées et options de l'export PDF.
#[derive(Debug, Clone, Default)]
pub struct PdfOptions {
    /// Identifiant stable du document (le chemin de la note, par exemple).
    pub ident: Option<String>,
    /// Produire un PDF/A-2b (archivage).
    pub pdf_a: bool,
}

impl Compiled {
    pub fn page_count(&self) -> usize {
        self.document.pages().len()
    }

    /// Empreinte du contenu d'une page : deux pages identiques ont la même,
    /// ce qui évite de redessiner les pages qu'une retouche n'a pas touchées.
    pub fn page_hash(&self, page: usize) -> Option<u128> {
        self.document.pages().get(page).map(typst::utils::hash128)
    }

    /// Taille d'une page en points.
    pub fn page_size(&self, page: usize) -> Option<(f64, f64)> {
        let size = self.document.pages().get(page)?.frame.size();
        Some((size.x.to_pt(), size.y.to_pt()))
    }

    pub fn pdf(&self, options: &PdfOptions) -> Result<Vec<u8>, EngineError> {
        let standards = if options.pdf_a {
            typst_pdf::PdfStandards::new(&[typst_pdf::PdfStandard::A_2b])
                .map_err(|e| EngineError::Pdf(vec![e.message().to_string()]))?
        } else {
            typst_pdf::PdfStandards::default()
        };
        let typst_options = typst_pdf::PdfOptions {
            ident: options.ident.clone().map(Smart::Custom).unwrap_or(Smart::Auto),
            creator: Smart::Custom(Some(format!("Nectar Render {}", env!("CARGO_PKG_VERSION")))),
            standards,
            timestamp: world::now_utc().map(typst_pdf::Timestamp::new_utc),
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

    /// Étendue réelle du contenu de chaque page, pied de page exclu (tout ce
    /// qui commence dans la marge du bas).
    pub fn page_metrics(&self, margin_bottom_pt: f64) -> Vec<nectar_core::assistant::PageMetrics> {
        self.document
            .pages()
            .iter()
            .map(|page| {
                let size = page.frame.size();
                let cutoff = size.y.to_pt() - margin_bottom_pt + 1.0;
                let mut extent: Option<[f64; 4]> = None;
                walk(&page.frame, typst::layout::Point::zero(), cutoff, &mut extent);
                nectar_core::assistant::PageMetrics { width: size.x.to_pt(), height: size.y.to_pt(), content: extent }
            })
            .collect()
    }

    /// Boîte réelle de chaque bloc sur chaque page : l'union des éléments
    /// dessinés entre son marqueur et le suivant (pied de page exclu).
    pub fn block_boxes(&self, margin_bottom_pt: f64) -> Vec<BlockBox> {
        let positions = self.block_positions();
        let mut boxes: Vec<BlockBox> = Vec::new();
        for (page_index, page) in self.document.pages().iter().enumerate() {
            let cutoff = page.frame.size().y.to_pt() - margin_bottom_pt + 1.0;
            let mut rects = Vec::new();
            collect(&page.frame, typst::layout::Point::zero(), cutoff, &mut rects);
            for rect in rects {
                let owner = positions
                    .iter()
                    .rfind(|p| p.page < page_index || (p.page == page_index && p.y <= rect[1] + 6.0))
                    .or_else(|| positions.iter().find(|p| p.page == page_index));
                let Some(owner) = owner else { continue };
                match boxes.iter_mut().find(|b| b.id == owner.id && b.page == page_index) {
                    Some(b) => {
                        b.rect = [
                            b.rect[0].min(rect[0]),
                            b.rect[1].min(rect[1]),
                            b.rect[2].max(rect[2]),
                            b.rect[3].max(rect[3]),
                        ]
                    }
                    None => boxes.push(BlockBox { id: owner.id.clone(), page: page_index, rect }),
                }
            }
        }
        boxes
    }

    /// Les images posées dans les pages, rattachées à leur bloc.
    pub fn figures(&self) -> Vec<nectar_core::assistant::FigureView> {
        let positions = self.block_positions();
        let mut out = Vec::new();
        for (page_index, page) in self.document.pages().iter().enumerate() {
            let mut images = Vec::new();
            collect_images(&page.frame, typst::layout::Point::zero(), &mut images);
            for (rect, pixels) in images {
                let owner =
                    positions.iter().rfind(|p| p.page < page_index || (p.page == page_index && p.y <= rect[1] + 6.0));
                let Some(owner) = owner else { continue };
                out.push(nectar_core::assistant::FigureView {
                    id: owner.id.clone(),
                    page: page_index,
                    width: rect[2] - rect[0],
                    height: rect[3] - rect[1],
                    pixels,
                });
            }
        }
        out
    }

    /// Remarques laissées par le template (`<nectar-issue>`), avec leur place.
    pub fn template_notes(&self) -> Vec<nectar_core::assistant::TemplateNote> {
        let introspector = self.document.introspector();
        let label = Label::new(PicoStr::intern("nectar-issue")).expect("étiquette valide");
        introspector
            .query(&Selector::Label(label))
            .iter()
            .filter_map(|content| {
                let meta = content.to_packed::<MetadataElem>()?;
                let Value::Dict(dict) = &meta.value else { return None };
                let kind = match dict.get("kind").ok()? {
                    Value::Str(s) => s.to_string(),
                    _ => return None,
                };
                let value = match dict.get("value").ok() {
                    Some(Value::Float(f)) => *f,
                    Some(Value::Int(i)) => *i as f64,
                    _ => 0.0,
                };
                let position = introspector.position(content.location()?)?;
                Some(nectar_core::assistant::TemplateNote {
                    kind,
                    page: position.page.get() - 1,
                    y: position.point.y.to_pt(),
                    value,
                })
            })
            .collect()
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

/// L'assistant de mise en page sur un document compilé.
pub fn inspect(
    compiled: &Compiled,
    document: &nectar_core::Document,
    layout: &nectar_core::Layout,
    style: &nectar_core::Style,
    generated: &Generated,
    missing_fonts: &[String],
) -> Vec<nectar_core::assistant::Issue> {
    inspect_tuned(compiled, document, layout, style, generated, missing_fonts, &nectar_core::Tuning::default())
}

/// [`inspect`] d'une mise en page faite par [`lay_out`] : les décisions
/// automatiques comptent comme voulues (une page paysage choisie d'office
/// n'est pas signalée comme un trou).
pub fn inspect_tuned(
    compiled: &Compiled,
    document: &nectar_core::Document,
    layout: &nectar_core::Layout,
    style: &nectar_core::Style,
    generated: &Generated,
    missing_fonts: &[String],
    tuning: &nectar_core::Tuning,
) -> Vec<nectar_core::assistant::Issue> {
    use nectar_core::assistant::{Inputs, Marker, analyse};
    use nectar_core::layout::{ImageOps, Placement};
    let margin_bottom = f64::from(style.page.margin_bottom_mm) * 72.0 / 25.4;
    let pages = compiled.page_metrics(margin_bottom);
    let markers: Vec<Marker> =
        compiled.block_positions().into_iter().map(|p| Marker { id: p.id, page: p.page, y: p.y }).collect();
    let notes = compiled.template_notes();
    let figures = compiled.figures();
    let mut ops = layout.resolve(document).ops;
    for id in &tuning.landscape {
        ops.entry(id.clone()).or_default().image.get_or_insert_with(ImageOps::default).placement = Placement::Landscape;
    }
    for id in &tuning.pushed {
        ops.entry(id.clone()).or_default().break_before = true;
    }
    let warnings: Vec<String> =
        document.warnings.iter().chain(&generated.warnings).chain(&compiled.warnings).cloned().collect();
    analyse(&Inputs {
        document,
        ops: &ops,
        style,
        pages: &pages,
        markers: &markers,
        notes: &notes,
        figures: &figures,
        warnings: &warnings,
        missing_fonts,
    })
}

/// Une mise en page complète : la source finale et son résultat.
pub struct LaidOut {
    pub generated: Generated,
    pub compiled: Result<Compiled, EngineError>,
    /// Blocs que le second passage a autorisés à se couper.
    pub relaxed: Vec<BlockId>,
    /// Toutes les décisions du placement automatique.
    pub tuning: nectar_core::Tuning,
    /// Les mêmes, lisibles, bloc par bloc.
    pub choices: Vec<nectar_core::auto::Choice>,
    /// Nombre de compositions faites (une, plus une par ajustement essayé).
    pub passes: usize,
    /// Calcul abandonné en route (voir [`lay_out_with`]) : le résultat est
    /// incomplet et ne doit pas être montré.
    pub stopped: bool,
}

/// Met en page un document avec le placement automatique.
///
/// Avant tout : les tableaux nettement trop larges pour une page portrait
/// passent en paysage (sur estimation, vérifiée ensuite). Puis on compose et
/// on relit les pages, autant de fois que nécessaire (chaque recomposition
/// est incrémentale ; `NECTAR_TRACE=1` détaille les étapes) :
/// 0. le format revient après les pages au format propre ;
/// 1. les schémas larges et détaillés passent en paysage ; un tableau qui
///    dépasse la marge est resserré, puis réparti par Typst, puis tourné ;
/// 2. un bloc gardé d'un seul tenant qui laisse une page à moitié vide est
///    autorisé à se couper ; un grand tableau sur plusieurs pages essaie le
///    paysage puis l'A3 paysage, et garde le format qui prend le moins de
///    pages ; un tableau qui déborde de quelques lignes est resserré ;
/// 3. une image un peu trop haute pour la place restante est réduite juste
///    assez (jamais sous 55 %) ; une légende ne reste pas seule en haut de
///    page ; une image horizontale seule sur sa page passe en paysage ;
/// 4. une dernière page de quelques lignes est résorbée ;
/// 5. une page paysage qui laisserait la page d'avant à moitié vide est
///    repoussée après le texte qui la suit ;
/// 6. et 7. : vérifications finales (retours de format, légendes).
///
/// Une retouche manuelle n'est jamais remise en cause, et un bloc « tel
/// quel » n'est jamais touché.
pub fn lay_out(
    engine: &Engine,
    document: &nectar_core::Document,
    layout: &nectar_core::Layout,
    style: &nectar_core::Style,
) -> LaidOut {
    lay_out_with(engine, document, layout, style, &|| false)
}

/// [`lay_out`], abandonné dès que `stop` répond oui (consulté avant chaque
/// recomposition) : l'atelier n'achève pas un calcul déjà périmé par une
/// nouvelle retouche.
pub fn lay_out_with(
    engine: &Engine,
    document: &nectar_core::Document,
    layout: &nectar_core::Layout,
    style: &nectar_core::Style,
    stop: &dyn Fn() -> bool,
) -> LaidOut {
    use nectar_core::assistant::FixAction;
    use nectar_core::layout::{PageChange, Placement};
    let ops = layout.resolve(document).ops;
    let rules = &style.pagination;
    let mut tuning = nectar_core::Tuning::default();
    if rules.auto_landscape {
        for (table, paper) in nectar_core::auto::wide_tables(document, &ops, &layout.page, style) {
            if let Some(paper) = paper {
                tuning.paper.insert(table.clone(), paper);
            }
            tuning.landscape.insert(table);
        }
    }
    let started = std::time::Instant::now();
    let trace = std::env::var_os("NECTAR_TRACE").is_some();
    let passes = std::cell::Cell::new(1usize);
    let stopped = std::cell::Cell::new(false);
    let mut generated = nectar_core::generate_tuned(document, layout, style, &tuning);
    let mut compiled = engine.compile(&generated);
    // Recompose avec de nouveaux réglages ; garde l'ancienne mise en page si
    // la nouvelle échoue (et rend alors `false`).
    let retry =
        |tuning: &nectar_core::Tuning, generated: &mut Generated, compiled: &mut Result<Compiled, EngineError>| {
            // Abandon : plus aucune recomposition, les étapes restantes passent.
            if stopped.get() || stop() {
                stopped.set(true);
                return false;
            }
            passes.set(passes.get() + 1);
            let attempt = nectar_core::generate_tuned(document, layout, style, tuning);
            match engine.compile(&attempt) {
                Ok(better) => {
                    *compiled = Ok(better);
                    *generated = attempt;
                    true
                }
                Err(_) => false,
            }
        };
    let fixes = |compiled: &Compiled, generated: &Generated, tuning: &nectar_core::Tuning| {
        inspect_tuned(compiled, document, layout, style, generated, &[], tuning)
            .into_iter()
            .flat_map(|issue| issue.fixes.into_iter().map(move |fix| (issue.page, fix)))
            .collect::<Vec<_>>()
    };
    let free = |id: &BlockId| nectar_core::auto::allowed(ops.get(id));

    // 0. Pages au format propre : on calcule d'abord où le format revient,
    // pour que les étapes suivantes voient chaque page à son vrai format
    // (vérifié de nouveau en fin de calcul, étape 6).
    let owners: Vec<&BlockId> = ops
        .iter()
        .filter(|(_, o)| matches!(o.page, Some(PageChange::Set(_))) && !o.page_onward && !o.hidden)
        .map(|(id, _)| id)
        .collect();
    let margin_bottom = f64::from(style.page.margin_bottom_mm) * 72.0 / 25.4;
    for _ in 0..3 {
        if owners.is_empty() {
            break;
        }
        let Ok(current) = &compiled else { break };
        let positions = current.block_positions();
        let boxes = current.block_boxes(margin_bottom);
        let wanted: HashSet<BlockId> =
            owners.iter().filter_map(|owner| return_point(&positions, &boxes, document, &ops, owner)).collect();
        if wanted == tuning.returns {
            break;
        }
        let previous = std::mem::replace(&mut tuning.returns, wanted);
        if !retry(&tuning, &mut generated, &mut compiled) {
            tuning.returns = previous;
            break;
        }
    }
    if trace {
        eprintln!("[{:>5} ms, {} compositions] 1. Schémas larges", started.elapsed().as_millis(), passes.get());
    }
    // 1. Schémas larges : en paysage d'office.
    if rules.auto_landscape
        && let Ok(current) = &compiled
    {
        let wide: Vec<BlockId> = fixes(current, &generated, &tuning)
            .into_iter()
            .filter(|(_, fix)| fix.action == FixAction::ImagePlacement(Placement::Landscape) && free(&fix.block))
            .map(|(_, fix)| fix.block)
            .collect();
        if !wide.is_empty() {
            let before = tuning.landscape.clone();
            tuning.landscape.extend(wide);
            if !retry(&tuning, &mut generated, &mut compiled) {
                tuning.landscape = before;
            }
        }
    }

    if trace {
        eprintln!(
            "[{:>5} ms, {} compositions] 1 bis. Tableau qui dépasse réellement la marge",
            started.elapsed().as_millis(),
            passes.get()
        );
    }
    // 1 bis. Tableau qui dépasse réellement la marge (police plus large que
    // prévu, adresses collées) : resserré, puis mis en paysage s'il dépasse
    // encore.
    for _ in 0..4 {
        let Ok(current) = &compiled else { break };
        let wide = overflowing_tables(current, document, style, &ops);
        // Par ordre : resserré ; puis colonnes réparties par Typst ; puis
        // page paysage.
        let fresh: Vec<BlockId> = wide.iter().filter(|id| !tuning.squeeze.contains(*id)).cloned().collect();
        let squeezed: Vec<BlockId> =
            wide.iter().filter(|id| tuning.squeeze.contains(*id) && !tuning.fluid.contains(*id)).cloned().collect();
        let stubborn: Vec<BlockId> = wide
            .iter()
            .filter(|id| tuning.fluid.contains(*id) && !tuning.landscape.contains(*id) && rules.auto_landscape)
            .cloned()
            .collect();
        if fresh.is_empty() && squeezed.is_empty() && stubborn.is_empty() {
            break;
        }
        let previous = tuning.clone();
        tuning.squeeze.extend(fresh);
        tuning.fluid.extend(squeezed);
        tuning.landscape.extend(stubborn);
        if !retry(&tuning, &mut generated, &mut compiled) {
            tuning = previous;
            break;
        }
    }

    if trace {
        eprintln!(
            "[{:>5} ms, {} compositions] 1 quater. Place pour le bloc suivant",
            started.elapsed().as_millis(),
            passes.get()
        );
    }
    // 1 quater. Un bloc (code, tableau, image…) qui ne tient pas en bas d'une
    // page contenant une image : l'image est un peu réduite pour lui faire
    // place, plutôt que de couper le bloc ou de laisser un blanc.
    if rules.fit_images {
        let mut tried: HashSet<BlockId> = HashSet::new();
        for _ in 0..6 {
            let Ok(current) = &compiled else { break };
            let Some((image, height, ratio, page, target)) =
                room_for_next(current, document, style, &ops, &tuning, &tried)
            else {
                break;
            };
            tried.insert(image.clone());
            let previous = tuning.clone();
            tuning.fit.insert(image.clone(), height as f32);
            tuning.fit_percent.insert(image.clone(), (ratio * 100.0).round() as u8);
            tuning.room_made.insert(image.clone());
            let moved = retry(&tuning, &mut generated, &mut compiled)
                && compiled
                    .as_ref()
                    .is_ok_and(|c| c.block_positions().iter().any(|p| p.id == target && p.page == page));
            if !moved {
                tuning = previous;
                retry(&tuning, &mut generated, &mut compiled);
            }
        }
    }

    if trace {
        eprintln!(
            "[{:>5} ms, {} compositions] 2. Blocs insécables qui laissent un trou",
            started.elapsed().as_millis(),
            passes.get()
        );
    }
    // 2. Blocs insécables qui laissent un trou : coupés.
    for _ in 0..3 {
        let Ok(current) = &compiled else { break };
        let holes: Vec<BlockId> = fixes(current, &generated, &tuning)
            .into_iter()
            .map(|(_, fix)| fix)
            .filter(|fix| fix.action == FixAction::KeepTogether(false))
            .map(|fix| fix.block)
            .filter(|id| {
                ops.get(id).is_none_or(|o| o.keep_together.is_none() && !o.manual) && !tuning.relaxed.contains(id)
            })
            .collect();
        if holes.is_empty() {
            break;
        }
        tuning.relaxed.extend(holes);
        if !retry(&tuning, &mut generated, &mut compiled) {
            break;
        }
    }

    if trace {
        eprintln!(
            "[{:>5} ms, {} compositions] 2 bis. Grand tableau sur plusieurs pages",
            started.elapsed().as_millis(),
            passes.get()
        );
    }
    // 2 bis. Grand tableau sur plusieurs pages (une fois les petits blocs
    // autorisés à se couper) : on essaie la page paysage,
    // puis une page A3 paysage, et on garde la première où il tient en
    // entier (à quelques lignes près, que le resserrage rattrape).
    if rules.auto_landscape {
        let margin = f64::from(style.page.margin_bottom_mm) * 72.0 / 25.4;
        let parts = |c: &Compiled, id: &BlockId| -> Vec<BlockBox> {
            c.block_boxes(margin).into_iter().filter(|b| &b.id == id).collect()
        };
        let one_page = |c: &Result<Compiled, EngineError>, id: &BlockId| {
            let Ok(c) = c else { return false };
            let parts = parts(c, id);
            match parts.as_slice() {
                [_] => true,
                [_, last] => {
                    let body = c.page_size(last.page).map(|(_, h)| h).unwrap_or(842.0) - 2.0 * margin;
                    last.rect[3] - last.rect[1] < body * 0.2
                }
                _ => false,
            }
        };
        let mut tried: HashSet<BlockId> = HashSet::new();
        for _ in 0..6 {
            let Ok(current) = &compiled else { break };
            let candidate = document.blocks.iter().find(|b| {
                matches!(&b.node, nectar_core::model::Node::Table(t) if nectar_core::auto::table_wraps(t, &layout.page, style))
                    && free(&b.id)
                    && ops.get(&b.id).is_none_or(|o| o.table.is_none())
                    && !tried.contains(&b.id)
                    && parts(current, &b.id).len() >= 2
                    && !one_page(&compiled, &b.id)
            });
            let Some(table) = candidate.map(|b| b.id.clone()) else { break };
            tried.insert(table.clone());
            let count =
                |c: &Result<Compiled, EngineError>| c.as_ref().map(|c| parts(c, &table).len()).unwrap_or(usize::MAX);
            let before_count = count(&compiled);
            let before = tuning.clone();
            // Déjà en paysage d'office (sur estimation) : une page A3 paysage
            // s'il y tient en entier ; sinon on vérifie qu'en portrait il ne
            // prendrait pas moins de pages.
            if tuning.landscape.contains(&table) && !tuning.paper.contains_key(&table) {
                if rules.larger_paper
                    && let Some(paper) = nectar_core::auto::larger_paper(&layout.page.paper)
                {
                    let mut larger = before.clone();
                    larger.paper.insert(table.clone(), paper.to_string());
                    if retry(&larger, &mut generated, &mut compiled) && one_page(&compiled, &table) {
                        tuning = larger;
                        continue;
                    }
                }
                let mut portrait = before.clone();
                portrait.landscape.remove(&table);
                if !(retry(&portrait, &mut generated, &mut compiled) && count(&compiled) < before_count) {
                    portrait = before;
                    retry(&portrait, &mut generated, &mut compiled);
                }
                tuning = portrait;
                continue;
            }
            let mut kept = false;
            // Sur une page paysage : gardée si le tableau y tient, ou s'il y
            // prend nettement moins de pages.
            let mut turned: Option<(nectar_core::Tuning, usize)> = None;
            if !tuning.landscape.contains(&table) {
                tuning.landscape.insert(table.clone());
                if retry(&tuning, &mut generated, &mut compiled) {
                    kept = one_page(&compiled, &table);
                    turned = Some((tuning.clone(), count(&compiled)));
                }
            }
            // Sur un papier plus grand : seulement s'il y tient en entier.
            if !kept
                && rules.larger_paper
                && let Some(paper) = nectar_core::auto::larger_paper(&layout.page.paper)
            {
                let mut larger = before.clone();
                larger.landscape.insert(table.clone());
                larger.paper.insert(table.clone(), paper.to_string());
                kept = retry(&larger, &mut generated, &mut compiled) && one_page(&compiled, &table);
                if kept {
                    tuning = larger;
                }
            }
            if !kept {
                match turned {
                    Some((landscape, pages)) if pages + 2 <= before_count || pages * 2 <= before_count => {
                        tuning = landscape;
                    }
                    _ => tuning = before,
                }
                retry(&tuning, &mut generated, &mut compiled);
            }
        }
    }

    if trace {
        eprintln!(
            "[{:>5} ms, {} compositions] 2 ter. Tableaux qui débordent de quelques lignes",
            started.elapsed().as_millis(),
            passes.get()
        );
    }
    // 2 ter. Tableaux qui débordent de quelques lignes : resserrés.
    if let Ok(current) = &compiled {
        let spills = spilling_tables(current, document, style, &ops);
        if !spills.is_empty() {
            let previous = tuning.clone();
            tuning.compact.extend(spills.iter().map(|(id, _)| id.clone()));
            if retry(&tuning, &mut generated, &mut compiled) {
                // Un tableau qui déborde encore ne gagne rien à être resserré.
                let pages = |c: &Compiled, id: &BlockId| {
                    c.block_boxes(f64::from(style.page.margin_bottom_mm) * 72.0 / 25.4)
                        .iter()
                        .filter(|b| &b.id == id)
                        .count()
                };
                let useless: Vec<BlockId> = match &compiled {
                    Ok(c) => {
                        spills.iter().filter(|(id, count)| pages(c, id) >= *count).map(|(id, _)| id.clone()).collect()
                    }
                    Err(_) => Vec::new(),
                };
                if !useless.is_empty() {
                    for id in &useless {
                        tuning.compact.remove(id);
                    }
                    retry(&tuning, &mut generated, &mut compiled);
                }
            } else {
                tuning = previous;
            }
        }
    }

    // 2 quater. Titres restés en bas de page avec seulement quelques lignes
    // de leur contenu : on n'hésite pas, ils passent à la page suivante.
    for _ in 0..4 {
        let Ok(current) = &compiled else { break };
        let stranded = stranded_headings(current, document, style, &ops, &tuning);
        if stranded.is_empty() {
            break;
        }
        let previous = tuning.clone();
        tuning.pushed.extend(stranded);
        if !retry(&tuning, &mut generated, &mut compiled) {
            tuning = previous;
            break;
        }
    }

    if trace {
        eprintln!(
            "[{:>5} ms, {} compositions] 3. Images un peu trop hautes",
            started.elapsed().as_millis(),
            passes.get()
        );
    }
    // 3. Images un peu trop hautes : réduites juste assez.
    if rules.fit_images {
        let mut tried: HashSet<BlockId> = HashSet::new();
        for _ in 0..16 {
            let Ok(current) = &compiled else { break };
            let candidate = fixes(current, &generated, &tuning).into_iter().find_map(|(page, fix)| {
                let page = page?;
                (matches!(fix.action, FixAction::ImageWidth(_))
                    && free(&fix.block)
                    && !tuning.is_landscape(ops.get(&fix.block), &fix.block)
                    && !tried.contains(&fix.block))
                .then_some((page, fix.block))
            });
            let Some((page, figure)) = candidate else { break };
            tried.insert(figure.clone());
            let Some((height, ratio)) = fit_height(current, style, page, &figure) else { continue };
            let previous = tuning.clone();
            tuning.fit.insert(figure.clone(), height as f32);
            tuning.fit_percent.insert(figure.clone(), (ratio * 100.0).round() as u8);
            if !retry(&tuning, &mut generated, &mut compiled) {
                tuning = previous;
                continue;
            }
            // L'estimation a pu être un peu juste : l'image doit être remontée.
            let landed = compiled.as_ref().ok().and_then(|c| c.block_positions().into_iter().find(|p| p.id == figure));
            if landed.is_none_or(|p| p.page != page) {
                tuning = previous;
                retry(&tuning, &mut generated, &mut compiled);
            }
        }
    }

    if trace {
        eprintln!(
            "[{:>5} ms, {} compositions] 3 bis. Légende rejetée seule en haut de la page suivante",
            started.elapsed().as_millis(),
            passes.get()
        );
    }
    // 3 bis. Légende rejetée seule en haut de la page suivante (image trop
    // haute avec ses titres) : l'image est réduite juste assez pour qu'elle
    // tienne avec elle.
    if rules.fit_images {
        for _ in 0..4 {
            let Ok(current) = &compiled else { break };
            let Some((figure, height, ratio)) = orphan_caption(current, document, style, &ops, &tuning) else { break };
            let previous = tuning.clone();
            tuning.fit.insert(figure.clone(), height as f32);
            tuning.fit_percent.insert(figure.clone(), (ratio * 100.0).round() as u8);
            if !retry(&tuning, &mut generated, &mut compiled) {
                tuning = previous;
                break;
            }
        }
    }

    if trace {
        eprintln!(
            "[{:>5} ms, {} compositions] 3 ter. Image horizontale seule sur sa page",
            started.elapsed().as_millis(),
            passes.get()
        );
    }
    // 3 ter. Image horizontale seule sur sa page (avec son titre et sa
    // légende) : la page passe en paysage, l'image grandit et le vide
    // disparaît. Jamais si cela ajoute une page.
    if rules.lonely_landscape
        && let Ok(current) = &compiled
    {
        let lonely = lonely_figures(current, document, &ops, style, &tuning);
        let count = current.page_count();
        let fits = |c: &Result<Compiled, EngineError>| c.as_ref().is_ok_and(|c| c.page_count() <= count);
        if !lonely.is_empty() {
            let previous = tuning.clone();
            for (figure, room, until) in &lonely {
                tuning.landscape.insert(figure.clone());
                tuning.reserve.insert(figure.clone(), *room);
                if let Some(until) = until {
                    tuning.until.insert(figure.clone(), until.clone());
                }
            }
            if !(retry(&tuning, &mut generated, &mut compiled) && fits(&compiled)) {
                // Ensemble, ça déborde : une par une, en gardant celles qui passent.
                tuning = previous;
                retry(&tuning, &mut generated, &mut compiled);
                for (figure, room, until) in lonely.iter().take(6) {
                    let before = tuning.clone();
                    tuning.landscape.insert(figure.clone());
                    tuning.reserve.insert(figure.clone(), *room);
                    if let Some(until) = until {
                        tuning.until.insert(figure.clone(), until.clone());
                    }
                    if !(retry(&tuning, &mut generated, &mut compiled) && fits(&compiled)) {
                        tuning = before;
                        retry(&tuning, &mut generated, &mut compiled);
                    }
                }
            }
        }
    }

    if trace {
        eprintln!(
            "[{:>5} ms, {} compositions] 3 quater. Images agrandies",
            started.elapsed().as_millis(),
            passes.get()
        );
    }
    // 3 quater. Un blanc en bas de page que rien ne peut remplir : une image
    // de la page grandit pour le combler (si sa résolution le permet), sans
    // rien changer à la pagination.
    if rules.fit_images {
        let mut tried: HashSet<BlockId> = HashSet::new();
        for _ in 0..4 {
            let Ok(current) = &compiled else { break };
            let Some((image, height, percent, page)) = grow_candidate(current, document, style, &ops, &tuning, &tried)
            else {
                break;
            };
            tried.insert(image.clone());
            let before_count = current.page_count();
            let margin = f64::from(style.page.margin_bottom_mm) * 72.0 / 25.4;
            let on_page = |c: &Compiled| -> Vec<BlockId> {
                let mut ids: Vec<BlockId> = c
                    .block_boxes(margin)
                    .into_iter()
                    .filter(|b| b.page == page || b.page == page + 1)
                    .map(|b| b.id)
                    .collect();
                ids.dedup();
                ids
            };
            let layout_before = on_page(current);
            let previous = tuning.clone();
            tuning.grow.insert(image.clone(), (height as f32, percent));
            let same = retry(&tuning, &mut generated, &mut compiled)
                && compiled.as_ref().is_ok_and(|c| c.page_count() == before_count && on_page(c) == layout_before);
            if !same {
                tuning = previous;
                retry(&tuning, &mut generated, &mut compiled);
            }
        }
    }

    if trace {
        eprintln!(
            "[{:>5} ms, {} compositions] 4. Dernière page de quelques lignes",
            started.elapsed().as_millis(),
            passes.get()
        );
    }
    // 4. Dernière page de quelques lignes : espacements resserrés.
    if rules.avoid_short_last_page
        && let Ok(current) = &compiled
        && short_last_page(current, document, &ops, style)
    {
        let count = current.page_count();
        for factor in [0.8, 0.6] {
            let mut attempt = tuning.clone();
            attempt.tighten = Some(factor);
            let mut g = generated.clone();
            let mut c: Result<Compiled, EngineError> = Err(EngineError::NoSuchPage(0));
            if retry(&attempt, &mut g, &mut c) && c.as_ref().is_ok_and(|c| c.page_count() < count) {
                tuning = attempt;
                generated = g;
                compiled = c;
                break;
            }
        }
    }

    if trace {
        eprintln!("[{:>5} ms, {} compositions] 5. Pages paysage", started.elapsed().as_millis(), passes.get());
    }
    // 5. Pages paysage : si la page d'avant reste à moitié vide, le texte qui
    // suit vient la remplir et la page paysage arrive juste après. De la
    // dernière à la première : une image plus loin, déjà repoussée, laisse
    // son texte disponible pour les précédentes. Une image qui suit son titre
    // ou la phrase qui l'annonce reste avec eux.
    let landscapes: Vec<BlockId> = document
        .blocks
        .iter()
        .enumerate()
        .filter(|(_, b)| {
            matches!(b.node, nectar_core::model::Node::Figure(_) | nectar_core::model::Node::Diagram { .. })
        })
        .filter(|(_, b)| tuning.is_landscape(ops.get(&b.id), &b.id))
        .filter(|(i, b)| *i == 0 || !nectar_core::codegen::leads_into(&document.blocks[i - 1], b))
        .filter(|(i, _)| document.blocks.get(i + 1).is_none_or(|n| !nectar_core::codegen::is_caption(&n.node)))
        .filter(|(_, b)| !tuning.until.contains_key(&b.id))
        .map(|(_, b)| b.id.clone())
        .collect();
    for figure in landscapes.iter().rev().take(10) {
        let Ok(current) = &compiled else { break };
        let moved: Vec<&BlockId> = tuning.deferred.iter().map(|(f, _)| f).collect();
        let Some(after) = defer_point(current, document, style, figure, &moved) else { continue };
        tuning.deferred.push((figure.clone(), after));
        if !retry(&tuning, &mut generated, &mut compiled) {
            tuning.deferred.pop();
        }
    }

    if trace {
        eprintln!("[{:>5} ms, {} compositions] 6. Pages au format propre", started.elapsed().as_millis(), passes.get());
    }
    // 6. Pages au format propre (« cette page seulement ») : le format revient
    // devant le premier bloc qui commence après elles.
    //
    // Une page au format propre commence toujours à son bloc (le changement de
    // format ouvre une page) : ce qu'elle contient ne dépend pas des retours
    // placés plus haut. Tous les retours se calculent donc sur une même mise
    // en page ; un second tour vérifie que rien n'a bougé.
    for _ in 0..3 {
        if owners.is_empty() {
            break;
        }
        let Ok(current) = &compiled else { break };
        let positions = current.block_positions();
        let boxes = current.block_boxes(margin_bottom);
        let wanted: HashSet<BlockId> =
            owners.iter().filter_map(|owner| return_point(&positions, &boxes, document, &ops, owner)).collect();
        if wanted == tuning.returns {
            break;
        }
        let previous = std::mem::replace(&mut tuning.returns, wanted);
        if !retry(&tuning, &mut generated, &mut compiled) {
            tuning.returns = previous;
            break;
        }
    }
    if trace {
        eprintln!("[{:>5} ms, {} compositions] 7. Dernier contrôle", started.elapsed().as_millis(), passes.get());
    }
    // 7. Dernier contrôle : aucun titre seul en bas de page, aucune légende
    // seule en haut de page (les étapes précédentes ont pu faire bouger le
    // texte).
    for _ in 0..2 {
        let Ok(current) = &compiled else { break };
        let stranded = stranded_headings(current, document, style, &ops, &tuning);
        if stranded.is_empty() {
            break;
        }
        let previous = tuning.clone();
        tuning.pushed.extend(stranded);
        if !retry(&tuning, &mut generated, &mut compiled) {
            tuning = previous;
            retry(&tuning, &mut generated, &mut compiled);
            break;
        }
    }
    if rules.fit_images {
        for _ in 0..3 {
            let Ok(current) = &compiled else { break };
            let Some((figure, height, ratio)) = orphan_caption(current, document, style, &ops, &tuning) else { break };
            let previous = tuning.clone();
            tuning.fit.insert(figure.clone(), height as f32);
            tuning.fit_percent.insert(figure.clone(), (ratio * 100.0).round() as u8);
            if !retry(&tuning, &mut generated, &mut compiled) {
                tuning = previous;
                retry(&tuning, &mut generated, &mut compiled);
                break;
            }
        }
    }
    let mut relaxed: Vec<BlockId> = tuning.relaxed.iter().cloned().collect();
    relaxed.sort();
    let choices = tuning.choices(document);
    if trace {
        eprintln!("[{:>5} ms, {} compositions] fin", started.elapsed().as_millis(), passes.get());
    }
    LaidOut { generated, compiled, relaxed, tuning, choices, passes: passes.get(), stopped: stopped.get() }
}

/// Hauteur à donner à l'image `figure`, rejetée en tête de la page
/// `page + 1`, pour qu'elle tienne au bas de la page `page` avec ce qui la
/// précède en tête de page (titre, phrase d'annonce) et sa légende ; et le
/// rapport de réduction. `None` si elle devrait trop rétrécir.
fn fit_height(compiled: &Compiled, style: &nectar_core::Style, page: usize, figure: &BlockId) -> Option<(f64, f64)> {
    let mm = 72.0 / 25.4;
    let (top, bottom) = (f64::from(style.page.margin_top_mm) * mm, f64::from(style.page.margin_bottom_mm) * mm);
    let metrics = compiled.page_metrics(bottom);
    let here = metrics.get(page)?;
    let content_bottom = here.content.map(|c| c[3]).unwrap_or(top);
    let gap = f64::from(style.text.size_pt) * f64::from(1.0 + style.text.paragraph_spacing_em) + 6.0;
    let room = here.height - bottom - content_bottom - gap;
    let image = compiled.figures().into_iter().find(|f| &f.id == figure && f.page == page + 1)?;
    let stack_bottom =
        compiled.block_boxes(bottom).into_iter().find(|b| &b.id == figure && b.page == page + 1).map(|b| b.rect[3])?;
    let stack = stack_bottom - top;
    let height = image.height - (stack - room) - 4.0;
    let ratio = height / image.height;
    ((0.55..1.0).contains(&ratio) && height >= 90.0).then_some((height, ratio))
}

/// Titres (ou lignes en gras qui en tiennent lieu) restés en bas d'une page
/// avec moins de quatre lignes de leur contenu, alors que la section continue
/// page suivante : le premier titre de chaque série.
fn stranded_headings(
    compiled: &Compiled,
    document: &nectar_core::Document,
    style: &nectar_core::Style,
    ops: &HashMap<BlockId, nectar_core::BlockOps>,
    tuning: &nectar_core::Tuning,
) -> Vec<BlockId> {
    use nectar_core::codegen::is_pseudo_heading;
    use nectar_core::model::Node;
    let bottom = f64::from(style.page.margin_bottom_mm) * 72.0 / 25.4;
    let positions = compiled.block_positions();
    let boxes = compiled.block_boxes(bottom);
    let metrics = compiled.page_metrics(bottom);
    let line = f64::from(style.text.size_pt * style.text.line_height);
    let blocks = &document.blocks;
    let is_title = |b: &nectar_core::Block| matches!(b.node, Node::Heading { .. }) || is_pseudo_heading(&b.node);
    let page_of = |id: &BlockId| positions.iter().find(|p| &p.id == id).map(|p| p.page);
    let mut out = Vec::new();
    let mut first = 0;
    while first < blocks.len() {
        if !is_title(&blocks[first]) {
            first += 1;
            continue;
        }
        // La série de titres blocks[first..=last].
        let mut last = first;
        while last + 1 < blocks.len() && is_title(&blocks[last + 1]) {
            last += 1;
        }
        let titles = &blocks[first..=last];
        let head = &blocks[first];
        first = last + 1;
        let Some(page) = page_of(&head.id) else { continue };
        if page + 1 >= compiled.page_count() || tuning.pushed.contains(&head.id) {
            continue;
        }
        let first_on_page = positions.iter().find(|p| p.page == page).map(|p| &p.id) == Some(&head.id);
        let movable = ops.get(&head.id).is_none_or(|o| !o.manual && !o.hidden && o.page.is_none() && !o.break_before);
        if first_on_page || !movable {
            continue;
        }
        // Ce qui suit les titres sur leur page, et la suite page suivante.
        let titles_bottom = boxes
            .iter()
            .filter(|b| b.page == page && titles.iter().any(|t| t.id == b.id))
            .map(|b| b.rect[3])
            .fold(0.0, f64::max);
        let Some(content_bottom) = metrics.get(page).and_then(|m| m.content).map(|c| c[3]) else { continue };
        let Some(next) = blocks.get(last + 1) else { continue };
        let continues =
            boxes.iter().any(|b| b.id == next.id && b.page > page) || page_of(&next.id).is_some_and(|p| p > page);
        if continues && titles_bottom > 0.0 && content_bottom - titles_bottom < 4.0 * line {
            out.push(head.id.clone());
        }
    }
    out
}

/// Tableaux (que le placement automatique peut toucher) qui dépassent la
/// marge de droite.
fn overflowing_tables(
    compiled: &Compiled,
    document: &nectar_core::Document,
    style: &nectar_core::Style,
    ops: &HashMap<BlockId, nectar_core::BlockOps>,
) -> Vec<BlockId> {
    let mm = 72.0 / 25.4;
    let bottom = f64::from(style.page.margin_bottom_mm) * mm;
    let right = f64::from(style.page.margin_right_mm) * mm;
    let boxes = compiled.block_boxes(bottom);
    document
        .blocks
        .iter()
        .filter(|b| matches!(b.node, nectar_core::model::Node::Table(_)) && nectar_core::auto::allowed(ops.get(&b.id)))
        .filter(|b| {
            boxes.iter().any(|x| {
                x.id == b.id && compiled.page_size(x.page).is_some_and(|(width, _)| x.rect[2] > width - right + 2.0)
            })
        })
        .map(|b| b.id.clone())
        .collect()
}

/// Une image dont la note fixe la taille (`![[image.png|400]]`) : un choix
/// de la personne, que le placement automatique ne change pas.
fn sized_by_hand(document: &nectar_core::Document, id: &BlockId) -> bool {
    document.blocks.iter().any(|b| {
        &b.id == id
            && matches!(&b.node, nectar_core::model::Node::Figure(image) if image.width_px.is_some() || image.height_px.is_some())
    })
}

/// Une page qui finit sur un blanc parce que le bloc suivant (avec son titre
/// et sa phrase d'annonce) n'y tenait pas, alors qu'elle contient une image
/// qu'un peu de réduction suffirait à rendre assez petite : (image, hauteur
/// visée, rapport, page, bloc qui doit remonter).
fn room_for_next(
    compiled: &Compiled,
    document: &nectar_core::Document,
    style: &nectar_core::Style,
    ops: &HashMap<BlockId, nectar_core::BlockOps>,
    tuning: &nectar_core::Tuning,
    tried: &HashSet<BlockId>,
) -> Option<(BlockId, f64, f64, usize, BlockId)> {
    use nectar_core::codegen::leads_into;
    let mm = 72.0 / 25.4;
    let (top, bottom) = (f64::from(style.page.margin_top_mm) * mm, f64::from(style.page.margin_bottom_mm) * mm);
    let positions = compiled.block_positions();
    let boxes = compiled.block_boxes(bottom);
    let metrics = compiled.page_metrics(bottom);
    let figures = compiled.figures();
    let gap = f64::from(style.text.size_pt) * f64::from(1.0 + style.text.paragraph_spacing_em) + 8.0;
    for page in 0..compiled.page_count().saturating_sub(1) {
        let here = metrics.get(page)?;
        if here.width > here.height {
            continue;
        }
        let content_bottom = here.content.map(|c| c[3]).unwrap_or(top);
        let room = here.height - bottom - content_bottom - gap;
        if room < 30.0 {
            continue;
        }
        // Le premier bloc de la page suivante ne doit pas être la suite d'un
        // bloc commencé ici.
        let Some(first) = positions.iter().find(|p| p.page == page + 1) else { continue };
        if boxes.iter().any(|b| b.page == page + 1 && b.rect[1] < first.y - 2.0) {
            continue;
        }
        let start = document.blocks.iter().position(|b| b.id == first.id)?;
        // Ce qui doit remonter ensemble : titres et phrase d'annonce, puis le bloc.
        let mut target = start;
        while target + 1 < document.blocks.len() && leads_into(&document.blocks[target], &document.blocks[target + 1]) {
            target += 1;
        }
        let target_id = &document.blocks[target].id;
        let parts: Vec<&BlockBox> = boxes.iter().filter(|b| &b.id == target_id).collect();
        if parts.len() != 1 || parts[0].page != page + 1 {
            continue;
        }
        let needed = parts[0].rect[3] - first.y + 4.0;
        let deficit = needed - room;
        if deficit <= 0.0 {
            continue;
        }
        // La plus grande image de la page, si la réduire un peu suffit.
        let candidate = figures
            .iter()
            .filter(|f| {
                f.page == page
                    && nectar_core::auto::allowed(ops.get(&f.id))
                    && !tuning.is_landscape(ops.get(&f.id), &f.id)
                    && !tuning.fit.contains_key(&f.id)
                    && !tuning.grow.contains_key(&f.id)
                    && !tried.contains(&f.id)
                    && !sized_by_hand(document, &f.id)
                    && figures.iter().filter(|g| g.id == f.id).count() == 1
            })
            .max_by(|a, b| a.height.total_cmp(&b.height))?;
        let height = candidate.height - deficit - 2.0;
        let ratio = height / candidate.height;
        if ratio >= 0.75 && height >= 100.0 {
            return Some((candidate.id.clone(), height, ratio, page, target_id.clone()));
        }
    }
    None
}

/// Une image d'une page qui finit sur un blanc, qui peut grandir pour le
/// combler (plus étroite que le texte, assez de pixels) : (image, hauteur
/// visée, pourcentage, page).
fn grow_candidate(
    compiled: &Compiled,
    document: &nectar_core::Document,
    style: &nectar_core::Style,
    ops: &HashMap<BlockId, nectar_core::BlockOps>,
    tuning: &nectar_core::Tuning,
    tried: &HashSet<BlockId>,
) -> Option<(BlockId, f64, u16, usize)> {
    let mm = 72.0 / 25.4;
    let p = &style.page;
    let (top, bottom) = (f64::from(p.margin_top_mm) * mm, f64::from(p.margin_bottom_mm) * mm);
    let (left, right) = (f64::from(p.margin_left_mm) * mm, f64::from(p.margin_right_mm) * mm);
    let metrics = compiled.page_metrics(bottom);
    let figures = compiled.figures();
    for page in 0..compiled.page_count().saturating_sub(1) {
        let here = metrics.get(page)?;
        let body = here.height - top - bottom;
        let content_bottom = here.content.map(|c| c[3]).unwrap_or(top);
        let room = here.height - bottom - content_bottom - 10.0;
        if room < body * 0.12 {
            continue;
        }
        let text_width = here.width - left - right;
        for f in figures.iter().filter(|f| {
            f.page == page
                && f.height > 0.0
                && nectar_core::auto::allowed(ops.get(&f.id))
                && !tuning.is_landscape(ops.get(&f.id), &f.id)
                && !tuning.fit.contains_key(&f.id)
                && !tuning.grow.contains_key(&f.id)
                && !tried.contains(&f.id)
                && !sized_by_hand(document, &f.id)
                && figures.iter().filter(|g| g.id == f.id).count() == 1
        }) {
            let ratio = f.height / f.width;
            let target = (f.height + room - 6.0).min(text_width * ratio).min(body * 0.8);
            if target < f.height * 1.1 || target - f.height < 30.0 {
                continue;
            }
            // Assez de pixels pour l'agrandir (110 ppp au moins).
            if f.pixels.is_some_and(|(px, _)| px / (target / ratio / 72.0) < 110.0) {
                continue;
            }
            return Some((f.id.clone(), target, (target / f.height * 100.0).round() as u16, page));
        }
    }
    None
}

/// Une image dont la légende est passée seule à la page suivante : son id,
/// la hauteur qui laisse la place à la légende, et le rapport de réduction.
fn orphan_caption(
    compiled: &Compiled,
    document: &nectar_core::Document,
    style: &nectar_core::Style,
    ops: &HashMap<BlockId, nectar_core::BlockOps>,
    tuning: &nectar_core::Tuning,
) -> Option<(BlockId, f64, f64)> {
    use nectar_core::model::Node;
    let bottom = f64::from(style.page.margin_bottom_mm) * 72.0 / 25.4;
    let positions = compiled.block_positions();
    let boxes = compiled.block_boxes(bottom);
    let figures = compiled.figures();
    for (index, block) in document.blocks.iter().enumerate() {
        let Some(next) = document.blocks.get(index + 1) else { continue };
        if !matches!(block.node, Node::Figure(_) | Node::Diagram { .. })
            || !nectar_core::codegen::is_caption(&next.node)
            || !nectar_core::auto::allowed(ops.get(&block.id))
            || !nectar_core::codegen::follows_freely(ops.get(&next.id))
            || tuning.is_landscape(ops.get(&block.id), &block.id)
        {
            continue;
        }
        let Some(view) = figures.iter().find(|f| f.id == block.id) else { continue };
        let Some(caption) = positions.iter().find(|p| p.id == next.id) else { continue };
        if caption.page <= view.page {
            continue;
        }
        let caption_height =
            boxes.iter().filter(|b| b.id == next.id).map(|b| b.rect[3] - b.rect[1]).sum::<f64>().max(14.0);
        let current = f64::from(tuning.fit.get(&block.id).copied().unwrap_or(view.height as f32)).min(view.height);
        let height = current - caption_height - 18.0;
        let original = view.height / f64::from(tuning.fit_percent.get(&block.id).copied().unwrap_or(100)) * 100.0;
        let ratio = height / original;
        if ratio >= 0.55 && height >= 90.0 {
            return Some((block.id.clone(), height, ratio));
        }
    }
    None
}

/// Images horizontales en tête de leur page portrait (avec leur titre, leur
/// phrase d'annonce et leurs légendes), seules sur la page ou suivies de
/// quelques blocs qui y tiennent avant un blanc : sur une page paysage,
/// l'image serait nettement plus grande et le blanc disparaîtrait. Rend
/// (image, place à garder pour ce qui l'accompagne, dernier bloc emporté).
fn lonely_figures(
    compiled: &Compiled,
    document: &nectar_core::Document,
    ops: &HashMap<BlockId, nectar_core::BlockOps>,
    style: &nectar_core::Style,
    tuning: &nectar_core::Tuning,
) -> Vec<(BlockId, f32, Option<BlockId>)> {
    use nectar_core::codegen::{is_caption, leads_into};
    use nectar_core::model::Node;
    let mm = 72.0 / 25.4;
    let p = &style.page;
    let (top, bottom) = (f64::from(p.margin_top_mm) * mm, f64::from(p.margin_bottom_mm) * mm);
    let (left, right) = (f64::from(p.margin_left_mm) * mm, f64::from(p.margin_right_mm) * mm);
    let positions = compiled.block_positions();
    let boxes = compiled.block_boxes(bottom);
    let figures = compiled.figures();
    let metrics = compiled.page_metrics(bottom);
    let blocks = &document.blocks;
    let mut out = Vec::new();
    for (index, block) in blocks.iter().enumerate() {
        let id = &block.id;
        if !matches!(block.node, Node::Figure(_) | Node::Diagram { .. })
            || !nectar_core::auto::allowed(ops.get(id))
            || tuning.is_landscape(ops.get(id), id)
            || tuning.fit.contains_key(id)
        {
            continue;
        }
        let views: Vec<_> = figures.iter().filter(|f| &f.id == id).collect();
        let [view] = views.as_slice() else { continue };
        let Some((width, height)) = compiled.page_size(view.page) else { continue };
        if width >= height || view.height <= 0.0 || view.width / view.height < 1.3 {
            continue;
        }
        // Ce qui l'accompagne : titres et phrase d'annonce avant, légendes après.
        let mut start = index;
        let separated = ops.get(id).is_some_and(|o| o.break_before);
        while start > 0
            && !separated
            && leads_into(&blocks[start - 1], &blocks[start])
            && nectar_core::codegen::leads_freely(ops.get(&blocks[start - 1].id))
        {
            start -= 1;
        }
        let mut end = index;
        while blocks
            .get(end + 1)
            .is_some_and(|n| is_caption(&n.node) && nectar_core::codegen::follows_freely(ops.get(&n.id)))
        {
            end += 1;
        }
        // L'image ouvre sa page (avec son titre).
        if positions.iter().find(|p| p.page == view.page).map(|p| &p.id) != Some(&blocks[start].id)
            || positions.iter().any(|p| p.page != view.page && blocks[start..=end].iter().any(|b| b.id == p.id))
        {
            continue;
        }
        // Les blocs qui la suivent sur la page doivent y tenir en entier, sans
        // retouche de page ni décision automatique à eux.
        let mut last = end;
        let mut tail_ok = true;
        while let Some(next) = blocks.get(last + 1)
            && positions.iter().any(|p| p.id == next.id && p.page == view.page)
        {
            let whole = !boxes.iter().any(|b| b.id == next.id && b.page != view.page);
            let plain = ops.get(&next.id).is_none_or(|o| o.page.is_none() && !o.break_after && !o.hidden)
                && !tuning.is_landscape(ops.get(&next.id), &next.id);
            if !whole || !plain || matches!(next.node, Node::Figure(_) | Node::Diagram { .. } | Node::Table(_)) {
                tail_ok = false;
                break;
            }
            last += 1;
        }
        let group: HashSet<&BlockId> = blocks[start..=last].iter().map(|b| &b.id).collect();
        // Rien d'autre sur la page (ni bloc qui commence, ni fin d'un bloc
        // d'avant ; l'en-tête courant, dans la marge du haut, ne compte pas).
        if !tail_ok
            || boxes.iter().any(|b| b.page == view.page && !group.contains(&b.id) && b.rect[3] > top - 2.0)
            || positions.iter().any(|p| p.page == view.page && !group.contains(&p.id))
        {
            continue;
        }
        // Ce que prennent titres, légendes, texte et espaces : la hauteur
        // occupée par le groupe sur la page, moins l'image (le texte, plus
        // large en paysage, y prendra un peu moins de place).
        let group_bottom = boxes
            .iter()
            .filter(|b| b.page == view.page && group.contains(&b.id))
            .map(|b| b.rect[3])
            .fold(top, f64::max);
        let others = (group_bottom - top - view.height).max(0.0) + 8.0;
        let room_height = width - top - bottom - others - 14.0;
        let landscape = (height - left - right).min(room_height * view.width / view.height);
        // Une photo n'est agrandie que si elle a les pixels pour (100 ppp).
        let sharp = view.pixels.is_none_or(|(px, _)| px / (landscape / 72.0) >= 100.0);
        let fill = metrics
            .get(view.page)
            .and_then(|m| m.content)
            .map(|c| (c[3] - top) / (height - top - bottom))
            .unwrap_or(1.0);
        let gain = landscape / view.width;
        let extra = last > end;
        // Seule sur sa page : nettement plus grande, ou un peu sur une page à
        // moitié vide. Suivie de texte : il faut un blanc à effacer en bas de
        // page, et une image qui y gagne vraiment.
        let worth = if extra { fill < 0.8 && gain >= 1.1 } else { gain >= 1.2 || (gain >= 1.05 && fill < 0.65) };
        if sharp && worth {
            out.push((id.clone(), others as f32 + 6.0, extra.then(|| blocks[last].id.clone())));
        }
    }
    out
}

/// Tableaux dont la dernière page ne porte que quelques lignes : (id,
/// nombre de pages occupées).
fn spilling_tables(
    compiled: &Compiled,
    document: &nectar_core::Document,
    style: &nectar_core::Style,
    ops: &HashMap<BlockId, nectar_core::BlockOps>,
) -> Vec<(BlockId, usize)> {
    let mm = 72.0 / 25.4;
    let (top, bottom) = (f64::from(style.page.margin_top_mm) * mm, f64::from(style.page.margin_bottom_mm) * mm);
    let boxes = compiled.block_boxes(bottom);
    document
        .blocks
        .iter()
        .filter(|b| matches!(b.node, nectar_core::model::Node::Table(_)) && nectar_core::auto::allowed(ops.get(&b.id)))
        .filter_map(|b| {
            let parts: Vec<&BlockBox> = boxes.iter().filter(|x| x.id == b.id).collect();
            let last = parts.last()?;
            let height = compiled.page_size(last.page)?.1 - top - bottom;
            (parts.len() >= 2 && last.rect[3] - last.rect[1] < height * 0.2).then(|| (b.id.clone(), parts.len()))
        })
        .collect()
}

/// La dernière page n'a que quelques lignes, sans saut voulu devant elles.
fn short_last_page(
    compiled: &Compiled,
    document: &nectar_core::Document,
    ops: &HashMap<BlockId, nectar_core::BlockOps>,
    style: &nectar_core::Style,
) -> bool {
    let mm = 72.0 / 25.4;
    let (top, bottom) = (f64::from(style.page.margin_top_mm) * mm, f64::from(style.page.margin_bottom_mm) * mm);
    let metrics = compiled.page_metrics(bottom);
    let (Some(last), true) = (metrics.last(), metrics.len() > 1) else { return false };
    let Some([_, _, _, content_bottom]) = last.content else { return false };
    let fill = (content_bottom - top) / (last.height - top - bottom);
    let page = metrics.len() - 1;
    let positions = compiled.block_positions();
    let first = positions.iter().find(|p| p.page == page);
    let wanted = first.is_some_and(|p| {
        ops.get(&p.id).is_some_and(|o| o.break_before || o.page.is_some())
            || document
                .blocks
                .iter()
                .position(|b| b.id == p.id)
                .is_some_and(|i| i > 0 && ops.get(&document.blocks[i - 1].id).is_some_and(|o| o.break_after))
    });
    // La page d'avant doit être pleine : sinon c'est un saut, pas un débordement.
    let previous_full = metrics
        .get(page - 1)
        .and_then(|m| m.content)
        .is_some_and(|c| (c[3] - top) / (metrics[page - 1].height - top - bottom) > 0.7);
    fill < 0.15 && !wanted && previous_full && metrics[page - 1].width == last.width
}

/// Le bloc de premier niveau devant lequel revenir au format courant, après
/// la (ou les) page(s) du bloc `owner` au format propre.
fn return_point(
    positions: &[BlockPosition],
    boxes: &[BlockBox],
    document: &nectar_core::Document,
    ops: &HashMap<BlockId, nectar_core::BlockOps>,
    owner: &BlockId,
) -> Option<BlockId> {
    let start = positions.iter().position(|p| &p.id == owner)?;
    let last = boxes.iter().filter(|b| &b.id == owner).map(|b| b.page).max().unwrap_or(0).max(positions[start].page);
    let top_level = |id: &BlockId| document.blocks.iter().any(|b| &b.id == id);
    let mut previous: Option<&BlockId> = None;
    for position in &positions[start + 1..] {
        if !top_level(&position.id) {
            continue;
        }
        if position.page > last {
            // Un bloc commencé sur la page au format propre et qui déborde
            // passe lui aussi au format courant, en entier.
            let target = match previous {
                Some(prev) if prev != owner && boxes.iter().any(|b| &b.id == prev && b.page >= position.page) => prev,
                _ => &position.id,
            };
            return ops.get(target).is_none_or(|o| o.page.is_none()).then(|| target.clone());
        }
        previous = Some(&position.id);
    }
    None
}

/// Le bloc après lequel placer la page paysage d'une image pour que le texte
/// qui la suit remplisse d'abord la page d'avant ; `None` si elle est déjà
/// bien remplie ou si rien ne peut y monter.
fn defer_point(
    compiled: &Compiled,
    document: &nectar_core::Document,
    style: &nectar_core::Style,
    figure: &BlockId,
    moved: &[&BlockId],
) -> Option<BlockId> {
    use nectar_core::model::Node;
    let mm = 72.0 / 25.4;
    let (top, bottom) = (f64::from(style.page.margin_top_mm) * mm, f64::from(style.page.margin_bottom_mm) * mm);
    let positions = compiled.block_positions();
    let metrics = compiled.page_metrics(bottom);
    let page = positions.iter().find(|p| &p.id == figure)?.page;
    let before = metrics.get(page.checked_sub(1)?)?;
    let after = metrics.get(page + 1)?;
    if before.width > before.height || after.width > after.height {
        return None;
    }
    let content_bottom = before.content.map(|c| c[3]).unwrap_or(top);
    let free = before.height - bottom - content_bottom - 12.0;
    if free < (before.height - top - bottom) * 0.25 {
        return None;
    }
    // Blocs de premier niveau qui suivent l'image et ouvrent la page d'après.
    let index = document.blocks.iter().position(|b| &b.id == figure)?;
    let following: Vec<&nectar_core::Block> =
        document.blocks[index + 1..].iter().filter(|b| !moved.contains(&&b.id)).collect();
    let y_of = |id: &BlockId| positions.iter().find(|p| &p.id == id && p.page == page + 1).map(|p| p.y);
    // Bas du dernier bloc de la page d'après, s'il s'y termine (une page
    // pleine laisse penser qu'il continue plus loin).
    let last_bottom =
        after.content.map(|c| c[3]).filter(|b| *b < after.height - bottom - (after.height - top - bottom) * 0.1);
    // Le schéma reste dans sa section : on ne passe pas un titre de même niveau.
    let section = document.blocks[..index].iter().rev().find_map(|b| match b.node {
        Node::Heading { level, .. } => Some(level),
        _ => None,
    });
    let mut chosen = None;
    for (i, block) in following.iter().enumerate() {
        let Some(_) = y_of(&block.id) else { break };
        if let (Node::Heading { level, .. }, Some(section)) = (&block.node, section)
            && *level <= section
        {
            break;
        }
        let next = following.get(i + 1);
        let end = match next.and_then(|n| y_of(&n.id)) {
            Some(y) => y,
            None => match last_bottom {
                Some(b) => b,
                None => break,
            },
        };
        if end - top > free {
            break;
        }
        // Jamais un titre ou une phrase d'annonce juste avant la page paysage,
        // ni entre une image (ou un tableau) et sa légende.
        let dangling = match &block.node {
            Node::Heading { .. } => true,
            Node::Paragraph(text) => nectar_core::model::plain_text(text).trim_end().ends_with(':'),
            _ => false,
        } || next.is_some_and(|n| nectar_core::codegen::is_caption(&n.node));
        if !dangling {
            chosen = Some(block.id.clone());
        }
        // Une autre image : on s'arrête avant, elle a sa propre place. Fin de
        // la page d'après : on ne mesure pas plus loin.
        if next.is_none_or(|n| matches!(n.node, Node::Figure(_) | Node::Diagram { .. }) || y_of(&n.id).is_none()) {
            break;
        }
    }
    chosen
}

/// Une image d'un cadre : son rectangle et, pour une photo, sa taille en pixels.
type PlacedImage = ([f64; 4], Option<(f64, f64)>);

/// Images d'un cadre : leur rectangle et, pour une photo, sa taille en pixels.
fn collect_images(frame: &typst::layout::Frame, offset: typst::layout::Point, out: &mut Vec<PlacedImage>) {
    use typst::layout::FrameItem;
    use typst::visualize::ImageKind;
    for (pos, item) in frame.items() {
        let p = offset + *pos;
        match item {
            FrameItem::Group(group) => {
                let shifted = typst::layout::Point::new(p.x + group.transform.tx, p.y + group.transform.ty);
                collect_images(&group.frame, shifted, out);
            }
            FrameItem::Image(image, size, _) => {
                let (x, y) = (p.x.to_pt(), p.y.to_pt());
                let pixels = matches!(image.kind(), ImageKind::Raster(_)).then(|| (image.width(), image.height()));
                out.push(([x, y, x + size.x.to_pt(), y + size.y.to_pt()], pixels));
            }
            _ => {}
        }
    }
}

/// Rectangles des éléments visibles d'un cadre, au-dessus de `cutoff`.
fn collect(frame: &typst::layout::Frame, offset: typst::layout::Point, cutoff: f64, out: &mut Vec<[f64; 4]>) {
    use typst::layout::FrameItem;
    for (pos, item) in frame.items() {
        let p = offset + *pos;
        let (x, y) = (p.x.to_pt(), p.y.to_pt());
        let rect = match item {
            FrameItem::Group(group) => {
                let shifted = typst::layout::Point::new(p.x + group.transform.tx, p.y + group.transform.ty);
                collect(&group.frame, shifted, cutoff, out);
                continue;
            }
            FrameItem::Text(text) => {
                let size = text.size.to_pt();
                [x, y - size * 0.8, x + text.width().to_pt(), y + size * 0.25]
            }
            FrameItem::Shape(shape, _) => {
                let bbox = shape.bbox(true);
                [x + bbox.min.x.to_pt(), y + bbox.min.y.to_pt(), x + bbox.max.x.to_pt(), y + bbox.max.y.to_pt()]
            }
            FrameItem::Image(_, size, _) => [x, y, x + size.x.to_pt(), y + size.y.to_pt()],
            _ => continue,
        };
        if rect[1] < cutoff {
            out.push(rect);
        }
    }
}

/// Parcourt un cadre et agrandit l'étendue avec chaque élément visible
/// commençant au-dessus de `cutoff`.
fn walk(frame: &typst::layout::Frame, offset: typst::layout::Point, cutoff: f64, extent: &mut Option<[f64; 4]>) {
    use typst::layout::FrameItem;
    for (pos, item) in frame.items() {
        let p = offset + *pos;
        let (x, y) = (p.x.to_pt(), p.y.to_pt());
        let rect = match item {
            FrameItem::Group(group) => {
                let shifted = typst::layout::Point::new(p.x + group.transform.tx, p.y + group.transform.ty);
                walk(&group.frame, shifted, cutoff, extent);
                continue;
            }
            FrameItem::Text(text) => {
                let size = text.size.to_pt();
                [x, y - size * 0.8, x + text.width().to_pt(), y + size * 0.25]
            }
            FrameItem::Shape(shape, _) => {
                let bbox = shape.bbox(true);
                [x + bbox.min.x.to_pt(), y + bbox.min.y.to_pt(), x + bbox.max.x.to_pt(), y + bbox.max.y.to_pt()]
            }
            FrameItem::Image(_, size, _) => [x, y, x + size.x.to_pt(), y + size.y.to_pt()],
            _ => continue,
        };
        if rect[1] >= cutoff {
            continue;
        }
        *extent = Some(match *extent {
            None => rect,
            Some(e) => [e[0].min(rect[0]), e[1].min(rect[1]), e[2].max(rect[2]), e[3].max(rect[3])],
        });
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
