//! Le moteur tourne dans son propre fil : l'interface ne gèle jamais
//! pendant une compilation.
//!
//! Deux temps : la mise en page (rapide, tout le document) puis le rendu en
//! images des seules pages visibles, à la demande. Un document de 50 pages
//! n'occupe ainsi que quelques pages en mémoire.
//!
//! Une retouche s'affiche sans attendre : quand le placement automatique est
//! long, la note est d'abord composée avec ses décisions précédentes, puis le
//! calcul complet la remplace (et s'arrête net si une nouvelle retouche
//! arrive entre-temps). Les dernières mises en page sont gardées : Annuler et
//! Rétablir sont immédiats.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use eframe::egui;
use nectar_typst::{BlockPosition, Compiled, Engine, FontSources, PdfOptions};

/// Une page rendue : largeur, hauteur, pixels RGBA.
type Rgba = (u32, u32, Vec<u8>);

/// Mémoire des images de pages gardées par le moteur (environ 20 pages A4 nettes).
const CACHE_BYTES: usize = 200 * 1024 * 1024;

/// Mises en page gardées pour Annuler et Rétablir.
const REMEMBERED: usize = 6;

/// Au-delà de cette durée de calcul, une retouche s'affiche d'abord
/// provisoirement.
const SLOW: Duration = Duration::from_millis(350);

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

#[derive(Clone)]
pub struct Layouted {
    pub generation: u64,
    /// Composée avec les décisions automatiques d'avant : le calcul complet
    /// est en cours et va la remplacer.
    pub provisional: bool,
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
        let engine = || Engine::new(FontSources::WithSystem).with_cache_dir(nectar_core::style::default_cache_dir());
        Self::spawn_with(ctx, engine, SLOW)
    }

    fn spawn_with(ctx: egui::Context, engine: impl FnOnce() -> Engine + Send + 'static, slow: Duration) -> Self {
        let (tx, requests) = channel::<Request>();
        let (responses, rx) = channel::<Response>();
        std::thread::Builder::new()
            .name("nectar-moteur".into())
            .spawn(move || run(ctx, engine(), slow, requests, responses))
            .expect("fil du moteur");
        Self { tx, rx }
    }

    pub fn send(&self, request: Request) {
        let _ = self.tx.send(request);
    }
}

/// Une mise en page déjà calculée.
struct Remembered {
    key: u64,
    compiled: Arc<Compiled>,
    tuning: nectar_core::Tuning,
    laid: Layouted,
}

/// Ce que le moteur garde d'une demande à l'autre.
#[derive(Default)]
struct State {
    current: Option<Arc<Compiled>>,
    cache: VecDeque<PageImage>,
    remembered: VecDeque<Remembered>,
    /// Décisions automatiques de la dernière mise en page complète.
    last_tuning: Option<nectar_core::Tuning>,
    /// La dernière mise en page complète a été longue.
    slow: bool,
    /// Durée à partir de laquelle elle l'est.
    slow_after: Duration,
    /// Les dernières pages demandées (indices, résolution).
    last_view: Option<(Vec<usize>, f32)>,
    /// Mise en page complète encore à faire (après une version provisoire).
    full: Option<(u64, Box<CheckInputs>, u64)>,
}

