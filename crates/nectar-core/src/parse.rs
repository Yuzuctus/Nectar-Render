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
}

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
            NodeValue::HtmlBlock(html) => {
                match directives::parse_html(&html.literal) {
                    DirectiveLine::Ops(ops) => pending.get_or_insert_with(BlockOps::default).merge(&ops),
                    DirectiveLine::Invalid(message) => cx.warn(line, message),
                    DirectiveLine::None => {}
                }
                continue;
            }
            NodeValue::Paragraph if is_legacy_pagebreak(child) => {
                pending.get_or_insert_with(BlockOps::default).break_before = true;
                continue;
            }
            _ => match cx.block(child) {
                Some(node) => node,
                None => continue,
            },
        };

        let text = node_text(&node);
        let id = ids.allocate(node.kind(), &text);
        let mut node = node;
        if let Node::List(list) = &mut node {
            for item in &mut list.items {
                let item_text = item.children.iter().map(node_text).collect::<Vec<_>>().join(" ");
                item.id = Some(ids.allocate(BlockKind::ListItem, &item_text));
            }
        }
        doc.blocks.push(Block { id, line, excerpt: excerpt(&text, EXCERPT_CHARS), node, inline_ops: pending.take() });
    }

    doc.warnings = cx.warnings;
    doc
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
                let lang = code.info.split_whitespace().next().map(str::to_string);
                let mut text = code.literal;
                if text.ends_with('\n') {
                    text.pop();
                }
                Some(Node::Code { lang, text })
            }
            NodeValue::BlockQuote | NodeValue::MultilineBlockQuote(_) => Some(self.quote(node)),
            NodeValue::Table(table) => Some(self.table(node, &table.alignments)),
            NodeValue::ThematicBreak => Some(Node::Rule),
            NodeValue::HtmlBlock(html) => {
                if !html.literal.trim_start().starts_with("<!--") {
                    self.warn(line, "bloc HTML ignoré");
                }
                None
            }
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
                        let label = if label.is_empty() { wikilink_label(&link.url) } else { label };
                        out.push(Inline::WikiLink { target: link.url.clone(), label });
                    }
                }
                NodeValue::Math(math) => out.push(Inline::Math(math.literal.trim().to_string())),
                NodeValue::FootnoteReference(r) => match self.footnotes.get(&r.name) {
                    Some(body) => out.push(Inline::Footnote(body.clone())),
                    None => self.warn(line, format!("note de bas de page « {} » introuvable", r.name)),
                },
                NodeValue::HtmlInline(html) => {
                    let tag = html.trim().to_ascii_lowercase();
                    if tag.starts_with("<br") {
                        out.push(Inline::LineBreak);
                    }
                }
                NodeValue::Raw(raw) => push_text(&mut out, &raw),
                other => self.warn(line, format!("élément en ligne non géré ({other:?})")),
            }
        }
        out
    }

    /// `![[cible|alias]]` : une image, ou une note incluse (pas encore gérée).
    fn embed(&mut self, out: &mut Vec<Inline>, target: &str, alias: &str, line: usize) {
        if is_image(target) {
            let (alt, width, height) = split_size(alias);
            let alt = if alias.is_empty() || width.is_some() { String::new() } else { alt };
            out.push(Inline::Image(self.image(target, alt, width, height, line)));
        } else {
            self.warn(line, format!("l'inclusion de note « {target} » n'est pas encore gérée"));
            let label = if alias.is_empty() { wikilink_label(target) } else { alias.to_string() };
            out.push(Inline::WikiLink { target: target.to_string(), label });
        }
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
        Image { target: target.to_string(), path, alt, width_px: width, height_px: height }
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
