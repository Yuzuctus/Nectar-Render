//! `nectar` : export en ligne de commande, et retouches à la main en
//! attendant l'atelier graphique.

mod mcp;

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use nectar_core::{Project, directives};
use nectar_typst::{Engine, FontSources, PdfOptions};

#[derive(Parser)]
#[command(name = "nectar", version, about = "Nectar Render : de la note Obsidian au PDF soigné.")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Exporte une note en PDF (et en PNG pour vérifier les pages).
    Export {
        note: PathBuf,
        /// Fichier PDF de sortie (défaut : à côté de la note).
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Écrit aussi chaque page en PNG dans ce dossier.
        #[arg(long)]
        png: Option<PathBuf>,
        /// Résolution des PNG.
        #[arg(long, default_value_t = 110.0)]
        ppi: f32,
        /// Utilise aussi les polices installées sur la machine.
        #[arg(long)]
        system_fonts: bool,
        /// Essaie un preset sans l'enregistrer.
        #[arg(long)]
        preset: Option<String>,
    },
    /// Vérifie la mise en page : pages à moitié vides, images réduites…
    Check { note: PathBuf },
    /// Liste les presets de style (intégrés et personnels).
    Presets,
    /// Choisit le preset de style d'une note (et efface ses réglages à la main).
    UsePreset { note: PathBuf, preset: String },
    /// Liste les blocs de la note, leur id et leur page.
    Blocks { note: PathBuf },
    /// Ajoute une retouche à un bloc : `nectar set note.md li-1a2b3c4d break-before`.
    Set {
        note: PathBuf,
        block: String,
        /// Retouches au format des commentaires `<!-- nectar: … -->`.
        #[arg(required = true, num_args = 1..)]
        ops: Vec<String>,
    },
    /// Retire toutes les retouches d'un bloc.
    Unset { note: PathBuf, block: String },
    /// Affiche la source Typst générée.
    Typst { note: PathBuf },
    /// Serveur MCP (sur l'entrée et la sortie standard) : une IA peut lire,
    /// vérifier, retoucher et exporter les notes. `nectar mcp install` le
    /// branche à Claude Desktop.
    Mcp {
        #[command(subcommand)]
        action: Option<McpAction>,
    },
}

#[derive(Subcommand)]
enum McpAction {
    /// Branche Nectar à Claude Desktop (et donne la commande pour Claude Code).
    Install,
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Export { note, output, png, ppi, system_fonts, preset } => {
            export(&note, output, png, ppi, system_fonts, preset)
        }
        Command::Check { note } => check(&note),
        Command::Presets => {
            let store = nectar_core::PresetStore::load(nectar_core::style::default_user_dir());
            for preset in store.presets() {
                let kind = if preset.builtin { "intégré" } else { "personnel" };
                println!("{:<14} {:<14} {kind}", preset.id, preset.label);
            }
            Ok(())
        }
        Command::UsePreset { note, preset } => {
            let mut project = open(&note)?;
            if project.presets.get(&preset).is_none() {
                bail!("preset « {preset} » inconnu (voir `nectar presets`)");
            }
            project.layout.style = nectar_core::StyleRef { preset, overrides: Default::default() };
            project.save_layout()?;
            Ok(())
        }
        Command::Blocks { note } => blocks(&note),
        Command::Set { note, block, ops } => set(&note, &block, &ops.join(",")),
        Command::Unset { note, block } => unset(&note, &block),
        Command::Mcp { action: None } => mcp::serve(),
        Command::Mcp { action: Some(McpAction::Install) } => mcp_install(),
        Command::Typst { note } => {
            let project = open(&note)?;
            let engine = Engine::new(FontSources::Bundled).with_cache_dir(nectar_core::style::default_cache_dir());
            print!("{}", lay_out(&project, &engine).0.source);
            Ok(())
        }
    }
}

/// Mise en page en deux temps (voir `nectar_typst::lay_out`).
fn lay_out(project: &Project, engine: &Engine) -> (nectar_core::Generated, Result<nectar_typst::Compiled>) {
    let (style, warning) = project.style();
    let laid = nectar_typst::lay_out(engine, &project.document, &project.layout, &style);
    let mut generated = laid.generated;
    generated.warnings.extend(warning);
    (generated, laid.compiled.map_err(Into::into))
}

