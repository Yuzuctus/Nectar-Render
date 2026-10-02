//! Dessins Excalidraw (plugin Obsidian) rendus en SVG, sans navigateur.
//!
//! Les fichiers `Dessin.excalidraw.md` du plugin contiennent la scène en JSON,
//! souvent compressée (lz-string). On la redessine avec le trait « à main
//! levée » d'Excalidraw : l'algorithme de lignes de rough.js (la bibliothèque
//! qu'utilise Excalidraw), les polices Excalifont et Virgil, les hachures.
//!
//! Si le plugin a exporté un SVG ou un PNG à côté du dessin, plus récent que
//! lui, on préfère cet export : c'est le rendu exact.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use serde::Deserialize;
use serde_json::Value;

/// Ce qu'on sait afficher pour un dessin.
pub enum Drawing {
    /// Un export SVG/PNG du plugin, à jour.
    Exported(PathBuf),
    /// Notre propre rendu.
    Svg(String),
}

/// Un dessin prêt à servir, partagé entre relectures.
pub enum Shared {
    Exported(PathBuf),
    Svg(std::sync::Arc<String>),
}

type Stamp = (std::time::SystemTime, u64);

/// Dessins déjà rendus (la note est relue à chaque enregistrement
/// d'Obsidian : un dessin inchangé n'est pas redessiné).
type Rendered = std::collections::HashMap<PathBuf, (Stamp, std::sync::Arc<String>)>;

static RENDERED: std::sync::LazyLock<std::sync::Mutex<Rendered>> = std::sync::LazyLock::new(Default::default);

/// [`load`], sans refaire le rendu d'un dessin qui n'a pas changé.
pub fn load_shared(path: &Path, resolve: &dyn Fn(&str) -> Option<PathBuf>) -> Result<Shared, String> {
    if let Some(export) = fresh_export(path) {
        return Ok(Shared::Exported(export));
    }
    let stamp = std::fs::metadata(path).ok().and_then(|m| Some((m.modified().ok()?, m.len())));
    if let Some(stamp) = stamp
        && let Ok(cache) = RENDERED.lock()
        && let Some((cached, svg)) = cache.get(path)
        && *cached == stamp
    {
        return Ok(Shared::Svg(svg.clone()));
    }
    match load(path, resolve)? {
        Drawing::Exported(file) => Ok(Shared::Exported(file)),
        Drawing::Svg(svg) => {
            let svg = std::sync::Arc::new(svg);
            if let Some(stamp) = stamp
                && let Ok(mut cache) = RENDERED.lock()
            {
                cache.insert(path.to_path_buf(), (stamp, svg.clone()));
            }
            Ok(Shared::Svg(svg))
        }
    }
}

/// Le fichier est-il un dessin Excalidraw ?
pub fn is_drawing(path: &Path) -> bool {
    let name = path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    name.ends_with(".excalidraw") || name.ends_with(".excalidraw.md") || name.ends_with(".excalidraw.json")
}

/// Le texte est-il celui d'une note du plugin Excalidraw ?
pub fn is_drawing_note(text: &str) -> bool {
    text.lines().take(12).any(|l| l.trim_start().starts_with("excalidraw-plugin:"))
}

/// Charge un dessin : export à jour s'il existe, sinon rendu SVG.
pub fn load(path: &Path, resolve: &dyn Fn(&str) -> Option<PathBuf>) -> Result<Drawing, String> {
    if let Some(export) = fresh_export(path) {
        return Ok(Drawing::Exported(export));
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("lecture impossible : {e}"))?;
    let (json, embedded) = extract(&text)?;
    let scene: Scene = serde_json::from_str(&json).map_err(|e| format!("scène illisible : {e}"))?;
    let base = path.parent().unwrap_or(Path::new("."));
    let file = |id: &str| -> Option<String> {
        if let Some(data) = scene.files.get(id).and_then(|f| f.get("dataURL")).and_then(Value::as_str) {
            return Some(data.to_string());
        }
        let target = embedded.get(id)?;
        let found = resolve(target).or_else(|| Some(base.join(target)).filter(|p| p.is_file()))?;
        let bytes = std::fs::read(&found).ok()?;
        Some(format!("data:{};base64,{}", mime(&found), base64::engine::general_purpose::STANDARD.encode(bytes)))
    };
    Ok(Drawing::Svg(render(&scene, &file)))
}

