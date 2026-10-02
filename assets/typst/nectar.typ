// Nectar Render · template unique, piloté par le style.
//
// Tout réglage vient de `/nectar/style.typ` (généré à partir du preset et des
// réglages faits à la main) : ce fichier ne contient aucune couleur ni police
// en dur, seulement la façon de les appliquer.

#import "/nectar/style.typ": style as S, code-theme
#import "/nectar/mitex/mod.typ": mitex-scope

#let pt(v) = v * 1pt
#let col(hex) = if hex == none or hex == "" { none } else { rgb(hex) }
/// `(font: (…), stretch: …)` → arguments de `text`.
#let face(spec) = (font: spec.font, stretch: spec.stretch)

// ------------------------------------------------------------------ blocs

/// Marqueur invisible : dit où un bloc a atterri dans les pages.
#let nb(id) = [#metadata(id)<nectar-block>]

/// Pixels CSS (ceux d'Obsidian) vers points typographiques.
#let px = 0.75pt

/// Image dimensionnée comme Obsidian l'affiche, jamais plus large que le
/// texte ni plus haute que la page.
#let nectar-image(
  src,
  alt: none,
  width-ratio: none,
  width-px: none,
  height-px: none,
  max-height: 100%,
  fit-height: none,
  portrait-cap: false,
  scale: 0.75,
  inline: false,
  page: none,
) = {
  // `page` ne vaut que pour un PDF inclus.
  let img(..args) = if page == none { image(src, alt: alt, ..args) } else { image(src, alt: alt, page: page, ..args) }
  if inline {
    return if width-px != none {
      img(width: width-px * px)
    } else if height-px != none {
      img(height: height-px * px)
    } else {
      img(height: 1.2em)
    }
  }
  layout(region => {
    let natural = measure(img())
    if natural.width == 0pt or natural.height == 0pt {
      return img(width: 100%)
    }
    let ratio = natural.height / natural.width
    let w = if width-ratio != none {
      region.width * width-ratio
    } else if width-px != none {
      width-px * px
    } else if height-px != none {
      height-px * px / ratio
    } else {
      natural.width * scale * S.images.scale
    }
    w = calc.min(w, region.width)
    // Une photo (ou capture) verticale sans taille imposée ne mange pas
    // toute la page ; un schéma vertical, lui, garde la place d'être lu.
    let explicit = width-ratio != none or width-px != none or height-px != none
    if portrait-cap and not explicit and ratio > 1.15 and region.height < 10000pt {
      let cap = region.height * S.images.portrait_max_percent / 100
      if w * ratio > cap { w = cap / ratio }
    }
    // Réduite d'office par le placement automatique pour tenir dans la
    // place restante : voulu, donc pas signalé.
    if fit-height != none and w * ratio > fit-height { w = fit-height / ratio }
    let wanted = w
    if region.height < 10000pt {
      let max-h = region.height * max-height
      if w * ratio > max-h { w = max-h / ratio }
    }
    // Réduite pour tenir en hauteur : l'assistant le signalera.
    if w < wanted * 0.9 {
      [#metadata((kind: "shrunk", value: w / wanted)) <nectar-issue>]
    }
    img(width: w)
  })
}

/// Une image seule dans son bloc, avec légende et placement.
#let nectar-figure(
  src,
  alt: none,
  caption: none,
  width-ratio: none,
  width-px: none,
  height-px: none,
  align-to: center,
  placement: "inline",
  scale: 0.75,
  page: none,
  fit-height: none,
  photo: false,
) = {
  let sized(max-height) = nectar-image(
    src,
    alt: alt,
    page: page,
    width-ratio: width-ratio,
    width-px: width-px,
    height-px: height-px,
    max-height: max-height,
    fit-height: fit-height,
    portrait-cap: photo and placement == "inline",
    scale: scale,
  )
  let body(max-height) = if caption == none { sized(max-height) } else { figure(sized(max-height), caption: caption) }
  if placement == "full-page" {
    pagebreak(weak: true)
    let big = nectar-image(src, alt: alt, page: page, width-ratio: 1.0, max-height: if caption == none { 100% } else { 90% })
    align(center + horizon, if caption == none { big } else { figure(big, caption: caption) })
    pagebreak(weak: true)
  } else if placement == "landscape" {
    // La page paysage elle-même est ouverte par la source générée.
    let big = nectar-image(src, alt: alt, page: page, width-ratio: 1.0, max-height: if caption == none { 100% } else { 92% })
    align(center + horizon, if caption == none { big } else { figure(big, caption: caption) })
  } else if placement == "top" or placement == "bottom" {
    place(
      (if placement == "top" { top } else { bottom }) + align-to,
      float: true,
      clearance: 1.5em,
      block(width: 100%, align(align-to, body(80%))),
    )
  } else {
    // 85 % de la page au plus : la phrase qui annonce l'image et sa légende
    // tiennent sur la même page qu'elle.
    align(align-to, body(85%))
  }
}

