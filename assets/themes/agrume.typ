// Nectar Render · thème Agrume.
//
// Le design Agrume v3 (yuzuctus.fr) adapté à la page imprimée : encre et
// papier, une seule famille IBM Plex, rayon 0, structure par les filets, le
// jaune yuzu pour le surlignage, le vert identité pour les noms.
//
// Écart assumé avec le kit web : l'italique existe ici, parce qu'un document
// long en a besoin pour l'emphase et les titres d'œuvres.
//
// Paramètres (`params` dans les retouches) :
//   paper-tone      "blanc" (défaut) ou "sapin" (#f3f6ea, pour l'écran)
//   cover           auto (page de garde s'il y a un titre), true, false
//   toc             false ; true ajoute un sommaire
//   justify         true ; false pour un texte en drapeau
//   font-size       10.5 (points)
//   figure-numbering false ; true numérote les figures légendées

// ---------------------------------------------------------------- tokens
#let ag = (
  paper: rgb("#f3f6ea"),
  surface: rgb("#eaeedd"),
  sunken: rgb("#e0e5d2"),
  ink: rgb("#141c17"),
  muted: rgb("#4b554f"),
  faint: rgb("#5f6761"),
  rule: rgb("#d2d8c6"),
  rule-strong: rgb("#7c8274"),
  identity: rgb("#0e6b4c"),
  accent: rgb("#f2e285"),
  marker: rgb("#ffe696"),
  gold: rgb("#6b5a00"),
  rose: rgb("#9e2452"),
  danger: rgb("#a52638"),
  mint: rgb("#a6e9d1"),
  cyan: rgb("#9bd8ec"),
  yellow: rgb("#f2e285"),
  pink: rgb("#f6b6c6"),
)

#let sans = "IBM Plex Sans"
#let mono = "IBM Plex Mono"
// Plex Sans resserrée : même famille pour Typst, chasse 75 %.
#let title-face = "IBM Plex Sans"
#let title-text = text.with(font: title-face, stretch: 75%, weight: 600, hyphenate: false)

#let page-margin = (x: 20mm, top: 20mm, bottom: 22mm)

/// L'étiquette mono en capitales : la seule voix en capitales d'Agrume.
#let kicker(body, fill: ag.muted) = text(
  font: mono,
  weight: 500,
  size: 7.5pt,
  tracking: 0.08em,
  fill: fill,
  upper(body),
)

// ------------------------------------------------------------- éléments
#let wikilink(body) = text(fill: ag.identity, body)

#let task(done) = box(
  width: 0.78em,
  height: 0.78em,
  baseline: 0.1em,
  fill: if done { ag.accent } else { none },
  stroke: 0.8pt + ag.ink,
  align(center + horizon, if done { text(size: 0.65em, weight: 600)[✓] }),
) + h(0.45em)

#let callout-names = (
  note: "Note", info: "Info", tip: "Astuce", hint: "Astuce", important: "Important",
  success: "Réussi", check: "Réussi", done: "Fait", question: "Question", help: "Question",
  faq: "Question", warning: "Attention", caution: "Attention", attention: "Attention",
  failure: "Échec", fail: "Échec", missing: "Manquant", danger: "Danger", error: "Erreur",
  bug: "Bogue", example: "Exemple", quote: "Citation", cite: "Citation",
  abstract: "Résumé", summary: "Résumé", tldr: "Résumé", todo: "À faire",
)

#let callout-pigment(kind) = {
  if kind in ("warning", "caution", "attention", "todo") { ag.yellow }
  else if kind in ("danger", "error", "failure", "fail", "missing", "bug") { ag.pink }
  else if kind in ("question", "help", "faq", "example", "abstract", "summary", "tldr", "info") { ag.cyan }
  else { ag.mint }
}

#let callout(kind, title: none, body) = {
  if kind in ("quote", "cite") {
    return quote(block: true, body)
  }
  block(
    width: 100%,
    fill: ag.surface,
    inset: (left: 14pt, right: 12pt, top: 10pt, bottom: 11pt),
    stroke: (left: 3pt + callout-pigment(kind)),
    breakable: true,
    {
      kicker(if title != none { title } else { callout-names.at(kind, default: kind) }, fill: ag.ink)
      v(0.55em, weak: true)
      body
    },
  )
}

#let nectar-rule() = block(above: 1.4em, below: 1.4em, line(length: 100%, stroke: 1pt + ag.rule))