/// `Dessin.excalidraw.md` → `Dessin.excalidraw.svg`, `Dessin.svg`, `.png`…
fn fresh_export(path: &Path) -> Option<PathBuf> {
    let name = path.file_name()?.to_string_lossy().into_owned();
    let stem = name.strip_suffix(".md").unwrap_or(&name);
    let short = stem.strip_suffix(".excalidraw").unwrap_or(stem);
    let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok()?;
    [stem, short]
        .iter()
        .flat_map(|s| [format!("{s}.svg"), format!("{s}.png")])
        .map(|candidate| path.with_file_name(candidate))
        .find(|p| p.is_file() && std::fs::metadata(p).and_then(|m| m.modified()).is_ok_and(|t| t >= modified))
}

fn mime(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(str::to_lowercase).as_deref() {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    }
}

/// Sort le JSON de la scène et la table des fichiers incorporés
/// (`id: [[image.png]]`) d'une note du plugin, ou d'un `.excalidraw` brut.
fn extract(text: &str) -> Result<(String, HashMap<String, String>), String> {
    let trimmed = text.trim_start();
    if trimmed.starts_with('{') {
        return Ok((trimmed.to_string(), HashMap::new()));
    }
    let mut embedded = HashMap::new();
    let mut in_files = false;
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with("## ") {
            in_files = l.eq_ignore_ascii_case("## Embedded Files");
            continue;
        }
        if in_files && let Some((id, target)) = l.split_once(':') {
            let target = target.trim().trim_start_matches("![[").trim_start_matches("[[").trim_end_matches("]]");
            let target = target.split('|').next().unwrap_or(target).trim();
            if !target.is_empty() {
                embedded.insert(id.trim().to_string(), target.to_string());
            }
        }
    }
    for (fence, compressed) in [("```compressed-json", true), ("```json", false)] {
        if let Some(start) = text.find(fence) {
            let body = &text[start + fence.len()..];
            let end = body.find("```").ok_or("bloc de scène non fermé")?;
            let body = &body[..end];
            if !compressed {
                return Ok((body.trim().to_string(), embedded));
            }
            let packed: String = body.chars().filter(|c| !c.is_whitespace()).collect();
            let chars = lz_str::decompress_from_base64(&packed).ok_or("scène compressée illisible")?;
            return Ok((String::from_utf16_lossy(&chars), embedded));
        }
    }
    Err("aucune scène Excalidraw dans ce fichier".into())
}

// ---------------------------------------------------------------- scène

#[derive(Deserialize, Default)]
#[serde(default)]
struct Scene {
    elements: Vec<Element>,
    files: HashMap<String, Value>,
}

#[derive(Deserialize, Clone)]
#[serde(default, rename_all = "camelCase")]
struct Element {
    #[serde(rename = "type")]
    kind: String,
    id: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    angle: f64,
    stroke_color: String,
    background_color: String,
    fill_style: String,
    stroke_width: f64,
    stroke_style: String,
    roughness: f64,
    opacity: f64,
    roundness: Option<Value>,
    seed: i64,
    is_deleted: bool,
    points: Vec<[f64; 2]>,
    start_arrowhead: Option<String>,
    end_arrowhead: Option<String>,
    text: String,
    font_size: f64,
    font_family: i64,
    text_align: String,
    vertical_align: String,
    line_height: Option<f64>,
    file_id: Option<String>,
    scale: Option<[f64; 2]>,
    frame_id: Option<String>,
}

impl Default for Element {
    fn default() -> Self {
        Self {
            kind: String::new(),
            id: String::new(),
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            angle: 0.0,
            stroke_color: "#1e1e1e".into(),
            background_color: "transparent".into(),
            fill_style: "solid".into(),
            stroke_width: 2.0,
            stroke_style: "solid".into(),
            roughness: 1.0,
            opacity: 100.0,
            roundness: None,
            seed: 1,
            is_deleted: false,
            points: Vec::new(),
            start_arrowhead: None,
            end_arrowhead: None,
            text: String::new(),
            font_size: 20.0,
            font_family: 5,
            text_align: "left".into(),
            vertical_align: "top".into(),
            line_height: None,
            file_id: None,
            scale: None,
            frame_id: None,
        }
    }
}

// ------------------------------------------------------- trait rough.js

/// Le générateur aléatoire de rough.js (Park–Miller), graine par élément :
/// un dessin a toujours le même trait.
struct Random(i64);

