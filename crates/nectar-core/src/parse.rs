//! Lecture d'une note Markdown (saveur Obsidian) vers le [`Document`].
//!
//! Gère en plus du CommonMark/GFM : `![[image.png|400]]`, `[[liens]]`,
//! callouts `> [!note] Titre`, `==surlignage==`, `%%commentaires%%`, ids de
//! bloc `^abc`, notes `^[en ligne]`, maths `$…$`, frontmatter YAML, et les
//! retouches écrites dans la note (`<!-- nectar: break-before -->`).

use std::collections::HashMap;
use std::path::Path;

use comrak::nodes::{ListType, NodeValue, TableAlignment};
use comrak::{Arena, Options};

use crate::directives::{self, DirectiveLine};
use crate::html;
use crate::ids::{IdAllocator, excerpt};
use crate::layout::BlockOps;
use crate::model::*;
use crate::vault::Vault;

type AstNode<'a> = comrak::nodes::Node<'a>;

const EXCERPT_CHARS: usize = 80;
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "avif", "pdf"];

/// Où chercher les fichiers référencés par la note.
#[derive(Default, Clone, Copy)]
pub struct ParseOptions<'a> {
    pub vault: Option<&'a Vault>,
    /// Dossier de la note, pour les chemins relatifs.
    pub note_dir: Option<&'a Path>,
    /// Profondeur d'inclusion (`![[note]]` dans une note incluse…).
    pub depth: u8,
}

/// Au-delà, une inclusion est refusée (boucle probable).
const MAX_DEPTH: u8 = 4;

/// Lit une note.
pub fn parse(markdown: &str, options: &ParseOptions<'_>) -> Document {
    let source = preprocess(markdown);
    let arena = Arena::new();
    let root = comrak::parse_document(&arena, &source, &comrak_options());

    let mut cx = Cx { options: *options, warnings: Vec::new(), footnotes: HashMap::new() };
    cx.collect_footnotes(root);

    let mut ids = IdAllocator::default();
    let mut doc = Document::default();
    let mut pending: Option<BlockOps> = None;

    for child in root.children() {
        let line = child.data().sourcepos.start.line;
        let value = child.data().value.clone();
        let node = match value {
            NodeValue::FrontMatter(text) => {
                doc.meta = crate::frontmatter::parse(&text, &mut cx.warnings);
                continue;
            }
            NodeValue::FootnoteDefinition(_) => continue,
            NodeValue::HtmlBlock(html) => match directives::parse_html(&html.literal) {
                DirectiveLine::Ops(ops) => {
                    pending.get_or_insert_with(BlockOps::default).merge(&ops);
                    continue;
                }
                DirectiveLine::Invalid(message) => {
                    cx.warn(line, message);
                    continue;
                }
                DirectiveLine::None => match cx.html_block(&html.literal, line) {
                    Some((node, ops)) => {
                        if let Some(ops) = ops {
                            pending.get_or_insert_with(BlockOps::default).merge(&ops);
                        }
                        node
                    }
                    None => continue,
                },
            },
            NodeValue::Paragraph if is_legacy_pagebreak(child) => {
                pending.get_or_insert_with(BlockOps::default).break_before = true;
                continue;
            }
            _ => match cx.block(child) {
                Some(node) => node,
                None => continue,
            },
        };

        // `![[note]]` seul dans son paragraphe : la note est incluse ici.
        if let Node::Paragraph(content) = &node
            && let Some(target) = single_embed(content)
        {
            let included = cx.transclude(&target, line);
            let mut first = true;
            for block in included {
                let ops = if first { pending.take().or(block.inline_ops) } else { block.inline_ops };
                first = false;
                push_block(&mut doc, &mut ids, block.node, line, ops);
            }
            continue;
        }
        push_block(&mut doc, &mut ids, node, line, pending.take());
    }

    doc.warnings = cx.warnings;
    doc
}

/// Ajoute un bloc au document en lui donnant ses ids (et ceux de ses puces).
fn push_block(doc: &mut Document, ids: &mut IdAllocator, mut node: Node, line: usize, ops: Option<BlockOps>) {
    let text = node_text(&node);
    let id = ids.allocate(node.kind(), &text);
    if let Node::List(list) = &mut node {
        for item in &mut list.items {
            let item_text = item.children.iter().map(node_text).collect::<Vec<_>>().join(" ");
            item.id = Some(ids.allocate(BlockKind::ListItem, &item_text));
        }
    }
    doc.blocks.push(Block { id, line, excerpt: excerpt(&text, EXCERPT_CHARS), node, inline_ops: ops });
}