fn mcp_install() -> Result<()> {
    let nectar = std::env::current_exe().context("chemin de nectar introuvable")?;
    match nectar_core::connect::connect_claude_desktop(&nectar) {
        Ok(files) => {
            for file in files {
                println!("✓ Claude Desktop : Nectar ajouté à {}", file.display());
            }
            println!("  Quittez complètement Claude Desktop (icône près de l'horloge → Quitter) puis rouvrez-le.");
        }
        Err(error) => println!("✗ Claude Desktop : {error}"),
    }
    println!();
    println!("Claude Code : lancez une fois cette commande dans un terminal :");
    println!("  {}", nectar_core::connect::claude_code_command(&nectar));
    println!();
    println!("Ensuite, demandez par exemple : « Mets en page ma note C:\\…\\TP WIFI.md et exporte le PDF ».");
    Ok(())
}

fn open(note: &Path) -> Result<Project> {
    let project = Project::open(note).with_context(|| format!("ouverture de {}", note.display()))?;
    for warning in &project.document.warnings {
        eprintln!("note : {warning}");
    }
    Ok(project)
}

fn export(
    note: &Path,
    output: Option<PathBuf>,
    png: Option<PathBuf>,
    ppi: f32,
    system_fonts: bool,
    preset: Option<String>,
) -> Result<()> {
    let started = Instant::now();
    let mut project = open(note)?;
    if let Some(preset) = preset {
        project.layout.style.preset = preset;
    }
    let engine = Engine::new(if system_fonts { FontSources::WithSystem } else { FontSources::Bundled })
        .with_cache_dir(nectar_core::style::default_cache_dir());
    let (generated, compiled) = lay_out(&project, &engine);
    for warning in &generated.warnings {
        eprintln!("avertissement : {warning}");
    }
    for font in engine.missing_fonts(&generated.fonts) {
        eprintln!("police absente : {font} (remplacée par une police de secours)");
    }
    let compiled = compiled?;
    for warning in &compiled.warnings {
        eprintln!("typst : {warning}");
    }

    let output = output.unwrap_or_else(|| note.with_extension("pdf"));
    let pdf_a = project.style().0.export.pdf_a;
    let pdf = compiled.pdf(&PdfOptions { ident: Some(project.note.display().to_string()), pdf_a })?;
    std::fs::write(&output, pdf).with_context(|| format!("écriture de {}", output.display()))?;

    if let Some(dir) = png {
        std::fs::create_dir_all(&dir)?;
        for page in 0..compiled.page_count() {
            let path = dir.join(format!("page-{:03}.png", page + 1));
            std::fs::write(&path, compiled.png(page, ppi)?)?;
        }
    }
    eprintln!(
        "{} → {} ({} pages, {} ms)",
        note.display(),
        output.display(),
        compiled.page_count(),
        started.elapsed().as_millis()
    );
    Ok(())
}

fn check(note: &Path) -> Result<()> {
    let project = open(note)?;
    let engine = Engine::new(FontSources::Bundled).with_cache_dir(nectar_core::style::default_cache_dir());
    let (style, warning) = project.style();
    let started = std::time::Instant::now();
    let laid = nectar_typst::lay_out(&engine, &project.document, &project.layout, &style);
    let millis = started.elapsed().as_millis();
    let mut generated = laid.generated;
    generated.warnings.extend(warning);
    let missing = engine.missing_fonts(&generated.fonts);
    let compiled = laid.compiled?;
    let issues = nectar_typst::inspect_tuned(
        &compiled,
        &project.document,
        &project.layout,
        &style,
        &generated,
        &missing,
        &laid.tuning,
    );
    println!("{} pages, mises en page en {millis} ms.", compiled.page_count());
    if !laid.choices.is_empty() {
        println!("Décidé automatiquement (« nectar set … tel-quel » pour refuser) :");
        let anchors = project.document.anchors();
        for choice in &laid.choices {
            let excerpt = anchors.iter().find(|a| a.id == &choice.block).map(|a| a.excerpt).unwrap_or("");
            println!(
                "  ✓ {} — {} « {} »",
                choice.describe(),
                choice.block,
                excerpt.chars().take(50).collect::<String>()
            );
        }
    }
    if issues.is_empty() {
        println!("Rien à signaler : les pages sont propres.");
    }
    for issue in issues {
        let mark = match issue.severity {
            nectar_core::assistant::Severity::Problem => "✗",
            nectar_core::assistant::Severity::Warning => "!",
            nectar_core::assistant::Severity::Info => "·",
        };
        println!("{mark} {}", issue.title);
        if !issue.detail.is_empty() {
            println!("    {}", issue.detail);
        }
        for fix in &issue.fixes {
            println!("    → {} (nectar set … {})", fix.label, fix.block);
        }
    }
    Ok(())
}