impl Random {
    fn new(seed: i64) -> Self {
        Self(if seed == 0 { 1 } else { seed })
    }
    fn next(&mut self) -> f64 {
        self.0 = (48271_i64.wrapping_mul(self.0)) & 0xFFFF_FFFF;
        let signed = self.0 as i32 as i64;
        ((2_i64.pow(31) - 1) & signed) as f64 / 2f64.powi(31)
    }
}

struct Rough<'a> {
    rng: &'a mut Random,
    roughness: f64,
    max_offset: f64,
    bowing: f64,
}

impl Rough<'_> {
    fn offset(&mut self, min: f64, max: f64, gain: f64) -> f64 {
        self.roughness * gain * (self.rng.next() * (max - min) + min)
    }
    fn offset_opt(&mut self, x: f64, gain: f64) -> f64 {
        self.offset(-x, x, gain)
    }

    /// Une ligne `_line` de rough.js : une courbe de Bézier légèrement arquée.
    fn line(&mut self, out: &mut String, a: (f64, f64), b: (f64, f64), mv: bool, overlay: bool) {
        let (x1, y1) = a;
        let (x2, y2) = b;
        let length_sq = (x1 - x2).powi(2) + (y1 - y2).powi(2);
        let length = length_sq.sqrt();
        let gain = if length < 200.0 {
            1.0
        } else if length > 500.0 {
            0.4
        } else {
            -0.0016668 * length + 1.233334
        };
        let mut offset = self.max_offset;
        if offset * offset * 100.0 > length_sq {
            offset = length / 10.0;
        }
        let half = offset / 2.0;
        let diverge = 0.2 + self.rng.next() * 0.2;
        let mut mid_x = self.bowing * self.max_offset * (y2 - y1) / 200.0;
        let mut mid_y = self.bowing * self.max_offset * (x1 - x2) / 200.0;
        mid_x = self.offset_opt(mid_x, gain);
        mid_y = self.offset_opt(mid_y, gain);
        let amount = if overlay { half } else { offset };
        if mv {
            let (dx, dy) = (self.offset_opt(amount, gain), self.offset_opt(amount, gain));
            let _ = write!(out, "M{:.2} {:.2} ", x1 + dx, y1 + dy);
        }
        let c1 = (
            mid_x + x1 + (x2 - x1) * diverge + self.offset_opt(amount, gain),
            mid_y + y1 + (y2 - y1) * diverge + self.offset_opt(amount, gain),
        );
        let c2 = (
            mid_x + x1 + 2.0 * (x2 - x1) * diverge + self.offset_opt(amount, gain),
            mid_y + y1 + 2.0 * (y2 - y1) * diverge + self.offset_opt(amount, gain),
        );
        let end = (x2 + self.offset_opt(amount, gain), y2 + self.offset_opt(amount, gain));
        let _ = write!(out, "C{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} ", c1.0, c1.1, c2.0, c2.1, end.0, end.1);
    }

    /// `_doubleLine` : deux passages, comme au crayon.
    fn double_line(&mut self, out: &mut String, a: (f64, f64), b: (f64, f64), single: bool) {
        self.line(out, a, b, true, false);
        if !single {
            self.line(out, a, b, true, true);
        }
    }

    /// Courbe fermée ou ouverte passant par des points (Catmull-Rom),
    /// légèrement perturbée, en deux passages.
    fn curve(&mut self, out: &mut String, points: &[(f64, f64)], closed: bool, single: bool) {
        let passes = if single { 1 } else { 2 };
        for pass in 0..passes {
            let jitter = if pass == 0 { 1.0 } else { 0.6 };
            let pts: Vec<(f64, f64)> = points
                .iter()
                .map(|&(x, y)| {
                    let amount = self.max_offset * 0.5 * jitter;
                    (x + self.offset_opt(amount, 1.0), y + self.offset_opt(amount, 1.0))
                })
                .collect();
            catmull_rom(out, &pts, closed);
        }
    }
}

