//! Génération de la source Typst à partir du document et des retouches.
//!
//! La source produite importe `/nectar/nectar.typ` (le template, fourni par
//! le moteur), qui lit lui-même `/nectar/style.typ`, généré ici à partir du
//! [`Style`]. Chaque bloc adressable est précédé d'un marqueur `#nb("id")`
//! qui permet ensuite de savoir où il a atterri dans les pages.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::layout::{BlockOps, HAlign, Layout, PageChange, PageSpec, Placement};
use crate::model::*;
use crate::style::Style;

/// Fichier référencé par la source, servi sous un chemin virtuel.
#[derive(Debug, Clone, PartialEq)]
pub struct Asset {
    pub vpath: String,
    pub path: PathBuf,
}

/// Résultat de la génération.
#[derive(Debug, Clone, Default)]
pub struct Generated {
    pub source: String,
    pub assets: Vec<Asset>,
    /// Ligne (1 = première) où commence chaque bloc dans la source.
    pub block_lines: Vec<(BlockId, usize)>,
    pub warnings: Vec<String>,
    /// Fichiers texte virtuels à servir en plus (`/nectar/style.typ`…).
    pub files: Vec<(String, String)>,
    /// Polices demandées par le style, à vérifier sur la machine.
    pub fonts: Vec<String>,
}

impl Generated {
    /// Le bloc qui contient une ligne de la source générée.
    pub fn block_at_line(&self, line: usize) -> Option<&BlockId> {
        self.block_lines.iter().take_while(|(_, l)| *l <= line).last().map(|(id, _)| id)
    }
}

/// Produit la source Typst d'un document retouché.
pub fn generate(doc: &Document, layout: &Layout, style: &Style) -> Generated {
    let mut resolution = layout.resolve(doc);
    hoist_over_headings(doc, &mut resolution.ops);
    let mut g = Gen {
        ops: &resolution.ops,
        layout,
        style,
        out: String::new(),
        assets: Vec::new(),
        asset_ids: HashMap::new(),
        warnings: Vec::new(),
        block_lines: Vec::new(),
    };
    for anchor in &resolution.orphans {
        g.warnings.push(format!(
            "retouche sans bloc correspondant ({} « {} »)",
            anchor.kind.label_fr(),
            anchor.excerpt
        ));
    }
    g.preamble(&doc.meta);
    for block in &doc.blocks {
        g.block(block);
    }
    let (style_source, fonts) = crate::typst_style::style_module(style);
    let theme = crate::code_themes::get(&style.code.theme).unwrap_or(&crate::code_themes::THEMES[0]);
    if crate::code_themes::get(&style.code.theme).is_none() {
        g.warnings.push(format!("thème de code « {} » inconnu", style.code.theme));
    }
    let files =
        vec![("/nectar/style.typ".to_string(), style_source), ("/nectar/code.tmTheme".to_string(), theme.tm_theme())];
    Generated { source: g.out, assets: g.assets, block_lines: g.block_lines, warnings: g.warnings, files, fonts }
}

/// Un titre ne reste jamais seul en bas de page : un saut de page ou un
/// changement de format demandé sur un bloc remonte avant les titres qui le
/// précèdent directement.
fn hoist_over_headings(doc: &Document, ops: &mut HashMap<BlockId, BlockOps>) {
    for index in 1..doc.blocks.len() {
        let id = &doc.blocks[index].id;
        let Some(current) = ops.get(id) else { continue };
        if !(current.break_before || current.page.is_some()) || current.hidden {
            continue;
        }
        let mut target = index;
        while target > 0 {
            let previous = &doc.blocks[target - 1];
            let previous_ops = ops.get(&previous.id);
            let free = previous_ops.is_none_or(|o| !o.break_before && o.page.is_none() && !o.hidden);
            if matches!(previous.node, Node::Heading { .. }) && free {
                target -= 1;
            } else {
                break;
            }
        }
        if target == index {
            continue;
        }
        let current = ops.get_mut(id).expect("présent");
        let moved = BlockOps { break_before: current.break_before, page: current.page.take(), ..BlockOps::default() };
        current.break_before = false;
        ops.entry(doc.blocks[target].id.clone()).or_default().merge(&moved);
    }
}

