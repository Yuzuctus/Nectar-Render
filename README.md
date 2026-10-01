# Nectar Render

**Atelier de mise en page pour Obsidian.** Tu écris dans Obsidian ; Nectar transforme la note en PDF soigné, et tu décides à la main où tombent les pages, quel format prend chaque page et comment se placent les images. Tout reste local.

> Version 2, réécrite de zéro en Rust. L'ancienne version Python/Tkinter reste dans l'historique de `main` (tag `python-final` à poser).

## Ce que ça sait faire aujourd'hui

- Lire une note Obsidian : `![[image.png|400]]` cherchée dans tout le coffre, `[[liens]]`, callouts `> [!tip] Titre`, `==surlignage==`, `%%commentaires%%`, ids de bloc `^abc`, notes `[^1]` et `^[en ligne]`, tâches, tableaux, code, maths LaTeX `$…$` / `$$…$$`, frontmatter (titre, auteur, date, tags, langue).
- Mettre en page avec [Typst](https://typst.app), en local, en ~100 ms : thème **Agrume** (IBM Plex, encre et papier, jaune yuzu), page de garde, numéros de page, PDF balisé (accessible).
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

## Essayer

```powershell
cargo run --release -p nectar-cli -- export "examples/coffre-demo/Démo Nectar.md" --png out
```

```text
nectar export <note.md> [-o sortie.pdf] [--png dossier] [--ppi 110] [--system-fonts]
nectar blocks <note.md>                  # ids, types, lignes et pages des blocs
nectar set <note.md> <id> <retouches>    # ex. : nectar set note.md li-8f4c06da break-before
nectar unset <note.md> <id>
nectar typst <note.md>                   # la source Typst générée
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
assets/
  fonts/         IBM Plex (OFL)
  themes/        thèmes Typst (agrume.typ + coloration du code)
  typst/         aides communes (lib.typ) et spécifications mitex (LaTeX → Typst)
```

Un thème est un fichier `.typ` qui exporte `template` et `page-margin`, et peut redéfinir les aides de `lib.typ` (`callout`, `wikilink`, `task`…).

## Feuille de route

1. ~~Cœur + CLI : note Obsidian → PDF, retouches, pages de formats mixtes~~
2. Atelier `egui` (Windows) : ouvrir une note, aperçu des pages, surveillance du fichier, réglages du thème
3. Clic sur un bloc dans l'aperçu → retouches, annuler/rétablir
4. Inclusion de notes `![[note]]`, diagrammes Mermaid, PDF/A, installateur Windows

## Licences

Code : PolyForm Noncommercial 1.0.0 (`LICENSE`). Polices IBM Plex : SIL OFL 1.1. Spécifications mitex : Apache-2.0.
