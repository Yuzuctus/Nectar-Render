//! Notes tirées au hasard : la mise en page doit toujours tenir ses
//! promesses (pas de page blanche, rien hors des marges, légende avec son
//! image, retouches respectées, même résultat d'un calcul à l'autre).
//!
//! `FUZZ_N=300 cargo test --profile essai -p nectar-typst --test fuzz --
//! --nocapture` pour un long passage ; `FUZZ_OFFSET=…` pour d'autres notes ;
//! `FUZZ_SEED=…` pour rejouer une note (`FUZZ_PNG=1` : ses pages et sa
//! source dans le dossier temporaire).

use std::collections::HashSet;
use std::path::Path;

use nectar_core::layout::{PageChange, PageSpec};
use nectar_core::model::Node;
use nectar_core::{BlockId, Layout, ParseOptions, parse};
use nectar_typst::{Compiled, Engine, FontSources, lay_out};

/// Générateur pseudo-aléatoire (xorshift) : reproductible sans dépendance.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }
}

const WORDS: &[&str] = &[
    "le",
    "point",
    "d'accès",
    "configuration",
    "réseau",
    "client",
    "serveur",
    "adresse",
    "passerelle",
    "portail",
    "captif",
    "authentification",
    "débit",
    "mesure",
    "VLAN",
    "routeur",
    "OPNsense",
    "règle",
    "pare-feu",
    "journal",
    "est",
    "puis",
    "avec",
    "sans",
    "pour",
    "dans",
    "une",
    "des",
    "la",
    "et",
    "on",
    "vérifie",
    "applique",
    "observe",
];

fn sentence(rng: &mut Rng, words: usize) -> String {
    let mut s: Vec<&str> = (0..words.max(1)).map(|_| WORDS[rng.below(WORDS.len())]).collect();
    let first = s[0].to_string();
    let mut out = first[..1].to_uppercase() + &first[1..];
    s.remove(0);
    for w in s {
        out.push(' ');
        out.push_str(w);
    }
    out.push('.');
    out
}

