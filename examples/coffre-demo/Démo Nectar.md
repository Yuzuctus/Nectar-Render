---
title: Nectar Render
subtitle: De la note Obsidian au PDF soigné, retouché à la main avant l'export.
author: Yuzuctus
date: Octobre 2026
tags: [démo, obsidian, typst]
---

# Pourquoi un atelier de mise en page

Exporter une note, c'est facile. L'exporter *proprement* demande de décider où tombent les pages. Voici la phrase qui introduit une liste à puces, et la première puce commence sur la page suivante :

- Première puce, envoyée en haut de la page suivante par une retouche.
- Deuxième puce avec du **gras**, de l'*italique*, du `code` et ==du surlignage==.
- Troisième puce avec un lien vers [[Une autre note|une autre note]] et vers [le site](https://yuzuctus.fr).

## Une image, puis le reste de la page vide

![[sequence.svg]]

Cette description vient après l'image. Une retouche « saut après » laisse le reste de la page de l'image vide : le texte commence sur la page suivante.

## Un grand schéma sur une page A3 paysage

![[pipeline.svg|Le pipeline de Nectar Render, de la note au PDF.]]

Le schéma ci-dessus est posé sur une page **A3 paysage** au milieu d'un document A4 : il garde sa taille réelle et reste lisible. L'explication suit sur la même page, ou sur la suivante si la place manque.

## Retour au format A4

Ce titre porte la retouche « format par défaut » : le document repart en A4 portrait.

> [!tip] Où vont les retouches ?
> Dans `.nectar/` à la racine du coffre. La note reste propre et Obsidian ne voit rien.

> [!warning]
> Si tu modifies beaucoup un paragraphe retouché, Nectar le retrouve par ressemblance ; sinon il te signale la retouche orpheline.

### Tableau

| Retouche | Effet | Clé JSON |
|:--|:--|:--|
| Saut avant | Le bloc commence une nouvelle page | `break_before` |
| Saut après | Le reste de la page reste vide | `break_after` |
| Format de page | Nouvelle page au format choisi | `page` |
| Pleine page | L'image occupe toute sa page | `image.placement` |

### Code et maths

```rust
fn main() {
    let note = Project::open("Démo Nectar.md")?;
    println!("{} blocs", note.document.blocks.len()); // 42
}
```

La moyenne d'un échantillon vaut $\bar{x} = \frac{1}{n}\sum_{i=1}^{n} x_i$, et sa variance corrigée :

$$
s^2 = \frac{1}{n-1}\sum_{i=1}^{n}\left(x_i - \bar{x}\right)^2
$$

### Reste à faire

1. Lecture des notes Obsidian
2. Thème Agrume
3. Atelier graphique

- [x] Moteur de rendu Typst
- [x] Retouches dans `.nectar/`
- [ ] Clic sur une page pour retoucher

> La typographie est l'art de donner au texte une forme lisible[^1].

[^1]: Une note de bas de page, rendue par Typst.