/// Le paragraphe ne contient qu'une inclusion de note.
fn single_embed(content: &[Inline]) -> Option<String> {
    let meaningful: Vec<&Inline> = content
        .iter()
        .filter(|i| !matches!(i, Inline::SoftBreak | Inline::LineBreak))
        .filter(|i| !matches!(i, Inline::Text(t) if t.trim().is_empty()))
        .collect();
    match meaningful.as_slice() {
        [Inline::Embed { target, .. }] => Some(target.clone()),
        _ => None,
    }
}

/// Lignes à surligner : `{1,3-5}` ou `hl_lines="1 3-5"`.
fn parse_ranges(spec: &str) -> Vec<u32> {
    let mut lines = Vec::new();
    for part in spec.split([',', ' ']).filter(|p| !p.is_empty()) {
        match part.split_once('-') {
            Some((a, b)) => {
                if let (Ok(a), Ok(b)) = (a.trim().parse::<u32>(), b.trim().parse::<u32>()) {
                    lines.extend(a.min(b)..=a.max(b).min(a.min(b) + 10_000));
                }
            }
            None => lines.extend(part.trim().parse::<u32>().ok()),
        }
    }
    lines.sort_unstable();
    lines.dedup();
    lines
}

/// Chaîne d'info d'un bloc de code : langage, titre, lignes surlignées.
/// Formats compris : ` ```rust title="main.rs" {3-5} `, ` ```rust:main.rs `,
/// ` ```python hl_lines="1 3" `.
fn code_info(info: &str) -> (Option<String>, Option<String>, Vec<u32>) {
    let info = info.trim();
    let (head, rest) = info.split_once(char::is_whitespace).unwrap_or((info, ""));
    let (lang, mut title) = match head.split_once(':') {
        Some((lang, file)) if !file.is_empty() => (lang.to_string(), Some(file.to_string())),
        _ => (head.to_string(), None),
    };
    let lang = (!lang.is_empty() && !lang.starts_with('{')).then_some(lang);
    let mut highlight = Vec::new();
    let mut rest = format!("{} {rest}", if head.starts_with('{') { head } else { "" });
    while let (Some(open), Some(close)) = (rest.find('{'), rest.find('}')) {
        if close < open {
            break;
        }
        highlight.extend(parse_ranges(&rest[open + 1..close]));
        rest.replace_range(open..=close, " ");
    }
    for key in ["title", "file", "filename", "hl_lines"] {
        if let Some(value) = html::attr(&format!(" {rest}"), key) {
            if key == "hl_lines" {
                highlight.extend(parse_ranges(&value));
            } else {
                title = Some(value);
            }
        }
    }
    highlight.sort_unstable();
    highlight.dedup();
    (lang, title, highlight)
}

fn comrak_options() -> Options<'static> {
    let mut options = Options::default();
    let ext = &mut options.extension;
    ext.strikethrough = true;
    ext.table = true;
    ext.autolink = true;
    ext.tasklist = true;
    ext.footnotes = true;
    ext.inline_footnotes = true;
    ext.front_matter_delimiter = Some("---".into());
    ext.math_dollars = true;
    ext.wikilinks_title_after_pipe = true;
    ext.highlight = true;
    options.parse.relaxed_tasklist_matching = true;
    options
}

/// Retire ce qu'Obsidian n'affiche pas, sans décaler les numéros de ligne :
/// commentaires `%%…%%` et ids de bloc ` ^abc-123` en fin de ligne.
fn preprocess(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len());
    let mut in_comment = false;
    let mut in_fence: Option<String> = None;
    for line in markdown.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if !in_comment {
            if let Some(fence) = &in_fence {
                if trimmed.starts_with(fence.as_str()) {
                    in_fence = None;
                }
                out.push_str(line);
                continue;
            }
            if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                in_fence = Some(trimmed[..3].to_string());
                out.push_str(line);
                continue;
            }
        }

        let mut kept = String::with_capacity(line.len());
        let mut rest = line;
        loop {
            match rest.find("%%") {
                Some(at) => {
                    if !in_comment {
                        kept.push_str(&rest[..at]);
                    }
                    in_comment = !in_comment;
                    rest = &rest[at + 2..];
                }
                None => {
                    if !in_comment {
                        kept.push_str(rest);
                    } else if rest.ends_with('\n') {
                        kept.push('\n');
                    }
                    break;
                }
            }
        }
        out.push_str(&rewrite_embeds(&strip_block_id(&kept)));
    }
    out
}