fn paragraph(rng: &mut Rng) -> String {
    (0..1 + rng.below(6))
        .map(|_| {
            let n = 6 + rng.below(14);
            sentence(rng, n)
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Images de test : (fichier, largeur, hauteur) en pixels.
const IMAGES: &[(&str, u32, u32)] = &[
    ("photo-large.jpg", 3200, 1800),
    ("photo-43.jpg", 2000, 1500),
    ("photo-haute.jpg", 1500, 2000),
    ("capture-haute.png", 700, 1400),
    ("capture.png", 1600, 900),
    ("icone.png", 120, 90),
];

fn write_images(dir: &Path) {
    for (name, w, h) in IMAGES {
        image::RgbImage::from_pixel(*w, *h, image::Rgb([110, 140, 170])).save(dir.join(name)).unwrap();
    }
    let mut svg = String::from(r##"<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="800">"##);
    for i in 0..30 {
        svg.push_str(&format!(
            r##"<rect x="{}" y="{}" width="60" height="30" fill="none" stroke="#333"/>"##,
            i * 50,
            i * 25
        ));
    }
    svg.push_str("</svg>");
    std::fs::write(dir.join("schema-reseau.svg"), svg).unwrap();
    std::fs::write(
        dir.join("schema-vertical.svg"),
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="600" height="1300"><rect width="600" height="1300" fill="#cde"/></svg>"##,
    )
    .unwrap();
}

fn table(rng: &mut Rng) -> String {
    let columns = 2 + rng.below(8);
    let tall = rng.chance(20);
    let rows = 2 + rng.below(if tall { 40 } else { 12 });
    let long = rng.chance(40);
    let cell = |rng: &mut Rng| {
        if long && rng.chance(60) {
            {
                let n = 3 + rng.below(14);
                sentence(rng, n)
            }
            .trim_end_matches('.')
            .to_string()
        } else if rng.chance(30) {
            format!("192.168.{}.{}", rng.below(255), rng.below(255))
        } else {
            WORDS[rng.below(WORDS.len())].to_string()
        }
    };
    let mut out = String::from("|");
    for i in 0..columns {
        out.push_str(&format!(" Colonne {i} |"));
    }
    out.push_str("\n|");
    out.push_str(&"---|".repeat(columns));
    out.push('\n');
    for _ in 0..rows {
        out.push('|');
        for _ in 0..columns {
            out.push_str(&format!(" {} |", cell(rng)));
        }
        out.push('\n');
    }
    out
}

/// Une note au hasard.
fn note(rng: &mut Rng) -> String {
    let mut parts = vec![format!("---\ntitle: Note {}\n---\n", rng.below(1000))];
    let blocks = 6 + rng.below(40);
    for _ in 0..blocks {
        let part = match rng.below(14) {
            0 => format!(
                "# {}",
                {
                    let n = 2 + rng.below(4);
                    sentence(rng, n)
                }
                .trim_end_matches('.')
            ),
            1 | 2 => format!(
                "## {}",
                {
                    let n = 2 + rng.below(4);
                    sentence(rng, n)
                }
                .trim_end_matches('.')
            ),
            3..=5 => paragraph(rng),
            6 => {
                let items: Vec<String> = (0..2 + rng.below(10))
                    .map(|_| {
                        format!("- {}", {
                            let n = 3 + rng.below(10);
                            sentence(rng, n)
                        })
                    })
                    .collect();
                format!("Les étapes :\n\n{}", items.join("\n"))
            }
            7 => {
                let lines: Vec<String> = (0..3 + rng.below(35)).map(|i| format!("ping -c {i} 192.168.1.{i}")).collect();
                format!("```bash\n{}\n```", lines.join("\n"))
            }
            8 | 9 => {
                let intro = if rng.chance(40) { "Le tableau suivant récapitule :\n\n" } else { "" };
                format!("{intro}{}", table(rng))
            }
            10..=12 => {
                let (file, _, _) = IMAGES[rng.below(IMAGES.len())];
                let file = if rng.chance(15) {
                    if rng.chance(50) { "schema-reseau.svg" } else { "schema-vertical.svg" }
                } else {
                    file
                };
                let mut s = format!("![[{file}]]");
                if rng.chance(60) {
                    s.push_str(&format!("\n\n*Capture : {}*", {
                        let n = 4 + rng.below(20);
                        sentence(rng, n)
                    }));
                }
                s
            }
            _ => format!("> [!note] Remarque\n> {}", {
                let n = 5 + rng.below(25);
                sentence(rng, n)
            }),
        };
        parts.push(part);
    }
    parts.join("\n\n") + "\n"
}

/// Retouches au hasard, comme une personne en ferait.
fn retouches(rng: &mut Rng, doc: &nectar_core::Document) -> Layout {
    let mut layout = Layout::default();
    let anchors = doc.anchors();
    for _ in 0..rng.below(4) {
        let anchor = anchors[rng.below(anchors.len())];
        let ops = layout.ops_mut(anchor);
        match rng.below(7) {
            0 => ops.break_before = true,
            6 => ops.hidden = true,
            1 => ops.page = Some(PageChange::Set(PageSpec::paper("a3", rng.chance(50)))),
            2 => ops.manual = true,
            3 => ops.keep_together = Some(true),
            4 => ops.space_before_mm = Some(rng.below(40) as f32 - 10.0),
            _ => ops.break_after = true,
        }
    }
    layout
}

struct Report {
    failures: Vec<String>,
    /// Remarques restantes de l'assistant, par sorte.
    remarks: std::collections::BTreeMap<String, usize>,
    pages: usize,
    millis: u128,
}

impl Report {
    fn check(&mut self, ok: bool, what: impl FnOnce() -> String) {
        if !ok {
            self.failures.push(what());
        }
    }
}

fn verify(seed: u64, engine: &Engine, dir: &Path, report: &mut Report) {
    let mut rng = Rng(seed);
    let text = note(&mut rng);
    let options = ParseOptions { note_dir: Some(dir), ..Default::default() };
    let doc = parse(&text, &options);
    let mut layout = if rng.chance(50) { retouches(&mut rng, &doc) } else { Layout::default() };
    // Un document A4 le plus souvent, parfois un autre format.
    layout.page = match rng.below(10) {
        0 => PageSpec::paper("a5", false),
        1 => PageSpec::paper("us-letter", false),
        2 => PageSpec::paper("a4", true),
        _ => PageSpec::default(),
    };
    let presets = nectar_core::style::PresetStore::builtin_only();
    let ids: Vec<String> = presets.presets().iter().map(|p| p.id.clone()).collect();
    let reference = nectar_core::style::StyleRef { preset: ids[rng.below(ids.len())].clone(), ..Default::default() };
    let (style, _) = presets.resolve(&reference);
    let started = std::time::Instant::now();
    let laid = lay_out(engine, &doc, &layout, &style);
    let millis = started.elapsed().as_millis();
    let tag = |what: &str| format!("graine {seed} : {what}");
    let compiled = match laid.compiled {
        Ok(c) => c,
        Err(e) => {
            report.failures.push(tag(&format!("compilation impossible : {e}")));
            return;
        }
    };
    report.check(millis < 8000, || tag(&format!("mise en page lente : {millis} ms")));
    report.pages += compiled.page_count();
    for choice in &laid.choices {
        *report
            .remarks
            .entry(format!("décision : {:?}", choice.kind).split('(').next().unwrap_or("").to_string())
            .or_default() += 1;
    }
    report.millis += millis;

    // Le placement automatique n'ajoute pas plus de pages que de pages paysage.
    let mut manual_style = style.clone();
    manual_style.pagination.auto_landscape = false;
    manual_style.pagination.fit_images = false;
    manual_style.pagination.avoid_short_last_page = false;
    manual_style.pagination.larger_paper = false;
    manual_style.pagination.lonely_landscape = false;
    if let Ok(plain) = lay_out(engine, &doc, &layout, &manual_style).compiled {
        let landscapes = laid.tuning.landscape.len();
        // Une page paysage coûte au plus deux pages : la sienne, et la page
        // d'avant qu'elle interrompt.
        report.check(compiled.page_count() <= plain.page_count() + 2 * landscapes, || {
            tag(&format!(
                "{} pages avec le placement automatique, {} sans ({landscapes} pages paysage)",
                compiled.page_count(),
                plain.page_count()
            ))
        });
    } else {
        report.failures.push(tag("compilation impossible sans placement automatique"));
    }

    // Même résultat d'un calcul à l'autre.
    let again = lay_out(engine, &doc, &layout, &style);
    report.check(again.generated.source == laid.generated.source, || tag("deux calculs, deux résultats"));

    let mm = 72.0 / 25.4;
    let (top, bottom) = (f64::from(style.page.margin_top_mm) * mm, f64::from(style.page.margin_bottom_mm) * mm);
    let right_margin = f64::from(style.page.margin_right_mm) * mm;
    let boxes = compiled.block_boxes(bottom);
    let positions = compiled.block_positions();

    // Pas de page blanche.
    for page in 0..compiled.page_count() {
        let used = boxes.iter().any(|b| b.page == page && b.rect[3] > top - 4.0);
        report.check(used || page == 0, || tag(&format!("page {} blanche", page + 1)));
    }

    // Rien hors de la marge de droite (au-delà de quelques points).
    for b in &boxes {
        let width = compiled.page_size(b.page).map(|(w, _)| w).unwrap_or(595.0);
        report.check(b.rect[2] <= width - right_margin + 4.0, || {
            tag(&format!(
                "{} dépasse la marge page {} ({:.0} > {:.0})",
                b.id,
                b.page + 1,
                b.rect[2],
                width - right_margin
            ))
        });
    }

    // Une légende reste sur la page de son image.
    let resolved = layout.resolve(&doc).ops;
    for (i, block) in doc.blocks.iter().enumerate() {
        let Some(next) = doc.blocks.get(i + 1) else { continue };
        if !matches!(block.node, Node::Figure(_)) || !nectar_core::codegen::is_caption(&next.node) {
            continue;
        }
        if resolved.get(&next.id).is_some_and(|o| o.break_before || o.page.is_some() || o.hidden)
            || resolved.get(&block.id).is_some_and(|o| o.break_after || o.hidden)
        {
            continue;
        }
        let last_image_page = boxes.iter().filter(|b| b.id == block.id).map(|b| b.page).max();
        let caption_page = positions.iter().find(|p| p.id == next.id).map(|p| p.page);
        report.check(last_image_page.is_none() || last_image_page == caption_page, || {
            tag(&format!("légende de {} page {:?}, image page {:?}", block.id, caption_page, last_image_page))
        });
    }

    // Un titre n'est jamais seul en bas de page : au moins deux lignes de
    // son contenu le suivent, ou il passe page suivante.
    let metrics = compiled.page_metrics(bottom);
    let line = f64::from(style.text.size_pt * style.text.line_height);
    for (i, block) in doc.blocks.iter().enumerate() {
        if !matches!(block.node, Node::Heading { .. }) || resolved.get(&block.id).is_some_and(|o| o.manual || o.hidden)
        {
            continue;
        }
        let Some(next) = doc.blocks.get(i + 1) else { continue };
        if matches!(next.node, Node::Heading { .. }) {
            continue;
        }
        let Some(page) = positions.iter().find(|p| p.id == block.id).map(|p| p.page) else { continue };
        let continues = boxes.iter().any(|b| b.id == next.id && b.page > page);
        let heading_bottom =
            boxes.iter().filter(|b| b.id == block.id && b.page == page).map(|b| b.rect[3]).fold(0.0, f64::max);
        let content_bottom = metrics.get(page).and_then(|m| m.content).map(|c| c[3]).unwrap_or(0.0);
        let first = positions.iter().find(|p| p.page == page).is_some_and(|p| p.id == block.id);
        report.check(first || !continues || content_bottom - heading_bottom >= 2.0 * line - 1.0, || {
            tag(&format!("titre {} seul en bas de la page {}", block.id, page + 1))
        });
    }

    // Un saut de page demandé est respecté : le bloc ouvre sa page.
    let first_on_page: HashSet<&BlockId> = (0..compiled.page_count())
        .filter_map(|page| positions.iter().find(|p| p.page == page).map(|p| &p.id))
        .collect();
    for directive in &layout.blocks {
        if !directive.ops.break_before || directive.ops.hidden {
            continue;
        }
        let mut id = &directive.anchor.id;
        // Un saut avant la première puce passe avant toute la liste.
        for block in &doc.blocks {
            if let Node::List(list) = &block.node
                && list.items.first().and_then(|i| i.id.as_ref()) == Some(id)
            {
                id = &block.id;
            }
        }
        // Le saut remonte devant les titres qui précèdent le bloc.
        let index = doc.blocks.iter().position(|b| &b.id == id);
        let mut target = id;
        if let Some(mut i) = index {
            while i > 0
                && matches!(doc.blocks[i - 1].node, Node::Heading { .. })
                && resolved.get(&doc.blocks[i - 1].id).is_none_or(|o| !o.break_before && o.page.is_none())
            {
                i -= 1;
            }
            target = &doc.blocks[i].id;
        }
        if positions.iter().any(|p| &p.id == target) {
            report.check(first_on_page.contains(target), || tag(&format!("saut avant {target} non respecté")));
        }
    }

    // Un format de page demandé s'applique à la page du bloc.
    for directive in &layout.blocks {
        let (Some(PageChange::Set(spec)), false) = (&directive.ops.page, directive.ops.hidden) else { continue };
        let Some(position) = positions.iter().find(|p| p.id == directive.anchor.id) else { continue };
        let Some((w, h)) = compiled.page_size(position.page) else { continue };
        let (ew, eh) = spec.size_mm();
        let (ew, eh) = (f64::from(ew) * mm, f64::from(eh) * mm);
        report.check((w - ew).abs() < 3.0 && (h - eh).abs() < 3.0, || {
            tag(&format!(
                "format demandé pour {} non appliqué : {w:.0}×{h:.0} au lieu de {ew:.0}×{eh:.0}",
                directive.anchor.id
            ))
        });
    }

    // « Tel quel » : aucune décision automatique sur le bloc.
    for directive in layout.blocks.iter().filter(|d| d.ops.manual) {
        let id = &directive.anchor.id;
        report.check(!laid.choices.iter().any(|c| &c.block == id), || tag(&format!("décision sur {id} « tel quel »")));
    }

    // Les défauts graves que l'assistant verrait encore.
    let issues = nectar_typst::inspect_tuned(&compiled, &doc, &layout, &style, &laid.generated, &[], &laid.tuning);
    for issue in &issues {
        if issue.severity == nectar_core::assistant::Severity::Problem {
            report.failures.push(tag(&format!("assistant : {}", issue.title)));
        }
        // Le placement automatique fait lui-même ce que l'assistant proposerait.
        if issue.title.starts_with("Schéma à lire en grand")
            && issue.block.as_ref().is_some_and(|b| nectar_core::auto::allowed(resolved.get(b)))
        {
            report.failures.push(tag(&format!("assistant propose encore : {}", issue.title)));
        }
        let kind: String = issue.title.chars().filter(|c| !c.is_ascii_digit()).collect();
        *report.remarks.entry(kind).or_default() += 1;
    }
    if std::env::var("FUZZ_PNG").is_ok() {
        dump(&compiled, seed);
        let dir = std::env::temp_dir().join(format!("fuzz-{seed}"));
        let _ = std::fs::write(dir.join("source.typ"), &laid.generated.source);
        let _ = std::fs::write(dir.join("note.md"), &text);
        let _ = std::fs::write(dir.join("layout.txt"), format!("{:#?}", layout.blocks));
        let _ = std::fs::write(dir.join("choices.txt"), format!("{:#?}", laid.choices));
    }
}

fn dump(compiled: &Compiled, seed: u64) {
    let dir = std::env::temp_dir().join(format!("fuzz-{seed}"));
    let _ = std::fs::create_dir_all(&dir);
    for page in 0..compiled.page_count() {
        let _ = std::fs::write(dir.join(format!("page-{:03}.png", page + 1)), compiled.png(page, 30.0).unwrap());
    }
}

#[test]
fn random_notes_keep_their_promises() {
    let dir = tempfile::tempdir().unwrap();
    write_images(dir.path());
    let engine = Engine::new(FontSources::Bundled);
    let count: u64 = std::env::var("FUZZ_N").ok().and_then(|n| n.parse().ok()).unwrap_or(6);
    let seeds: Vec<u64> = match std::env::var("FUZZ_SEED").ok().and_then(|s| s.parse().ok()) {
        Some(seed) => vec![seed],
        None => {
            let offset: u64 = std::env::var("FUZZ_OFFSET").ok().and_then(|n| n.parse().ok()).unwrap_or(0);
            (1..=count).map(|i| i * 7919 + 13 + offset).collect()
        }
    };
    let mut report = Report { failures: Vec::new(), remarks: Default::default(), pages: 0, millis: 0 };
    let notes = seeds.len();
    for seed in seeds {
        verify(seed, &engine, dir.path(), &mut report);
    }
    eprintln!("{notes} notes, {} pages, {} ms de mise en page en tout", report.pages, report.millis);
    for (kind, count) in &report.remarks {
        eprintln!("  {count:>4} × {kind}");
    }
    assert!(report.failures.is_empty(), "{} écarts :\n{}", report.failures.len(), report.failures.join("\n"));
}
