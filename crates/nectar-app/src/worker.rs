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

/// Une page rendue : largeur, hauteur, pixels RGBA.
type Rgba = (u32, u32, Vec<u8>);

/// Mémoire des images de pages gardées par le moteur (environ 20 pages A4 nettes).
const CACHE_BYTES: usize = 200 * 1024 * 1024;

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
    /// Le moteur a rencontré une erreur interne (il continue de tourner).
    Failed(String),
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
    /// Ce que le placement automatique a décidé.
    pub choices: Vec<nectar_core::auto::Choice>,
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
    let engine = Engine::new(FontSources::WithSystem).with_cache_dir(nectar_core::style::default_cache_dir());
    let _ = responses.send(Response::Ready { families: engine.font_families() });
    ctx.request_repaint();

    let mut current: Option<Compiled> = None;
    let mut cache: VecDeque<PageImage> = VecDeque::new();

    // Demandes arrivées pendant le rendu des pages, à traiter au tour suivant.
    let mut backlog: Vec<Request> = Vec::new();
    loop {
        // Seules la dernière mise en page et la dernière demande d'images comptent.
        let mut pending = std::mem::take(&mut backlog);
        if pending.is_empty() {
            match requests.recv() {
                Ok(first) => pending.push(first),
                Err(_) => break,
            }
        }
        while let Ok(more) = requests.try_recv() {
            pending.push(more);
        }
        // Une erreur interne du moteur (bogue de Typst, image corrompue…) ne
        // doit jamais l'arrêter : on la signale et on continue.
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
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
                let tuning = laid.tuning;
                let choices = laid.choices;
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
                        issues: nectar_typst::inspect_tuned(
                            compiled,
                            &check.document,
                            &check.layout,
                            &check.style,
                            &generated,
                            &missing_fonts,
                            &tuning,
                        ),
                        choices,
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
                        choices: Vec::new(),
                        missing_fonts,
                        error,
                        millis: started.elapsed().as_millis(),
                    },
                };
                if started.elapsed() > std::time::Duration::from_secs(3) {
                    crate::journal::write(&format!(
                        "mise en page lente : {} ms pour {} pages",
                        started.elapsed().as_millis(),
                        laid.pages.len()
                    ));
                }
                let _ = responses.send(Response::Laid(Box::new(laid)));
                ctx.request_repaint();
            }

            if let (Some((indices, ppi)), Some(compiled)) = (pages, &current) {
                let send = |image: PageImage| {
                    let _ = responses.send(Response::Images(vec![image]));
                    ctx.request_repaint();
                };
                // Déjà prêtes : envoyées tout de suite.
                let mut todo: Vec<(usize, u128)> = Vec::new();
                for index in indices {
                    let Some(hash) = compiled.page_hash(index) else { continue };
                    match cache.iter().position(|c| c.hash == hash && c.ppi == ppi) {
                        Some(i) => {
                            let entry = cache.remove(i).expect("présent");
                            send(PageImage { hash, ppi, image: entry.image.clone() });
                            cache.push_back(entry);
                        }
                        None => todo.push((index, hash)),
                    }
                }
                // Une page jamais vue s'affiche d'abord en basse résolution (quelques
                // millisecondes), puis nette ; plusieurs pages se rendent à la fois.
                let quick_ppi = (ppi * 0.35).max(24.0);
                let unseen: Vec<(usize, u128)> =
                    todo.iter().filter(|(_, hash)| !cache.iter().any(|c| c.hash == *hash)).copied().collect();
                let mut passes = Vec::new();
                if quick_ppi < ppi * 0.8 && !unseen.is_empty() {
                    passes.push((unseen, quick_ppi, false));
                }
                passes.push((todo, ppi, true));
                let workers =
                    std::thread::available_parallelism().map(|n| n.get().saturating_sub(1)).unwrap_or(1).clamp(1, 4);
                'passes: for (list, pass_ppi, keep) in passes {
                    for chunk in list.chunks(workers) {
                        // Une retouche ou un défilement arrivé entre-temps passe avant
                        // les pages qui restent (elles seraient périmées).
                        if let Ok(newer) = requests.try_recv() {
                            let export = matches!(newer, Request::Export { .. });
                            backlog.push(newer);
                            if !export {
                                break 'passes;
                            }
                        }
                        let rendered: Vec<(u128, Option<Rgba>)> = std::thread::scope(|scope| {
                            let handles: Vec<_> = chunk
                                .iter()
                                .map(|&(index, hash)| scope.spawn(move || (hash, compiled.rgba(index, pass_ppi).ok())))
                                .collect();
                            handles.into_iter().filter_map(|h| h.join().ok()).collect()
                        });
                        for (hash, result) in rendered {
                            let Some((w, h, rgba)) = result else { continue };
                            let image =
                                Arc::new(egui::ColorImage::from_rgba_premultiplied([w as usize, h as usize], &rgba));
                            if keep {
                                cache.push_back(PageImage { hash, ppi: pass_ppi, image: image.clone() });
                                // Mémoire bornée : on oublie les plus anciennes.
                                while cache.len() > 1
                                    && cache.iter().map(|c| c.image.pixels.len() * 4).sum::<usize>() > CACHE_BYTES
                                {
                                    cache.pop_front();
                                }
                            }
                            send(PageImage { hash, ppi: pass_ppi, image });
                        }
                    }
                }
            }

            for (path, ident, pdf_a) in exports {
                let result = match &current {
                    Some(compiled) => compiled
                        .pdf(&PdfOptions { ident: Some(ident), pdf_a })
                        .map_err(|e| e.to_string())
                        .and_then(|pdf| {
                            std::fs::write(&path, pdf).map_err(|e| match e.kind() {
                                std::io::ErrorKind::PermissionDenied => format!(
                                    "{} est peut-être ouvert dans un autre programme : ferme-le et réessaie",
                                    path.display()
                                ),
                                _ => e.to_string(),
                            })
                        })
                        .map(|()| path),
                    None => Err("rien à exporter : le document n'a pas encore été mis en page".into()),
                };
                let _ = responses.send(Response::Exported(result));
                ctx.request_repaint();
            }
        }));
        if let Err(panic) = outcome {
            let message = panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|s| (*s).to_string()))
                .unwrap_or_else(|| "erreur inconnue".into());
            let _ = responses.send(Response::Failed(message));
            ctx.request_repaint();
        }
    }
}