/// `![[cible|alias]]` → `![alias](<cible>)` : comrak lit `![` comme le début
/// d'une image et ne verrait jamais le lien wiki.
fn rewrite_embeds(line: &str) -> String {
    if !line.contains("![[") {
        return line.to_string();
    }
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    let mut in_code = false;
    while !rest.is_empty() {
        if !in_code
            && rest.starts_with("![[")
            && let Some(end) = rest.find("]]")
        {
            let inner = &rest[3..end];
            let (target, alias) = inner.split_once('|').unwrap_or((inner, ""));
            out.push_str(&format!("![{}](<{}>)", alias.replace(['[', ']'], ""), target.trim()));
            rest = &rest[end + 2..];
            continue;
        }
        let c = rest.chars().next().unwrap_or_default();
        if c == '`' {
            in_code = !in_code;
        }
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

fn strip_block_id(line: &str) -> String {
    let (body, newline) = match line.strip_suffix('\n') {
        Some(body) => (body.trim_end_matches('\r'), "\n"),
        None => (line, ""),
    };
    if let Some(at) = body.rfind(" ^") {
        let id = &body[at + 2..];
        if !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return format!("{}{newline}", body[..at].trim_end());
        }
    }
    line.to_string()
}

/// `\pagebreak` ou `[[PAGEBREAK]]` seul dans un paragraphe (anciennes notes).
fn is_legacy_pagebreak(node: AstNode<'_>) -> bool {
    let mut text = String::new();
    for d in node.descendants() {
        match &d.data().value {
            NodeValue::Text(t) => text.push_str(t),
            NodeValue::WikiLink(link) => text.push_str(&format!("[[{}]]", link.url)),
            NodeValue::Paragraph => {}
            _ => return false,
        }
    }
    let text = text.trim();
    text == "\\pagebreak" || text.eq_ignore_ascii_case("[[pagebreak]]") || text == "pagebreak"
}

struct Cx<'o> {
    options: ParseOptions<'o>,
    warnings: Vec<String>,
    footnotes: HashMap<String, Vec<Node>>,
}