fn catmull_rom(out: &mut String, pts: &[(f64, f64)], closed: bool) {
    if pts.len() < 2 {
        return;
    }
    let n = pts.len();
    let get = |i: isize| -> (f64, f64) {
        if closed { pts[i.rem_euclid(n as isize) as usize] } else { pts[i.clamp(0, n as isize - 1) as usize] }
    };
    let _ = write!(out, "M{:.2} {:.2} ", pts[0].0, pts[0].1);
    let segments = if closed { n } else { n - 1 };
    for i in 0..segments as isize {
        let (p0, p1, p2, p3) = (get(i - 1), get(i), get(i + 1), get(i + 2));
        let c1 = (p1.0 + (p2.0 - p0.0) / 6.0, p1.1 + (p2.1 - p0.1) / 6.0);
        let c2 = (p2.0 - (p3.0 - p1.0) / 6.0, p2.1 - (p3.1 - p1.1) / 6.0);
        let _ = write!(out, "C{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} ", c1.0, c1.1, c2.0, c2.1, p2.0, p2.1);
    }
}

// ------------------------------------------------------------- rendu SVG

fn color(value: &str) -> Option<&str> {
    let v = value.trim();
    (!v.is_empty() && v != "transparent").then_some(v)
}

fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Famille Excalidraw → polices disponibles dans Nectar.
fn font_family(id: i64) -> &'static str {
    match id {
        1 => "Virgil, Excalifont",
        2 | 6 | 9 => "IBM Plex Sans, Helvetica, Arial",
        3 => "JetBrains Mono, Cascadia Code, monospace",
        7 => "IBM Plex Sans",
        _ => "Excalifont, Virgil",
    }
}

/// Métriques (ascendante, descendante) en em, pour placer la ligne de base
/// exactement comme Excalidraw.
fn font_metrics(id: i64) -> (f64, f64) {
    match id {
        1 | 5 | 8 => (0.886, 0.374),
        3 => (1.02, 0.3),
        _ => (1.025, 0.275),
    }
}

fn render(scene: &Scene, file: &dyn Fn(&str) -> Option<String>) -> String {
    let elements: Vec<&Element> = scene.elements.iter().filter(|e| !e.is_deleted && e.kind != "frame").collect();
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for e in &elements {
        let (x0, y0, x1, y1) = bounds(e);
        let pad = e.stroke_width.max(1.0) * 2.0;
        min_x = min_x.min(x0 - pad);
        min_y = min_y.min(y0 - pad);
        max_x = max_x.max(x1 + pad);
        max_y = max_y.max(y1 + pad);
    }
    if elements.is_empty() {
        (min_x, min_y, max_x, max_y) = (0.0, 0.0, 10.0, 10.0);
    }
    let margin = 10.0;
    let (w, h) = (max_x - min_x + 2.0 * margin, max_y - min_y + 2.0 * margin);
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" width=\"{w:.0}\" height=\"{h:.0}\" viewBox=\"{:.2} {:.2} {w:.2} {h:.2}\">\n",
        min_x - margin,
        min_y - margin
    );
    for e in elements {
        element(&mut svg, e, file);
    }
    svg.push_str("</svg>\n");
    svg
}

fn bounds(e: &Element) -> (f64, f64, f64, f64) {
    if !e.points.is_empty() && matches!(e.kind.as_str(), "line" | "arrow" | "freedraw") {
        let xs = e.points.iter().map(|p| e.x + p[0]);
        let ys = e.points.iter().map(|p| e.y + p[1]);
        let (x0, x1) = xs.fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(v), b.max(v)));
        let (y0, y1) = ys.fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(v), b.max(v)));
        return (x0 - 12.0, y0 - 12.0, x1 + 12.0, y1 + 12.0);
    }
    let (x0, y0, x1, y1) =
        (e.x.min(e.x + e.width), e.y.min(e.y + e.height), e.x.max(e.x + e.width), e.y.max(e.y + e.height));
    if e.angle.abs() > 1e-6 {
        // Boîte englobante de la boîte tournée.
        let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let (s, c) = e.angle.sin_cos();
        let corners = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)].map(|(x, y)| {
            let (dx, dy) = (x - cx, y - cy);
            (cx + dx * c - dy * s, cy + dx * s + dy * c)
        });
        let xs = corners.iter().map(|p| p.0);
        let ys = corners.iter().map(|p| p.1);
        return (
            xs.clone().fold(f64::MAX, f64::min),
            ys.clone().fold(f64::MAX, f64::min),
            xs.fold(f64::MIN, f64::max),
            ys.fold(f64::MIN, f64::max),
        );
    }
    (x0, y0, x1, y1)
}

