//! Le moteur tourne dans son propre fil : l'interface ne gèle jamais
//! pendant une compilation. Seule la dernière demande compte.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Instant;

use eframe::egui;
use nectar_core::Generated;
use nectar_typst::{BlockPosition, Compiled, Engine, FontSources, PdfOptions};

pub enum Request {
    /// Compiler une nouvelle source et rendre ses pages.
    Render {
        generation: u64,
        generated: Box<Generated>,
        ppi: f32,
    },
    /// Rendre de nouveau les pages à une autre échelle (zoom).
    Rescale {
        ppi: f32,
    },
    Export {
        path: PathBuf,
        ident: String,
    },
}

pub enum Response {
    /// Le moteur est prêt ; voici les polices disponibles.
    Ready {
        families: Vec<String>,
    },
    Rendered(Box<Rendered>),
    Exported(Result<PathBuf, String>),
}

pub struct PageImage {
    pub hash: u128,
    pub size_pt: egui::Vec2,
    pub image: Arc<egui::ColorImage>,
}

pub struct Rendered {
    pub generation: u64,
    pub pages: Vec<PageImage>,
    pub positions: Vec<BlockPosition>,
    pub warnings: Vec<String>,
    pub missing_fonts: Vec<String>,
    /// Erreur de compilation : les pages précédentes restent affichées.
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

/// La dernière mise en page réussie : génération, document, positions,
/// avertissements, polices absentes.
type Current = (u64, Compiled, Vec<BlockPosition>, Vec<String>, Vec<String>);

fn run(ctx: egui::Context, requests: Receiver<Request>, responses: Sender<Response>) {
    let engine = Engine::new(FontSources::WithSystem);
    let _ = responses.send(Response::Ready { families: engine.font_families() });
    ctx.request_repaint();

    let mut current: Option<Current> = None;
    let mut cache: HashMap<(u128, u32), Arc<egui::ColorImage>> = HashMap::new();
    let mut ppi = 96.0_f32;

    while let Ok(first) = requests.recv() {
        // Ne garder que la dernière demande de rendu en attente.
        let mut pending = vec![first];
        while let Ok(more) = requests.try_recv() {
            pending.push(more);
        }
        let mut render: Option<(u64, Box<Generated>)> = None;
        let mut rescale = false;
        for request in pending {
            match request {
                Request::Render { generation, generated, ppi: p } => {
                    render = Some((generation, generated));
                    ppi = p;
                }
                Request::Rescale { ppi: p } => {
                    ppi = p;
                    rescale = true;
                }
                Request::Export { path, ident } => {
                    let result = match &current {
                        Some((_, compiled, ..)) => compiled
                            .pdf(&PdfOptions { ident: Some(ident) })
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

        let started = Instant::now();
        let mut error = None;
        if let Some((generation, generated)) = render {
            let missing = engine.missing_fonts(&generated.fonts);
            match engine.compile(&generated) {
                Ok(compiled) => {
                    let positions = compiled.block_positions();
                    let warnings = generated.warnings.iter().chain(&compiled.warnings).cloned().collect();
                    current = Some((generation, compiled, positions, warnings, missing));
                }
                Err(e) => error = Some(e.to_string()),
            }
            if current.is_none() && error.is_some() {
                let _ = responses.send(Response::Rendered(Box::new(Rendered {
                    generation,
                    pages: Vec::new(),
                    positions: Vec::new(),
                    warnings: Vec::new(),
                    missing_fonts: Vec::new(),
                    error,
                    millis: started.elapsed().as_millis(),
                })));
                ctx.request_repaint();
                continue;
            }
            if error.is_some()
                && let Some((g, ..)) = &mut current
            {
                *g = generation;
            }
        } else if !rescale {
            continue;
        }

        let Some((generation, compiled, positions, warnings, missing)) = &current else { continue };
        let key_ppi = ppi.to_bits();
        let mut pages = Vec::with_capacity(compiled.page_count());
        let mut used = HashMap::new();
        for index in 0..compiled.page_count() {
            let hash = compiled.page_hash(index).unwrap_or_default();
            let image = match cache.get(&(hash, key_ppi)) {
                Some(image) => image.clone(),
                None => {
                    let Ok((w, h, rgba)) = compiled.rgba(index, ppi) else { continue };
                    Arc::new(egui::ColorImage::from_rgba_premultiplied([w as usize, h as usize], &rgba))
                }
            };
            used.insert((hash, key_ppi), image.clone());
            let (w, h) = compiled.page_size(index).unwrap_or((595.0, 842.0));
            pages.push(PageImage { hash, size_pt: egui::vec2(w as f32, h as f32), image });
        }
        cache = used;
        let _ = responses.send(Response::Rendered(Box::new(Rendered {
            generation: *generation,
            pages,
            positions: positions.clone(),
            warnings: warnings.clone(),
            missing_fonts: missing.clone(),
            error,
            millis: started.elapsed().as_millis(),
        })));
        ctx.request_repaint();
    }
}