/// Image introuvable : un cadre lisible plutôt qu'une erreur.
#let missing-image(target, inline: false) = {
  let label = text(size: 0.85em, fill: rgb("#cf222e"))[Image introuvable : #raw(target)]
  if inline { label } else {
    block(width: 100%, inset: 12pt, stroke: (paint: rgb("#cf222e"), thickness: 0.8pt, dash: "dashed"), label)
  }
}

/// Formule LaTeX déjà convertie en Typst par mitex.
#let nectar-math(code, block: false) = math.equation(block: block, eval("$" + code + "$", scope: mitex-scope))

#let math-fallback(latex, block: false) = if block { align(center, raw(latex, lang: "latex")) } else {
  raw(latex, lang: "latex")
}

#let wikilink(body) = text(fill: col(S.links.wikilink_color), body)

/// Touche de clavier : petite étiquette encadrée.
#let kbd(key) = box(
  inset: (x: 3pt, y: 0pt),
  outset: (y: 2pt),
  radius: 2.5pt,
  stroke: 0.6pt + col(S.table.border_color),
  fill: col(S.code.inline_background),
  text(..face(S.code.font), size: 0.82em, key),
)

#let task(done) = {
  let ink = col(S.text.color)
  box(
    width: 0.78em,
    height: 0.78em,
    baseline: 0.1em,
    radius: 1.5pt,
    fill: if done { ink } else { none },
    stroke: 0.7pt + ink,
    align(center + horizon, if done { text(font: "DejaVu Sans Mono", size: 0.62em, weight: 700, fill: white)[✓] }),
  )
  h(0.45em)
}

// ------------------------------------------------------------- encadrés

#let callout-names = (
  note: "Note", info: "Info", tip: "Astuce", hint: "Astuce", important: "Important",
  success: "Réussi", check: "Réussi", done: "Fait", question: "Question", help: "Question",
  faq: "Question", warning: "Attention", caution: "Attention", attention: "Attention",
  failure: "Échec", fail: "Échec", missing: "Manquant", danger: "Danger", error: "Erreur",
  bug: "Bogue", example: "Exemple", quote: "Citation", cite: "Citation",
  abstract: "Résumé", summary: "Résumé", tldr: "Résumé", todo: "À faire",
)

#let callout-color(kind) = {
  if not S.callout.colored { return col(S.text.color) }
  if kind in ("tip", "hint", "success", "check", "done") { rgb("#1a7f37") }
  else if kind in ("warning", "caution", "attention", "todo") { rgb("#9a6700") }
  else if kind in ("danger", "error", "failure", "fail", "missing", "bug") { rgb("#cf222e") }
  else if kind in ("important", "example") { rgb("#8250df") }
  else if kind in ("quote", "cite") { rgb("#57606a") }
  else { rgb("#0969da") }
}

#let callout(kind, title: none, body) = {
  let C = S.callout
  let color = callout-color(kind)
  let heading = text(weight: 700, fill: color, if title != none { title } else { callout-names.at(kind, default: kind) })
  if C.variant == "plain" {
    block(width: 100%, above: 1.2em, below: 1.2em, {
      heading
      v(0.35em, weak: true)
      pad(left: 1em, body)
    })
  } else {
    block(
      width: 100%,
      inset: (x: 12pt, y: 10pt),
      radius: pt(C.radius_pt),
      fill: if C.variant == "soft" { color.lighten(93%) } else { none },
      stroke: if C.variant == "bordered" { 0.7pt + col(S.table.border_color) } else { none },
      breakable: true,
      {
        heading
        v(0.4em, weak: true)
        body
      },
    )
  }
}

#let nectar-rule() = if S.rules {
  block(above: 1.4em, below: 1.4em, line(length: 100%, stroke: 0.6pt + col(S.table.border_color)))
}