struct Gen<'a> {
    ops: &'a HashMap<BlockId, BlockOps>,
    layout: &'a Layout,
    style: &'a Style,
    out: String,
    assets: Vec<Asset>,
    asset_ids: HashMap<PathBuf, String>,
    warnings: Vec<String>,
    block_lines: Vec<(BlockId, usize)>,
}

impl Gen<'_> {
    fn line(&self) -> usize {
        self.out.matches('\n').count() + 1
    }

    fn preamble(&mut self, meta: &Meta) {
        let opt = |v: &Option<String>| v.as_deref().map(string).unwrap_or_else(|| "none".into());
        let tags = meta.tags.iter().map(|t| string(t)).collect::<Vec<_>>();
        let page = page_dict(&self.layout.page);
        let _ = write!(
            self.out,
            "#import \"/nectar/nectar.typ\": *\n\
             #show: template.with(\n  \
               title: {title},\n  subtitle: {subtitle},\n  author: {author},\n  date: {date},\n  \
               lang: {lang},\n  tags: {tags},\n  page: {page},\n)\n\n",
            title = opt(&meta.title),
            subtitle = opt(&meta.subtitle),
            author = opt(&meta.author),
            date = opt(&meta.date),
            lang = string(meta.lang.as_deref().unwrap_or("fr")),
            tags = array(&tags),
        );
    }

    fn block(&mut self, block: &Block) {
        let ops = self.ops.get(&block.id).cloned().unwrap_or_default();
        self.before(&ops);
        // Un saut demandé avant la première puce passe avant toute la liste,
        // pour que le marqueur de la liste atterrisse sur la bonne page.
        let mut first_item_done = false;
        if !ops.hidden
            && let Node::List(list) = &block.node
            && let Some(first) = list.items.first().and_then(|i| i.id.as_ref()).and_then(|id| self.ops.get(id))
        {
            let first = first.clone();
            self.before(&first);
            first_item_done = true;
        }
        if ops.hidden {
            return;
        }
        self.block_lines.push((block.id.clone(), self.line()));
        let _ = writeln!(self.out, "#nb({})", string(block.id.as_str()));

        let body = match &block.node {
            Node::List(list) => {
                self.top_list(list, first_item_done);
                None
            }
            Node::Figure(image) => Some(self.figure(image, ops.image.as_ref())),
            node => Some(self.node(node)),
        };
        if let Some(body) = body {
            if ops.keep_with_next {
                let _ = writeln!(self.out, "#block(sticky: true)[\n{body}\n]");
            } else {
                self.out.push_str(&body);
                self.out.push('\n');
            }
        }
        self.after(&ops);
        self.out.push('\n');
    }

    /// Ce qui se place avant un bloc : format de page, saut, espace.
    fn before(&mut self, ops: &BlockOps) {
        match &ops.page {
            Some(PageChange::Set(spec)) => {
                let _ = writeln!(self.out, "#set page({})", page_args(spec, self.style));
            }
            Some(PageChange::Default(_)) => {
                let _ = writeln!(self.out, "#set page({})", page_args(&self.layout.page, self.style));
            }
            None => {}
        }
        if ops.hidden {
            return;
        }
        if ops.break_before {
            self.out.push_str("#pagebreak(weak: true)\n");
        }
        if let Some(mm) = ops.space_before_mm {
            let _ = writeln!(self.out, "#v({}mm)", num(mm));
        }
        if ops.push_to_bottom {
            self.out.push_str("#v(1fr)\n");
        }
    }

    fn after(&mut self, ops: &BlockOps) {
        if ops.break_after {
            self.out.push_str("#pagebreak(weak: true)\n");
        }
    }

    /// Liste de premier niveau : on la coupe là où une puce porte une retouche
    /// (saut de page avant une puce, changement de format…), en gardant la
    /// numérotation.
    fn top_list(&mut self, list: &List, first_before_done: bool) {
        let mut segment: Vec<&ListItem> = Vec::new();
        let mut segment_start = list.start;
        for (index, item) in list.items.iter().enumerate() {
            let mut ops = item.id.as_ref().and_then(|id| self.ops.get(id)).cloned().unwrap_or_default();
            if index == 0 && first_before_done {
                ops = BlockOps { hidden: ops.hidden, break_after: ops.break_after, ..BlockOps::default() };
            }
            let splits_before =
                ops.page.is_some() || ops.break_before || ops.space_before_mm.is_some() || ops.push_to_bottom;
            if splits_before && !segment.is_empty() {
                self.emit_list(list, &segment, segment_start);
                segment.clear();
            }
            if segment.is_empty() {
                segment_start = list.start + index;
            }
            self.before(&ops);
            if !ops.hidden {
                if let Some(id) = &item.id {
                    self.block_lines.push((id.clone(), self.line()));
                }
                segment.push(item);
            }
            if ops.break_after {
                self.emit_list(list, &segment, segment_start);
                segment.clear();
                self.after(&ops);
            }
        }
        if !segment.is_empty() {
            self.emit_list(list, &segment, segment_start);
        }
    }

    fn emit_list(&mut self, list: &List, items: &[&ListItem], start: usize) {
        let text = self.list_markup(list, items, start);
        self.out.push_str(&text);
        self.out.push('\n');
    }

    fn list_markup(&mut self, list: &List, items: &[&ListItem], start: usize) -> String {
        let all_tasks = items.iter().all(|i| i.task.is_some());
        let mut args = vec![format!("tight: {}", list.tight)];
        if list.ordered {
            args.push(format!("start: {start}"));
        } else if all_tasks {
            args.push("marker: []".into());
        }
        let mut out = format!("#{}({}", if list.ordered { "enum" } else { "list" }, args.join(", "));
        for item in items {
            out.push_str(",\n  [");
            if let Some(id) = &item.id {
                let _ = write!(out, "#nb({});", string(id.as_str()));
            }
            if let Some(done) = item.task {
                let _ = write!(out, "#task({done});");
            }
            let children: Vec<String> = item.children.iter().map(|c| self.node(c)).collect();
            out.push_str(&children.join(if list.tight { "\n" } else { "\n\n" }));
            out.push(']');
        }
        out.push_str(",\n)");
        out
    }

    /// Nœud de bloc sans retouche (dans une citation, une puce, une note…).
    fn node(&mut self, node: &Node) -> String {
        match node {
            Node::Heading { level, content } => {
                format!("#heading(level: {level})[{}]", self.inlines(content))
            }
            Node::Paragraph(content) => self.inlines(content),
            Node::Figure(image) => self.figure(image, None),
            Node::List(list) => {
                let items: Vec<&ListItem> = list.items.iter().collect();
                self.list_markup(list, &items, list.start)
            }
            Node::Code { lang, text } => {
                let lang = lang.as_deref().map(string).unwrap_or_else(|| "none".into());
                format!("#raw(block: true, lang: {lang}, {})", string(text))
            }
            Node::Quote(children) => format!("#quote(block: true)[\n{}\n]", self.nodes(children)),
            Node::Callout(callout) => {
                let title = match &callout.title {
                    Some(title) => format!("[{}]", self.inlines(title)),
                    None => "none".into(),
                };
                format!("#callout({}, title: {title})[\n{}\n]", string(&callout.kind), self.nodes(&callout.body))
            }
            Node::Table(table) => self.table(table),
            Node::Math(latex) => self.math(latex, true),
            Node::Rule => "#nectar-rule()".into(),
        }
    }

    fn nodes(&mut self, nodes: &[Node]) -> String {
        nodes.iter().map(|n| self.node(n)).collect::<Vec<_>>().join("\n\n")
    }

    fn table(&mut self, table: &Table) -> String {
        let columns = table.align.len().max(table.header.len()).max(1);
        let align: Vec<&str> = (0..columns)
            .map(|i| match table.align.get(i).copied().unwrap_or(Align::Auto) {
                Align::Auto | Align::Left => "start",
                Align::Center => "center",
                Align::Right => "end",
            })
            .collect();
        let mut out = format!(
            "#table(\n  columns: {columns},\n  align: {},\n",
            array(&align.iter().map(|s| s.to_string()).collect::<Vec<_>>())
        );
        let row = |out: &mut String, cells: &[Vec<Inline>], g: &mut Self| {
            for i in 0..columns {
                let cell = cells.get(i).map(|c| g.inlines(c)).unwrap_or_default();
                let _ = write!(out, "[{cell}], ");
            }
        };
        if !table.header.is_empty() {
            out.push_str("  table.header(");
            row(&mut out, &table.header, self);
            out.push_str("),\n");
        }
        for cells in &table.rows {
            out.push_str("  ");
            row(&mut out, cells, self);
            out.push('\n');
        }
        out.push(')');
        out
    }

    fn figure(&mut self, image: &Image, ops: Option<&crate::layout::ImageOps>) -> String {
        let Some(path) = &image.path else {
            return format!("#missing-image({})", string(&image.target));
        };
        let vpath = self.asset(path);
        let caption =
            ops.and_then(|o| o.caption.clone()).or_else(|| Some(image.alt.clone())).filter(|c| !c.trim().is_empty());
        let mut args = vec![string(&vpath)];
        if !image.alt.is_empty() {
            args.push(format!("alt: {}", string(&image.alt)));
        }
        if let Some(caption) = caption {
            args.push(format!("caption: [{}]", escape(&caption)));
        }
        if let Some(percent) = ops.and_then(|o| o.width_percent) {
            args.push(format!("width-ratio: {}", num(percent / 100.0)));
        } else if let Some(px) = image.width_px {
            args.push(format!("width-px: {px}"));
        }
        if let Some(px) = image.height_px {
            args.push(format!("height-px: {px}"));
        }
        if let Some(align) = ops.and_then(|o| o.align) {
            let align = match align {
                HAlign::Left => "left",
                HAlign::Center => "center",
                HAlign::Right => "right",
            };
            args.push(format!("align-to: {align}"));
        }
        let placement = ops.map(|o| o.placement).unwrap_or_default();
        if placement != Placement::Inline {
            let placement = match placement {
                Placement::Inline => "inline",
                Placement::Top => "top",
                Placement::Bottom => "bottom",
                Placement::FullPage => "full-page",
            };
            args.push(format!("placement: {}", string(placement)));
        }
        if is_pdf(path) {
            args.push("scale: 1.0".into());
        }
        format!("#nectar-figure({})", args.join(", "))
    }

    fn asset(&mut self, path: &Path) -> String {
        if let Some(vpath) = self.asset_ids.get(path) {
            return vpath.clone();
        }
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let safe: String =
            name.chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') { c.to_ascii_lowercase() } else { '_' }
                })
                .collect();
        let vpath = format!("/assets/{:04}-{safe}", self.assets.len() + 1);
        self.assets.push(Asset { vpath: vpath.clone(), path: path.to_path_buf() });
        self.asset_ids.insert(path.to_path_buf(), vpath.clone());
        vpath
    }

    fn math(&mut self, latex: &str, block: bool) -> String {
        match mitex::convert_math(latex, None) {
            Ok(code) => format!("#nectar-math({}, block: {block})", string(&code)),
            Err(error) => {
                self.warnings.push(format!("formule non convertie ({error}) : {latex}"));
                format!("#math-fallback({}, block: {block})", string(latex))
            }
        }
    }

    fn inlines(&mut self, inlines: &[Inline]) -> String {
        let mut out = String::new();
        for inline in inlines {
            match inline {
                Inline::Text(text) => out.push_str(&escape(text)),
                Inline::SoftBreak => out.push(' '),
                Inline::LineBreak => out.push_str("#linebreak();"),
                Inline::Code(code) => {
                    let _ = write!(out, "#raw({});", string(code));
                }
                Inline::Emph(c) => wrap(&mut out, "emph", &self.inlines(c)),
                Inline::Strong(c) => wrap(&mut out, "strong", &self.inlines(c)),
                Inline::Strike(c) => wrap(&mut out, "strike", &self.inlines(c)),
                Inline::Highlight(c) => wrap(&mut out, "highlight", &self.inlines(c)),
                Inline::Superscript(c) => wrap(&mut out, "super", &self.inlines(c)),
                Inline::Subscript(c) => wrap(&mut out, "sub", &self.inlines(c)),
                Inline::Link { url, content } => {
                    let label = self.inlines(content);
                    if is_external(url) {
                        let _ = write!(out, "#link({})[{label}];", string(url));
                    } else {
                        out.push_str(&label);
                    }
                }
                Inline::WikiLink { label, .. } => wrap(&mut out, "wikilink", &escape(label)),
                Inline::Image(image) => match &image.path {
                    Some(path) => {
                        let vpath = self.asset(path);
                        let width = image.width_px.map(|w| format!(", width-px: {w}")).unwrap_or_default();
                        let _ = write!(out, "#box(nectar-image({}{width}, inline: true));", string(&vpath));
                    }
                    None => {
                        let _ = write!(out, "#missing-image({}, inline: true);", string(&image.target));
                    }
                },
                Inline::Math(latex) => {
                    let math = self.math(latex, false);
                    out.push_str(&math);
                    out.push(';');
                }
                Inline::Footnote(body) => {
                    let body = self.nodes(body);
                    let _ = write!(out, "#footnote[{body}];");
                }
            }
        }
        out
    }
}