fn element(svg: &mut String, e: &Element, file: &dyn Fn(&str) -> Option<String>) {
    let mut rng = Random::new(e.seed);
    let solid = e.stroke_style == "solid";
    let mut rough = Rough {
        rng: &mut rng,
        // Les traits pointillés d'Excalidraw sont tracés sans tremblement.
        roughness: if solid { e.roughness } else { 0.0 },
        max_offset: 2.0,
        bowing: 1.0,
    };
    let single = e.roughness <= 0.0 || !solid;
    let stroke = color(&e.stroke_color);
    let fill = color(&e.background_color);
    let sw = e.stroke_width.max(0.5);
    let dash = match e.stroke_style.as_str() {
        "dashed" => format!(" stroke-dasharray=\"{:.1} {:.1}\"", 8.0 + sw, 8.0 + sw * 1.5),
        "dotted" => format!(" stroke-dasharray=\"1.5 {:.1}\"", 6.0 + sw),
        _ => String::new(),
    };
    let (cx, cy) = (e.x + e.width / 2.0, e.y + e.height / 2.0);
    let mut attrs = String::new();
    if e.angle.abs() > 1e-6 {
        let _ = write!(attrs, " transform=\"rotate({:.3} {cx:.2} {cy:.2})\"", e.angle.to_degrees());
    }
    if e.opacity < 100.0 {
        let _ = write!(attrs, " opacity=\"{:.2}\"", (e.opacity / 100.0).clamp(0.0, 1.0));
    }
    let _ = writeln!(svg, "<g{attrs}>");
    let stroke_path = |svg: &mut String, d: &str, width: f64| {
        if let Some(c) = stroke {
            let _ = writeln!(
                svg,
                "<path d=\"{d}\" fill=\"none\" stroke=\"{c}\" stroke-width=\"{width:.2}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"{dash}/>"
            );
        }
    };

    match e.kind.as_str() {
        "rectangle" | "diamond" | "ellipse" => {
            let outline = shape_outline(e);
            if let Some(f) = fill {
                fill_shape(svg, &mut rough, e, &outline, f);
            }
            let mut d = String::new();
            match e.kind.as_str() {
                "ellipse" => rough.curve(&mut d, &outline, true, single),
                "rectangle" if rounded(e) => {
                    // Côtés droits au crayon, coins en petites courbes (4 points par coin).
                    for corner in 0..4 {
                        let arc = &outline[corner * 4..corner * 4 + 4];
                        rough.curve(&mut d, arc, false, single);
                        let next = outline[((corner + 1) * 4) % outline.len()];
                        rough.double_line(&mut d, arc[3], next, single);
                    }
                }
                _ if rounded(e) => rough.curve(&mut d, &outline, true, single),
                _ => {
                    let corners = polygon(e);
                    for i in 0..corners.len() {
                        rough.double_line(&mut d, corners[i], corners[(i + 1) % corners.len()], single);
                    }
                }
            }
            stroke_path(svg, &d, sw);
        }
        "line" | "arrow" => {
            let pts: Vec<(f64, f64)> = e.points.iter().map(|p| (e.x + p[0], e.y + p[1])).collect();
            if pts.len() >= 2 {
                let closed = e.kind == "line" && pts.len() > 2 && dist(pts[0], pts[pts.len() - 1]) < 1.0;
                if closed && let Some(f) = fill {
                    fill_shape(svg, &mut rough, e, &pts, f);
                }
                let mut d = String::new();
                if rounded(e) && pts.len() > 2 {
                    rough.curve(&mut d, &pts, false, single);
                } else {
                    for pair in pts.windows(2) {
                        rough.double_line(&mut d, pair[0], pair[1], single);
                    }
                }
                stroke_path(svg, &d, sw);
                if e.kind == "arrow" {
                    if let Some(head) = &e.end_arrowhead {
                        arrowhead(svg, &mut rough, head, pts[pts.len() - 2], pts[pts.len() - 1], sw, stroke, single);
                    }
                    if let Some(head) = &e.start_arrowhead {
                        arrowhead(svg, &mut rough, head, pts[1], pts[0], sw, stroke, single);
                    }
                }
            }
        }
        "freedraw" => {
            let pts: Vec<(f64, f64)> = e.points.iter().map(|p| (e.x + p[0], e.y + p[1])).collect();
            if let (Some(c), true) = (stroke, pts.len() >= 2) {
                let mut d = String::new();
                catmull_rom(&mut d, &pts, false);
                let _ = writeln!(
                    svg,
                    "<path d=\"{d}\" fill=\"none\" stroke=\"{c}\" stroke-width=\"{:.2}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>",
                    sw * 1.8
                );
            }
        }
        "text" => text(svg, e),
        "image" => {
            if let Some(href) = e.file_id.as_deref().and_then(file) {
                let [sx, sy] = e.scale.unwrap_or([1.0, 1.0]);
                let flip = if sx < 0.0 || sy < 0.0 {
                    format!(
                        " transform=\"translate({:.2} {:.2}) scale({sx} {sy})\"",
                        if sx < 0.0 { 2.0 * e.x + e.width } else { 0.0 },
                        if sy < 0.0 { 2.0 * e.y + e.height } else { 0.0 }
                    )
                } else {
                    String::new()
                };
                let _ = writeln!(
                    svg,
                    "<image x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" preserveAspectRatio=\"none\" href=\"{}\"{flip}/>",
                    e.x,
                    e.y,
                    e.width,
                    e.height,
                    escape_xml(&href)
                );
            }
        }
        _ => {}
    }
    svg.push_str("</g>\n");
    let _ = (&e.id, &e.frame_id);
}