impl Cx<'_> {
    fn warn(&mut self, line: usize, message: impl std::fmt::Display) {
        self.warnings.push(format!("ligne {line} : {message}"));
    }

    fn collect_footnotes<'a>(&mut self, root: AstNode<'a>) {
        for node in root.children() {
            let name = match &node.data().value {
                NodeValue::FootnoteDefinition(def) => def.name.clone(),
                _ => continue,
            };
            let body = node.children().filter_map(|c| self.block(c)).collect();
            self.footnotes.insert(name, body);
        }
    }

    /// Convertit un nœud de bloc ; `None` pour ce qui ne s'affiche pas.
    fn block<'a>(&mut self, node: AstNode<'a>) -> Option<Node> {
        let line = node.data().sourcepos.start.line;
        let value = node.data().value.clone();
        match value {
            NodeValue::Paragraph => {
                let parts: Vec<_> = node
                    .children()
                    .filter(|c| !matches!(c.data().value, NodeValue::SoftBreak | NodeValue::LineBreak))
                    .collect();
                if let [only] = parts.as_slice()
                    && let NodeValue::Math(math) = &only.data().value
                    && math.display_math
                {
                    return Some(Node::Math(math.literal.trim().to_string()));
                }
                let content = self.inlines(node);
                Some(paragraph_or_figure(content))
            }
            NodeValue::Heading(h) => Some(Node::Heading { level: h.level, content: self.inlines(node) }),
            NodeValue::List(list) => Some(Node::List(self.list(node, list))),
            NodeValue::CodeBlock(code) => {
                let (lang, title, highlight) = code_info(&code.info);
                let mut text = code.literal;
                if text.ends_with('\n') {
                    text.pop();
                }
                Some(Node::Code { lang, text, title, highlight })
            }
            NodeValue::BlockQuote | NodeValue::MultilineBlockQuote(_) => Some(self.quote(node)),
            NodeValue::Table(table) => Some(self.table(node, &table.alignments)),
            NodeValue::ThematicBreak => Some(Node::Rule),
            NodeValue::HtmlBlock(html) => self.html_block(&html.literal, line).map(|(node, _)| node),
            NodeValue::FootnoteDefinition(_) | NodeValue::FrontMatter(_) => None,
            other => {
                self.warn(line, format!("élément non géré ({other:?})"));
                None
            }
        }
    }

    fn list<'a>(&mut self, node: AstNode<'a>, list: comrak::nodes::NodeList) -> List {
        let items = node
            .children()
            .map(|item| {
                let task = match &item.data().value {
                    NodeValue::TaskItem(t) => Some(t.symbol.is_some_and(|s| s != ' ')),
                    _ => None,
                };
                let children: Vec<Node> = item.children().filter_map(|c| self.block(c)).collect();
                let text = children.iter().map(node_text).collect::<Vec<_>>().join(" ");
                ListItem {
                    id: None,
                    line: item.data().sourcepos.start.line,
                    excerpt: excerpt(&text, EXCERPT_CHARS),
                    task,
                    children,
                }
            })
            .collect();
        List { ordered: list.list_type == ListType::Ordered, start: list.start.max(1), tight: list.tight, items }
    }

    fn quote<'a>(&mut self, node: AstNode<'a>) -> Node {
        let mut children: Vec<Node> = node.children().filter_map(|c| self.block(c)).collect();
        if let Some(Node::Paragraph(first)) = children.first()
            && let Some((kind, title, rest)) = split_callout(first)
        {
            children.remove(0);
            if !rest.is_empty() {
                children.insert(0, paragraph_or_figure(rest));
            }
            return Node::Callout(Callout { kind, title, body: children });
        }
        Node::Quote(children)
    }

    fn table<'a>(&mut self, node: AstNode<'a>, alignments: &[TableAlignment]) -> Node {
        let align = alignments
            .iter()
            .map(|a| match a {
                TableAlignment::None => Align::Auto,
                TableAlignment::Left => Align::Left,
                TableAlignment::Center => Align::Center,
                TableAlignment::Right => Align::Right,
            })
            .collect();
        let mut header = Vec::new();
        let mut rows = Vec::new();
        for row in node.children() {
            let is_header = matches!(row.data().value, NodeValue::TableRow(true));
            let cells: Vec<Vec<Inline>> = row.children().map(|cell| self.inlines(cell)).collect();
            if is_header {
                header = cells;
            } else {
                rows.push(cells);
            }
        }
        Node::Table(Table { align, header, rows })
    }

    fn inlines<'a>(&mut self, node: AstNode<'a>) -> Vec<Inline> {
        let mut out: Vec<Inline> = Vec::new();
        for child in node.children() {
            let line = child.data().sourcepos.start.line;
            let value = child.data().value.clone();
            match value {
                NodeValue::Text(text) => push_text(&mut out, &text),
                NodeValue::Code(code) => out.push(Inline::Code(code.literal)),
                NodeValue::SoftBreak => out.push(Inline::SoftBreak),
                NodeValue::LineBreak => out.push(Inline::LineBreak),
                NodeValue::Emph => out.push(Inline::Emph(self.inlines(child))),
                NodeValue::Strong => out.push(Inline::Strong(self.inlines(child))),
                NodeValue::Strikethrough => out.push(Inline::Strike(self.inlines(child))),
                NodeValue::Highlight => out.push(Inline::Highlight(self.inlines(child))),
                NodeValue::Superscript => out.push(Inline::Superscript(self.inlines(child))),
                NodeValue::Subscript => out.push(Inline::Subscript(self.inlines(child))),
                NodeValue::Escaped | NodeValue::Underline | NodeValue::Insert | NodeValue::SpoileredText => {
                    for inline in self.inlines(child) {
                        match inline {
                            Inline::Text(t) => push_text(&mut out, &t),
                            other => out.push(other),
                        }
                    }
                }
                NodeValue::Link(link) => {
                    let content = self.inlines(child);
                    out.push(Inline::Link { url: link.url.clone(), content });
                }
                NodeValue::Image(link) => {
                    let alt = plain_text(&self.inlines(child));
                    if is_image(&link.url) || link.url.contains("://") {
                        let (alt, width, height) = split_size(&alt);
                        let image = self.image(&link.url, alt, width, height, line);
                        out.push(Inline::Image(image));
                    } else {
                        self.embed(&mut out, &link.url, &alt, line);
                    }
                }
                NodeValue::WikiLink(link) => {
                    let label = plain_text(&self.inlines(child));
                    let embed = matches!(out.last(), Some(Inline::Text(t)) if t.ends_with('!'));
                    if embed {
                        if let Some(Inline::Text(t)) = out.last_mut() {
                            t.pop();
                            if t.is_empty() {
                                out.pop();
                            }
                        }
                        self.embed(&mut out, &link.url, &label, line);
                    } else {
                        let label =
                            if label.is_empty() || label == link.url { wikilink_label(&link.url) } else { label };
                        out.push(Inline::WikiLink { target: link.url.clone(), label });
                    }
                }
                NodeValue::Math(math) => out.push(Inline::Math(math.literal.trim().to_string())),
                NodeValue::FootnoteReference(r) => match self.footnotes.get(&r.name) {
                    Some(body) => out.push(Inline::Footnote(body.clone())),
                    None => self.warn(line, format!("note de bas de page « {} » introuvable", r.name)),
                },
                NodeValue::HtmlInline(html) => {
                    let tag = html.trim().to_string();
                    let lower = tag.to_ascii_lowercase();
                    if lower.starts_with("<br") {
                        out.push(Inline::LineBreak);
                    } else if lower.starts_with("<img") {
                        match html::attr(&tag, "src") {
                            Some(src) => {
                                let width = html::attr(&tag, "width").and_then(|w| html::pixels(&w));
                                let height = html::attr(&tag, "height").and_then(|h| html::pixels(&h));
                                let alt = html::attr(&tag, "alt").unwrap_or_default();
                                out.push(Inline::Image(self.image(&src, alt, width, height, line)));
                            }
                            None => self.warn(line, "balise <img> sans src"),
                        }
                    } else if !lower.starts_with("<!--") {
                        out.push(Inline::Html(tag));
                    }
                }
                NodeValue::Raw(raw) => push_text(&mut out, &raw),
                other => self.warn(line, format!("élément en ligne non géré ({other:?})")),
            }
        }
        html::fold(out)
    }

    /// Bloc HTML : images (`<img>`, centrées ou non) et texte simplifié.
    fn html_block(&mut self, literal: &str, line: usize) -> Option<(Node, Option<BlockOps>)> {
        let trimmed = literal.trim();
        if trimmed.is_empty() || trimmed.starts_with("<!--") {
            return None;
        }
        let centered = html::centered(trimmed);
        let images: Vec<Image> = html::tags(trimmed, "img")
            .into_iter()
            .filter_map(|tag| {
                let src = html::attr(&tag, "src")?;
                let width = html::attr(&tag, "width").and_then(|w| html::pixels(&w));
                let height = html::attr(&tag, "height").and_then(|h| html::pixels(&h));
                let alt = html::attr(&tag, "alt").unwrap_or_default();
                Some(self.image(&src, alt, width, height, line))
            })
            .collect();
        let text = html::strip(trimmed);
        let ops = centered.then(|| BlockOps {
            image: Some(crate::layout::ImageOps { align: Some(crate::layout::HAlign::Center), ..Default::default() }),
            ..Default::default()
        });
        match (images.len(), text.trim().is_empty()) {
            (1, true) => Some((Node::Figure(images.into_iter().next().expect("une image")), ops)),
            (0, true) => None,
            _ => {
                let mut content: Vec<Inline> = Vec::new();
                if !text.trim().is_empty() {
                    content.push(Inline::Text(text.trim().to_string()));
                    self.warn(line, "bloc HTML simplifié en texte");
                }
                for image in images {
                    if !content.is_empty() {
                        content.push(Inline::Text(" ".into()));
                    }
                    content.push(Inline::Image(image));
                }
                Some((Node::Paragraph(content), None))
            }
        }
    }

    /// `![[cible|alias]]` : une image, ou une note incluse (pas encore gérée).
    fn embed(&mut self, out: &mut Vec<Inline>, target: &str, alias: &str, line: usize) {
        if is_image(target) {
            let (alt, width, height) = split_size(alias);
            let alt = if alias.is_empty() || width.is_some() { String::new() } else { alt };
            out.push(Inline::Image(self.image(target, alt, width, height, line)));
        } else {
            let label = if alias.is_empty() { wikilink_label(target) } else { alias.to_string() };
            out.push(Inline::Embed { target: target.to_string(), label });
        }
    }

    /// Les blocs d'une note incluse (`![[note]]`, `![[note#Titre]]`).
    fn transclude(&mut self, target: &str, line: usize) -> Vec<Block> {
        if self.options.depth >= MAX_DEPTH {
            self.warn(line, format!("inclusion trop profonde ignorée : {target}"));
            return Vec::new();
        }
        let (name, section) = match target.split_once('#') {
            Some((name, section)) => (name, Some(section)),
            None => (target, None),
        };
        let file = if Path::new(name).extension().is_some() { name.to_string() } else { format!("{name}.md") };
        let vault_here;
        let vault = match self.options.vault {
            Some(vault) => vault,
            None => {
                vault_here = Vault::at(self.options.note_dir.unwrap_or(Path::new(".")));
                &vault_here
            }
        };
        let dir = self.options.note_dir.unwrap_or(vault.root());
        let Some(path) = vault.resolve(&file, dir) else {
            self.warn(line, format!("note incluse introuvable : {name}"));
            return Vec::new();
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            self.warn(line, format!("note incluse illisible : {}", path.display()));
            return Vec::new();
        };
        let sub_dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let options = ParseOptions { vault: Some(vault), note_dir: Some(&sub_dir), depth: self.options.depth + 1 };
        let mut included = parse(&text, &options);
        self.warnings.extend(included.warnings.drain(..).map(|w| format!("{name} : {w}")));
        let Some(section) = section else { return included.blocks };
        if section.starts_with('^') {
            self.warn(line, format!("inclusion d'un bloc ^id non gérée : {target}"));
            return Vec::new();
        }
        let wanted = crate::ids::slug(section);
        let start = included.blocks.iter().position(
            |b| matches!(&b.node, Node::Heading { content, .. } if crate::ids::slug(&plain_text(content)) == wanted),
        );
        let Some(start) = start else {
            self.warn(line, format!("section « {section} » introuvable dans {name}"));
            return Vec::new();
        };
        let level = match &included.blocks[start].node {
            Node::Heading { level, .. } => *level,
            _ => 1,
        };
        let end = included.blocks[start + 1..]
            .iter()
            .position(|b| matches!(&b.node, Node::Heading { level: l, .. } if *l <= level))
            .map(|i| start + 1 + i)
            .unwrap_or(included.blocks.len());
        included.blocks.drain(start..end).collect()
    }

    fn image(&mut self, target: &str, alt: String, width: Option<u32>, height: Option<u32>, line: usize) -> Image {
        let path = match (self.options.vault, self.options.note_dir) {
            (Some(vault), Some(dir)) => vault.resolve(target, dir),
            (Some(vault), None) => vault.resolve(target, vault.root()),
            (None, Some(dir)) => Vault::at(dir).resolve(target, dir),
            (None, None) => None,
        };
        if path.is_none() && (self.options.vault.is_some() || self.options.note_dir.is_some()) {
            if target.contains("://") {
                self.warn(line, format!("image distante ignorée (tout reste local) : {target}"));
            } else {
                self.warn(line, format!("image introuvable : {target}"));
            }
        }
        let page = target.split_once("#page=").and_then(|(_, p)| p.trim().parse().ok());
        Image { target: target.to_string(), path, alt, width_px: width, height_px: height, page }
    }
}

