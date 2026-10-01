// Nectar Render · aides communes.
//
// La source générée importe ce module puis le thème : un thème peut
// redéfinir n'importe quelle fonction ci-dessous (callout, wikilink, task…).
// Seule `template` et `page-margin` sont obligatoires dans un thème.

#import "/nectar/mitex/mod.typ": mitex-scope

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
  scale: 0.75,
  inline: false,
) = {
  if inline {
    return if width-px != none {
      image(src, alt: alt, width: width-px * px)
    } else if height-px != none {
      image(src, alt: alt, height: height-px * px)
    } else {
      image(src, alt: alt, height: 1.2em)
    }
  }
  layout(region => {
    let natural = measure(image(src))
    if natural.width == 0pt or natural.height == 0pt {
      return image(src, alt: alt, width: 100%)
    }
    let ratio = natural.height / natural.width
    let w = if width-ratio != none {
      region.width * width-ratio
    } else if width-px != none {
      width-px * px
    } else if height-px != none {
      height-px * px / ratio
    } else {
      natural.width * scale
    }
    w = calc.min(w, region.width)
    // Hauteur finie seulement dans une page (pas dans un conteneur libre).
    if region.height < 10000pt {
      let max-h = region.height * max-height
      if w * ratio > max-h { w = max-h / ratio }
    }
    image(src, alt: alt, width: w)
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
) = {
  let sized(max-height) = nectar-image(
    src,
    alt: alt,
    width-ratio: width-ratio,
    width-px: width-px,
    height-px: height-px,
    max-height: max-height,
    scale: scale,
  )
  let body(max-height) = if caption == none {
    sized(max-height)
  } else {
    figure(sized(max-height), caption: caption)
  }
  if placement == "full-page" {
    pagebreak(weak: true)
    let big = nectar-image(src, alt: alt, width-ratio: 1.0, max-height: if caption == none { 100% } else { 90% }, scale: scale)
    align(center + horizon, if caption == none { big } else { figure(big, caption: caption) })
    pagebreak(weak: true)
  } else if placement == "top" or placement == "bottom" {
    place(
      (if placement == "top" { top } else { bottom }) + align-to,
      float: true,
      clearance: 1.5em,
      block(width: 100%, align(align-to, body(80%))),
    )
  } else {
    align(align-to, body(100%))
  }
}

/// Image introuvable : un cadre lisible plutôt qu'une erreur.
#let missing-image(target, inline: false) = {
  let label = text(size: 0.85em, fill: rgb("#a52638"))[Image introuvable : #raw(target)]
  if inline { label } else {
    block(width: 100%, inset: 12pt, stroke: (paint: rgb("#a52638"), thickness: 1pt, dash: "dashed"), label)
  }
}

/// Formule LaTeX déjà convertie en Typst par mitex.
#let nectar-math(code, block: false) = math.equation(
  block: block,
  eval("$" + code + "$", scope: mitex-scope),
)

/// Formule non convertible : affichée telle quelle.
#let math-fallback(latex, block: false) = if block {
  align(center, raw(latex, lang: "latex"))
} else {
  raw(latex, lang: "latex")
}

#let wikilink(body) = body

#let task(done) = box(
  width: 0.8em,
  height: 0.8em,
  baseline: 0.1em,
  stroke: 0.6pt,
  inset: 0pt,
  align(center + horizon, if done { text(size: 0.7em)[✓] }),
) + h(0.4em)

#let callout(kind, title: none, body) = block(
  width: 100%,
  inset: (left: 12pt, rest: 10pt),
  stroke: (left: 2pt),
  {
    strong(if title != none { title } else { upper(kind) })
    parbreak()
    body
  },
)

#let nectar-rule() = line(length: 100%, stroke: 0.5pt)
