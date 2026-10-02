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
- Annuler et rétablir (un glisser = une seule étape), export PDF, zoom, notes récentes, « ouvrir dans Obsidian », thème clair ou sombre au design Agrume.

## Placé intelligemment dès le départ

- Les titres restent avec leur contenu ; une phrase qui finit par « : » reste avec ce qu'elle annonce.
- Liste courte, code court, petit tableau, encadré : jamais coupés entre deux pages.
- Lignes isolées (veuves, orphelines) évitées ; images limitées à 85 % de la page pour garder leur annonce et leur légende.
- Tableaux : les colonnes courtes (adresses, nombres, dates) gardent leur largeur et ne passent pas à la ligne ; les colonnes de texte se partagent le reste.
- **Second passage** : Nectar relit les pages produites. Un bloc gardé d'un seul tenant qui laisserait une demi-page vide est autorisé à se couper (le tableau répète son en-tête).
- **Schémas en grand** : un schéma large et détaillé est repéré et peut passer, en un clic, sur une page paysage. Le texte qui suit remplit d'abord la page en cours, puis vient la page paysage, sans quitter la section.
- Une retouche l'emporte toujours sur ces règles.

**Assistant** (`Vérifier` dans l'atelier, `nectar check` en ligne de commande) : il relit les pages réelles et signale :

- une page à moitié vide, avec le bloc responsable ;
- un schéma à lire en grand (« Mettre les N schémas en paysage » d'un coup) ;
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

## Ligne de commande

```text
nectar export <note.md> [-o sortie.pdf] [--png dossier] [--system-fonts] [--preset magazine]
nectar check <note.md>                   # l'assistant de mise en page
nectar blocks <note.md>                  # ids, types, lignes et pages des blocs
nectar set <note.md> <id> <retouches>    # ex. : nectar set note.md li-8f4c06da saut-avant
nectar unset <note.md> <id>
nectar presets | nectar use-preset <note.md> <preset>
nectar typst <note.md>                   # la source Typst générée
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