fn wrap(out: &mut String, func: &str, body: &str) {
    let _ = write!(out, "#{func}[{body}];");
}

fn is_external(url: &str) -> bool {
    url.contains("://") || url.starts_with("mailto:")
}

fn is_pdf(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

/// Échappe du texte pour le balisage Typst.
///
/// Tous les caractères qui ont un sens en balisage sont précédés d'une barre
/// oblique inverse. Les guillemets restent libres : Typst les rend en
/// guillemets typographiques selon la langue (« » en français).
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    let mut prev_digit = false;
    for c in text.chars() {
        match c {
            '\\' | '#' | '$' | '*' | '_' | '`' | '<' | '>' | '@' | '[' | ']' | '~' | '=' | '-' | '+' | '/' | ';' => {
                out.push('\\');
                out.push(c);
            }
            // `1.` en début de ligne ouvrirait une liste numérotée.
            '.' if prev_digit => out.push_str("\\."),
            '\n' | '\r' | '\t' => out.push(' '),
            _ => out.push(c),
        }
        prev_digit = c.is_ascii_digit();
    }
    out
}

/// Littéral de chaîne Typst.
pub fn string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

pub(crate) fn array(items: &[String]) -> String {
    match items.len() {
        0 => "()".into(),
        1 => format!("({},)", items[0]),
        _ => format!("({})", items.join(", ")),
    }
}

