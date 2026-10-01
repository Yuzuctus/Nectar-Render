//! `nectar` : export en ligne de commande, et retouches à la main en
//! attendant l'atelier graphique.

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
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Export { note, output, png, ppi, system_fonts, preset } => {
            export(&note, output, png, ppi, system_fonts, preset)
        }
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
        Command::Typst { note } => {
            let project = open(&note)?;
            print!("{}", project.generate().source);
            Ok(())
        }
    }
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
    let generated = project.generate();
    for warning in &generated.warnings {
        eprintln!("retouches : {warning}");
    }
    let engine = Engine::new(if system_fonts { FontSources::WithSystem } else { FontSources::Bundled });
    for font in engine.missing_fonts(&generated.fonts) {
        eprintln!("police absente : {font} (remplacée par une police de secours)");
    }
    let compiled = engine.compile(&generated)?;
    for warning in &compiled.warnings {
        eprintln!("typst : {warning}");
    }

    let output = output.unwrap_or_else(|| note.with_extension("pdf"));
    let pdf = compiled.pdf(&PdfOptions { ident: Some(project.note.display().to_string()) })?;
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

fn blocks(note: &Path) -> Result<()> {
    let project = open(note)?;
    let generated = project.generate();
    let engine = Engine::new(FontSources::Bundled);
    let positions = match engine.compile(&generated) {
        Ok(compiled) => compiled.block_positions(),
        Err(error) => {
            eprintln!("{error}");
            Vec::new()
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
    Ok(())
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
