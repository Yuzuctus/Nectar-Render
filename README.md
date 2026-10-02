# Nectar Render

**L'atelier de mise en page des notes Obsidian.** Tu écris dans Obsidian ; Nectar transforme n'importe quelle note Markdown en PDF soigné, place tout intelligemment dès le départ, puis te laisse tout retoucher à la main sur un aperçu fidèle avant l'export. Tout reste local.

> Version 2, réécrite de zéro en Rust pour Windows. L'ancienne version Python/Tkinter reste dans l'historique de `main`.

## Installer

- **Installateur Windows** (`NectarRender-x.y.z-installation.exe`, sans droits administrateur) ou **version portable** (`.zip`), dans les [releases](https://github.com/Yuzuctus/Nectar-Render/releases).
- **Plugin Obsidian** : copier le dossier `nectar-render` (fourni dans l'installation, sous `plugin-obsidian`) dans `<coffre>/.obsidian/plugins/`, puis l'activer. Il ajoute « Ouvrir dans Nectar Render », « Exporter en PDF » et « Vérifier la mise en page » (palette de commandes, clic droit sur une note, icône du ruban).

## L'atelier

- Les **pages du PDF** au centre, mises en page en direct ; la note est surveillée, chaque enregistrement dans Obsidian met l'aperçu à jour. Seules les pages visibles sont dessinées : 50 pages restent fluides.
- **À la souris, directement sur la page** :
  - clic sur un bloc : il est surligné et une **barre d'actions** apparaît dessus (nouvelle page avant, garder avec la suite, d'un seul tenant, page paysage, pleine page…) ;
  - **glisser un bloc** vers le haut ou le bas le déplace au demi-millimètre, avec tout ce qui suit, en direct ;
  - la **poignée** à droite d'une image règle sa largeur ;
  - **clic droit** : les mêmes actions rapides ;
  - **clic sur le numéro d'une page** (ou dans sa marge, ou dans le blanc sous le texte) : la page est sélectionnée et on choisit son format en un clic (A4 paysage, A3, A3 paysage, autre).
- Au clavier : Alt + ↑ / ↓ déplace le bloc d'1 mm (Maj : 5 mm), Ctrl + Entrée lui donne une nouvelle page, Suppr efface ses retouches, ↑ / ↓ passe au bloc voisin.
- Onglet **Retoucher** : l'essentiel d'abord (actions, décalage, image, colonnes du tableau), le reste dans « Plus d'options » (format de la page du bloc, apparence, masquer, appliquer à tous les titres du même niveau).
- **Format d'une page** : la page prend ce format, se remplit avec la suite, puis le document reprend tout seul son format. Cocher « Et les pages suivantes » pour le garder jusqu'au prochain changement.
- **Plan** dans la barre du haut : aller directement à un titre.
- **Repères jaunes dans la marge** sur les blocs retouchés (résumé au survol).
- Onglet **Style** : un preset en un clic, l'essentiel (police, taille, marges, thème du code, page de garde, numéros, sommaire), tout le reste dans « Réglages détaillés » ; « Enregistrer comme preset… ».
- Onglet **Vérifier** : l'assistant de mise en page (voir plus bas).
- Annuler et rétablir (un glisser = une seule étape), export PDF, zoom (Ctrl + molette), notes récentes, « ouvrir dans Obsidian », thème clair ou sombre au design Agrume.
- Chaque option dit ce qu'elle fait, en une phrase ; **Aide** (F1) résume gestes et raccourcis ; **Affichage** règle la taille de l'interface (90 à 150 %).

## Placé intelligemment dès le départ

- Les titres restent avec leur contenu ; une phrase qui finit par « : » reste avec ce qu'elle annonce.
- Liste courte, code court, petit tableau, encadré : jamais coupés entre deux pages.
- Lignes isolées (veuves, orphelines) évitées ; images limitées à 85 % de la page pour garder leur annonce et leur légende ; une photo ou une capture verticale à 60 % (réglable), un schéma vertical garde sa taille.
- Tableaux : chaque colonne garde sa largeur naturelle si tout tient ; sinon les colonnes les plus longues passent à la ligne en premier, sans jamais couper un mot ni sortir de la marge.

Puis Nectar relit les pages produites et corrige ce qui gâche le PDF, sans qu'on le lui demande :

- **Tableaux trop larges** (5 colonnes ou plus, qui déborderaient ou passeraient beaucoup à la ligne) : sur une page paysage, avec leur titre et leur phrase d'annonce ; le texte reprend ensuite en portrait.
- **Grands tableaux sur plusieurs pages** : Nectar essaie la page A4 paysage, puis la page A3 paysage, et garde la première où le tableau tient en entier (sinon il reste en portrait).
- **Schémas larges et détaillés** (Mermaid, Excalidraw, image nommée « schéma », « architecture », « topologie »…) : page paysage. Une image verticale reste toujours en portrait.
- **Image horizontale en tête de sa page** (avec son titre, sa légende, et le texte qui la suit sur la page) quand un blanc reste en bas : la page passe en paysage, l'image grandit et le blanc disparaît ; jamais si cela ajoute une page.
- **Titres** : un titre (ou une ligne courte tout en gras) suivi de moins de quatre lignes de son contenu en bas de page passe en haut de la page suivante.
- **Taille des images** : une image est un peu réduite (jusqu'à 25 %) pour que le bloc suivant tienne sur la page plutôt que d'être coupé ou rejeté ; elle grandit pour combler un blanc en bas de page que rien d'autre ne peut remplir, si sa résolution le permet. Une taille fixée dans la note (`![[image.png|400]]`) est respectée.
- **Légendes** : un paragraphe « Capture : … », « Figure … » ou tout en italique, juste sous une image ou un tableau, ne le quitte jamais.
- **Pas de trou en bas de page** : une image un peu trop haute pour la place restante est réduite juste assez (jamais sous 55 %) ; un bloc gardé d'un seul tenant est autorisé à se couper (le tableau répète son en-tête) ; un tableau qui déborde de 2 ou 3 lignes est un peu resserré ; une page paysage qui laisserait la page d'avant à moitié vide passe après le texte qui la suit, sans quitter la section.
- **Dernière page de quelques lignes** : l'espace entre paragraphes se resserre un peu pour la supprimer.

Ces décisions ne sont pas écrites dans les retouches : elles sont recalculées à chaque mise en page, listées par `nectar check` et dans l'atelier (« Fait automatiquement »). Pour en refuser une : « Laisser ce bloc tel quel » (retouche `tel-quel`) ; pour toutes, Style → Placement automatique. Une retouche l'emporte toujours sur ces règles.

**Assistant** (`Vérifier` dans l'atelier, `nectar check` en ligne de commande) : il relit les pages réelles et signale :

- une page à moitié vide, avec le bloc responsable ;
- un schéma à lire en grand, si le placement automatique est coupé (« Mettre les N schémas en paysage » d'un coup) ;
- un titre isolé, un contenu qui dépasse la marge, une image réduite pour tenir ;
- une dernière page presque vide ;
- une image ou une note introuvable, une formule non convertie, une police absente.

Corrections en un clic : réduire ou faire flotter l'image, lui donner une page, autoriser la coupure, passer le titre à la page suivante.

## Tout le Markdown, rendu proprement

- **Obsidian** :
  - images du coffre `![[image.png|400]]` et `![[doc.pdf#page=3]]` ;
  - inclusion de notes `![[note]]` et de sections `![[note#Titre]]` ;
  - liens `[[…]]`, callouts `> [!tip]`, `==surlignage==`, `%%commentaires%%`, ids de bloc, notes `[^1]` et `^[en ligne]`, tâches, frontmatter.
- **Schémas** :
  - **Mermaid** (organigrammes, séquences, classes, états, entités, Gantt, camemberts, cartes mentales…) dessiné localement avec la police du document ;
  - dessins **Excalidraw** du plugin Obsidian redessinés avec leur trait à main levée.
- **Code** façon éditeur :
  - 11 thèmes (VS Code, GitHub, One Dark, Monokai, Dracula, Solarized, Nord, Xcode, Agrume) ;
  - onglet avec le nom du fichier (` ```rust title="main.rs" `), numéros de ligne, lignes surlignées (`{2,4-5}`) ;
  - les lignes trop longues repartent alignées sur le code.
- **Maths** LaTeX `$…$` et `$$…$$`, police des formules au choix.
- **Typographie française** : espaces fines insécables devant `; ! ?` et dans « », insécable devant `:`, guillemets « ».
- **Émojis** en couleur, **HTML courant** (`<img width>`, `<u>`, `<sup>`, `<mark>`, `<kbd>`, images centrées), **liens internes** cliquables vers les titres (`[[#Titre]]`).
- **PDF** balisé (accessible), PDF/A-2b en option, photos trop lourdes allégées.

## Styles

10 presets repris de la v1 : **Agrume, Académique, Magazine, Entreprise, Technique, Minimal, Carnet, Créatif, Développeur, Élégant**.

Presets personnels dans `%APPDATA%\Nectar Render\presets\`. Un style se compose en couches : défaut ← preset ← réglages de la note (seul ce qui diffère du preset est rangé). Une police absente est remplacée par une police de secours de la même famille.

## Où vivent les retouches

Dans `<coffre>/.nectar/<chemin de la note>.json`, invisible dans Obsidian : la note reste intacte. Chaque retouche désigne un bloc par un id tiré de son contenu. Si tu modifies le texte, Nectar retrouve le bloc par ressemblance et réécrit l'ancre ; sinon il signale la retouche orpheline.

Repli possible dans la note, juste avant le bloc :

```markdown
<!-- nectar: saut-avant, page=a3-paysage, largeur=80, centre -->
<!-- nectar: placement=paysage -->
<!-- nectar: page=a3, suite -->   (A3 pour cette page et les suivantes)
![[schema.png]]
```

Une retouche faite dans l'atelier remplace celle écrite dans la note. `<!-- pagebreak -->` et `\pagebreak` (ancien Nectar) restent compris.

## Avec une IA (serveur MCP)

`nectar mcp` est un serveur [MCP](https://modelcontextprotocol.io) : une IA (Claude Desktop, Claude Code…) peut lire une note, voir ses pages, la vérifier, la retoucher, régler son style et exporter le PDF. Les retouches vont dans le même fichier que celles de l'atelier : s'il est ouvert, il les montre aussitôt.

**Il faut le brancher une fois** : une IA ne découvre pas seule ses outils.

- **Claude Desktop** : dans l'atelier, Aide → « Connecter à Claude Desktop » (ou `nectar mcp install`), puis quitter complètement Claude Desktop et le rouvrir. Nectar est ajouté à ses réglages sans toucher aux autres outils (copie de l'ancien fichier en `.bak`).
- **Claude Code** : une fois, dans un terminal (la commande exacte est dans l'Aide de l'atelier, bouton « Copier ») : `claude mcp add nectar-render --scope user -- "<chemin>\nectar.exe" mcp`.
- **Autre client MCP** : la même chose à la main, dans ses réglages :

```json
{
  "mcpServers": {
    "nectar-render": {
      "command": "C:\\Users\\<toi>\\AppData\\Local\\Programs\\Nectar Render\\nectar.exe",
      "args": ["mcp"]
    }
  }
}
```

`check_layout` liste aussi ce que le placement automatique a déjà décidé : l'IA ne retouche que ce qui reste à revoir.

| Outil | Ce qu'il fait |
|---|---|
| `read_note` | pages (numéro, format) et blocs (id, type, page, extrait, retouches) |
| `check_layout` | défauts de mise en page, avec la retouche proposée pour chacun |
| `render_page` | une page en image, pour juger le rendu |
| `set_block` | retoucher un bloc (`saut-avant`, `largeur=70`, `placement=paysage`…) |
| `set_page_format` | format d'une page (`a3-paysage`, `normal`…), cette page seulement ou la suite |
| `list_presets`, `get_style`, `set_style` | presets et réglages du style |
| `export_pdf` | le PDF final |

Exemple de demande : « Mets en page mon compte rendu `C:\Coffre\TP réseau.md` : style Académique, schémas en paysage, aucune page à moitié vide, puis exporte le PDF. »

## Ligne de commande

```text
nectar export <note.md> [-o sortie.pdf] [--png dossier] [--system-fonts] [--preset magazine]
nectar check <note.md>                   # décisions automatiques, défauts restants, temps de calcul
nectar blocks <note.md>                  # ids, types, lignes et pages des blocs
nectar set <note.md> <id> <retouches>    # ex. : nectar set note.md li-8f4c06da saut-avant
nectar unset <note.md> <id>
nectar presets | nectar use-preset <note.md> <preset>
nectar typst <note.md>                   # la source Typst générée
nectar mcp                               # serveur MCP pour les IA
nectar mcp install                       # le brancher à Claude Desktop (+ commande pour Claude Code)
```

Le coffre `examples/coffre-demo` montre les cas d'origine : saut entre la phrase d'introduction et la première puce, image suivie d'une page vide, grand schéma sur une page A3 paysage au milieu d'un A4. Il contient aussi un dessin Excalidraw et un diagramme Mermaid.

## Développer

```powershell
cargo run -p nectar-app -- "examples/coffre-demo/Démo Nectar.md"
cargo test --workspace
cd obsidian-plugin; npm ci; npm run build
```

```
crates/
  nectar-core    lecture Markdown/Obsidian, blocs à ids stables, retouches,
                 styles et presets, génération Typst, Excalidraw, assistant
  nectar-typst   moteur Typst hors ligne : polices embarquées, PDF, pages
                 en images, positions et boîtes des blocs, analyse des pages
  nectar-cli     ligne de commande (nectar.exe)
  nectar-app     l'atelier egui (nectar-render.exe)
assets/          polices, presets, template Typst, icône
obsidian-plugin/ le plugin Obsidian (TypeScript)
packaging/       installateur Windows (Inno Setup)
```

Un tag `vX.Y.Z` publie l'installateur, la version portable et le plugin.

## Dépannage

- **Journal** : les erreurs du moteur et les plantages sont notés, datés, dans `%LOCALAPPDATA%\Nectar Render\journal.txt` (à joindre à un signalement).
- **Photos** : réduites une seule fois puis gardées dans `%LOCALAPPDATA%\Nectar Render\photos` (400 Mo au plus, élagué tout seul) ; on peut vider ce dossier sans risque.
- **Retouches** : écrites sans jamais laisser de fichier à moitié écrit ; un fichier de retouches illisible est mis de côté (`….json.illisible`) et la note s'ouvre quand même.

## Licences

Code : PolyForm Noncommercial 1.0.0 (`LICENSE`). Polices :
- IBM Plex, JetBrains Mono, Virgil et Excalifont : SIL OFL 1.1 ;
- Twemoji : CC-BY 4.0 / Apache 2.0.

Spécifications mitex : Apache-2.0.
