//! Le modèle de document : ce que Nectar comprend d'une note, indépendamment
//! du Markdown d'origine et de Typst.
//!
//! Seuls les blocs de premier niveau (et les éléments des listes de premier
//! niveau) portent un identifiant : ce sont eux que l'utilisateur retouche.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Identifiant stable d'un bloc, dérivé de son type et de son contenu.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BlockId(pub String);

impl BlockId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for BlockId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.pad(&self.0)
    }
}

/// Une note analysée.
#[derive(Debug, Clone, Default)]
pub struct Document {
    pub meta: Meta,
    pub blocks: Vec<Block>,
    /// Problèmes non bloquants rencontrés à la lecture (image introuvable…).
    pub warnings: Vec<String>,
}

impl Document {
    /// Tous les blocs adressables, éléments de liste compris, dans l'ordre.
    pub fn anchors(&self) -> Vec<AnchorInfo<'_>> {
        let mut out = Vec::new();
        for block in &self.blocks {
            out.push(AnchorInfo { id: &block.id, kind: block.node.kind(), line: block.line, excerpt: &block.excerpt });
            if let Node::List(list) = &block.node {
                for item in &list.items {
                    if let Some(id) = &item.id {
                        out.push(AnchorInfo { id, kind: BlockKind::ListItem, line: item.line, excerpt: &item.excerpt });
                    }
                }
            }
        }
        out
    }
}

/// Vue légère d'un bloc adressable.
#[derive(Debug, Clone, Copy)]
pub struct AnchorInfo<'a> {
    pub id: &'a BlockId,
    pub kind: BlockKind,
    pub line: usize,
    pub excerpt: &'a str,
}

/// Métadonnées lues dans le frontmatter YAML.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Meta {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub author: Option<String>,
    pub date: Option<String>,
    pub lang: Option<String>,
    pub tags: Vec<String>,
}

/// Bloc de premier niveau.
#[derive(Debug, Clone)]
pub struct Block {
    pub id: BlockId,
    /// Ligne de début dans le fichier source (1 = première ligne).
    pub line: usize,
    /// Début du texte du bloc, pour retrouver un bloc dont l'id a changé.
    pub excerpt: String,
    pub node: Node,
    /// Retouches écrites directement dans la note (`<!-- nectar: … -->`).
    pub inline_ops: Option<crate::layout::BlockOps>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BlockKind {
    Heading,
    Paragraph,
    Figure,
    List,
    ListItem,
    Code,
    Quote,
    Callout,
    Table,
    Math,
    Rule,
}

impl BlockKind {
    pub fn prefix(self) -> &'static str {
        match self {
            BlockKind::Heading => "h",
            BlockKind::Paragraph => "p",
            BlockKind::Figure => "fig",
            BlockKind::List => "list",
            BlockKind::ListItem => "li",
            BlockKind::Code => "code",
            BlockKind::Quote => "quote",
            BlockKind::Callout => "callout",
            BlockKind::Table => "table",
            BlockKind::Math => "math",
            BlockKind::Rule => "rule",
        }
    }

    pub fn label_fr(self) -> &'static str {
        match self {
            BlockKind::Heading => "Titre",
            BlockKind::Paragraph => "Paragraphe",
            BlockKind::Figure => "Image",
            BlockKind::List => "Liste",
            BlockKind::ListItem => "Puce",
            BlockKind::Code => "Code",
            BlockKind::Quote => "Citation",
            BlockKind::Callout => "Encadré",
            BlockKind::Table => "Tableau",
            BlockKind::Math => "Équation",
            BlockKind::Rule => "Séparateur",
        }
    }
}

/// Contenu de niveau bloc (récursif).
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Heading {
        level: u8,
        content: Vec<Inline>,
    },
    Paragraph(Vec<Inline>),
    /// Une image seule dans son paragraphe.
    Figure(Image),
    List(List),
    Code {
        lang: Option<String>,
        text: String,
    },
    Quote(Vec<Node>),
    Callout(Callout),
    Table(Table),
    Math(String),
    Rule,
}