fn run(ctx: egui::Context, engine: Engine, slow: Duration, requests: Receiver<Request>, responses: Sender<Response>) {
    let _ = responses.send(Response::Ready { families: engine.font_families() });
    ctx.request_repaint();

    let mut state = State { slow_after: slow, ..State::default() };
    // Demandes arrivées pendant un calcul, à traiter au tour suivant.
    let mut backlog: Vec<Request> = Vec::new();
    loop {
        // Seules la dernière mise en page et la dernière demande d'images comptent.
        let mut pending = std::mem::take(&mut backlog);
        if pending.is_empty() && state.full.is_none() {
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
                if start(&engine, &mut state, generation, check, &responses, &ctx) {
                    // Version provisoire : les pages qu'on regardait, tout de suite.
                    pages = pages.or_else(|| state.last_view.clone());
                } else {
                    // Calcul rapide : fait avant les pages, qui en dépendent.
                    complete(&engine, &mut state, &requests, &mut backlog, &responses, &ctx);
                }
            }

            if let Some((indices, ppi)) = pages {
                state.last_view = Some((indices.clone(), ppi));
                render(&mut state, indices, ppi, &requests, &mut backlog, &responses, &ctx);
            }

            for (path, ident, pdf_a) in exports {
                let result = match &state.current {
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

            // Après une version provisoire : le calcul complet, s'il n'y a rien
            // de plus récent à traiter d'abord.
            if backlog.is_empty() {
                complete(&engine, &mut state, &requests, &mut backlog, &responses, &ctx);
            }
        }));
        if let Err(panic) = outcome {
            state.full = None;
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

/// Prend en charge une demande de mise en page : déjà calculée, elle est
/// rendue aussitôt ; sinon elle est prévue (`state.full`), précédée d'une
/// version provisoire quand le calcul complet est long (rend alors `true`).
fn start(
    engine: &Engine,
    state: &mut State,
    generation: u64,
    check: Box<CheckInputs>,
    responses: &Sender<Response>,
    ctx: &egui::Context,
) -> bool {
    let started = Instant::now();
    let key = inputs_key(&check);
    state.full = None;
    if let Some(at) = state.remembered.iter().position(|r| r.key == key) {
        // Annuler, Rétablir, aller-retour d'un réglage : rien à recomposer.
        let entry = state.remembered.remove(at).expect("présente");
        state.current = Some(entry.compiled.clone());
        state.last_tuning = Some(entry.tuning.clone());
        let mut laid = entry.laid.clone();
        laid.generation = generation;
        laid.millis = started.elapsed().as_millis();
        state.remembered.push_back(entry);
        let _ = responses.send(Response::Laid(Box::new(laid)));
        ctx.request_repaint();
        return false;
    }
    let mut provisional = false;
    if state.slow
        && let Some(tuning) = &state.last_tuning
    {
        let generated = nectar_core::generate_tuned(&check.document, &check.layout, &check.style, tuning);
        if let Ok(compiled) = engine.compile(&generated) {
            let compiled = Arc::new(compiled);
            let choices = tuning.choices(&check.document);
            let mut laid = describe(engine, &compiled, &generated, &check, tuning, choices, None);
            laid.generation = generation;
            laid.provisional = true;
            laid.millis = started.elapsed().as_millis();
            state.current = Some(compiled);
            let _ = responses.send(Response::Laid(Box::new(laid)));
            ctx.request_repaint();
            provisional = true;
        }
    }
    state.full = Some((generation, check, key));
    provisional
}

/// Le calcul complet prévu, abandonné si une nouvelle retouche arrive.
fn complete(
    engine: &Engine,
    state: &mut State,
    requests: &Receiver<Request>,
    backlog: &mut Vec<Request>,
    responses: &Sender<Response>,
    ctx: &egui::Context,
) {
    let Some((generation, check, key)) = state.full.take() else { return };
    let started = Instant::now();
    let incoming = RefCell::new(Vec::new());
    let stop = || {
        let mut incoming = incoming.borrow_mut();
        while let Ok(request) = requests.try_recv() {
            incoming.push(request);
        }
        incoming.iter().any(|r| matches!(r, Request::Layout { .. }))
    };
    let laid = nectar_typst::lay_out_with(engine, &check.document, &check.layout, &check.style, &stop);
    backlog.extend(incoming.into_inner());
    if laid.stopped {
        return;
    }
    let passes = laid.passes;
    let (layouted, compiled) = match laid.compiled {
        Ok(compiled) => {
            let compiled = Arc::new(compiled);
            (describe(engine, &compiled, &laid.generated, &check, &laid.tuning, laid.choices, None), Some(compiled))
        }
        // Erreur : la mise en page précédente reste affichée.
        Err(e) => {
            let error = Some(e.to_string());
            let layouted = match &state.current {
                Some(current) => describe(engine, current, &laid.generated, &check, &laid.tuning, laid.choices, error),
                None => Layouted {
                    generation,
                    provisional: false,
                    pages: Vec::new(),
                    positions: Vec::new(),
                    boxes: Vec::new(),
                    warnings: laid.generated.warnings.clone(),
                    issues: Vec::new(),
                    choices: Vec::new(),
                    missing_fonts: engine.missing_fonts(&laid.generated.fonts),
                    error,
                    millis: 0,
                },
            };
            (layouted, None)
        }
    };
    let mut layouted = layouted;
    layouted.generation = generation;
    layouted.millis = started.elapsed().as_millis();
    state.slow = started.elapsed() > state.slow_after;
    if started.elapsed() > Duration::from_secs(3) {
        crate::journal::write(&format!(
            "mise en page lente : {} ms pour {} pages ({passes} compositions)",
            started.elapsed().as_millis(),
            layouted.pages.len()
        ));
    }
    if let Some(compiled) = compiled {
        state.remembered.push_back(Remembered {
            key,
            compiled: compiled.clone(),
            tuning: laid.tuning.clone(),
            laid: layouted.clone(),
        });
        while state.remembered.len() > REMEMBERED {
            state.remembered.pop_front();
        }
        state.last_tuning = Some(laid.tuning);
        state.current = Some(compiled);
    }
    let _ = responses.send(Response::Laid(Box::new(layouted)));
    ctx.request_repaint();
}

/// Ce que l'atelier reçoit d'une mise en page.
fn describe(
    engine: &Engine,
    compiled: &Compiled,
    generated: &nectar_core::Generated,
    check: &CheckInputs,
    tuning: &nectar_core::Tuning,
    choices: Vec<nectar_core::auto::Choice>,
    error: Option<String>,
) -> Layouted {
    let missing_fonts = engine.missing_fonts(&generated.fonts);
    Layouted {
        generation: 0,
        provisional: false,
        pages: (0..compiled.page_count())
            .map(|i| {
                let (w, h) = compiled.page_size(i).unwrap_or((595.0, 842.0));
                PageInfo { hash: compiled.page_hash(i).unwrap_or_default(), size_pt: egui::vec2(w as f32, h as f32) }
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
            generated,
            &missing_fonts,
            tuning,
        ),
        choices,
        missing_fonts,
        error,
        millis: 0,
    }
}

/// Empreinte de la note, de ses retouches et de son style.
fn inputs_key(check: &CheckInputs) -> u64 {
    use std::hash::Hasher;
    /// Hache un texte au fil de son écriture, sans le garder en mémoire.
    struct Feed(std::hash::DefaultHasher);
    impl std::fmt::Write for Feed {
        fn write_str(&mut self, s: &str) -> std::fmt::Result {
            self.0.write(s.as_bytes());
            Ok(())
        }
    }
    let mut feed = Feed(std::hash::DefaultHasher::new());
    let _ =
        std::fmt::Write::write_fmt(&mut feed, format_args!("{:?}{:?}{:?}", check.document, check.layout, check.style));
    feed.0.finish()
}

/// Rend les pages demandées de la mise en page affichée.
fn render(
    state: &mut State,
    indices: Vec<usize>,
    ppi: f32,
    requests: &Receiver<Request>,
    backlog: &mut Vec<Request>,
    responses: &Sender<Response>,
    ctx: &egui::Context,
) {
    let Some(compiled) = state.current.clone() else { return };
    let compiled: &Compiled = &compiled;
    let cache = &mut state.cache;
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
    let workers = std::thread::available_parallelism().map(|n| n.get().saturating_sub(1)).unwrap_or(1).clamp(1, 4);
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
                let image = Arc::new(egui::ColorImage::from_rgba_premultiplied([w as usize, h as usize], &rgba));
                if keep {
                    cache.push_back(PageImage { hash, ppi: pass_ppi, image: image.clone() });
                    // Mémoire bornée : on oublie les plus anciennes.
                    while cache.len() > 1 && cache.iter().map(|c| c.image.pixels.len() * 4).sum::<usize>() > CACHE_BYTES
                    {
                        cache.pop_front();
                    }
                }
                send(PageImage { hash, ppi: pass_ppi, image });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une note qui demande plusieurs ajustements du placement automatique.
    fn inputs(paragraphs: usize) -> Box<CheckInputs> {
        let mut text = String::from("# Compte rendu\n\n");
        for _ in 0..paragraphs {
            text.push_str(
                "Un paragraphe assez long pour occuper plusieurs lignes de la page, avec le détail de la séance.\n\n",
            );
        }
        text.push_str("## Plan\n\n| Ressource | Adresse | Rôle |\n|---|---|---|\n");
        for i in 0..10 {
            text.push_str(&format!("| Machine {i} | `10.0.{i}.1` | Un rôle décrit en plusieurs mots, sur plusieurs lignes de sa colonne |\n"));
        }
        let document = nectar_core::parse(&text, &nectar_core::ParseOptions::default());
        Box::new(CheckInputs { document, layout: Default::default(), style: Default::default() })
    }

    fn next_laid(worker: &Worker) -> Layouted {
        loop {
            match worker.rx.recv_timeout(Duration::from_secs(60)).expect("réponse du moteur") {
                Response::Laid(laid) => return *laid,
                Response::Failed(message) => panic!("{message}"),
                _ => {}
            }
        }
    }

    #[test]
    fn a_layout_already_seen_comes_back_without_recomputing() {
        let worker = Worker::spawn_with(egui::Context::default(), || Engine::new(FontSources::Bundled), SLOW);
        worker.send(Request::Layout { generation: 1, check: inputs(9) });
        let first = next_laid(&worker);
        assert_eq!(first.generation, 1);
        assert!(!first.provisional);
        worker.send(Request::Layout { generation: 2, check: inputs(3) });
        let other = next_laid(&worker);
        assert_eq!(other.generation, 2);
        // Annuler : la première mise en page revient telle quelle.
        worker.send(Request::Layout { generation: 3, check: inputs(9) });
        let back = next_laid(&worker);
        assert_eq!(back.generation, 3);
        assert!(!back.provisional);
        assert_eq!(back.positions, first.positions);
        assert_eq!(back.choices, first.choices);
        assert!(back.millis < 50, "rendue sans recalcul ({} ms)", back.millis);
    }

    #[test]
    fn only_the_latest_of_quick_successive_edits_is_completed() {
        let worker = Worker::spawn_with(egui::Context::default(), || Engine::new(FontSources::Bundled), SLOW);
        for (generation, paragraphs) in (1..=5).zip(4..) {
            worker.send(Request::Layout { generation, check: inputs(paragraphs) });
        }
        // La dernière mise en page reçue est complète et correspond à la
        // dernière retouche ; aucune n'arrive après elle.
        let mut last = next_laid(&worker);
        while last.generation < 5 || last.provisional {
            last = next_laid(&worker);
        }
        assert!(worker.rx.recv_timeout(Duration::from_millis(500)).is_err());
    }

    #[test]
    fn a_slow_layout_shows_the_edit_first_then_the_full_result() {
        let worker = Worker::spawn_with(egui::Context::default(), || Engine::new(FontSources::Bundled), Duration::ZERO);
        worker.send(Request::Layout { generation: 1, check: inputs(9) });
        assert!(!next_laid(&worker).provisional, "rien d'antérieur : calcul complet d'emblée");
        worker.send(Request::Layout { generation: 2, check: inputs(8) });
        let quick = next_laid(&worker);
        assert!(quick.provisional && quick.generation == 2);
        let full = next_laid(&worker);
        assert!(!full.provisional && full.generation == 2);
        let alone = nectar_typst::lay_out(
            &Engine::new(FontSources::Bundled),
            &inputs(8).document,
            &Default::default(),
            &Default::default(),
        );
        assert_eq!(full.choices, alone.choices, "même résultat que sans version provisoire");
    }
}
