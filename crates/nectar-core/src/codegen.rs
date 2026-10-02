//! Génération de la source Typst à partir du document et des retouches.
//!
//! La source produite importe `/nectar/nectar.typ` (le template, fourni par
//! le moteur), qui lit lui-même `/nectar/style.typ`, généré ici à partir du
//! [`Style`]. Chaque bloc adressable est précédé d'un marqueur `#nb("id")`
//! qui permet ensuite de savoir où il a atterri dans les pages.

use std::collections::{HashMap, HashSet};
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
    /// Contenu produit en mémoire (SVG d'un diagramme), sinon lu sur `path`.
    pub data: Option<std::sync::Arc<String>>,
    /// Réduire la photo si son plus grand côté dépasse ce nombre de pixels.
    pub max_px: Option<u32>,
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

/// Ajustements du placement automatique décidés après une première mise en
/// page (voir `nectar_typst::lay_out`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Tuning {
    /// Blocs gardés d'un seul tenant par la règle automatique, mais qui
    /// laissaient une page à moitié vide : ils peuvent être coupés.
    pub relaxed: HashSet<BlockId>,
    /// Pages paysage repoussées (image, bloc après lequel la placer) : le
    /// texte qui suit remplit d'abord la page en cours, comme un flottant.
    pub deferred: Vec<(BlockId, BlockId)>,
    /// Blocs devant lesquels le format revient, après une page au format
    /// propre (« cette page seulement »).
    pub returns: HashSet<BlockId>,
}

/// Produit la source Typst d'un document retouché.
pub fn generate(doc: &Document, layout: &Layout, style: &Style) -> Generated {
    generate_tuned(doc, layout, style, &Tuning::default())
}