// -------------------------------------------------------- blocs de code

#let lang-names = (
  rs: "Rust", rust: "Rust", py: "Python", python: "Python", js: "JavaScript", javascript: "JavaScript",
  ts: "TypeScript", typescript: "TypeScript", jsx: "JSX", tsx: "TSX", c: "C", cpp: "C++", "c++": "C++",
  cs: "C#", csharp: "C#", java: "Java", kotlin: "Kotlin", go: "Go", rb: "Ruby", ruby: "Ruby",
  php: "PHP", sh: "Shell", bash: "Bash", zsh: "Zsh", ps1: "PowerShell", powershell: "PowerShell",
  sql: "SQL", html: "HTML", css: "CSS", scss: "SCSS", json: "JSON", yaml: "YAML", yml: "YAML",
  toml: "TOML", xml: "XML", md: "Markdown", markdown: "Markdown", typ: "Typst", typst: "Typst",
  lua: "Lua", r: "R", swift: "Swift", dart: "Dart", latex: "LaTeX", tex: "LaTeX", diff: "Diff",
  dockerfile: "Dockerfile", ini: "INI", txt: "Texte", text: "Texte",
)

/// Bloc de code façon éditeur : bandeau (onglet ou fenêtre), numéros de
/// ligne dans une gouttière, lignes surlignées. Chaque ligne est une rangée de
/// grille : une ligne trop longue repart à la ligne alignée sur le code, pas
/// sous les numéros.
#let nectar-code(source, lang: none, title: none, highlight: ()) = {
  let C = S.code
  let T = code-theme
  let fg = rgb(T.foreground)
  let lang-label = if lang == none or lang == "" { none } else { lang-names.at(lower(lang), default: lang) }
  let label = if title != none { title } else { lang-label }
  let lines = source.split("\n")
  let digits = str(lines.len()).len()
  let hl-fill = if T.dark { rgb(255, 255, 255, 18) } else { rgb(255, 213, 0, 45) }

  let header = if C.header == "tab" and label != none {
    block(width: 100%, fill: rgb(T.header), spacing: 0pt, {
      box(fill: rgb(T.background), inset: (x: 12pt, top: 6.5pt, bottom: 6pt), text(
        ..face(S.text.font),
        size: 0.78em,
        fill: fg.transparentize(15%),
        label,
      ))
    })
  } else if C.header == "window" {
    block(width: 100%, fill: rgb(T.header), inset: (x: 11pt, y: 7pt), spacing: 0pt, {
      let dot(c) = box(circle(radius: 3.6pt, fill: rgb(c)))
      dot("#ff5f57")
      h(5pt)
      dot("#febc2e")
      h(5pt)
      dot("#28c840")
      if label != none {
        h(1fr)
        text(..face(S.text.font), size: 0.75em, fill: rgb(T.gutter), label)
      }
    })
  }

  let body = {
    set text(..face(C.font), size: pt(C.size_pt), fill: fg)
    set par(justify: false, first-line-indent: 0pt, leading: (C.line_height - 0.75) * 1em)
    let leading = (C.line_height - 0.75) * 1em
    show raw.where(block: true): it => {
      let rows = it.lines.map(line => {
        // Le caractère invisible garde la hauteur des lignes vides.
        let code = [#sym.zws#line.body]
        if C.line_numbers {
          (align(right, text(fill: rgb(T.gutter), str(line.number))), code)
        } else {
          (code,)
        }
      })
      grid(
        columns: if C.line_numbers { (digits * 0.62em, 1fr) } else { (1fr,) },
        column-gutter: 1.1em,
        inset: (x: 0pt, y: leading / 2),
        fill: (_, y) => if (y + 1) in highlight { hl-fill },
        ..rows.flatten(),
      )
    }
    raw(source, block: true, lang: lang)
  }

  block(
    width: 100%,
    fill: rgb(T.background),
    radius: pt(C.radius_pt),
    stroke: if C.border { 0.6pt + rgb(T.border) } else { none },
    clip: true,
    breakable: true,
    above: 1.2em,
    below: 1.2em,
    {
      header
      block(width: 100%, inset: (x: 13pt, y: 10pt - 0.2em), spacing: 0pt, body)
    },
  )
}

// -------------------------------------------------------------- template