fn rounded(e: &Element) -> bool {
    e.roundness.as_ref().is_some_and(|r| !r.is_null())
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// Sommets d'un rectangle ou d'un losange.
fn polygon(e: &Element) -> Vec<(f64, f64)> {
    let (x, y, w, h) = (e.x, e.y, e.width, e.height);
    match e.kind.as_str() {
        "diamond" => vec![(x + w / 2.0, y), (x + w, y + h / 2.0), (x + w / 2.0, y + h), (x, y + h / 2.0)],
        _ => vec![(x, y), (x + w, y), (x + w, y + h), (x, y + h)],
    }
}

/// Contour échantillonné (ellipse, coins arrondis) ou polygone.
fn shape_outline(e: &Element) -> Vec<(f64, f64)> {
    let (x, y, w, h) = (e.x, e.y, e.width, e.height);
    match e.kind.as_str() {
        "ellipse" => {
            let (cx, cy, rx, ry) = (x + w / 2.0, y + h / 2.0, w / 2.0, h / 2.0);
            let n = ((w + h) / 18.0).clamp(10.0, 48.0) as usize;
            (0..n)
                .map(|i| {
                    let a = i as f64 / n as f64 * std::f64::consts::TAU;
                    (cx + rx * a.cos(), cy + ry * a.sin())
                })
                .collect()
        }
        "rectangle" if rounded(e) => {
            // Rayon « adaptatif » d'Excalidraw : 32 px au plus, un quart du petit côté.
            let r = (w.abs().min(h.abs()) * 0.25).min(32.0);
            let mut pts = Vec::new();
            let corner = |pts: &mut Vec<(f64, f64)>, cx: f64, cy: f64, start: f64| {
                for k in 0..=3 {
                    let a = start + k as f64 / 3.0 * std::f64::consts::FRAC_PI_2;
                    pts.push((cx + r * a.cos(), cy + r * a.sin()));
                }
            };
            use std::f64::consts::PI;
            corner(&mut pts, x + w - r, y + r, -PI / 2.0);
            corner(&mut pts, x + w - r, y + h - r, 0.0);
            corner(&mut pts, x + r, y + h - r, PI / 2.0);
            corner(&mut pts, x + r, y + r, PI);
            pts
        }
        "diamond" if rounded(e) => {
            let p = polygon(e);
            let mut pts = Vec::new();
            for i in 0..4 {
                let (a, b) = (p[i], p[(i + 1) % 4]);
                pts.push((a.0 + (b.0 - a.0) * 0.12, a.1 + (b.1 - a.1) * 0.12));
                pts.push((a.0 + (b.0 - a.0) * 0.88, a.1 + (b.1 - a.1) * 0.88));
            }
            pts
        }
        _ => polygon(e),
    }
}

/// Remplissage : plein, hachures (rough.js : −41°, écart 4 × trait),
/// hachures croisées.
fn fill_shape(svg: &mut String, rough: &mut Rough<'_>, e: &Element, outline: &[(f64, f64)], color: &str) {
    if e.fill_style == "solid" || outline.len() < 3 {
        let mut d = String::new();
        if e.kind == "ellipse" || rounded(e) {
            catmull_rom(&mut d, outline, true);
        } else {
            let _ = write!(d, "M{:.2} {:.2} ", outline[0].0, outline[0].1);
            for p in &outline[1..] {
                let _ = write!(d, "L{:.2} {:.2} ", p.0, p.1);
            }
            d.push('Z');
        }
        let _ = writeln!(svg, "<path d=\"{d}\" fill=\"{color}\" stroke=\"none\"/>");
        return;
    }
    let gap = (e.stroke_width * 4.0).max(5.0);
    let weight = (e.stroke_width / 2.0).max(0.5);
    let mut angles = vec![-41.0_f64];
    if e.fill_style == "cross-hatch" {
        angles.push(49.0);
    }
    let mut d = String::new();
    for angle in angles {
        for (a, b) in hachure(outline, angle.to_radians(), gap) {
            rough.line(&mut d, a, b, true, false);
        }
    }
    let _ = writeln!(
        svg,
        "<path d=\"{d}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"{weight:.2}\" stroke-linecap=\"round\"/>"
    );
}

/// Segments parallèles d'angle donné, découpés dans le polygone.
fn hachure(poly: &[(f64, f64)], angle: f64, gap: f64) -> Vec<((f64, f64), (f64, f64))> {
    let (s, c) = angle.sin_cos();
    // On tourne le polygone pour que les hachures soient horizontales.
    let rot: Vec<(f64, f64)> = poly.iter().map(|&(x, y)| (x * c + y * s, -x * s + y * c)).collect();
    let back = |(x, y): (f64, f64)| (x * c - y * s, x * s + y * c);
    let (y0, y1) = rot.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.1), b.max(p.1)));
    let mut out = Vec::new();
    let mut y = y0 + gap / 2.0;
    while y < y1 {
        let mut xs = Vec::new();
        for i in 0..rot.len() {
            let (a, b) = (rot[i], rot[(i + 1) % rot.len()]);
            if (a.1 <= y && b.1 > y) || (b.1 <= y && a.1 > y) {
                xs.push(a.0 + (y - a.1) / (b.1 - a.1) * (b.0 - a.0));
            }
        }
        xs.sort_by(f64::total_cmp);
        for pair in xs.chunks(2) {
            if let [xa, xb] = pair
                && xb - xa > 1.0
            {
                out.push((back((*xa, y)), back((*xb, y))));
            }
        }
        y += gap;
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn arrowhead(
    svg: &mut String,
    rough: &mut Rough<'_>,
    kind: &str,
    from: (f64, f64),
    tip: (f64, f64),
    sw: f64,
    stroke: Option<&str>,
    single: bool,
) {
    let Some(c) = stroke else { return };
    let len = dist(from, tip);
    if len < 0.5 {
        return;
    }
    let (ux, uy) = ((tip.0 - from.0) / len, (tip.1 - from.1) / len);
    let size = (10.0 + sw * 4.0).min(len * 0.6).max(6.0);
    let side = |angle: f64| {
        let (s, co) = angle.sin_cos();
        let (dx, dy) = (-ux * co + uy * s, -uy * co - ux * s);
        (tip.0 + dx * size, tip.1 + dy * size)
    };
    let (left, right) = (side(0.45), side(-0.45));
    let mut d = String::new();
    match kind {
        "triangle" | "triangle_outline" => {
            let fill = if kind == "triangle" { c } else { "none" };
            let _ = writeln!(
                svg,
                "<path d=\"M{:.2} {:.2} L{:.2} {:.2} L{:.2} {:.2} Z\" fill=\"{fill}\" stroke=\"{c}\" stroke-width=\"{sw:.2}\" stroke-linejoin=\"round\"/>",
                tip.0, tip.1, left.0, left.1, right.0, right.1
            );
            return;
        }
        "dot" | "circle" | "circle_outline" => {
            let r = (sw * 2.0 + 2.0).min(8.0);
            let fill = if kind == "circle_outline" { "none" } else { c };
            let _ = writeln!(
                svg,
                "<circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"{r:.2}\" fill=\"{fill}\" stroke=\"{c}\" stroke-width=\"{sw:.2}\"/>",
                tip.0, tip.1
            );
            return;
        }
        "bar" => {
            let half = size * 0.5;
            let a = (tip.0 - uy * half, tip.1 + ux * half);
            let b = (tip.0 + uy * half, tip.1 - ux * half);
            rough.double_line(&mut d, a, b, single);
        }
        "diamond" | "diamond_outline" => {
            let back = (tip.0 - ux * size, tip.1 - uy * size);
            let mid = (tip.0 - ux * size / 2.0, tip.1 - uy * size / 2.0);
            let w = size * 0.35;
            let a = (mid.0 - uy * w, mid.1 + ux * w);
            let b = (mid.0 + uy * w, mid.1 - ux * w);
            let fill = if kind == "diamond" { c } else { "none" };
            let _ = writeln!(
                svg,
                "<path d=\"M{:.2} {:.2} L{:.2} {:.2} L{:.2} {:.2} L{:.2} {:.2} Z\" fill=\"{fill}\" stroke=\"{c}\" stroke-width=\"{sw:.2}\"/>",
                tip.0, tip.1, a.0, a.1, back.0, back.1, b.0, b.1
            );
            return;
        }
        _ => {
            rough.double_line(&mut d, left, tip, single);
            rough.double_line(&mut d, right, tip, single);
        }
    }
    let _ = writeln!(
        svg,
        "<path d=\"{d}\" fill=\"none\" stroke=\"{c}\" stroke-width=\"{sw:.2}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>"
    );
}

fn text(svg: &mut String, e: &Element) {
    let Some(c) = color(&e.stroke_color) else { return };
    let size = if e.font_size > 0.0 { e.font_size } else { 20.0 };
    let line_px = size * e.line_height.unwrap_or(1.25);
    let (ascent, descent) = font_metrics(e.font_family);
    // Même calcul que `getVerticalOffset` d'Excalidraw.
    let baseline = size * ascent + (line_px - size * ascent - size * descent) / 2.0;
    let (anchor, x) = match e.text_align.as_str() {
        "center" => ("middle", e.x + e.width / 2.0),
        "right" => ("end", e.x + e.width),
        _ => ("start", e.x),
    };
    for (i, line) in e.text.split('\n').enumerate() {
        let y = e.y + i as f64 * line_px + baseline;
        let _ = writeln!(
            svg,
            "<text x=\"{x:.2}\" y=\"{y:.2}\" font-family=\"{}\" font-size=\"{size:.2}\" fill=\"{c}\" text-anchor=\"{anchor}\" xml:space=\"preserve\">{}</text>",
            font_family(e.font_family),
            escape_xml(line)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene() -> String {
        serde_json::json!({
            "type": "excalidraw",
            "elements": [
                {"type": "rectangle", "x": 0, "y": 0, "width": 200, "height": 100, "seed": 7,
                 "backgroundColor": "#a5d8ff", "fillStyle": "hachure", "roundness": {"type": 3}},
                {"type": "text", "x": 20, "y": 30, "width": 160, "height": 25, "text": "Bonjour\nà tous",
                 "fontSize": 20, "fontFamily": 5, "textAlign": "center"},
                {"type": "arrow", "x": 200, "y": 50, "points": [[0, 0], [120, 0]], "endArrowhead": "arrow", "seed": 3},
                {"type": "ellipse", "x": 320, "y": 0, "width": 100, "height": 100, "isDeleted": true}
            ],
            "files": {}
        })
        .to_string()
    }

    #[test]
    fn renders_shapes_text_and_arrows() {
        let scene: Scene = serde_json::from_str(&scene()).unwrap();
        let svg = render(&scene, &|_| None);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("Excalifont"));
        assert!(svg.contains(">à tous</text>"));
        assert_eq!(svg.matches("<g").count(), 3, "l'élément supprimé est ignoré");
        // Même graine, même trait.
        assert_eq!(svg, render(&scene, &|_| None));
    }

    #[test]
    fn reads_obsidian_plugin_notes() {
        let packed = lz_str::compress_to_base64(&scene());
        let wrapped: String =
            packed.chars().collect::<Vec<_>>().chunks(60).map(|c| c.iter().collect::<String>() + "\n").collect();
        let note = format!(
            "---\nexcalidraw-plugin: parsed\n---\n# Excalidraw Data\n## Text Elements\nBonjour ^abc\n\n## Embedded Files\n5d2e: [[photo.png]]\n\n%%\n## Drawing\n```compressed-json\n{wrapped}```\n%%\n"
        );
        assert!(is_drawing_note(&note));
        let (json, files) = extract(&note).unwrap();
        assert!(json.contains("\"rectangle\""));
        assert_eq!(files.get("5d2e").map(String::as_str), Some("photo.png"));
    }
}