// -------------------------------------------------------------- template
#let template(
  title: none,
  subtitle: none,
  author: none,
  date: none,
  lang: "fr",
  tags: (),
  page: (:),
  params: (:),
  body,
) = {
  let p(key, default) = params.at(key, default: default)
  let ground = if p("paper-tone", "blanc") == "sapin" { ag.paper } else { white }
  let cover = p("cover", auto)
  let cover = if cover == auto { title != none } else { cover }
  let size = p("font-size", 10.5) * 1pt

  set document(title: title, author: if author != none { author } else { () })

  let margin = page.at("margin", default: auto)
  let size-args = if page.at("width", default: auto) != auto {
    (width: page.width, height: page.height)
  } else {
    (paper: page.at("paper", default: "a4"), flipped: page.at("flipped", default: false))
  }
  set std.page(
    ..size-args,
    margin: if margin == auto { page-margin } else { margin },
    fill: ground,
    footer: context {
      let n = counter(std.page).get().first()
      if cover and n == 1 { return }
      set text(font: mono, size: 7pt, fill: ag.faint, tracking: 0.04em)
      grid(
        columns: (1fr, auto),
        align: (left, right),
        if title != none { upper(title) },
        [#n / #counter(std.page).final().first()],
      )
    },
  )

  set text(font: sans, size: size, fill: ag.ink, lang: lang, hyphenate: auto)
  set par(justify: p("justify", true), leading: 0.7em, spacing: 1.2em)
  set strong(delta: 200)
  set highlight(fill: ag.marker, extent: 1.2pt)
  show link: it => highlight(fill: ag.marker.transparentize(45%), extent: 1pt, it)

  // Titres : Plex resserrée pour les grands, mono pour les petits.
  set heading(numbering: none)
  show heading.where(level: 1): it => block(width: 100%, sticky: true, above: 2.2em, below: 1.1em, {
    line(length: 100%, stroke: 2pt + ag.ink)
    v(0.55em)
    title-text(size: size * 2.3, it.body)
  })
  show heading.where(level: 2): it => block(sticky: true, above: 1.9em, below: 0.9em,
    title-text(size: size * 1.65, it.body))
  show heading.where(level: 3): it => block(sticky: true, above: 1.6em, below: 0.8em,
    text(weight: 600, size: size * 1.2, it.body))
  show heading.where(level: 4): it => block(sticky: true, above: 1.4em, below: 0.7em, kicker(it.body, fill: ag.ink))
  show heading.where(level: 5): it => block(sticky: true, above: 1.3em, below: 0.7em, kicker(it.body))
  show heading.where(level: 6): it => block(sticky: true, above: 1.3em, below: 0.7em, kicker(it.body, fill: ag.faint))

  // Code.
  set raw(theme: "agrume.tmTheme")
  show raw: set text(font: mono, weight: 400)
  show raw.where(block: false): it => box(
    fill: ag.surface,
    inset: (x: 1.5pt),
    outset: (y: 2.5pt),
    text(size: 0.92em, it),
  )
  show raw.where(block: true): it => block(
    width: 100%,
    fill: ag.surface,
    inset: (x: 12pt, y: 11pt),
    stroke: (left: 2pt + ag.rule-strong),
    {
      if it.lang != none and it.lang != "" {
        place(top + right, dx: 4pt, dy: -5pt, kicker(it.lang, fill: ag.faint))
      }
      set par(justify: false)
      text(size: size * 0.84, it)
    },
  )

  // Citations, listes, notes.
  show quote.where(block: true): it => block(
    inset: (left: 14pt, y: 2pt),
    stroke: (left: 2pt + ag.rule-strong),
    text(fill: ag.muted, it.body),
  )
  set list(marker: ([•], [–], [·]), indent: 0.3em, body-indent: 0.6em)
  set enum(indent: 0.3em, body-indent: 0.6em)
  set footnote.entry(separator: line(length: 25%, stroke: 0.6pt + ag.rule-strong), gap: 0.6em)
  show footnote.entry: set text(size: size * 0.8, fill: ag.muted)

  // Tableaux : filets horizontaux, en-tête mono, chiffres tabulaires.
  set table(
    stroke: (_, y) => (
      top: if y == 1 { 1.5pt + ag.ink } else if y > 1 { 0.5pt + ag.rule } else { none },
    ),
    inset: (x: 7pt, y: 6pt),
  )
  show table: set text(number-width: "tabular")
  show table: set par(justify: false)
  show table.cell.where(y: 0): set text(font: mono, weight: 500, size: size * 0.72, fill: ag.muted, tracking: 0.06em)

  // Figures : rayon de 2 pt pour les œuvres, légende mono façon crédit.
  set figure(numbering: if p("figure-numbering", false) { "1" } else { none }, gap: 0.8em)
  show figure.caption: set text(font: mono, size: 7.5pt, fill: ag.faint)
  show image: it => box(radius: 2pt, clip: true, it)

  // Page de garde.
  if cover {
    std.page(footer: none, {
      line(length: 100%, stroke: 2pt + ag.ink)
      v(0.8em)
      let meta = (date, author).filter(x => x != none)
      if meta.len() > 0 { kicker(meta.join("  ·  ")) }
      v(1fr)
      title-text(size: 44pt, fill: ag.ink, par(leading: 0.25em, justify: false, title))
      if subtitle != none {
        v(1.1em)
        block(width: 85%, text(size: 14pt, fill: ag.muted, par(justify: false, subtitle)))
      }
      v(2fr)
      if tags.len() > 0 {
        tags.map(t => box(stroke: 1pt + ag.rule-strong, inset: (x: 6pt, y: 4pt), kicker(t, fill: ag.ink))).join(h(6pt))
      }
    })
  } else if title != none {
    block(below: 2em, {
      line(length: 100%, stroke: 2pt + ag.ink)
      v(0.6em)
      let meta = (date, author).filter(x => x != none)
      if meta.len() > 0 { kicker(meta.join("  ·  ")); v(0.4em) }
      title-text(size: size * 2.9, par(justify: false, leading: 0.3em, title))
      if subtitle != none { v(0.6em); text(size: size * 1.25, fill: ag.muted, subtitle) }
    })
  }

  if p("toc", false) {
    show outline.entry.where(level: 1): set text(weight: 600)
    outline(title: kicker[Sommaire], indent: auto)
    pagebreak(weak: true)
  }

  body
}