impl Node {
    pub fn kind(&self) -> BlockKind {
        match self {
            Node::Heading { .. } => BlockKind::Heading,
            Node::Paragraph(_) => BlockKind::Paragraph,
            Node::Figure(_) => BlockKind::Figure,
            Node::List(_) => BlockKind::List,
            Node::Code { .. } => BlockKind::Code,
            Node::Quote(_) => BlockKind::Quote,
            Node::Callout(_) => BlockKind::Callout,
            Node::Table(_) => BlockKind::Table,
            Node::Math(_) => BlockKind::Math,
            Node::Rule => BlockKind::Rule,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct List {
    pub ordered: bool,
    pub start: usize,
    pub tight: bool,
    pub items: Vec<ListItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListItem {
    /// Présent seulement pour les éléments d'une liste de premier niveau.
    pub id: Option<BlockId>,
    pub line: usize,
    pub excerpt: String,
    /// `Some(true)` pour `- [x]`, `Some(false)` pour `- [ ]`.
    pub task: Option<bool>,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Callout {
    /// Type Obsidian en minuscules : `note`, `tip`, `warning`…
    pub kind: String,
    pub title: Option<Vec<Inline>>,
    pub body: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub align: Vec<Align>,
    pub header: Vec<Vec<Inline>>,
    pub rows: Vec<Vec<Vec<Inline>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Align {
    Auto,
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    /// Cible telle qu'écrite dans la note.
    pub target: String,
    /// Fichier trouvé sur le disque, s'il existe.
    pub path: Option<PathBuf>,
    pub alt: String,
    /// Largeur demandée en pixels (`![[img.png|400]]`).
    pub width_px: Option<u32>,
    pub height_px: Option<u32>,
}

/// Contenu en ligne.
#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    Text(String),
    Code(String),
    Emph(Vec<Inline>),
    Strong(Vec<Inline>),
    Strike(Vec<Inline>),
    Highlight(Vec<Inline>),
    Superscript(Vec<Inline>),
    Subscript(Vec<Inline>),
    Link {
        url: String,
        content: Vec<Inline>,
    },
    /// Lien interne Obsidian `[[Note|libellé]]` : sans cible dans un PDF.
    WikiLink {
        target: String,
        label: String,
    },
    Image(Image),
    Math(String),
    Footnote(Vec<Node>),
    LineBreak,
    SoftBreak,
}

/// Texte brut d'une suite d'éléments en ligne.
pub fn plain_text(inlines: &[Inline]) -> String {
    let mut out = String::new();
    push_plain(inlines, &mut out);
    out
}

fn push_plain(inlines: &[Inline], out: &mut String) {
    for inline in inlines {
        match inline {
            Inline::Text(t) | Inline::Code(t) | Inline::Math(t) => out.push_str(t),
            Inline::Emph(c)
            | Inline::Strong(c)
            | Inline::Strike(c)
            | Inline::Highlight(c)
            | Inline::Superscript(c)
            | Inline::Subscript(c)
            | Inline::Link { content: c, .. } => push_plain(c, out),
            Inline::WikiLink { label, .. } => out.push_str(label),
            Inline::Image(img) => out.push_str(&img.alt),
            Inline::Footnote(_) => {}
            Inline::LineBreak | Inline::SoftBreak => out.push(' '),
        }
    }
}

/// Texte brut d'un nœud de bloc.
pub fn node_text(node: &Node) -> String {
    let mut out = String::new();
    push_node_text(node, &mut out);
    out
}

fn push_node_text(node: &Node, out: &mut String) {
    let sep = |out: &mut String| {
        if !out.is_empty() && !out.ends_with(' ') {
            out.push(' ');
        }
    };
    match node {
        Node::Heading { content, .. } | Node::Paragraph(content) => push_plain(content, out),
        Node::Figure(img) => {
            out.push_str(&img.target);
            if !img.alt.is_empty() {
                out.push(' ');
                out.push_str(&img.alt);
            }
        }
        Node::List(list) => {
            for item in &list.items {
                for child in &item.children {
                    sep(out);
                    push_node_text(child, out);
                }
            }
        }
        Node::Code { text, .. } | Node::Math(text) => out.push_str(text),
        Node::Quote(children) => {
            for child in children {
                sep(out);
                push_node_text(child, out);
            }
        }
        Node::Callout(c) => {
            if let Some(title) = &c.title {
                push_plain(title, out);
            }
            for child in &c.body {
                sep(out);
                push_node_text(child, out);
            }
        }
        Node::Table(t) => {
            for cell in t.header.iter().chain(t.rows.iter().flatten()) {
                sep(out);
                push_plain(cell, out);
            }
        }
        Node::Rule => out.push_str("---"),
    }
}