fn push_text(out: &mut Vec<Inline>, text: &str) {
    match out.last_mut() {
        Some(Inline::Text(prev)) => prev.push_str(text),
        _ => out.push(Inline::Text(text.to_string())),
    }
}

/// Un paragraphe qui ne contient qu'une image devient une figure ; une
/// équation seule devient un bloc d'équation.
fn paragraph_or_figure(content: Vec<Inline>) -> Node {
    let meaningful: Vec<&Inline> = content
        .iter()
        .filter(|i| !matches!(i, Inline::SoftBreak | Inline::LineBreak))
        .filter(|i| !matches!(i, Inline::Text(t) if t.trim().is_empty()))
        .collect();
    if let [Inline::Image(image)] = meaningful.as_slice() {
        return Node::Figure(image.clone());
    }
    Node::Paragraph(content)
}

/// `400`, `400x300` ou `légende|400` → (texte, largeur, hauteur).
fn split_size(alt: &str) -> (String, Option<u32>, Option<u32>) {
    let (text, size) = match alt.rsplit_once('|') {
        Some((text, size)) => (text.trim(), size.trim()),
        None => ("", alt.trim()),
    };
    let parsed = match size.split_once('x') {
        Some((w, h)) => w.parse().ok().map(|w| (Some(w), h.parse().ok())),
        None => size.parse().ok().map(|w| (Some(w), None)),
    };
    match parsed {
        Some((w, h)) => (text.to_string(), w, h),
        None => (alt.to_string(), None, None),
    }
}