/// [`generate`], avec les ajustements d'une mise en page précédente.
pub fn generate_tuned(doc: &Document, layout: &Layout, style: &Style, tuning: &Tuning) -> Generated {
    let mut resolution = layout.resolve(doc);
    hoist_over_headings(doc, &mut resolution.ops);
    let mut g = Gen {
        ops: &resolution.ops,
        relaxed: &tuning.relaxed,
        returns: &tuning.returns,
        persistent: layout.page.clone(),
        layout,
        style,
        french: style.text.french_typography && doc.meta.lang.as_deref().unwrap_or("fr").starts_with("fr"),
        headings: heading_slugs(doc),
        heading_index: 0,
        doc_name: doc.name.as_deref().map(str::to_lowercase),
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
    let deferred: HashSet<&BlockId> = tuning.deferred.iter().map(|(figure, _)| figure).collect();
    let emitted: Vec<&Block> = doc.blocks.iter().filter(|b| !deferred.contains(&b.id)).collect();
    for (index, block) in emitted.iter().enumerate() {
        g.block(block, emitted.get(index + 1).copied());
        // Les pages paysage repoussées ici, dans l'ordre du document.
        for figure in doc.blocks.iter().filter(|b| tuning.deferred.iter().any(|(f, a)| *f == b.id && *a == block.id)) {
            g.block(figure, None);
        }
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
        let moved = BlockOps {
            break_before: current.break_before,
            page: current.page.take(),
            page_onward: current.page_onward,
            ..BlockOps::default()
        };
        current.break_before = false;
        current.page_onward = false;
        ops.entry(doc.blocks[target].id.clone()).or_default().merge(&moved);
    }
}

struct Gen<'a> {
    ops: &'a HashMap<BlockId, BlockOps>,
    relaxed: &'a HashSet<BlockId>,
    returns: &'a HashSet<BlockId>,
    /// Le format en vigueur hors des pages au format propre.
    persistent: PageSpec,
    layout: &'a Layout,
    style: &'a Style,
    /// Typographie française active (langue `fr` et réglage du style).
    french: bool,
    /// Ancre (slug) de chaque titre de premier niveau → son numéro d'étiquette.
    headings: HashMap<String, usize>,
    heading_index: usize,
    doc_name: Option<String>,
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

    fn block(&mut self, block: &Block, next: Option<&Block>) {
        let ops = self.ops.get(&block.id).cloned().unwrap_or_default();
        // Numéro d'étiquette du titre, compté même s'il est masqué.
        let heading_label = matches!(block.node, Node::Heading { .. }).then(|| {
            self.heading_index += 1;
            self.heading_index - 1
        });
        // Fin d'une page au format propre : on revient au format courant.
        if self.returns.contains(&block.id) && ops.page.is_none() {
            let _ = writeln!(self.out, "#set page({})", page_args(&self.persistent, self.style));
        }
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
        // Image sur sa propre page paysage : le marqueur va dans la page.
        if ops.image.as_ref().is_some_and(|i| i.placement == Placement::Landscape)
            && let Some(body) = match &block.node {
                Node::Figure(image) => Some(self.figure(image, ops.image.as_ref())),
                Node::Diagram { lang, source } => Some(self.diagram(lang, source, ops.image.as_ref())),
                _ => None,
            }
        {
            self.out.push_str("#page(flipped: true)[\n");
            self.block_lines.push((block.id.clone(), self.line()));
            let _ = writeln!(self.out, "#nb({})\n{body}\n]", string(block.id.as_str()));
            self.after(&ops);
            self.out.push('\n');
            return;
        }
        self.block_lines.push((block.id.clone(), self.line()));
        let _ = writeln!(self.out, "#nb({})", string(block.id.as_str()));

        let p = &self.style.pagination;
        // Une retouche de saut l'emporte toujours sur la règle automatique.
        let next_breaks = next.is_some_and(|n| {
            let starts_page = |id: &BlockId| self.ops.get(id).is_some_and(|o| o.break_before || o.page.is_some());
            starts_page(&n.id)
                || matches!(&n.node, Node::List(l) if l.items.first().and_then(|i| i.id.as_ref()).is_some_and(starts_page))
        });
        let sticky = ops.keep_with_next
            || (p.keep_intro_with_next && !ops.break_after && !next_breaks && announces(&block.node, next));
        let unbreakable = ops
            .keep_together
            .unwrap_or(p.keep_small_blocks && is_small(&block.node) && !self.relaxed.contains(&block.id));
        let body = match &block.node {
            Node::List(list) => {
                let start = self.out.len();
                self.top_list(list, first_item_done);
                let item_ops = list.items.iter().any(|i| i.id.as_ref().is_some_and(|id| self.ops.contains_key(id)));
                let keep = !item_ops && (sticky || unbreakable);
                if keep || ops.style.is_some() {
                    let inner = self.out.split_off(start);
                    let wrapped = wrap_block(&inner, keep && sticky, keep && unbreakable, ops.style.as_ref());
                    self.out.push_str(&wrapped);
                }
                None
            }
            Node::Table(table) => Some(self.table(table, ops.table.as_ref())),
            Node::Figure(image) => Some(self.figure(image, ops.image.as_ref())),
            Node::Diagram { lang, source } => Some(self.diagram(lang, source, ops.image.as_ref())),
            node @ Node::Heading { .. } => {
                // Étiquette pour les liens internes `[[#Titre]]`.
                Some(format!("{} <nectar-h-{}>", self.node(node), heading_label.unwrap_or_default()))
            }
            node => Some(self.node(node)),
        };
        if let Some(body) = body {
            let wrapped = wrap_block(&body, sticky, unbreakable, ops.style.as_ref());
            self.out.push_str(&wrapped);
            if !wrapped.ends_with('\n') {
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
                if ops.page_onward {
                    self.persistent = spec.clone();
                }
            }
            Some(PageChange::Default(_)) => {
                let _ = writeln!(self.out, "#set page({})", page_args(&self.layout.page, self.style));
                self.persistent = self.layout.page.clone();
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
            Node::Diagram { lang, source } => self.diagram(lang, source, None),
            Node::List(list) => {
                let items: Vec<&ListItem> = list.items.iter().collect();
                self.list_markup(list, &items, list.start)
            }
            Node::Code { lang, text, title, highlight } => {
                let lang = lang.as_deref().map(string).unwrap_or_else(|| "none".into());
                let title = title.as_deref().map(string).unwrap_or_else(|| "none".into());
                let lines: Vec<String> = highlight.iter().map(u32::to_string).collect();
                format!("#nectar-code({}, lang: {lang}, title: {title}, highlight: {})", string(text), array(&lines))
            }
            Node::Quote(children) => format!("#quote(block: true)[\n{}\n]", self.nodes(children)),
            Node::Callout(callout) => {
                let title = match &callout.title {
                    Some(title) => format!("[{}]", self.inlines(title)),
                    None => "none".into(),
                };
                format!("#callout({}, title: {title})[\n{}\n]", string(&callout.kind), self.nodes(&callout.body))
            }
            Node::Table(table) => self.table(table, None),
            Node::Math(latex) => self.math(latex, true),
            Node::Rule => "#nectar-rule()".into(),
        }
    }

    fn nodes(&mut self, nodes: &[Node]) -> String {
        nodes.iter().map(|n| self.node(n)).collect::<Vec<_>>().join("\n\n")
    }

    fn table(&mut self, table: &Table, ops: Option<&crate::layout::TableOps>) -> String {
        let columns = table.align.len().max(table.header.len()).max(1);
        let align: Vec<&str> = (0..columns)
            .map(|i| {
                let manual = ops.and_then(|o| o.align.get(i).copied().flatten());
                match manual {
                    Some(HAlign::Left) => "start",
                    Some(HAlign::Center) => "center",
                    Some(HAlign::Right) => "end",
                    None => match table.align.get(i).copied().unwrap_or(Align::Auto) {
                        Align::Auto | Align::Left => "start",
                        Align::Center => "center",
                        Align::Right => "end",
                    },
                }
            })
            .collect();
        let widths = match ops.filter(|o| o.widths.iter().any(|w| *w > 0.0)) {
            Some(o) => array(
                &(0..columns)
                    .map(|i| match o.widths.get(i).copied().unwrap_or(0.0) {
                        w if w > 0.0 => format!("{}fr", num(w)),
                        _ => "auto".into(),
                    })
                    .collect::<Vec<_>>(),
            ),
            None => auto_widths(table, columns),
        };
        let mut out = format!(
            "#table(\n  columns: {widths},\n  align: {},\n",
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

    /// Diagramme Mermaid dessiné en SVG, puis placé comme une image.
    fn diagram(&mut self, lang: &str, source: &str, ops: Option<&crate::layout::ImageOps>) -> String {
        let d = &self.style.diagrams;
        let mut theme = mermaid_svg::Theme::by_name(&d.theme).unwrap_or_else(mermaid_svg::Theme::neutral);
        if d.document_font {
            let family = crate::typst_style::family_name(&self.style.text.font);
            theme = theme.with_font(format!("{family}, sans-serif"));
        }
        match mermaid_svg::render_with(source, &theme) {
            Ok(svg) => {
                let image = Image {
                    target: format!("{lang}.svg"),
                    path: None,
                    alt: String::new(),
                    width_px: None,
                    height_px: None,
                    page: None,
                    svg: Some(std::sync::Arc::new(svg)),
                };
                let mut figure = self.figure(&image, ops);
                // Un diagramme se lit à sa taille naturelle (et non à 75 %).
                figure.insert_str(figure.len() - 1, &format!(", scale: {}", num(d.scale)));
                figure
            }
            Err(error) => {
                self.warnings.push(format!("diagramme {lang} non dessiné : {error}"));
                self.node(&Node::Code {
                    lang: Some(lang.to_string()),
                    text: source.to_string(),
                    title: None,
                    highlight: Vec::new(),
                })
            }
        }
    }

    fn figure(&mut self, image: &Image, ops: Option<&crate::layout::ImageOps>) -> String {
        let vpath = match (&image.path, &image.svg) {
            (_, Some(svg)) => self.memory_asset(svg),
            (Some(path), None) => self.asset(path),
            (None, None) => return format!("#missing-image({})", string(&image.target)),
        };
        let path = image.path.clone().unwrap_or_default();
        let path = &path;
        let caption =
            ops.and_then(|o| o.caption.clone()).or_else(|| Some(image.alt.clone())).filter(|c| !c.trim().is_empty());
        let mut args = vec![string(&vpath)];
        if let Some(page) = image.page {
            args.push(format!("page: {page}"));
        }
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
                Placement::Landscape => "landscape",
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
        let e = &self.style.export;
        let max_px = e.downscale_images.then_some(e.max_image_px.max(600));
        self.assets.push(Asset { vpath: vpath.clone(), path: path.to_path_buf(), data: None, max_px });
        self.asset_ids.insert(path.to_path_buf(), vpath.clone());
        vpath
    }

    /// Fichier produit en mémoire (SVG d'un dessin ou d'un diagramme).
    fn memory_asset(&mut self, svg: &std::sync::Arc<String>) -> String {
        let vpath = format!("/assets/{:04}-dessin.svg", self.assets.len() + 1);
        self.assets.push(Asset { vpath: vpath.clone(), path: PathBuf::new(), data: Some(svg.clone()), max_px: None });
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
                Inline::Text(text) => {
                    let text = if self.french {
                        let after_word = out.chars().last().is_some_and(|c| !c.is_whitespace());
                        crate::typo::french(text, after_word)
                    } else {
                        text.clone()
                    };
                    out.push_str(&escape(&text));
                }
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
                Inline::Underline(c) => wrap(&mut out, "underline", &self.inlines(c)),
                Inline::Kbd(key) => {
                    let _ = write!(out, "#kbd({});", string(key));
                }
                Inline::Html(_) => {}
                Inline::Link { url, content } => {
                    let label = self.inlines(content);
                    if is_external(url) {
                        let _ = write!(out, "#link({})[{label}];", string(url));
                    } else if let Some(index) = self.internal(url) {
                        let _ = write!(out, "#link(<nectar-h-{index}>)[{label}];");
                    } else {
                        out.push_str(&label);
                    }
                }
                Inline::WikiLink { target, label } | Inline::Embed { target, label } => match self.internal(target) {
                    Some(index) => {
                        let _ = write!(out, "#link(<nectar-h-{index}>)[#wikilink[{}]];", escape(label));
                    }
                    None => wrap(&mut out, "wikilink", &escape(label)),
                },
                Inline::Image(image) => match image
                    .path
                    .as_ref()
                    .map(|p| (p.clone(), None))
                    .or(image.svg.as_ref().map(|s| (PathBuf::new(), Some(s.clone()))))
                {
                    Some((path, svg)) => {
                        let vpath = match svg {
                            Some(svg) => self.memory_asset(&svg),
                            None => self.asset(&path),
                        };
                        let width = image.width_px.map(|w| format!(", width-px: {w}")).unwrap_or_default();
                        let page = image.page.map(|p| format!(", page: {p}")).unwrap_or_default();
                        let _ = write!(out, "#box(nectar-image({}{width}{page}, inline: true));", string(&vpath));
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

/// Largeurs automatiques des colonnes d'un tableau : une colonne aux contenus
/// courts (adresses, nombres, dates) garde sa largeur naturelle et ne passe
/// jamais à la ligne ; les colonnes de texte se partagent le reste selon la
/// longueur de leur contenu.
fn auto_widths(table: &Table, columns: usize) -> String {
    const SHORT: usize = 24;
    let mut longest = vec![0usize; columns];
    let mut total = vec![0usize; columns];
    let rows = std::iter::once(&table.header).chain(&table.rows).filter(|r| !r.is_empty());
    let mut count = 0usize;
    for row in rows {
        count += 1;
        for (i, cell) in row.iter().enumerate().take(columns) {
            let length = plain_text(cell).chars().count();
            longest[i] = longest[i].max(length);
            total[i] += length;
        }
    }
    if longest.iter().all(|l| *l <= SHORT) {
        // Tout est court : chaque colonne à sa largeur naturelle.
        return columns.to_string();
    }
    let widths: Vec<String> = (0..columns)
        .map(|i| {
            if longest[i] <= SHORT {
                "auto".to_string()
            } else {
                let average = total[i] as f32 / count.max(1) as f32;
                format!("{}fr", num((average / 12.0).clamp(1.0, 4.0)))
            }
        })
        .collect();
    array(&widths)
}

/// Enveloppe un bloc : insécable, collé au suivant, apparence propre.
fn wrap_block(body: &str, sticky: bool, unbreakable: bool, style: Option<&crate::layout::BlockStyle>) -> String {
    use crate::layout::TextAlign;
    let Some(s) = style.filter(|s| **s != crate::layout::BlockStyle::default()) else {
        return if sticky || unbreakable {
            format!("#block(sticky: {sticky}, breakable: {})[\n{body}\n]\n", !unbreakable)
        } else {
            body.to_string()
        };
    };
    let mut args = vec![format!("sticky: {sticky}"), format!("breakable: {}", !unbreakable), "width: 100%".into()];
    if let Some(bg) = &s.background {
        args.push(format!("fill: rgb({})", string(bg)));
    }
    if s.border {
        args.push("stroke: 0.6pt + luma(160)".into());
    }
    if s.background.is_some() || s.border {
        args.push("inset: (x: 10pt, y: 8pt)".into());
    }
    let mut sets = String::new();
    match s.align {
        Some(TextAlign::Left) => sets.push_str("#set align(left)\n#set par(justify: false)\n"),
        Some(TextAlign::Center) => sets.push_str("#set align(center)\n#set par(justify: false)\n"),
        Some(TextAlign::Right) => sets.push_str("#set align(right)\n#set par(justify: false)\n"),
        Some(TextAlign::Justify) => sets.push_str("#set par(justify: true)\n"),
        None => {}
    }
    let mut text = Vec::new();
    if let Some(p) = s.size_percent {
        text.push(format!("size: {}em", num(p / 100.0)));
    }
    if let Some(c) = &s.color {
        text.push(format!("fill: rgb({})", string(c)));
    }
    if s.italic {
        text.push("style: \"italic\"".into());
    }
    if s.bold {
        text.push("weight: \"bold\"".into());
    }
    if !text.is_empty() {
        let _ = writeln!(sets, "#set text({})", text.join(", "));
    }
    let mut inner = format!("{sets}{body}");
    if let Some(n) = s.columns.filter(|n| *n > 1) {
        inner = format!("#columns({n}, gutter: 1.4em)[\n{inner}\n]");
    }
    if let Some(mm) = s.indent_mm {
        inner = format!("#pad(left: {}mm)[\n{inner}\n]", num(mm));
    }
    format!("#block({})[\n{inner}\n]\n", args.join(", "))
}

/// Une phrase qui annonce la suite (« Voici les étapes : ») et ce qui la suit.
fn announces(node: &Node, next: Option<&Block>) -> bool {
    let Node::Paragraph(content) = node else { return false };
    let text = plain_text(content);
    let announcing = text.trim_end().ends_with(':');
    announcing
        && next.is_some_and(|n| {
            matches!(
                n.node,
                Node::List(_)
                    | Node::Code { .. }
                    | Node::Table(_)
                    | Node::Figure(_)
                    | Node::Diagram { .. }
                    | Node::Math(_)
                    | Node::Quote(_)
                    | Node::Callout(_)
            )
        })
}

/// Un bloc assez court pour ne jamais être coupé entre deux pages.
fn is_small(node: &Node) -> bool {
    match node {
        Node::List(list) => list.items.len() <= 6 && node_text(node).chars().count() <= 600,
        Node::Code { text, .. } => text.lines().count() <= 15,
        Node::Table(table) => table.rows.len() <= 10,
        Node::Callout(_) | Node::Quote(_) => node_text(node).chars().count() <= 500,
        _ => false,
    }
}

impl Gen<'_> {
    /// Un lien vers un titre de cette note : `#titre`, `[[#Titre]]`,
    /// `[[Cette note#Titre]]`.
    fn internal(&self, target: &str) -> Option<usize> {
        let (note, section) = target.split_once('#')?;
        let note = note.trim().trim_end_matches(".md").to_lowercase();
        if !note.is_empty() && self.doc_name.as_deref() != Some(note.as_str()) {
            return None;
        }
        let wanted = crate::ids::slug(&crate::vault::percent_decode(section));
        self.headings.get(&wanted).copied()
    }
}

/// Ancres des titres de premier niveau, dans l'ordre (la première gagne).
fn heading_slugs(doc: &Document) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    let headings = doc.blocks.iter().filter_map(|b| match &b.node {
        Node::Heading { content, .. } => Some(crate::ids::slug(&plain_text(content))),
        _ => None,
    });
    for (index, slug) in headings.enumerate() {
        map.entry(slug).or_insert(index);
    }
    map
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
    fn internal_links_point_to_headings() {
        let mut doc = parse(
            "# Début\n\nVoir [[#Fin]], [ici](#fin) et [[Note#Début|là]], pas [[Autre#Fin]].\n\n## Fin\n",
            &ParseOptions::default(),
        );
        doc.name = Some("Note".into());
        let g = generate(&doc, &Layout::default(), &Style::default());
        assert!(g.source.contains("<nectar-h-0>") && g.source.contains("<nectar-h-1>"));
        assert_eq!(g.source.matches("#link(<nectar-h-1>)").count(), 2, "{}", g.source);
        assert_eq!(g.source.matches("#link(<nectar-h-0>)").count(), 1);
        assert_eq!(g.source.matches("#link(").count(), 3, "[[Autre#Fin]] reste un simple libellé");
    }

    #[test]
    fn block_style_and_table_columns() {
        let g = gen_with("Texte centré.\n\n| a | b |\n|---|---|\n| 1 | 2 |\n", |doc, layout| {
            let anchors = doc.anchors();
            layout.ops_mut(anchors[0]).style = Some(crate::layout::BlockStyle {
                align: Some(crate::layout::TextAlign::Center),
                size_percent: Some(120.0),
                background: Some("#fff3a3".into()),
                columns: Some(2),
                ..Default::default()
            });
            layout.ops_mut(anchors[1]).table =
                Some(crate::layout::TableOps { widths: vec![2.0, 0.0], align: vec![None, Some(HAlign::Right)] });
        });
        assert!(g.source.contains("fill: rgb(\"#fff3a3\")"), "{}", g.source);
        assert!(g.source.contains("#set align(center)") && g.source.contains("size: 1.2em"));
        assert!(g.source.contains("#columns(2"));
        assert!(g.source.contains("columns: (2fr, auto)") && g.source.contains("align: (start, end)"));
    }

    #[test]
    fn short_columns_keep_their_natural_width() {
        let g = gen_with(
            "| Ressource | IP | Rôle |\n|---|---|---|\n\
             | Windows 11 (VM, windows 11 famille) | `192.168.107.11` | Passerelle et administration du switch |\n",
            |_, _| {},
        );
        assert!(g.source.contains("columns: (1.833fr, auto, 1.75fr)"), "{}", g.source);
        let g = gen_with("| a | b |\n|---|---|\n| 1 | 2 |\n", |_, _| {});
        assert!(g.source.contains("columns: 2,"), "tout est court : largeurs naturelles");
    }

    #[test]
    fn landscape_figure_gets_its_own_page() {
        let g = gen_with("Intro.\n\n![schéma](https://exemple.org/s.png)\n\nSuite.\n", |doc, layout| {
            let figure = doc.anchors()[1];
            layout.ops_mut(figure).image =
                Some(crate::layout::ImageOps { placement: Placement::Landscape, ..Default::default() });
        });
        let page = g.source.find("#page(flipped: true)[").expect("page paysage");
        let marker = g.source[page..].find("#nb(").expect("marqueur dans la page");
        assert!(marker < 40, "{}", g.source);
    }

    #[test]
    fn block_at_line_maps_back() {
        let g = gen_with("A\n\nB\n", |_, _| {});
        let (id, line) = &g.block_lines[1];
        assert_eq!(g.block_at_line(*line + 1), Some(id));
    }
}
