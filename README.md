# Nectar Render

**Atelier de mise en page pour Obsidian.** Tu écris dans Obsidian ; Nectar transforme la note en PDF soigné, et tu décides à la main où tombent les pages, quel format prend chaque page et comment se placent les images. Tout reste local.

> Version 2, réécrite de zéro en Rust. L'ancienne version Python/Tkinter reste dans l'historique de `main` (tag `python-final` à poser).

## L'atelier

```powershell
cargo run --release -p nectar-app -- "chemin\vers\note.md"
```

- Au centre, les **pages du PDF**, mises en page en direct. La note est surveillée : dès que tu enregistres dans Obsidian, l'aperçu se met à jour.
- **Clic sur un bloc** (dans les pages ou dans la liste de gauche) : à droite, ses retouches (nouvelle page avant, page vide après, garder avec le suivant, pousser en bas, espace avant, format de page à partir d'ici, largeur, alignement, placement et légende d'une image, masquer).
- Onglet **Style** : choisir un preset, puis tout régler à la main (texte, titres H1–H6, code, encadrés, citations, tableaux, liens, images, notes, page, pied de page, page de garde). « Enregistrer comme preset… » le rend réutilisable pour toutes les notes.
- Annuler / rétablir (Ctrl+Z / Ctrl+Y), export PDF (Ctrl+E), zoom, thème clair ou sombre. L'interface porte le design Agrume.

## Styles et presets

10 presets intégrés, repris de la v1 : **Agrume, Académique, Magazine, Entreprise, Technique, Minimal, Carnet, Créatif, Développeur, Élégant**. Les presets personnels vivent dans `%APPDATA%\Nectar Render\presets\*.json`.

Un style se compose en couches : défaut ← preset ← réglages de la note (rangés dans `.nectar/`, seulement ce qui diffère du preset). Si une police manque sur la machine, Typst prend une police de secours de la même famille (serif, sans, mono) et l'atelier le signale.

Blocs de code façon éditeur, avec 11 thèmes : VS Code Dark+ et Light+, GitHub clair et sombre, One Dark, Monokai, Dracula, Solarized clair, Nord, Xcode clair, Agrume. Bandeau au choix (onglet du langage, barre de fenêtre, aucun), numéros de ligne, arrondi, bordure.

## Ce que ça sait faire aujourd'hui

- Lire une note Obsidian : `![[image.png|400]]` cherchée dans tout le coffre, `[[liens]]`, callouts `> [!tip] Titre`, `==surlignage==`, `%%commentaires%%`, ids de bloc `^abc`, notes `[^1]` et `^[en ligne]`, tâches, tableaux, code, maths LaTeX `$…$` / `$$…$$`, frontmatter (titre, auteur, date, tags, langue).
- Mettre en page avec [Typst](https://typst.app), en local, en ~100 ms : page de garde, numéros de page, PDF balisé (accessible).
- Appliquer des **retouches** par bloc :

| Retouche | Clé JSON | Commentaire dans la note |
|---|---|---|
| Nouvelle page avant le bloc | `break_before` | `break-before` / `saut-avant` |
| Reste de la page vide après le bloc | `break_after` | `break-after` / `saut-apres` |
| Ne pas séparer du bloc suivant | `keep_with_next` | `keep-with-next` |
| Pousser en bas de page | `push_to_bottom` | `push-to-bottom` |
| Masquer à l'export | `hidden` | `hidden` / `masquer` |
| Espace avant (mm) | `space_before_mm` | `space-before=10` |
| Format de page à partir d'ici | `page` | `page=a3-paysage`, `page=210x99`, `page=default` |
| Largeur d'image (% du texte) | `image.width_percent` | `width=60` |
| Image en haut / bas / pleine page | `image.placement` | `placement=top\|bottom\|full-page` |
| Légende d'image | `image.caption` | `caption=…` |

Un saut avant une puce coupe la liste en gardant la numérotation. Un saut ou un changement de format posé sur un bloc remonte avant les titres qui le précèdent : un titre ne reste jamais seul en bas de page.

## Où vivent les retouches

Dans `<coffre>/.nectar/<chemin de la note>.json`. Obsidian ignore les dossiers qui commencent par un point : la note reste intacte. Chaque retouche désigne un bloc par un **id stable** tiré de son contenu (`p-8d8700e1`) ; si tu modifies le texte, Nectar retrouve le bloc par ressemblance, sinon il signale la retouche orpheline.

Repli possible directement dans la note, juste avant le bloc :

```markdown
<!-- nectar: saut-avant, page=a3-paysage -->
![[schema.png]]
```

`<!-- pagebreak -->` et `\pagebreak` (ancien Nectar) restent compris.

## Ligne de commande

```powershell
cargo run --release -p nectar-cli -- export "examples/coffre-demo/Démo Nectar.md" --png out
```

```text
nectar export <note.md> [-o sortie.pdf] [--png dossier] [--ppi 110] [--system-fonts]
nectar blocks <note.md>                  # ids, types, lignes et pages des blocs
nectar set <note.md> <id> <retouches>    # ex. : nectar set note.md li-8f4c06da break-before
nectar unset <note.md> <id>
nectar typst <note.md>                   # la source Typst générée
nectar presets                           # presets intégrés et personnels
nectar use-preset <note.md> <preset>     # choisir le preset d'une note
nectar export <note.md> --preset magazine  # essayer un preset sans l'enregistrer
```

Le coffre `examples/coffre-demo` montre les trois cas d'origine : saut entre la phrase d'introduction et la première puce, image suivie d'une page vide, grand schéma sur une page A3 paysage au milieu d'un document A4.

## Architecture

```
crates/
  nectar-core    lecture Markdown/Obsidian (comrak), modèle de blocs à ids stables,
                 retouches (.nectar/*.json), génération de la source Typst
  nectar-typst   monde Typst virtuel et hors ligne, polices embarquées,
                 export PDF, rendu PNG/RGBA des pages, position des blocs
  nectar-cli     ligne de commande
  nectar-app     l'atelier (egui) : aperçu, retouches à la souris, styles
assets/
  fonts/         IBM Plex et JetBrains Mono (OFL)
  presets/       les 10 presets intégrés (JSON partiels)
  typst/         le template unique (nectar.typ) et les spécifications mitex (LaTeX → Typst)
```

Le template `nectar.typ` ne contient aucune couleur ni police en dur : il lit `/nectar/style.typ`, généré à partir du style de la note.

## Feuille de route

1. ~~Cœur + CLI : note Obsidian → PDF, retouches, pages de formats mixtes~~
2. ~~Styles réglables, 10 presets, blocs de code façon éditeur~~
3. ~~Atelier : aperçu en direct, clic sur un bloc → retouches, panneau de style, annuler/rétablir~~
4. Glisser une image sur la page pour la déplacer, poignées de redimensionnement
5. Inclusion de notes `![[note]]`, diagrammes Mermaid, PDF/A, installateur Windows

## Licences

Code : PolyForm Noncommercial 1.0.0 (`LICENSE`). Polices IBM Plex et JetBrains Mono : SIL OFL 1.1. Spécifications mitex : Apache-2.0.
