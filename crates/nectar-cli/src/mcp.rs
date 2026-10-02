//! `nectar mcp` : un serveur MCP (Model Context Protocol) sur l'entrée et la
//! sortie standard, pour qu'une IA (Claude Desktop, Claude Code…) lise une
//! note, voie ses pages, la vérifie, la retouche et règle son style.
//!
//! Les retouches sont écrites dans le même fichier que celui de l'atelier
//! (`.nectar/…json`) : si l'atelier est ouvert, il les montre aussitôt.

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use base64::Engine as _;
use nectar_core::assistant::{FixAction, Severity};
use nectar_core::layout::{DefaultPage, PageChange, PageSpec, Placement};
use nectar_core::{Project, directives};
use nectar_typst::{Compiled, Engine, FontSources, PdfOptions};
use serde_json::{Value, json};

const PROTOCOL: &str = "2025-06-18";

const INSTRUCTIONS: &str = "Nectar Render met en page des notes Markdown (Obsidian) en PDF soignés. \
Démarche conseillée : read_note pour connaître les blocs (id, type, page) et les pages ; check_layout pour \
les défauts repérés ; render_page pour voir une page ; puis set_block / set_page_format / set_style pour \
retoucher, et de nouveau check_layout ou render_page pour vérifier. La note elle-même n'est jamais \
modifiée : les retouches sont rangées à côté (dossier .nectar), et l'atelier Nectar Render les affiche en direct. \
Le placement automatique décide déjà seul des cas courants (tableaux et schémas larges en paysage, images un peu \
réduites pour ne pas laisser de trou, dernière page résorbée) : check_layout les liste dans automatic_decisions. \
Ne retouchez que ce qui reste à revoir ; « tel-quel » sur un bloc refuse les décisions automatiques.";

const DIRECTIVES: &str = "Retouches séparées par des virgules : saut-avant (le bloc et la suite passent page \
suivante), saut-apres (reste de la page vide après), garder-avec-suivant, insecable (jamais coupé), secable \
(coupure autorisée), bas-de-page, masquer, espace-avant=MM (négatif pour rapprocher), page=a3 | a3-paysage | \
a4-paysage | defaut (format de la page du bloc), suite (le format vaut aussi pour les pages suivantes), \
largeur=60 (image, % de la largeur du texte), alignement=gauche|centre|droite, placement=texte|haut|bas|\
pleine-page|paysage, legende=\"…\", centre, droite, colonnes=2, taille=120 (texte en %), tel-quel (le placement \
automatique ne touche pas à ce bloc).";

/// Le serveur : un moteur partagé entre les appels (polices, photos en cache).
#[derive(Default)]
struct Server {
    engine: Option<Engine>,
}

pub fn serve() -> Result<()> {
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    let mut server = Server::default();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(message) => server.handle(&message),
            Err(e) => Some(json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": e.to_string()}})),
        };
        if let Some(reply) = reply {
            writeln!(out, "{reply}")?;
            out.flush()?;
        }
    }
    Ok(())
}

