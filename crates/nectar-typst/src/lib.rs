//! Compilation Typst locale pour Nectar Render.
//!
//! [`Engine`] garde les polices et la bibliothèque standard entre deux
//! compilations : seule la source change, et Typst ne refait que ce qui a
//! bougé (compilation incrémentale), ce qui permet l'aperçu en direct.

mod images;
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

    /// Compile une source générée.
    pub fn compile(&self, generated: &Generated) -> Result<Compiled, EngineError> {
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
                    .rfind(|p| p.page < page_index || (p.page == page_index && p.y <= rect[1] + 2.0))
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
                    positions.iter().rfind(|p| p.page < page_index || (p.page == page_index && p.y <= rect[1] + 2.0));
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
    use nectar_core::assistant::{Inputs, Marker, analyse};
    let margin_bottom = f64::from(style.page.margin_bottom_mm) * 72.0 / 25.4;
    let pages = compiled.page_metrics(margin_bottom);
    let markers: Vec<Marker> =
        compiled.block_positions().into_iter().map(|p| Marker { id: p.id, page: p.page, y: p.y }).collect();
    let notes = compiled.template_notes();
    let figures = compiled.figures();
    let ops = layout.resolve(document).ops;
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
}

/// Met en page un document avec le placement automatique en deux temps.
///
/// Premier passage : les règles (petits blocs d'un seul tenant, annonces
/// gardées avec la suite…). Puis on relit les pages : si un bloc gardé d'un
/// seul tenant par la seule règle automatique laisse une page à moitié vide,
/// il est autorisé à se couper (un tableau répète alors son en-tête) et on
/// recompose. Une retouche manuelle n'est jamais remise en cause.
pub fn lay_out(
    engine: &Engine,
    document: &nectar_core::Document,
    layout: &nectar_core::Layout,
    style: &nectar_core::Style,
) -> LaidOut {
    use nectar_core::assistant::FixAction;
    use nectar_core::layout::Placement;
    let mut tuning = nectar_core::Tuning::default();
    let mut generated = nectar_core::generate_tuned(document, layout, style, &tuning);
    let mut compiled = engine.compile(&generated);
    let ops = layout.resolve(document).ops;
    for _ in 0..3 {
        let Ok(current) = &compiled else { break };
        let holes: Vec<BlockId> = inspect(current, document, layout, style, &generated, &[])
            .into_iter()
            .flat_map(|issue| issue.fixes)
            .filter(|fix| fix.action == FixAction::KeepTogether(false))
            .map(|fix| fix.block)
            .filter(|id| ops.get(id).is_none_or(|o| o.keep_together.is_none()) && !tuning.relaxed.contains(id))
            .collect();
        if holes.is_empty() {
            break;
        }
        tuning.relaxed.extend(holes);
        let retry = nectar_core::generate_tuned(document, layout, style, &tuning);
        match engine.compile(&retry) {
            Ok(better) => {
                compiled = Ok(better);
                generated = retry;
            }
            Err(_) => break,
        }
    }
    // Pages paysage : si la page d'avant reste à moitié vide, le texte qui
    // suit vient la remplir et la page paysage arrive juste après.
    let landscapes: Vec<BlockId> = document
        .blocks
        .iter()
        .filter(|b| ops.get(&b.id).and_then(|o| o.image.as_ref()).is_some_and(|i| i.placement == Placement::Landscape))
        .map(|b| b.id.clone())
        .collect();
    // De la dernière à la première : une image plus loin, déjà repoussée,
    // laisse son texte disponible pour les précédentes.
    for figure in landscapes.iter().rev().take(6) {
        let Ok(current) = &compiled else { break };
        let moved: Vec<&BlockId> = tuning.deferred.iter().map(|(f, _)| f).collect();
        let Some(after) = defer_point(current, document, style, figure, &moved) else { continue };
        tuning.deferred.push((figure.clone(), after));
        let retry = nectar_core::generate_tuned(document, layout, style, &tuning);
        match engine.compile(&retry) {
            Ok(better) => {
                compiled = Ok(better);
                generated = retry;
            }
            Err(_) => {
                tuning.deferred.pop();
            }
        }
    }
    let mut relaxed: Vec<BlockId> = tuning.relaxed.into_iter().collect();
    relaxed.sort();
    LaidOut { generated, compiled, relaxed }
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
        // Jamais un titre ou une phrase d'annonce juste avant la page paysage.
        let dangling = match &block.node {
            Node::Heading { .. } => true,
            Node::Paragraph(text) => nectar_core::model::plain_text(text).trim_end().ends_with(':'),
            _ => false,
        };
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
