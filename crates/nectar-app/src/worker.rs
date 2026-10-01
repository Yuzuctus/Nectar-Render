//! Le moteur tourne dans son propre fil : l'interface ne gèle jamais
//! pendant une compilation.
//!
//! Deux temps : la mise en page (rapide, tout le document) puis le rendu en
//! images des seules pages visibles, à la demande. Un document de 50 pages
//! n'occupe ainsi que quelques pages en mémoire.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Instant;

use eframe::egui;
use nectar_typst::{BlockPosition, Compiled, Engine, FontSources, PdfOptions};

/// Images de pages gardées en mémoire par le moteur.
const CACHE_PAGES: usize = 24;

pub enum Request {
    /// Mettre en page la note retouchée (et la vérifier avec l'assistant).
    Layout {
        generation: u64,
        check: Box<CheckInputs>,
    },
    /// Rendre ces pages (indices) à cette résolution.
    Pages {
        pages: Vec<usize>,
        ppi: f32,
    },
    Export {
        path: PathBuf,
        ident: String,
        pdf_a: bool,
    },
}

/// La note, ses retouches et son style : de quoi la mettre en page.
pub struct CheckInputs {
    pub document: nectar_core::Document,
    pub layout: nectar_core::Layout,
    pub style: nectar_core::Style,
}

pub enum Response {
    /// Le moteur est prêt ; voici les polices disponibles.
    Ready {
        families: Vec<String>,
    },
    Laid(Box<Layouted>),
    Images(Vec<PageImage>),
    Exported(Result<PathBuf, String>),
}

/// Une page mise en page, sans son image.
#[derive(Clone)]
pub struct PageInfo {
    pub hash: u128,
    pub size_pt: egui::Vec2,
}

pub struct PageImage {
    pub hash: u128,
    pub ppi: f32,
    pub image: Arc<egui::ColorImage>,
}

pub struct Layouted {
    pub generation: u64,
    pub pages: Vec<PageInfo>,
    pub positions: Vec<BlockPosition>,
    pub warnings: Vec<String>,
    pub missing_fonts: Vec<String>,
    /// Remarques de l'assistant de mise en page.
    pub issues: Vec<nectar_core::assistant::Issue>,
    /// Boîte réelle de chaque bloc, page par page.
    pub boxes: Vec<nectar_typst::BlockBox>,
    /// Erreur de compilation : la mise en page précédente reste affichée.
    pub error: Option<String>,
    pub millis: u128,
}

pub struct Worker {
    tx: Sender<Request>,
    pub rx: Receiver<Response>,
}

impl Worker {
    pub fn spawn(ctx: egui::Context) -> Self {
        let (tx, requests) = channel::<Request>();
        let (responses, rx) = channel::<Response>();
        std::thread::Builder::new()
            .name("nectar-moteur".into())
            .spawn(move || run(ctx, requests, responses))
            .expect("fil du moteur");
        Self { tx, rx }
    }

    pub fn send(&self, request: Request) {
        let _ = self.tx.send(request);
    }
}

fn run(ctx: egui::Context, requests: Receiver<Request>, responses: Sender<Response>) {
    let engine = Engine::new(FontSources::WithSystem);
    let _ = responses.send(Response::Ready { families: engine.font_families() });
    ctx.request_repaint();

    let mut current: Option<Compiled> = None;
    let mut cache: VecDeque<PageImage> = VecDeque::new();

    while let Ok(first) = requests.recv() {
        // Seules la dernière mise en page et la dernière demande d'images comptent.
        let mut pending = vec![first];
        while let Ok(more) = requests.try_recv() {
            pending.push(more);
        }
        let mut layout = None;
        let mut pages = None;
        let mut exports = Vec::new();
        for request in pending {
            match request {
                Request::Layout { generation, check } => layout = Some((generation, check)),
                Request::Pages { pages: p, ppi } => pages = Some((p, ppi)),
                Request::Export { path, ident, pdf_a } => exports.push((path, ident, pdf_a)),
            }
        }

        if let Some((generation, check)) = layout {
            let started = Instant::now();
            let laid = nectar_typst::lay_out(&engine, &check.document, &check.layout, &check.style);
            let generated = laid.generated;
            let missing_fonts = engine.missing_fonts(&generated.fonts);
            let mut error = None;
            match laid.compiled {
                Ok(compiled) => current = Some(compiled),
                Err(e) => error = Some(e.to_string()),
            }
            let laid = match &current {
                Some(compiled) => Layouted {
                    generation,
                    pages: (0..compiled.page_count())
                        .map(|i| {
                            let (w, h) = compiled.page_size(i).unwrap_or((595.0, 842.0));
                            PageInfo {
                                hash: compiled.page_hash(i).unwrap_or_default(),
                                size_pt: egui::vec2(w as f32, h as f32),
                            }
                        })
                        .collect(),
                    positions: compiled.block_positions(),
                    boxes: compiled.block_boxes(f64::from(check.style.page.margin_bottom_mm) * 72.0 / 25.4),
                    warnings: generated.warnings.iter().chain(&compiled.warnings).cloned().collect(),
                    issues: nectar_typst::inspect(
                        compiled,
                        &check.document,
                        &check.layout,
                        &check.style,
                        &generated,
                        &missing_fonts,
                    ),
                    missing_fonts,
                    error,
                    millis: started.elapsed().as_millis(),
                },
                None => Layouted {
                    generation,
                    pages: Vec::new(),
                    positions: Vec::new(),
                    boxes: Vec::new(),
                    warnings: generated.warnings.clone(),
                    issues: Vec::new(),
                    missing_fonts,
                    error,
                    millis: started.elapsed().as_millis(),
                },
            };
            let _ = responses.send(Response::Laid(Box::new(laid)));
            ctx.request_repaint();
        }

        if let (Some((indices, ppi)), Some(compiled)) = (pages, &current) {
            let mut images = Vec::new();
            for index in indices {
                let Some(hash) = compiled.page_hash(index) else { continue };
                let cached = cache.iter().position(|c| c.hash == hash && c.ppi == ppi);
                let image = match cached {
                    Some(i) => {
                        let entry = cache.remove(i).expect("présent");
                        let image = entry.image.clone();
                        cache.push_back(entry);
                        image
                    }
                    None => {
                        let Ok((w, h, rgba)) = compiled.rgba(index, ppi) else { continue };
                        let image =
                            Arc::new(egui::ColorImage::from_rgba_premultiplied([w as usize, h as usize], &rgba));
                        cache.push_back(PageImage { hash, ppi, image: image.clone() });
                        while cache.len() > CACHE_PAGES {
                            cache.pop_front();
                        }
                        image
                    }
                };
                images.push(PageImage { hash, ppi, image });
            }
            if !images.is_empty() {
                let _ = responses.send(Response::Images(images));
                ctx.request_repaint();
            }
        }

        for (path, ident, pdf_a) in exports {
            let result = match &current {
                Some(compiled) => compiled
                    .pdf(&PdfOptions { ident: Some(ident), pdf_a })
                    .map_err(|e| e.to_string())
                    .and_then(|pdf| std::fs::write(&path, pdf).map_err(|e| e.to_string()))
                    .map(|()| path),
                None => Err("rien à exporter : le document n'a pas encore été mis en page".into()),
            };
            let _ = responses.send(Response::Exported(result));
            ctx.request_repaint();
        }
    }
}