impl Server {
    fn handle(&mut self, message: &Value) -> Option<Value> {
        let id = message.get("id").cloned();
        let method = message.get("method").and_then(Value::as_str).unwrap_or_default();
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        // Une notification n'attend pas de réponse.
        let id = id?;
        let result = match method {
            "initialize" => Ok(json!({
                "protocolVersion": params.get("protocolVersion").and_then(Value::as_str).unwrap_or(PROTOCOL),
                "capabilities": {"tools": {"listChanged": false}},
                "serverInfo": {"name": "nectar-render", "version": env!("CARGO_PKG_VERSION")},
                "instructions": INSTRUCTIONS,
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({ "tools": tools() })),
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                Ok(match self.call(name, &args) {
                    Ok(content) => json!({ "content": content }),
                    Err(e) => json!({ "content": [text(&format!("Erreur : {e:#}"))], "isError": true }),
                })
            }
            _ => Err(json!({"code": -32601, "message": format!("méthode inconnue : {method}")})),
        };
        Some(match result {
            Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
            Err(error) => json!({"jsonrpc": "2.0", "id": id, "error": error}),
        })
    }

    fn engine(&mut self) -> &Engine {
        self.engine.get_or_insert_with(|| {
            Engine::new(FontSources::WithSystem).with_cache_dir(nectar_core::style::default_cache_dir())
        })
    }

    fn call(&mut self, name: &str, args: &Value) -> Result<Vec<Value>> {
        match name {
            "list_presets" => list_presets(),
            "read_note" => self.read_note(args),
            "check_layout" => self.check_layout(args),
            "render_page" => self.render_page(args),
            "set_block" => set_block(args),
            "set_page_format" => self.set_page_format(args),
            "get_style" => get_style(args),
            "set_style" => set_style(args),
            "export_pdf" => self.export_pdf(args),
            _ => bail!("outil inconnu : {name}"),
        }
    }

    /// Met en page la note comme l'atelier (placement automatique en deux temps).
    fn lay_out(&mut self, project: &Project) -> Result<Compiled> {
        let (style, _) = project.style();
        let laid = nectar_typst::lay_out(self.engine(), &project.document, &project.layout, &style);
        laid.compiled.map_err(|e| anyhow!("{e}"))
    }

    fn read_note(&mut self, args: &Value) -> Result<Vec<Value>> {
        let project = open(args)?;
        let compiled = self.lay_out(&project)?;
        let positions = compiled.block_positions();
        let resolved = project.layout.resolve(&project.document).ops;
        let pages: Vec<Value> = (0..compiled.page_count())
            .filter_map(|i| compiled.page_size(i).map(|(w, h)| json!({"page": i + 1, "format": page_name(w, h)})))
            .collect();
        let blocks: Vec<Value> = project
            .document
            .anchors()
            .into_iter()
            .map(|a| {
                let mut block = json!({
                    "id": a.id.as_str(),
                    "type": a.kind.label_fr(),
                    "line": a.line,
                    "page": positions.iter().find(|p| &p.id == a.id).map(|p| p.page + 1),
                    "excerpt": a.excerpt,
                });
                if let Some(ops) = resolved.get(a.id) {
                    block["retouches"] = serde_json::to_value(ops).unwrap_or(Value::Null);
                }
                block
            })
            .collect();
        let summary = json!({
            "note": project.note.display().to_string(),
            "title": project.document.meta.title,
            "document_format": format!("{}{}", project.layout.page.paper, if project.layout.page.landscape { " paysage" } else { "" }),
            "preset": project.layout.style.preset,
            "style_overrides": project.layout.style.overrides,
            "pages": pages,
            "blocks": blocks,
            "warnings": project.document.warnings,
        });
        Ok(vec![text(&serde_json::to_string_pretty(&summary)?)])
    }

    fn check_layout(&mut self, args: &Value) -> Result<Vec<Value>> {
        let project = open(args)?;
        let (style, _) = project.style();
        let laid = nectar_typst::lay_out(self.engine(), &project.document, &project.layout, &style);
        let compiled = laid.compiled.map_err(|e| anyhow!("{e}"))?;
        let missing = self.engine().missing_fonts(&laid.generated.fonts);
        let issues = nectar_typst::inspect_tuned(
            &compiled,
            &project.document,
            &project.layout,
            &style,
            &laid.generated,
            &missing,
            &laid.tuning,
        );
        let automatic: Vec<Value> = laid
            .choices
            .iter()
            .map(|c| json!({ "block": c.block.as_str(), "decision": c.describe(), "refuse_with": "as-is" }))
            .collect();
        let list: Vec<Value> = issues
            .iter()
            .map(|issue| {
                json!({
                    "severity": match issue.severity {
                        Severity::Problem => "problème",
                        Severity::Warning => "à revoir",
                        Severity::Info => "info",
                    },
                    "page": issue.page.map(|p| p + 1),
                    "block": issue.block.as_ref().map(|b| b.as_str()),
                    "title": issue.title,
                    "detail": issue.detail,
                    "fixes": issue.fixes.iter().map(|f| json!({
                        "label": f.label,
                        "block": f.block.as_str(),
                        "directives": fix_directives(&f.action),
                    })).collect::<Vec<_>>(),
                })
            })
            .collect();
        let report = json!({ "pages": compiled.page_count(), "automatic_decisions": automatic, "issues": list });
        Ok(vec![text(&serde_json::to_string_pretty(&report)?)])
    }

    fn render_page(&mut self, args: &Value) -> Result<Vec<Value>> {
        let project = open(args)?;
        let page = page_arg(args)?;
        let ppi = args.get("ppi").and_then(Value::as_f64).unwrap_or(80.0).clamp(30.0, 200.0) as f32;
        let compiled = self.lay_out(&project)?;
        if page >= compiled.page_count() {
            bail!("la note n'a que {} pages", compiled.page_count());
        }
        let png = compiled.png(page, ppi).map_err(|e| anyhow!("{e}"))?;
        let (w, h) = compiled.page_size(page).unwrap_or((0.0, 0.0));
        Ok(vec![
            text(&format!("Page {} sur {} · {}", page + 1, compiled.page_count(), page_name(w, h))),
            json!({"type": "image", "mimeType": "image/png", "data": base64::engine::general_purpose::STANDARD.encode(png)}),
        ])
    }

    fn set_page_format(&mut self, args: &Value) -> Result<Vec<Value>> {
        let mut project = open(args)?;
        let page = page_arg(args)?;
        let format = args.get("format").and_then(Value::as_str).context("« format » manquant")?.to_lowercase();
        let onward = args.get("onward").and_then(Value::as_bool).unwrap_or(false);
        let compiled = self.lay_out(&project)?;
        let positions = compiled.block_positions();
        let resolved = project.layout.resolve(&project.document).ops;
        let on_page: Vec<_> = positions.iter().filter(|p| p.page == page).map(|p| p.id.clone()).collect();
        if on_page.is_empty() {
            bail!("aucun bloc ne commence sur la page {}", page + 1);
        }
        // Une image en page paysage porte le format de sa page ; sinon le bloc
        // qui a déjà changé le format ; sinon le premier bloc de la page.
        let landscape = on_page.iter().find(|id| {
            resolved.get(*id).and_then(|o| o.image.as_ref()).is_some_and(|i| i.placement == Placement::Landscape)
        });
        let owner = landscape
            .or_else(|| on_page.iter().find(|id| resolved.get(*id).is_some_and(|o| o.page.is_some())))
            .unwrap_or(&on_page[0])
            .clone();
        let anchors = project.document.anchors();
        let anchor = anchors.iter().find(|a| *a.id == owner).copied().context("bloc introuvable")?;
        let ops = project.layout.ops_mut(anchor);
        if let Some(image) = &mut ops.image
            && image.placement == Placement::Landscape
        {
            image.placement = Placement::Inline;
        }
        if matches!(format.as_str(), "normal" | "defaut" | "défaut" | "default") {
            ops.page = match ops.page {
                Some(PageChange::Set(_)) => None,
                _ => Some(PageChange::Default(DefaultPage::Default)),
            };
            ops.page_onward = false;
        } else {
            let spec = PageSpec::parse(&format).with_context(|| format!("format inconnu : {format}"))?;
            ops.page = Some(PageChange::Set(spec));
            ops.page_onward = onward;
        }
        project.save_layout()?;
        Ok(vec![text(&format!(
            "Page {} : format « {format} »{} (porté par le bloc {owner}).",
            page + 1,
            if onward { ", et les pages suivantes" } else { ", cette page seulement" }
        ))])
    }

    fn export_pdf(&mut self, args: &Value) -> Result<Vec<Value>> {
        let project = open(args)?;
        let output = args
            .get("output")
            .and_then(Value::as_str)
            .map(PathBuf::from)
            .unwrap_or_else(|| project.note.with_extension("pdf"));
        let compiled = self.lay_out(&project)?;
        let pdf_a = project.style().0.export.pdf_a;
        let pdf = compiled
            .pdf(&PdfOptions { ident: Some(project.note.display().to_string()), pdf_a })
            .map_err(|e| anyhow!("{e}"))?;
        std::fs::write(&output, pdf).with_context(|| format!("écriture de {}", output.display()))?;
        Ok(vec![text(&format!("PDF exporté : {} ({} pages)", output.display(), compiled.page_count()))])
    }
}

fn list_presets() -> Result<Vec<Value>> {
    let store = nectar_core::PresetStore::load(nectar_core::style::default_user_dir());
    let list: Vec<Value> =
        store.presets().iter().map(|p| json!({"id": p.id, "label": p.label, "builtin": p.builtin})).collect();
    Ok(vec![text(&serde_json::to_string_pretty(&list)?)])
}

fn set_block(args: &Value) -> Result<Vec<Value>> {
    let mut project = open(args)?;
    let block = args.get("block_id").and_then(Value::as_str).context("« block_id » manquant")?;
    let anchors = project.document.anchors();
    let anchor = anchors
        .iter()
        .find(|a| a.id.as_str() == block)
        .copied()
        .with_context(|| format!("aucun bloc « {block} » (voir read_note)"))?;
    let id = anchor.id.clone();
    let mut layout = project.layout.clone();
    if args.get("clear").and_then(Value::as_bool).unwrap_or(false) {
        layout.blocks.retain(|d| d.anchor.id != id);
    }
    if let Some(list) = args.get("directives").and_then(Value::as_str).filter(|s| !s.trim().is_empty()) {
        let ops = directives::parse_ops(list).map_err(|e| anyhow!("{e}. {DIRECTIVES}"))?;
        layout.ops_mut(anchor).merge(&ops);
    }
    project.layout = layout;
    project.save_layout()?;
    let now = project.layout.ops_for(&project.document, &id);
    Ok(vec![text(&format!("Retouches de {id} : {}", serde_json::to_string(&now)?))])
}

fn get_style(args: &Value) -> Result<Vec<Value>> {
    let project = open(args)?;
    let (style, warning) = project.style();
    let report = json!({
        "preset": project.layout.style.preset,
        "overrides": project.layout.style.overrides,
        "resolved": style,
        "warning": warning,
    });
    Ok(vec![text(&serde_json::to_string_pretty(&report)?)])
}

fn set_style(args: &Value) -> Result<Vec<Value>> {
    let mut project = open(args)?;
    if let Some(preset) = args.get("preset").and_then(Value::as_str) {
        if project.presets.get(preset).is_none() {
            bail!("preset inconnu : {preset} (voir list_presets)");
        }
        project.layout.style.preset = preset.to_string();
        project.layout.style.overrides.clear();
    }
    if args.get("reset_overrides").and_then(Value::as_bool).unwrap_or(false) {
        project.layout.style.overrides.clear();
    }
    if let Some(Value::Object(changes)) = args.get("overrides") {
        let mut merged = Value::Object(project.layout.style.overrides.clone());
        deep_merge(&mut merged, &Value::Object(changes.clone()));
        let Value::Object(map) = merged else { unreachable!() };
        // On vérifie que le style obtenu est valide avant d'écrire.
        let mut trial = project.layout.style.clone();
        trial.overrides = map.clone();
        let (style, warning) = project.presets.resolve(&trial);
        if let Some(warning) = warning {
            bail!("{warning}");
        }
        let check = serde_json::to_value(&style)?;
        for (key, value) in &map {
            if check.get(key).is_none() {
                bail!("réglage inconnu : {key} (voir get_style, champ « resolved »)");
            }
            let _ = value;
        }
        project.layout.style.overrides = map;
    }
    project.save_layout()?;
    Ok(vec![text(&format!(
        "Style : preset « {} », réglages {}",
        project.layout.style.preset,
        serde_json::to_string(&project.layout.style.overrides)?
    ))])
}

fn deep_merge(base: &mut Value, changes: &Value) {
    match (base, changes) {
        (Value::Object(base), Value::Object(changes)) => {
            for (key, value) in changes {
                match base.get_mut(key) {
                    Some(existing) if existing.is_object() && value.is_object() => deep_merge(existing, value),
                    _ => {
                        base.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        (base, changes) => *base = changes.clone(),
    }
}

fn open(args: &Value) -> Result<Project> {
    let note = args.get("note").and_then(Value::as_str).context("« note » manquant (chemin du fichier .md)")?;
    let path = Path::new(note);
    if !path.is_file() {
        bail!("note introuvable : {note}");
    }
    Project::open(path).with_context(|| format!("ouverture de {note}"))
}

/// Numéro de page (à partir de 1) → indice.
fn page_arg(args: &Value) -> Result<usize> {
    let page = args.get("page").and_then(Value::as_u64).context("« page » manquant (à partir de 1)")?;
    if page == 0 {
        bail!("les pages sont numérotées à partir de 1");
    }
    Ok(page as usize - 1)
}

fn fix_directives(action: &FixAction) -> String {
    match action {
        FixAction::BreakBefore => "saut-avant".into(),
        FixAction::ImageWidth(w) => format!("largeur={w:.0}"),
        FixAction::KeepTogether(true) => "insecable".into(),
        FixAction::KeepTogether(false) => "secable".into(),
        FixAction::ImagePlacement(p) => format!(
            "placement={}",
            match p {
                Placement::Inline => "texte",
                Placement::Top => "haut",
                Placement::Bottom => "bas",
                Placement::FullPage => "pleine-page",
                Placement::Landscape => "paysage",
            }
        ),
    }
}

fn page_name(width: f64, height: f64) -> String {
    crate::page_name(width, height)
}

fn text(content: &str) -> Value {
    json!({"type": "text", "text": content})
}

fn note_schema() -> Value {
    json!({"type": "string", "description": "Chemin complet de la note Markdown (.md)."})
}

fn tools() -> Vec<Value> {
    vec![
        json!({
            "name": "read_note",
            "description": "Lit une note : ses pages (numéro, format) et ses blocs (id, type, ligne, page, extrait, retouches). À appeler en premier : les id servent à set_block.",
            "inputSchema": {"type": "object", "properties": {"note": note_schema()}, "required": ["note"]},
        }),
        json!({
            "name": "check_layout",
            "description": "Vérifie la mise en page réelle : pages à moitié vides, titres isolés, schémas à agrandir, images réduites… avec, pour chaque défaut, les retouches proposées (à passer à set_block).",
            "inputSchema": {"type": "object", "properties": {"note": note_schema()}, "required": ["note"]},
        }),
        json!({
            "name": "render_page",
            "description": "Montre une page du PDF en image (PNG), pour juger le rendu.",
            "inputSchema": {"type": "object", "properties": {
                "note": note_schema(),
                "page": {"type": "integer", "minimum": 1, "description": "Numéro de page, à partir de 1."},
                "ppi": {"type": "number", "description": "Résolution (30 à 200, 80 par défaut)."}
            }, "required": ["note", "page"]},
        }),
        json!({
            "name": "set_block",
            "description": format!("Retouche un bloc (id venant de read_note). {DIRECTIVES} Les retouches s'ajoutent à celles du bloc ; « clear » les retire d'abord."),
            "inputSchema": {"type": "object", "properties": {
                "note": note_schema(),
                "block_id": {"type": "string"},
                "directives": {"type": "string", "description": "Ex. : « saut-avant » ou « largeur=70, alignement=centre »."},
                "clear": {"type": "boolean", "description": "Retirer d'abord toutes les retouches du bloc."}
            }, "required": ["note", "block_id"]},
        }),
        json!({
            "name": "set_page_format",
            "description": "Change le format d'une page : elle prend ce format, se remplit avec la suite, puis le document reprend le sien (sauf onward). « normal » remet le format du document.",
            "inputSchema": {"type": "object", "properties": {
                "note": note_schema(),
                "page": {"type": "integer", "minimum": 1},
                "format": {"type": "string", "description": "normal, a4, a4-paysage, a3, a3-paysage, a5, a2, us-letter…"},
                "onward": {"type": "boolean", "description": "Garder ce format pour les pages suivantes."}
            }, "required": ["note", "page", "format"]},
        }),
        json!({
            "name": "list_presets",
            "description": "Liste les presets de style (Agrume, Académique, Magazine, Entreprise, Technique, Minimal, Carnet, Créatif, Développeur, Élégant, et ceux de l'utilisateur).",
            "inputSchema": {"type": "object", "properties": {}},
        }),
        json!({
            "name": "get_style",
            "description": "Le style de la note : preset, réglages faits à la main, et style complet obtenu (tous les champs réglables : text, headings, code, table, page, footer, cover, images…).",
            "inputSchema": {"type": "object", "properties": {"note": note_schema()}, "required": ["note"]},
        }),
        json!({
            "name": "set_style",
            "description": "Règle le style de la note : un preset (efface les réglages à la main), et/ou des réglages par-dessus, avec les mêmes champs que get_style (ex. {\"text\": {\"size_pt\": 11}, \"code\": {\"theme\": \"github\"}}).",
            "inputSchema": {"type": "object", "properties": {
                "note": note_schema(),
                "preset": {"type": "string"},
                "overrides": {"type": "object"},
                "reset_overrides": {"type": "boolean"}
            }, "required": ["note"]},
        }),
        json!({
            "name": "export_pdf",
            "description": "Exporte le PDF final (à côté de la note par défaut).",
            "inputSchema": {"type": "object", "properties": {
                "note": note_schema(),
                "output": {"type": "string", "description": "Chemin du PDF (facultatif)."}
            }, "required": ["note"]},
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(server: &mut Server, id: u64, method: &str, params: Value) -> Value {
        server.handle(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})).unwrap()
    }

    #[test]
    fn an_ai_can_read_check_and_retouch_a_note() {
        let dir = tempfile::tempdir().unwrap();
        let note = dir.path().join("Rapport.md");
        let mut text = String::from("# Rapport\n\n## Partie\n\n");
        for i in 0..30 {
            text.push_str(&format!("Paragraphe {i} avec du texte pour remplir la page.\n\n"));
        }
        std::fs::write(&note, text).unwrap();
        let note = note.display().to_string();
        let mut server = Server { engine: Some(Engine::new(FontSources::Bundled)) };

        let init = call(&mut server, 1, "initialize", json!({"protocolVersion": PROTOCOL}));
        assert_eq!(init["result"]["serverInfo"]["name"], "nectar-render");
        assert!(server.handle(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"})).is_none());
        let tools = call(&mut server, 2, "tools/list", json!({}));
        assert_eq!(tools["result"]["tools"].as_array().unwrap().len(), 9);

        let read = call(&mut server, 3, "tools/call", json!({"name": "read_note", "arguments": {"note": note}}));
        let summary: Value = serde_json::from_str(read["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
        let block = summary["blocks"][5]["id"].as_str().unwrap().to_string();

        let set = call(
            &mut server,
            4,
            "tools/call",
            json!({"name": "set_block", "arguments": {"note": note, "block_id": block, "directives": "saut-avant"}}),
        );
        assert!(set["result"]["isError"].is_null(), "{set}");
        let read = call(&mut server, 5, "tools/call", json!({"name": "read_note", "arguments": {"note": note}}));
        let summary: Value = serde_json::from_str(read["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
        let moved = summary["blocks"].as_array().unwrap().iter().find(|b| b["id"] == block.as_str()).unwrap();
        assert_eq!(moved["page"], 2, "le bloc ouvre la page 2");

        let style = call(
            &mut server,
            6,
            "tools/call",
            json!({"name": "set_style", "arguments": {"note": note, "preset": "academique", "overrides": {"text": {"size_pt": 12}}}}),
        );
        assert!(style["result"]["isError"].is_null(), "{style}");
        let wrong = call(
            &mut server,
            7,
            "tools/call",
            json!({"name": "set_style", "arguments": {"note": note, "overrides": {"inexistant": 1}}}),
        );
        assert_eq!(wrong["result"]["isError"], true);

        let format = call(
            &mut server,
            8,
            "tools/call",
            json!({"name": "set_page_format", "arguments": {"note": note, "page": 2, "format": "a4-paysage"}}),
        );
        assert!(format["result"]["isError"].is_null(), "{format}");
        let page = call(
            &mut server,
            9,
            "tools/call",
            json!({"name": "render_page", "arguments": {"note": note, "page": 2, "ppi": 40}}),
        );
        assert!(page["result"]["content"][0]["text"].as_str().unwrap().contains("paysage"), "{page}");
        assert_eq!(page["result"]["content"][1]["type"], "image");
    }
}