fn is_image(target: &str) -> bool {
    Path::new(target.split(['#', '|']).next().unwrap_or(target))
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| IMAGE_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

fn wikilink_label(target: &str) -> String {
    let target = target.trim_start_matches('#');
    let name = target.rsplit('/').next().unwrap_or(target);
    let name = name.strip_suffix(".md").unwrap_or(name);
    name.replace('#', " › ")
}

/// Détecte `[!type]± Titre` en tête du premier paragraphe d'une citation.
fn split_callout(first: &[Inline]) -> Option<(String, Option<Vec<Inline>>, Vec<Inline>)> {
    let Some(Inline::Text(head)) = first.first() else { return None };
    let rest = head.strip_prefix("[!")?;
    let close = rest.find(']')?;
    let kind = rest[..close].trim().to_lowercase();
    if kind.is_empty() || !kind.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        return None;
    }
    let after = rest[close + 1..].trim_start_matches(['+', '-']);

    // Le titre court jusqu'à la fin de la première ligne.
    let mut title: Vec<Inline> = Vec::new();
    let mut body: Vec<Inline> = Vec::new();
    let mut in_title = true;
    if !after.trim().is_empty() {
        title.push(Inline::Text(after.trim_start().to_string()));
    }
    for inline in &first[1..] {
        if in_title && matches!(inline, Inline::SoftBreak | Inline::LineBreak) {
            in_title = false;
            continue;
        }
        if in_title { title.push(inline.clone()) } else { body.push(inline.clone()) }
    }
    let title = (!title.is_empty()).then_some(title);
    Some((kind, title, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(md: &str) -> Document {
        parse(md, &ParseOptions::default())
    }

    #[test]
    fn standalone_image_becomes_figure() {
        let doc = p("Texte\n\n![[schema.png|400]]\n\nSuite");
        let Node::Figure(img) = &doc.blocks[1].node else { panic!("{:?}", doc.blocks[1].node) };
        assert_eq!(img.target, "schema.png");
        assert_eq!(img.width_px, Some(400));
        assert_eq!(doc.blocks[1].id.0.split('-').next(), Some("fig"));
    }

    #[test]
    fn markdown_image_with_size() {
        let doc = p("![Une vue|300x200](img/vue.jpg)");
        let Node::Figure(img) = &doc.blocks[0].node else { panic!() };
        assert_eq!((img.alt.as_str(), img.width_px, img.height_px), ("Une vue", Some(300), Some(200)));
    }

    #[test]
    fn callout_with_title_and_body() {
        let doc = p("> [!warning]- Attention ici\n> Le corps du texte.\n> Encore.");
        let Node::Callout(c) = &doc.blocks[0].node else { panic!("{:?}", doc.blocks[0].node) };
        assert_eq!(c.kind, "warning");
        assert_eq!(plain_text(c.title.as_ref().unwrap()), "Attention ici");
        assert_eq!(node_text(&c.body[0]), "Le corps du texte. Encore.");
    }

    #[test]
    fn list_items_get_ids() {
        let doc = p("Intro\n\n- un\n- deux\n  - imbriqué\n");
        let Node::List(list) = &doc.blocks[1].node else { panic!() };
        assert!(list.items.iter().all(|i| i.id.is_some()));
        let Node::List(nested) = &list.items[1].children[1] else { panic!() };
        assert!(nested.items[0].id.is_none());
        assert_eq!(doc.anchors().len(), 4);
    }

    #[test]
    fn obsidian_comments_and_block_ids_are_removed() {
        let doc = p("Visible %%caché%% texte ^abc-1\n\n%%\nbloc\ncaché\n%%\n\nFin");
        assert_eq!(doc.blocks.len(), 2);
        assert_eq!(node_text(&doc.blocks[0].node), "Visible  texte");
        assert_eq!(doc.blocks[1].line, 8);
    }

    #[test]
    fn inline_directives_attach_to_next_block() {
        let doc = p(
            "A\n\n<!-- nectar: break-before, page=a3-paysage -->\n\nB\n\n<!-- pagebreak -->\n\nC\n\n\\pagebreak\n\nD",
        );
        assert_eq!(doc.blocks.len(), 4);
        let ops = doc.blocks[1].inline_ops.as_ref().unwrap();
        assert!(ops.break_before && ops.page.is_some());
        assert!(doc.blocks[2].inline_ops.as_ref().unwrap().break_before);
        assert!(doc.blocks[3].inline_ops.as_ref().unwrap().break_before);
    }

    #[test]
    fn frontmatter_and_footnotes() {
        let doc = p(
            "---\ntitle: Mon rapport\nauthor: [A, B]\ntags: [x]\n---\n\nTexte[^1] et ^[en ligne].\n\n[^1]: La note.\n",
        );
        assert_eq!(doc.meta.title.as_deref(), Some("Mon rapport"));
        assert_eq!(doc.meta.author.as_deref(), Some("A, B"));
        let Node::Paragraph(content) = &doc.blocks[0].node else { panic!() };
        assert_eq!(content.iter().filter(|i| matches!(i, Inline::Footnote(_))).count(), 2);
    }

    #[test]
    fn code_info_strings() {
        assert_eq!(
            code_info("rust title=\"src/main.rs\" {2,4-5}"),
            (Some("rust".into()), Some("src/main.rs".into()), vec![2, 4, 5])
        );
        assert_eq!(code_info("python:outils.py"), (Some("python".into()), Some("outils.py".into()), vec![]));
        assert_eq!(code_info("py hl_lines=\"1 3\""), (Some("py".into()), None, vec![1, 3]));
        assert_eq!(code_info(""), (None, None, vec![]));
    }

    #[test]
    fn note_sections_are_included() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Autre.md"), "# A\n\nintro\n\n## B\n\ndedans\n\n## C\n\ndehors\n").unwrap();
        let options = ParseOptions { note_dir: Some(dir.path()), ..Default::default() };
        let doc = parse("Avant\n\n![[Autre#B]]\n\nAprès [[#B]] ![[Autre]] en ligne.\n", &options);
        let texts: Vec<String> = doc.blocks.iter().map(|b| node_text(&b.node)).collect();
        assert_eq!(texts[..3], ["Avant", "B", "dedans"]);
        assert!(texts[3].starts_with("Après B"), "{texts:?}");
        assert!(doc.warnings.is_empty(), "{:?}", doc.warnings);
    }

    #[test]
    fn common_html() {
        let doc = p(
            "Du <u>souligné</u>, <sup>2</sup>, <mark>marqué</mark>, <kbd>Ctrl</kbd> et <b><i>gras</i></b>.\n\n<p align=\"center\"><img src=\"x.png\" width=\"300\"></p>\n\nTexte <img src='y.png' width=50> fin.\n",
        );
        let Node::Paragraph(c) = &doc.blocks[0].node else { panic!() };
        assert!(c.iter().any(|i| matches!(i, Inline::Underline(_))));
        assert!(c.iter().any(|i| matches!(i, Inline::Superscript(_))));
        assert!(c.iter().any(|i| matches!(i, Inline::Highlight(_))));
        assert!(c.iter().any(|i| matches!(i, Inline::Kbd(k) if k == "Ctrl")));
        assert!(c.iter().any(|i| matches!(i, Inline::Strong(inner) if matches!(inner[0], Inline::Emph(_)))));
        assert!(!c.iter().any(|i| matches!(i, Inline::Html(_))));
        let Node::Figure(img) = &doc.blocks[1].node else { panic!("{:?}", doc.blocks[1].node) };
        assert_eq!((img.target.as_str(), img.width_px), ("x.png", Some(300)));
        let ops = doc.blocks[1].inline_ops.as_ref().unwrap();
        assert_eq!(ops.image.as_ref().unwrap().align, Some(crate::layout::HAlign::Center));
        let Node::Paragraph(c) = &doc.blocks[2].node else { panic!() };
        assert!(c.iter().any(|i| matches!(i, Inline::Image(img) if img.width_px == Some(50))));
    }

    #[test]
    fn highlight_math_and_tasks() {
        let doc = p("Un ==mot== et $x^2$.\n\n$$\n\\frac{a}{b}\n$$\n\n- [x] fait\n- [ ] à faire\n");
        let Node::Paragraph(content) = &doc.blocks[0].node else { panic!() };
        assert!(content.iter().any(|i| matches!(i, Inline::Highlight(_))));
        assert!(content.iter().any(|i| matches!(i, Inline::Math(m) if m == "x^2")));
        assert!(matches!(&doc.blocks[1].node, Node::Math(m) if m.contains("frac")), "{:?}", doc.blocks[1].node);
        let Node::List(list) = &doc.blocks[2].node else { panic!() };
        assert_eq!(list.items.iter().map(|i| i.task).collect::<Vec<_>>(), vec![Some(true), Some(false)]);
    }
}