#let template(
  title: none,
  subtitle: none,
  author: none,
  date: none,
  lang: "fr",
  tags: (),
  page: (:),
  body,
) = {
  let T = S.text
  let H = S.headings
  let ink = col(T.color)
  let cover = S.cover.mode
  let cover = if cover == "auto" { if title != none { "page" } else { "none" } } else { cover }

  set document(title: title, author: if author != none { author } else { () })

  // Page.
  let margin = page.at("margin", default: auto)
  let size-args = if page.at("width", default: auto) != auto {
    (width: page.width, height: page.height)
  } else {
    (paper: page.at("paper", default: "a4"), flipped: page.at("flipped", default: false))
  }
  let F = S.footer
  set std.page(
    ..size-args,
    margin: if margin == auto {
      (
        top: S.page.margin_top_mm * 1mm,
        right: S.page.margin_right_mm * 1mm,
        bottom: S.page.margin_bottom_mm * 1mm,
        left: S.page.margin_left_mm * 1mm,
      )
    } else { margin },
    fill: col(S.page.background),
    footer: context {
      let n = counter(std.page).get().first()
      if cover == "page" and n == 1 { return }
      set text(..face(T.font), size: pt(F.size_pt), fill: col(F.color))
      let label = F.text.replace("{title}", if title != none { title } else { "" }).trim()
      let number = if F.page_numbers [#n / #counter(std.page).final().first()]
      if F.align == "center" {
        align(center, (label, number).filter(x => x != none and x != "").join([ — ]))
      } else if F.align == "left" {
        grid(columns: (auto, 1fr), align: (left, right), number, label)
      } else {
        grid(columns: (1fr, auto), align: (left, right), label, number)
      }
    },
  )

  // Texte.
  set text(..face(T.font), size: pt(T.size_pt), fill: ink, lang: lang, hyphenate: T.hyphenate)
  let leading = (T.line_height - 0.75) * 1em
  set par(
    justify: T.justify,
    leading: leading,
    spacing: leading + T.paragraph_spacing_em * 1em,
    first-line-indent: (amount: T.first_line_indent_em * 1em, all: false),
  )
  set highlight(fill: col(S.highlight), extent: 1pt)
  // Lignes isolées (veuves, orphelines) : fortement pénalisées.
  set text(costs: (widow: if T.avoid_widows { 1000% } else { 100% }, orphan: if T.avoid_widows { 1000% } else { 100% }))
  show math.equation: set text(font: (T.math_font, "New Computer Modern Math"))
  // Guillemets français avec espaces fines insécables.
  show: body => if T.french_typography and lang.starts-with("fr") {
    set smartquote(quotes: (double: ("«\u{202F}", "\u{202F}»"), single: ("‹\u{202F}", "\u{202F}›")))
    body
  } else { body }
  show link: set text(fill: col(S.links.color))
  show link: it => if S.links.underline { underline(offset: 2pt, it) } else { it }
  set list(marker: ([•], [◦], [▪]), indent: 0.4em, body-indent: 0.55em)
  set enum(indent: 0.4em, body-indent: 0.55em)

  // Titres.
  set heading(numbering: if H.numbering { "1.1" } else { none })
  show heading: it => {
    let L = H.levels.at(calc.min(it.level, 6) - 1)
    let color = col(if L.color != none { L.color } else { H.color })
    let f = if L.font != none { L.font } else { H.font }
    if it.level == 1 and H.h1_new_page { pagebreak(weak: true) }
    block(width: 100%, sticky: true, above: L.space_above_em * 1em, below: L.space_below_em * 1em, {
      if L.rule_above_pt > 0 {
        line(length: 100%, stroke: pt(L.rule_above_pt) + color)
        v(0.5em, weak: true)
      }
      set text(..face(f), size: pt(L.size_pt), weight: H.weight, fill: color, tracking: L.tracking_em * 1em, hyphenate: false)
      set par(justify: false, first-line-indent: 0pt)
      let content = if it.numbering != none { counter(heading).display(it.numbering) + h(0.6em) + it.body } else { it.body }
      if L.uppercase { upper(content) } else { content }
      if L.rule_below_pt > 0 {
        v(0.35em, weak: true)
        line(length: 100%, stroke: pt(L.rule_below_pt) + color)
      }
    })
  }

  // Code.
  set raw(theme: "code.tmTheme")
  show raw: set text(..face(S.code.font), size: pt(S.code.size_pt))
  show raw.where(block: false): it => box(
    fill: col(S.code.inline_background),
    inset: (x: 2.5pt),
    outset: (y: 2.5pt),
    radius: 2pt,
    text(fill: col(S.code.inline_color), size: pt(T.size_pt) * 0.9, it),
  )

  // Citations.
  let Q = S.quote
  show quote.where(block: true): it => {
    let body = text(fill: col(Q.color), style: if Q.italic { "italic" } else { "normal" }, it.body)
    if Q.variant == "bar" {
      block(width: 100%, inset: (left: 12pt, y: 2pt), stroke: (left: 1pt + col(S.table.border_color)), body)
    } else if Q.variant == "quotes" {
      block(width: 100%, inset: (left: 1.9em, right: 1.2em), {
        place(top + left, dx: -1.9em, dy: -0.15em, text(
          size: 2.8em,
          fill: col(Q.color).transparentize(50%),
          top-edge: "cap-height",
          style: "normal",
          sym.quote.l.double,
        ))
        body
      })
    } else {
      pad(left: 1.6em, right: 1.6em, body)
    }
  }

  // Notes de bas de page.
  set footnote.entry(separator: line(length: 25%, stroke: 0.5pt + col(S.table.border_color)))
  show footnote.entry: set text(size: pt(S.footnotes.size_pt), fill: col(S.footnotes.color))

  // Tableaux.
  let B = S.table
  let border = col(B.border_color)
  set table(
    inset: (x: pt(B.cell_padding_x_pt), y: pt(B.cell_padding_y_pt)),
    fill: (_, y) => if y == 0 { col(B.header_background) } else if B.stripes and calc.even(y) { col(B.stripe_color) },
    stroke: if B.variant == "grid" { 0.5pt + border } else if B.variant == "plain" { none } else {
      (_, y) => (
        top: if y == 0 { 0.9pt + ink } else { none },
        bottom: if y == 0 { 0.6pt + ink } else { 0.4pt + border },
      )
    },
  )
  show table: set par(justify: false, first-line-indent: 0pt)
  show table: set text(number-width: "tabular")
  show table: it => if B.size_pt != none { text(size: pt(B.size_pt), it) } else { it }
  show table.cell.where(y: 0): set text(weight: 700, fill: col(B.header_color))

  // Figures.
  let I = S.images
  set figure(numbering: if I.numbering { "1" } else { none }, gap: 0.7em)
  show figure.caption: set text(size: pt(I.caption_size_pt), fill: col(I.caption_color), style: if I.caption_italic { "italic" } else { "normal" })
  show image: it => if I.radius_pt > 0 { box(radius: pt(I.radius_pt), clip: true, it) } else { it }

  // Titre du document.
  let L1 = H.levels.at(0)
  let title-face = face(if L1.font != none { L1.font } else { H.font })
  let title-color = col(if L1.color != none { L1.color } else { H.color })
  let meta = (date, author).filter(x => x != none)
  if cover == "page" {
    std.page(footer: none, {
      v(1fr)
      if meta.len() > 0 { text(size: 0.95em, fill: col(F.color), meta.join("  ·  ")); v(1.2em) }
      text(..title-face, size: pt(S.cover.title_size_pt), weight: H.weight, fill: title-color, hyphenate: false, par(
        leading: 0.3em,
        justify: false,
        title,
      ))
      if subtitle != none {
        v(1em)
        block(width: 85%, text(size: 1.3em, fill: col(Q.color), par(justify: false, subtitle)))
      }
      v(2fr)
      if tags.len() > 0 {
        text(size: 0.85em, fill: col(F.color), tags.map(t => "#" + t).join("   "))
      }
    })
  } else if cover == "header" and title != none {
    block(below: 1.8em, width: 100%, {
      text(..title-face, size: pt(L1.size_pt) * 1.25, weight: H.weight, fill: title-color, hyphenate: false, par(
        justify: false,
        leading: 0.3em,
        title,
      ))
      if subtitle != none { v(0.5em); text(size: 1.15em, fill: col(Q.color), subtitle) }
      if meta.len() > 0 { v(0.5em); text(size: 0.9em, fill: col(F.color), meta.join("  ·  ")) }
    })
  }

  if S.cover.table_of_contents {
    outline(title: [Sommaire], indent: auto)
    pagebreak(weak: true)
  }

  body
}