fn blocks(note: &Path) -> Result<()> {
    let project = open(note)?;
    let engine = Engine::new(FontSources::Bundled).with_cache_dir(nectar_core::style::default_cache_dir());
    let (positions, sizes) = match lay_out(&project, &engine).1 {
        Ok(compiled) => {
            let sizes: Vec<(f64, f64)> = (0..compiled.page_count()).filter_map(|i| compiled.page_size(i)).collect();
            (compiled.block_positions(), sizes)
        }
        Err(error) => {
            eprintln!("{error}");
            (Vec::new(), Vec::new())
        }
    };
    let resolution = project.layout.resolve(&project.document);
    println!("{:<22} {:<11} {:>5} {:>5}  extrait", "id", "type", "ligne", "page");
    for anchor in project.document.anchors() {
        let page = positions
            .iter()
            .find(|p| &p.id == anchor.id)
            .map(|p| (p.page + 1).to_string())
            .unwrap_or_else(|| "–".into());
        let marked = if resolution.ops.contains_key(anchor.id) { "*" } else { " " };
        println!(
            "{:<22} {:<11} {:>5} {:>5} {marked}{}",
            anchor.id,
            anchor.kind.label_fr(),
            anchor.line,
            page,
            anchor.excerpt
        );
    }
    let formats: Vec<String> =
        sizes.iter().enumerate().map(|(i, (w, h))| format!("{} {}", i + 1, page_name(*w, *h))).collect();
    println!("\npages : {}", formats.join(" · "));
    Ok(())
}

/// Nom d'un format de page d'après sa taille en points.
fn page_name(width: f64, height: f64) -> String {
    const PAPERS: &[(&str, f64, f64)] =
        &[("A4", 595.3, 841.9), ("A3", 841.9, 1190.6), ("A5", 419.5, 595.3), ("Letter", 612.0, 792.0)];
    let (short, long) = (width.min(height), width.max(height));
    let name = PAPERS
        .iter()
        .find(|(_, w, h)| (w - short).abs() < 2.0 && (h - long).abs() < 2.0)
        .map(|(n, ..)| (*n).to_string())
        .unwrap_or_else(|| format!("{:.0}×{:.0} mm", width * 25.4 / 72.0, height * 25.4 / 72.0));
    if width > height { format!("{name} paysage") } else { name }
}

fn set(note: &Path, block: &str, ops: &str) -> Result<()> {
    let mut project = open(note)?;
    let ops = directives::parse_ops(ops).map_err(anyhow::Error::msg)?;
    let anchors = project.document.anchors();
    let Some(anchor) = anchors.iter().find(|a| a.id.as_str() == block).copied() else {
        bail!("aucun bloc « {block} » (voir `nectar blocks`)");
    };
    let id = anchor.id.clone();
    let mut layout = project.layout.clone();
    layout.ops_mut(anchor).merge(&ops);
    project.layout = layout;
    project.save_layout()?;
    eprintln!("retouche enregistrée sur {id} dans {}", project.layout_path.display());
    Ok(())
}

fn unset(note: &Path, block: &str) -> Result<()> {
    let mut project = open(note)?;
    let before = project.layout.blocks.len();
    project.layout.blocks.retain(|d| d.anchor.id.as_str() != block);
    if project.layout.blocks.len() == before {
        bail!("aucune retouche sur « {block} »");
    }
    project.save_layout()?;
    eprintln!("retouches de {block} retirées");
    Ok(())
}