pub(crate) fn num(value: f32) -> String {
    let text = format!("{value:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text.is_empty() || text == "-" { "0".into() } else { text.to_string() }
}

/// Arguments de `set page(…)` pour un format.
fn page_args(spec: &PageSpec, style: &Style) -> String {
    let mut args = match (spec.width_mm, spec.height_mm) {
        (Some(w), Some(h)) => {
            let (w, h) = if spec.landscape { (w.max(h), w.min(h)) } else { (w, h) };
            vec![format!("width: {}mm", num(w)), format!("height: {}mm", num(h))]
        }
        _ => vec![format!("paper: {}", string(&spec.paper)), format!("flipped: {}", spec.landscape)],
    };
    args.push(match spec.margin_mm {
        Some(mm) => format!("margin: {}mm", num(mm)),
        None => {
            let p = &style.page;
            format!(
                "margin: (top: {}mm, right: {}mm, bottom: {}mm, left: {}mm)",
                num(p.margin_top_mm),
                num(p.margin_right_mm),
                num(p.margin_bottom_mm),
                num(p.margin_left_mm)
            )
        }
    });
    args.join(", ")
}

/// Le format par défaut, passé au template sous forme de dictionnaire.
fn page_dict(spec: &PageSpec) -> String {
    let mm = |v: Option<f32>| v.map(|v| format!("{}mm", num(v))).unwrap_or_else(|| "auto".into());
    format!(
        "(paper: {}, flipped: {}, width: {}, height: {}, margin: {})",
        string(&spec.paper),
        spec.landscape,
        mm(spec.width_mm),
        mm(spec.height_mm),
        mm(spec.margin_mm),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{DefaultPage, PageChange};
    use crate::parse::{ParseOptions, parse};

    fn gen_with(md: &str, edit: impl FnOnce(&Document, &mut Layout)) -> Generated {
        let doc = parse(md, &ParseOptions::default());
        let mut layout = Layout::default();
        edit(&doc, &mut layout);
        generate(&doc, &layout, &Style::default())
    }

    #[test]
    fn escapes_markup() {
        assert_eq!(escape("a*b_c #x $1 // 3.5 @ref <l>"), "a\\*b\\_c \\#x \\$1 \\/\\/ 3\\.5 \\@ref \\<l\\>");
        assert_eq!(string("dit \"oui\"\n\\"), "\"dit \\\"oui\\\"\\n\\\\\"");
    }

    #[test]
    fn page_break_between_intro_and_first_bullet() {
        let g = gen_with("Phrase qui introduit la liste :\n\n- première puce\n- deuxième\n", |doc, layout| {
            let anchors = doc.anchors();
            layout.ops_mut(anchors[2]).break_before = true;
        });
        insta::assert_snapshot!(g.source);
    }

    #[test]
    fn break_inside_ordered_list_keeps_numbering() {
        let g = gen_with("1. un\n2. deux\n3. trois\n", |doc, layout| {
            layout.ops_mut(doc.anchors()[3]).break_before = true;
        });
        assert!(g.source.contains("start: 1"));
        assert!(g.source.contains("start: 3"));
        assert_eq!(g.source.matches("#enum(").count(), 2);
    }

    #[test]
    fn image_page_then_default_page() {
        let g = gen_with("Texte.\n\n![[schema.png]]\n\nExplication.\n\nSuite normale.\n", |doc, layout| {
            let anchors = doc.anchors();
            let fig = layout.ops_mut(anchors[1]);
            fig.page = Some(PageChange::Set(PageSpec::paper("a3", true)));
            layout.ops_mut(anchors[3]).page = Some(PageChange::Default(DefaultPage::Default));
        });
        insta::assert_snapshot!(g.source);
    }

    #[test]
    fn hidden_block_is_skipped_but_keeps_page_change() {
        let g = gen_with("A\n\nB\n\nC\n", |doc, layout| {
            let ops = layout.ops_mut(doc.anchors()[1]);
            ops.hidden = true;
            ops.page = Some(PageChange::Set(PageSpec::paper("a5", false)));
        });
        assert!(!g.source.contains("\nB\n"));
        assert!(g.source.contains("#set page(paper: \"a5\""));
    }

    #[test]
    fn inline_constructs() {
        let g = gen_with(
            "Du **gras**, de l'*italique*, du `code`, un [lien](https://ex.com), [[Note|une note]], ==surligné==, $x^2$ et une note[^n].\n\n[^n]: Bas de page.\n",
            |_, _| {},
        );
        insta::assert_snapshot!(g.source);
    }

    #[test]
    fn callout_table_code() {
        let g = gen_with(
            "> [!tip] Astuce\n> Corps.\n\n| a | b |\n|:--|--:|\n| 1 | 2 |\n\n```rust\nfn main() {}\n```\n",
            |_, _| {},
        );
        insta::assert_snapshot!(g.source);
    }

    #[test]
    fn heading_follows_its_content_to_the_new_page() {
        let g = gen_with("Texte.\n\n## Titre\n\n### Sous-titre\n\n![[schema.png]]\n", |doc, layout| {
            let ops = layout.ops_mut(doc.anchors()[3]);
            ops.page = Some(PageChange::Set(PageSpec::paper("a3", true)));
        });
        let page = g.source.find("#set page(paper: \"a3\"").unwrap();
        let heading = g.source.find("Titre]").unwrap();
        assert!(page < heading, "{}", g.source);
    }

    #[test]
    fn block_at_line_maps_back() {
        let g = gen_with("A\n\nB\n", |_, _| {});
        let (id, line) = &g.block_lines[1];
        assert_eq!(g.block_at_line(*line + 1), Some(id));
    }
}
