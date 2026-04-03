# FinalAudit - Audit consolidé et vérifié

## Sources consolidées

Les cinq audits racine ont été relus intégralement, mais aucun constat n'a été repris sans revalidation dans le code actuel du workspace.

- `Audit.md`
- `ClaudeAudit.md`
- `DolaAudit.md`
- `GeminiAudit.md`
- `GPT5.4Audit.md`

Qualité relative des sources relues :

- `Audit.md` contenait plusieurs points utiles sur la sanitisation par défaut, le bootstrap Windows et la dette d'outillage.
- `ClaudeAudit.md` contenait plusieurs constats utiles sur le cache image, le no-op du CLI, la liste Pygments et la dette structurelle UI.
- `DolaAudit.md` contenait quelques pistes utiles, mais aussi plusieurs faux positifs ou hypothèses spéculatives. Il a été utilisé avec prudence.
- `GeminiAudit.md` était surtout utile pour confirmer la dette structurelle, mais plusieurs assertions sur les exceptions étaient inexactes.
- `GPT5.4Audit.md` servait de base de consolidation, puis a été revérifié ligne par ligne.

## Méthodologie de vérification

Code et fichiers relus :

- 52 fichiers du workspace ont été pris en compte pour ce rapport.
- Répartition :
- 35 fichiers trackés par Git dans le dépôt.
- 12 fichiers source/tests/config non trackés mais présents dans le workspace actuel.
- 5 audits racine consolidés.

Vérifications automatiques relancées sur l'état actuel du worktree :

- `.\.venv\Scripts\python.exe -m pytest` -> 130 tests passés.
- `.\.venv\Scripts\python.exe -m ruff check .` -> aucun échec.

Vérifications ciblées exécutées manuellement :

- Probe de sanitisation : `parse_markdown(..., sanitize_html=True)` conserve encore aujourd'hui des URI `file:` et `data:`.
- Probe d'import frais : `import nectar_render.cli` charge bien `nectar_render.ui`, `nectar_render.ui.app` et `tkinter`.
- Probe d'import frais : `import nectar_render.main` charge aussi la chaîne GUI à cause de l'import du CLI.
- Probe métier : `PdfCompressionService.compress(..., CompressionOptions(enabled=False, remove_metadata=True))` est actuellement un no-op.
- Vérification Git : `git ls-files` et `git status --ignored --short` ont été utilisés pour distinguer code tracké, fichiers non trackés et artefacts ignorés.

Important sur le périmètre :

- Le dépôt Git tracké ne reflète pas exactement tout le workspace actuel. Il existe des fichiers de code/tests/config non trackés mais présents localement.
- `examples/output/` et `.vscode/` sont ignorés par Git dans l'état observé. Ils existent localement, mais ne doivent pas être présentés comme des fichiers du dépôt.
- Ce rapport décrit l'état réel du projet dans ce workspace, en distinguant explicitement ce qui est tracké de ce qui est seulement local.

## Résumé global

- Problèmes retenus et vérifiés : 20
- Constats écartés après vérification : 12

Répartition par sévérité :

- Critiques : 2
- Importants : 9
- Mineurs : 9

Répartition par catégorie :

- Sécurité : 2
- Performance : 3
- Structure : 6
- Duplication : 1
- Lisibilité : 1
- Maintenance : 7

Les 3 priorités absolues :

1. Activer une sanitisation HTML sûre par défaut et retirer `file:` / `data:` de la liste blanche.
2. Découpler le CLI de la couche GUI pour qu'un import de `nectar_render.cli` ou `nectar_render.main` ne charge plus `tkinter`.
3. Corriger la chaîne PDF pour que `remove_metadata` fonctionne indépendamment de la compression et pour éviter la double génération HTML en `PDF+HTML`.

## Problèmes retenus et vérifiés

### FA-01 - HTML brut autorisé par défaut

Priorité : Critique  
Catégorie : sécurité

Preuve actuelle :

- `src/nectar_render/config.py:68` définit `StyleOptions.sanitize_html = False`.
- `src/nectar_render/converter/markdown_parser.py:373` définit `parse_markdown(... sanitize_html: bool = False)`.
- `src/nectar_render/converter/markdown_parser.py:402-403` n'applique la sanitisation que si ce booléen vaut `True`.
- `src/nectar_render/converter/exporter.py:22-27` propage directement `style.sanitize_html` vers le parseur.
- `tests/test_markdown_parser.py:259-265` verrouille explicitement le comportement "garder le HTML brut quand la sanitisation est désactivée".

Localisation :

- `src/nectar_render/config.py:26-70`
- `src/nectar_render/converter/markdown_parser.py:369-404`
- `src/nectar_render/converter/exporter.py:15-30`
- `tests/test_markdown_parser.py:249-265`

Pourquoi c'est un problème :

- Du HTML brut injecté dans le Markdown passe aujourd'hui jusqu'au HTML exporté, et donc jusqu'au rendu PDF WeasyPrint, sans protection par défaut.
- Le comportement par défaut n'est pas défensif. Il élargit inutilement la surface d'attaque et rend le rendu dépendant du niveau de confiance accordé à l'entrée.
- Le problème est aggravé par le fait qu'un test existant verrouille l'état non sûr.

Correction attendue :

- Passer en mode sûr par défaut dans `StyleOptions` et dans `parse_markdown`.
- Conserver au besoin une possibilité explicite d'opt-out pour un usage local et maîtrisé, mais ne plus l'activer implicitement.
- Mettre à jour les tests existants pour qu'ils valident le comportement sécurisé par défaut.

Fonctions / symboles à modifier :

- `StyleOptions.sanitize_html`
- `parse_markdown`
- `build_html_from_markdown`
- `tests/test_markdown_parser.py::test_parse_markdown_keeps_raw_html_when_sanitization_disabled`

Tests à ajouter ou adapter :

- Adapter le test actuel pour valider que le comportement par défaut est désormais sûr.
- Ajouter un test montrant qu'un `<script>` ou un tag HTML brut non autorisé ne passe plus par défaut.
- Conserver un test dédié au mode explicitement non sanitizé si ce mode reste supporté.

Statut de vérification :

- Confirmé dans le code actuel.

### FA-02 - La sanitisation autorise encore les protocoles `file:` et `data:`

Priorité : Important  
Catégorie : sécurité

Preuve actuelle :

- `src/nectar_render/converter/markdown_parser.py:67` contient `_ALLOWED_HTML_PROTOCOLS = ["http", "https", "mailto", "data", "file"]`.
- `src/nectar_render/converter/markdown_parser.py:359-366` transmet cette liste blanche à `bleach.clean(...)`.
- Probe exécutée dans l'environnement actuel :
- entrée : `<a href="file:///C:/secret.txt">x</a> <img src="data:image/png;base64,AAAA" alt="x"/>`
- sortie observée avec `sanitize_html=True` : `<p><a href="file:///C:/secret.txt">x</a> <img alt="x" src="data:image/png;base64,AAAA"/></p>`

Localisation :

- `src/nectar_render/converter/markdown_parser.py:67`
- `src/nectar_render/converter/markdown_parser.py:359-366`

Pourquoi c'est un problème :

- En l'état, même lorsque la sanitisation est explicitement activée, le parseur conserve des URI locales `file:` et des `data:` inline.
- `file:` peut exposer des chemins locaux dans les exports HTML et produire des liens non désirés vers le filesystem.
- `data:` permet de conserver du contenu inline arbitraire et augmente le périmètre à sécuriser.

Correction attendue :

- Retirer `file` et `data` de `_ALLOWED_HTML_PROTOCOLS`.
- Ne pas essayer de préserver les URI `data:image/...` dans cette phase.
- Si nécessaire, laisser l'attribut vide, supprimer l'attribut ou laisser `bleach` neutraliser la valeur. Le point important est de ne plus autoriser ces schémas.

Fonctions / symboles à modifier :

- `_ALLOWED_HTML_PROTOCOLS`
- `sanitize_html_fragment`
- `parse_markdown`

Tests à ajouter ou adapter :

- Ajouter un test qui vérifie qu'un lien `file:///...` est neutralisé quand `sanitize_html=True`.
- Ajouter un test qui vérifie qu'une image `data:image/...` est neutralisée quand `sanitize_html=True`.

Statut de vérification :

- Confirmé dans le code actuel.

### FA-03 - Le CLI charge encore la GUI et `tkinter` à l'import

Priorité : Critique  
Catégorie : structure

Preuve actuelle :

- `src/nectar_render/main.py:6` importe `build_parser` et `run_cli` depuis `.cli` au chargement du module.
- `src/nectar_render/cli.py:18` importe `BUILTIN_PRESET_NAMES` et `BUILTIN_PRESETS` depuis `.ui.presets`.
- `src/nectar_render/ui/__init__.py:1` réexporte `NectarRenderApp` via `from .app import NectarRenderApp`.
- Dans un interpréteur frais, `import nectar_render.cli` charge `nectar_render.ui`, `nectar_render.ui.app` et plusieurs modules `tkinter`.
- Dans un interpréteur frais, `import nectar_render.main` charge aussi `nectar_render.ui`, `nectar_render.ui.app` et `tkinter`.
- `tests/test_main.py:17-44` passe malgré ce bug car les tests ne simulent pas un import vraiment frais : ils retirent `nectar_render.main` et `nectar_render.ui.app`, mais pas `nectar_render.ui` ni `nectar_render.ui.presets`.

Localisation :

- `src/nectar_render/main.py:6`
- `src/nectar_render/cli.py:18`
- `src/nectar_render/ui/__init__.py:1-3`
- `tests/test_main.py:17-44`

Pourquoi c'est un problème :

- Le chemin CLI n'est pas isolé de la GUI. Un simple import du CLI charge déjà Tkinter.
- Cela casse l'objectif d'un mode CLI léger, complique les usages headless et masque les dépendances réelles du chemin console.
- Les tests actuels donnent un faux sentiment de sécurité sur ce point précis.

Correction attendue :

- Supprimer l'effet de bord d'import depuis `src/nectar_render/ui/__init__.py`, ou éviter complètement de faire passer le CLI par le package `ui`.
- Déplacer les presets vers un module neutre, ou les importer par un chemin qui n'exécute pas `ui/__init__.py`.
- Renforcer les tests avec une vérification en interpréteur frais.

Fonctions / symboles à modifier :

- `src/nectar_render/ui/__init__.py`
- `src/nectar_render/cli.py`
- `src/nectar_render/main.py`
- `tests/test_main.py`
- éventuellement `tests/test_cli.py`

Tests à ajouter ou adapter :

- Ajouter un test dédié qui lance un interpréteur Python frais et vérifie que `import nectar_render.cli` ne charge pas `tkinter`.
- Ajouter le même type de test pour `import nectar_render.main`.
- Corriger `tests/test_main.py` pour qu'il vide aussi `nectar_render.ui` et `nectar_render.ui.presets` s'il reste sur une logique intra-process.

Statut de vérification :

- Confirmé dans le code actuel.

### FA-04 - La chaîne de compression PDF est incohérente

Priorité : Important  
Catégorie : structure

Preuve actuelle :

- `src/nectar_render/services/pdf_compression_service.py:32-38` retourne immédiatement si `options.enabled` vaut `False`.
- `src/nectar_render/services/pdf_compression_service.py:60-66` n'exécute `_remove_metadata(...)` qu'après cette garde, donc jamais quand la compression est désactivée.
- Probe exécutée : `CompressionOptions(enabled=False, remove_metadata=True)` retourne le même chemin, `applied=False`, `tool=None`, sans tentative de retrait des métadonnées.
- `src/nectar_render/converter/exporter.py:79-100` applique aussi une compression/optimisation à la génération (`optimize_images`, `jpeg_quality`, `dpi`) quand `compression.enabled=True`.
- `src/nectar_render/services/conversion_service.py:86-88` appelle ensuite `PdfCompressionService.compress(...)`, qui peut encore lancer `qpdf` sur le PDF déjà optimisé.
- `src/nectar_render/ui/panels.py:108-125` présente pourtant "Automatic PDF compression" et "Remove metadata" comme deux cases séparées, alors que l'implémentation mélange plusieurs responsabilités sous les mêmes options.

Localisation :

- `src/nectar_render/services/pdf_compression_service.py:27-38`
- `src/nectar_render/services/pdf_compression_service.py:60-66`
- `src/nectar_render/converter/exporter.py:79-100`
- `src/nectar_render/ui/panels.py:108-125`
- `tests/test_pdf_compression_service.py:24-37`

Pourquoi c'est un problème :

- L'UI expose deux intentions métier distinctes, mais l'implémentation les couple.
- L'utilisateur peut croire qu'il retire les métadonnées tout en laissant la compression désactivée, alors que rien ne se passe.
- La même option de compression pilote aujourd'hui deux étages différents : optimisation à la génération côté WeasyPrint, puis post-traitement côté `qpdf`.
- Ce couplage rend le comportement difficile à raisonner, complique les tests et peut ajouter du travail CPU sans bénéfice clairement mesuré.

Correction attendue :

- Découpler clairement "compression" et "suppression des métadonnées".
- Faire fonctionner `remove_metadata=True` même lorsque `enabled=False`.
- Appliquer `custom_metadata=False` dans `export_pdf` dès que `remove_metadata=True`, indépendamment du profil de compression.
- Clarifier aussi la responsabilité de compression entre `export_pdf` et `PdfCompressionService` :
- soit une seule couche porte la compression,
- soit les deux couches restent, mais avec des rôles explicitement séparés et testés.

Fonctions / symboles à modifier :

- `PdfCompressionService.compress`
- `PdfCompressionService._remove_metadata`
- `ConversionService.convert`
- `export_pdf`
- `CompressionOptions`

Tests à ajouter ou adapter :

- Ajouter un test qui couvre explicitement `CompressionOptions(enabled=False, remove_metadata=True)`.
- Ajouter ou adapter un test côté exporter pour vérifier que le chemin WeasyPrint reçoit aussi la bonne option quand la compression est désactivée.
- Ajouter un test qui verrouille la stratégie retenue entre compression WeasyPrint et compression post-traitement.

Statut de vérification :

- Confirmé dans le code actuel.

### FA-05 - Le chemin `PDF+HTML` reconstruit deux fois le HTML complet

Priorité : Important  
Catégorie : performance

Preuve actuelle :

- `src/nectar_render/services/conversion_service.py:65-74` appelle `export_html(...)` pour `HTML` et `PDF+HTML`.
- `src/nectar_render/services/conversion_service.py:76-99` appelle ensuite `export_pdf(...)` pour `PDF` et `PDF+HTML`.
- `src/nectar_render/converter/exporter.py:41-47` fait reconstruire le HTML via `build_html_from_markdown(...)`.
- `src/nectar_render/converter/exporter.py:68-74` refait exactement la même reconstruction HTML pour le PDF.

Localisation :

- `src/nectar_render/services/conversion_service.py:30-109`
- `src/nectar_render/converter/exporter.py:15-30`
- `src/nectar_render/converter/exporter.py:33-50`
- `src/nectar_render/converter/exporter.py:53-102`

Pourquoi c'est un problème :

- En `PDF+HTML`, le Markdown est parsé deux fois, les notes sont réinjectées deux fois, la résolution d'images est refaite et le CSS Pygments est régénéré.
- Cela consomme plus de CPU et de mémoire que nécessaire.
- Toute divergence future entre les deux chemins de rendu sera plus difficile à éviter.

Correction attendue :

- Construire le HTML une seule fois pour le chemin `PDF+HTML`.
- Réutiliser exactement la même chaîne HTML pour l'écriture du fichier `.html` et pour `weasyprint.HTML(...).render()`.
- Le refactor doit rester local et ne pas introduire de nouvelle fonctionnalité.

Fonctions / symboles à modifier :

- `ConversionService.convert`
- `build_html_from_markdown`
- `export_html`
- `export_pdf`

Tests à ajouter ou adapter :

- Ajouter un test qui monkeypatche `build_html_from_markdown` et vérifie qu'en `PDF+HTML`, l'appel n'a lieu qu'une seule fois.
- Conserver un test garantissant que les chemins `HTML` et `PDF` seuls gardent le même comportement.

Statut de vérification :

- Confirmé dans le code actuel.

### FA-06 - Le fallback de résolution d'images scanne récursivement tout l'arbre et garde un cache global non borné

Priorité : Important  
Catégorie : performance

Preuve actuelle :

- `src/nectar_render/converter/markdown_parser.py:68` déclare `_IMAGE_INDEX_CACHE` comme dictionnaire global sans limite.
- `src/nectar_render/converter/markdown_parser.py:152-159` construit un index par `root.rglob("*")`.
- `src/nectar_render/converter/markdown_parser.py:166-173` ajoute ensuite l'index en cache global, sans borne de taille.
- `src/nectar_render/converter/markdown_parser.py:319-346` déclenche ce fallback lors de la résolution d'images quand les résolutions directes ont échoué.

Localisation :

- `src/nectar_render/converter/markdown_parser.py:68`
- `src/nectar_render/converter/markdown_parser.py:152-181`
- `src/nectar_render/converter/markdown_parser.py:319-346`

Pourquoi c'est un problème :

- Sur un répertoire d'assets volumineux, un seul échec de résolution peut lancer un scan récursif complet.
- Le cache est global, non borné et indexé par racine. Il peut donc grossir avec plusieurs projets ou plusieurs répertoires de test.
- La stratégie actuelle est simple mais coûteuse à l'échelle.

Correction attendue :

- Borner le cache, par exemple via un LRU simple ou un plafond de clés.
- Restreindre `_build_image_index` aux extensions image utiles.
- Ne déclencher le scan global qu'en dernier recours et, si possible, s'arrêter dès que les noms recherchés sont trouvés.

Fonctions / symboles à modifier :

- `_IMAGE_INDEX_CACHE`
- `_build_image_index`
- `_get_image_index`
- `_resolve_image_sources`
- `invalidate_image_index_cache`

Tests à ajouter ou adapter :

- Ajouter un test sur l'invalidation ciblée du cache.
- Ajouter un test sur le chemin fallback pour vérifier qu'il ne s'exécute que lorsque les résolutions directes échouent réellement.

Statut de vérification :

- Confirmé dans le code actuel.

### FA-07 - `install_dependencies.bat` bootstrappe le runtime avec une sentinelle incomplète et installe des dépendances de dev

Priorité : Important  
Catégorie : maintenance

Preuve actuelle :

- `install_dependencies.bat:38` vérifie `markdown`, `pygments`, `bs4`, `weasyprint` et `pytest`.
- `pyproject.toml:28-35` déclare aussi `bleach` et `pypdf` comme dépendances runtime, mais elles ne sont pas vérifiées par cette sentinelle.
- `install_dependencies.bat:43` installe `.[dev]` au lieu d'un runtime minimal.

Localisation :

- `install_dependencies.bat:37-44`
- `pyproject.toml:28-41`

Pourquoi c'est un problème :

- Le script de lancement peut considérer l'environnement "prêt" alors qu'une dépendance runtime utile au produit est absente.
- Il installe aussi `pytest` et `ruff` chez un simple utilisateur final, alors qu'ils ne servent pas à lancer l'application.
- Le coût réseau, le temps d'installation et le bruit de l'environnement augmentent sans bénéfice fonctionnel.

Correction attendue :

- Séparer clairement le bootstrap runtime du bootstrap contribution/dev.
- Vérifier les vraies dépendances runtime du produit : `markdown`, `pygments`, `bs4`, `weasyprint`, `bleach`, `pypdf`.
- Installer `.` pour le runtime, réserver `.[dev]` à la CI et aux contributeurs.

Fonctions / symboles à modifier :

- `install_dependencies.bat` (bloc "Checking Python dependencies")
- `pyproject.toml` si la stratégie d'installation est clarifiée

Tests à ajouter ou adapter :

- Pas de test unitaire prioritaire.
- Ajouter au minimum une vérification manuelle documentée ou un smoke test de script si ce bootstrap doit rester supporté.

Statut de vérification :

- Confirmé dans le code actuel.

### FA-08 - Les installations système et plusieurs probes runtime masquent leur sortie

Priorité : Important  
Catégorie : maintenance

Preuve actuelle :

- `install_dependencies.bat:55` masque la sortie de `winget install --id qpdf.qpdf ... >nul 2>&1`.
- `install_dependencies.bat:62` masque la sortie de `choco install qpdf -y >nul 2>&1`.
- `install_dependencies.bat:81` et `install_dependencies.bat:87` masquent aussi les probes `prepare_weasyprint_environment(); import weasyprint`.
- `install_dependencies.bat:132` masque l'installation `winget install --id MSYS2.MSYS2 ... >nul 2>&1`.
- `install_dependencies.bat:143` masque `pacman -S --noconfirm --needed mingw-w64-ucrt-x86_64-pango >nul 2>&1`.

Localisation :

- `install_dependencies.bat:49-66`
- `install_dependencies.bat:81-87`
- `install_dependencies.bat:123-143`

Pourquoi c'est un problème :

- En cas d'échec, le script reste peu observable et rend le support utilisateur plus difficile.
- Des installations partielles ou des problèmes réseau/droits peuvent être masqués.
- L'expérience de dépannage sous Windows devient inutilement opaque.

Correction attendue :

- Ne plus envoyer systématiquement les sorties vers `nul`.
- Afficher explicitement les erreurs critiques, ou capturer les sorties dans un log visible.
- Garder le script lisible, mais observable en cas d'échec.

Fonctions / symboles à modifier :

- `install_dependencies.bat`
- branches `qpdf`
- branche `:install_weasyprint_runtime`

Tests à ajouter ou adapter :

- Pas de test unitaire prioritaire.
- Prévoir au minimum une vérification manuelle documentée sur un environnement Windows vierge.

Statut de vérification :

- Confirmé dans le code actuel.

### FA-09 - Les écritures JSON d'état et de presets ne sont pas atomiques

Priorité : Important  
Catégorie : structure

Preuve actuelle :

- `src/nectar_render/ui/state_manager.py:292-295` écrit directement dans le fichier final avec `path.write_text(...)`.
- `src/nectar_render/ui/state_manager.py:213` et `src/nectar_render/ui/state_manager.py:254` passent par ce helper pour l'état et les presets.

Localisation :

- `src/nectar_render/ui/state_manager.py:213`
- `src/nectar_render/ui/state_manager.py:254`
- `src/nectar_render/ui/state_manager.py:292-295`

Pourquoi c'est un problème :

- En cas d'interruption, de crash ou d'écriture concurrente mal synchronisée, le fichier JSON peut rester tronqué ou corrompu.
- Le problème touche directement la persistance d'état utilisateur et des presets.

Correction attendue :

- Écrire d'abord dans un fichier temporaire du même répertoire, puis promouvoir atomiquement via `Path.replace(...)`.
- Créer le répertoire cible si nécessaire avant l'écriture.
- Garder le helper central comme point unique.

Fonctions / symboles à modifier :

- `_save_json_file`
- `StateManager.save_last_state`
- `StateManager.save_preset`

Tests à ajouter ou adapter :

- Ajouter un test dédié au helper d'écriture atomique.
- Ajouter au moins un test garantissant que le fichier final est bien remplacé avec un JSON complet.

Statut de vérification :

- Confirmé dans le code actuel.

### FA-10 - Les lectures JSON invalides échouent silencieusement

Priorité : Important  
Catégorie : maintenance

Preuve actuelle :

- `src/nectar_render/ui/state_manager.py:297-305` capture `json.JSONDecodeError`, `OSError` et `UnicodeDecodeError`, puis retourne `{}` sans log ni signal utilisateur.
- `tests/test_state_manager.py:127-137` valide déjà l'absence de crash sur JSON invalide, ce qui est un bon comportement, mais ne vérifie aucun logging.

Localisation :

- `src/nectar_render/ui/state_manager.py:297-305`
- `tests/test_state_manager.py:127-137`

Pourquoi c'est un problème :

- Un état ou un preset corrompu est silencieusement ignoré.
- Le comportement est robuste du point de vue UX, mais très mauvais du point de vue diagnostic : l'utilisateur ou le développeur ne sait pas pourquoi l'état a été "oublié".

Correction attendue :

- Garder le fallback non bloquant vers `{}`.
- Ajouter au minimum un `logger.warning(...)` avec le chemin du fichier et le type d'erreur.
- Réutiliser le `logger` déjà déclaré mais actuellement inutilisé dans ce module.

Fonctions / symboles à modifier :

- `_load_json_file`
- `logger` de `src/nectar_render/ui/state_manager.py`

Tests à ajouter ou adapter :

- Ajouter un test avec `caplog` qui vérifie qu'une lecture JSON invalide journalise un warning tout en gardant le fallback.

Statut de vérification :

- Confirmé dans le code actuel.

### FA-11 - Le modèle de style et les bornes métier sont dupliqués entre plusieurs couches

Priorité : Important  
Catégorie : duplication

Preuve actuelle :

- `src/nectar_render/cli.py:34-79` reconstruit manuellement un `StyleOptions` depuis un dictionnaire d'état.
- `src/nectar_render/ui/app.py:608-678` reconstruit aussi manuellement un `StyleOptions` depuis des variables Tk.
- `src/nectar_render/ui/state_manager.py:30-63` maintient `_INT_RANGES`, `_DOUBLE_RANGES`, `_clamp_int_value` et `_clamp_float_value`.
- `src/nectar_render/converter/html_builder.py:33-45` maintient aussi `_clamp_int` et `_clamp_float`.
- `src/nectar_render/converter/html_builder.py:87-114` répète plusieurs bornes déjà présentes côté UI, par exemple les tailles de headings `8-96`, les line heights `1.0-2.4`, les marges `0-100`, etc.

Localisation :

- `src/nectar_render/cli.py:34-79`
- `src/nectar_render/ui/app.py:608-678`
- `src/nectar_render/ui/state_manager.py:30-83`
- `src/nectar_render/converter/html_builder.py:33-45`
- `src/nectar_render/converter/html_builder.py:87-114`
- `src/nectar_render/config.py:26-70`

Pourquoi c'est un problème :

- Chaque ajout ou renommage de champ impose plusieurs modifications synchronisées.
- Les bornes métier peuvent diverger silencieusement entre l'UI, la persistance, le CLI et le rendu.
- Le coût de maintenance augmente vite et le risque de régression de mapping est réel.

Correction attendue :

- Extraire une source de vérité unique pour :
- la liste des champs de style,
- leur sérialisation/désérialisation,
- les bornes numériques,
- les normalisations d'énums.
- Le refactor doit rester pragmatique : centraliser d'abord les mappings et les ranges, sans refondre tout le modèle applicatif.

Fonctions / symboles à modifier :

- `_state_dict_to_style`
- `_collect_style`
- `_INT_RANGES`
- `_DOUBLE_RANGES`
- `_clamp_int_value`
- `_clamp_float_value`
- `_clamp_int`
- `_clamp_float`
- `StyleOptions`

Tests à ajouter ou adapter :

- Mettre à jour les tests CLI et StateManager pour vérifier la source de vérité partagée.
- Ajouter au moins un test qui garantit qu'un même champ et sa borne sont appliqués identiquement côté UI et côté rendu.

Statut de vérification :

- Confirmé dans le code actuel.

### FA-12 - `NectarRenderApp` reste un monolithe difficile à faire évoluer

Priorité : Mineur  
Catégorie : structure

Preuve actuelle :

- `src/nectar_render/ui/app.py` concentre à la fois l'initialisation des variables Tk, le wiring des services, les panels, les bindings, la collecte du style et une partie des règles métier UI.
- `src/nectar_render/ui/app.py:231-433` contient un `_build_ui()` dense qui orchestre plusieurs panneaux, callbacks et widgets.
- `src/nectar_render/ui/app.py:42-179` combine aussi beaucoup d'assemblage applicatif dans `__init__`.

Localisation :

- `src/nectar_render/ui/app.py:42-179`
- `src/nectar_render/ui/app.py:231-433`
- `src/nectar_render/ui/app.py:608-678`

Pourquoi c'est un problème :

- Le fichier reste difficile à relire, à tester et à modifier sans effet de bord.
- La classe mélange état, composition de widgets, binding et transformation métier.
- Les régressions UI sont plus difficiles à isoler.

Correction attendue :

- Ne pas attaquer ce refactor avant les correctifs FA-01 a FA-11.
- Découper ensuite par responsabilité :
- factory de variables Tk,
- layout/assemblage des panels,
- collecte et normalisation du style,
- wiring des bindings/controllers.
- Garder `NectarRenderApp` comme façade si cela réduit la casse.

Fonctions / symboles à modifier :

- `NectarRenderApp.__init__`
- `NectarRenderApp._build_ui`
- `NectarRenderApp._collect_style`
- éventuellement `NectarRenderApp._configure_bindings`
- éventuellement `NectarRenderApp._refresh_heading_controls_from_markdown`

Tests à ajouter ou adapter :

- Pas de test unitaire prioritaire avant le découpage.
- Après extraction, conserver au minimum les tests de non-régression déjà présents sur les services et mappings.

Statut de vérification :

- Confirmé dans le code actuel.

### FA-13 - Plusieurs résidus de code mort ou de symboles inutilisés restent dans le projet

Priorité : Mineur  
Catégorie : lisibilité

Preuve actuelle :

- `src/nectar_render/cli.py:153-154` contient un no-op explicite :
- `if fmt == "PDF+HTML":`
- `    fmt = "PDF+HTML"`
- `src/nectar_render/ui/controllers.py:56` déclare `self.live_preview_path`, puis `src/nectar_render/ui/controllers.py:99` lui affecte une valeur, sans relecture ailleurs dans le module.
- `src/nectar_render/ui/app.py:40` déclare un `logger` qui n'est jamais utilisé dans le fichier.
- `src/nectar_render/ui/state_manager.py:22` déclare aussi un `logger` non utilisé aujourd'hui.
- `src/nectar_render/ui/panels.py:25` reçoit `root: tk.Tk` dans `BasicSettingsPanel.__init__`, mais ce paramètre n'est pas utilisé dans cette classe.
- `src/nectar_render/converter/markdown_parser.py:112-113` expose `normalize_pagebreak_markers(...)` comme simple wrapper sans logique additionnelle autour de `_replace_pagebreak_markers_outside_fences(...)`.
- `src/nectar_render/utils/markdown.py:46-50` déclare `is_inside_fence(...)`, mais aucune autre référence n'existe dans `src/` ou `tests/`.
- `src/nectar_render/config.py:9` déclare `CODE_THEMES`, alors que la liste réellement utilisée vient de `list_available_styles()` dans `highlight.py` et `state_manager.py`.

Localisation :

- `src/nectar_render/cli.py:153-154`
- `src/nectar_render/ui/controllers.py:56`
- `src/nectar_render/ui/controllers.py:99`
- `src/nectar_render/ui/app.py:40`
- `src/nectar_render/ui/state_manager.py:22`
- `src/nectar_render/ui/panels.py:22-41`
- `src/nectar_render/converter/markdown_parser.py:112-113`
- `src/nectar_render/utils/markdown.py:46-50`
- `src/nectar_render/config.py:9`

Pourquoi c'est un problème :

- Ces éléments créent du bruit et induisent en erreur sur l'intention réelle du code.
- Ils alourdissent les refactors et font perdre du temps pendant la revue.

Correction attendue :

- Supprimer le no-op du CLI.
- Supprimer `live_preview_path` si elle ne sert pas, ou l'utiliser réellement.
- Réutiliser ou supprimer les loggers inutilisés.
- Retirer le paramètre `root` de `BasicSettingsPanel` si aucune réutilisation n'est prévue.
- Fusionner le wrapper `normalize_pagebreak_markers(...)` avec sa vraie implémentation si l'API publique doit rester unique.
- Supprimer `is_inside_fence(...)` si aucune réutilisation réelle n'est prévue.
- Supprimer `CODE_THEMES` si la liste dynamique Pygments reste la seule source de vérité.

Fonctions / symboles à modifier :

- `run_cli`
- `PreviewController.live_preview_path`
- `logger` de `src/nectar_render/ui/app.py`
- `logger` de `src/nectar_render/ui/state_manager.py`
- `BasicSettingsPanel.__init__`
- `normalize_pagebreak_markers`
- `_replace_pagebreak_markers_outside_fences`
- `is_inside_fence`
- `CODE_THEMES`

Tests à ajouter ou adapter :

- Adapter les tests si la signature de `BasicSettingsPanel` change.
- Aucun autre test prioritaire.

Statut de vérification :

- Confirmé dans le code actuel.

### FA-14 - Les exemples sont désynchronisés du produit et un artefact local expose des chemins absolus

Priorité : Mineur  
Catégorie : maintenance

Preuve actuelle :

- `examples/sample.md:68` documente des presets `Academic, Modern, Technical, Minimal, Dark Code`.
- `examples/sample.md:110` montre un JSON avec `"preset": "Modern"`.
- `tests/test_cli.py:72-87` confirme que `Modern` est actuellement rejeté comme preset inconnu par le CLI.
- `examples/output/sample.html:426-428` contient des `src="file:///C:/Users/sammy/Desktop/Projet_Perso/MD-TO-PDF/..."`
- `git status --ignored --short` montre que `examples/output/` est un artefact local ignoré, pas un dossier tracké dans le dépôt.

Localisation :

- `examples/sample.md:68`
- `examples/sample.md:110`
- `tests/test_cli.py:72-87`
- `examples/output/sample.html:426-428` (artefact local ignoré)

Pourquoi c'est un problème :

- La doc d'exemple induit les utilisateurs et les tests manuels en erreur sur les presets réellement disponibles.
- Un export HTML local contenant des chemins absolus peut fuiter un chemin poste si ce fichier est partagé hors dépôt.

Correction attendue :

- Mettre à jour `examples/sample.md` pour n'utiliser que des presets réellement disponibles.
- Si un exemple HTML généré doit être partagé localement, supprimer ou neutraliser les `file:///...` absolus avant diffusion.
- Ne pas présenter `examples/output/` comme un problème de dépôt tracké : c'est un artefact local, à traiter comme tel.

Fonctions / symboles à modifier :

- `examples/sample.md`
- éventuellement le workflow local qui génère `examples/output/sample.html`

Tests à ajouter ou adapter :

- Pas de test unitaire prioritaire.
- Une vérification documentaire ou un test simple de cohérence des presets d'exemple peut être envisagé si le fichier doit rester une référence stable.

Statut de vérification :

- Confirmé dans le code actuel.

### FA-15 - `launch.bat` force `PYTHONPATH`, ce qui masque des problèmes de packaging

Priorité : Mineur  
Catégorie : maintenance

Preuve actuelle :

- `launch.bat:7` force `set "PYTHONPATH=%PROJECT_ROOT%src;%PYTHONPATH%"`.

Localisation :

- `launch.bat:7`

Pourquoi c'est un problème :

- Le chemin de lancement local ne correspond plus exactement au comportement d'un package installé normalement.
- Cela peut masquer des problèmes d'import et rendre le débogage packaging plus confus.

Correction attendue :

- Éviter de muter `PYTHONPATH` par défaut si l'environnement editable install est déjà correctement préparé.
- Si un contournement local reste nécessaire, le rendre explicite et documenté comme mode dev, pas comme chemin standard.

Fonctions / symboles à modifier :

- `launch.bat`

Tests à ajouter ou adapter :

- Pas de test unitaire prioritaire.
- Vérification manuelle recommandée via `python -m nectar_render.main` après installation editable.

Statut de vérification :

- Confirmé dans le code actuel.

### FA-16 - La liste des thèmes Pygments est recalculée trop souvent et trop tôt

Priorité : Mineur  
Catégorie : performance

Preuve actuelle :

- `src/nectar_render/converter/highlight.py:61-79` reconstruit toute la liste via `get_all_styles()` et deux passes `_is_dark_style(...)`.
- `src/nectar_render/ui/state_manager.py:26` appelle `list_available_styles()` à l'import pour construire `_VALID_CODE_THEMES`.
- `src/nectar_render/ui/panels.py:309` rappelle `list_available_styles()` dans `CodePanel`.

Localisation :

- `src/nectar_render/converter/highlight.py:61-79`
- `src/nectar_render/ui/state_manager.py:26`
- `src/nectar_render/ui/panels.py:305-310`

Pourquoi c'est un problème :

- Le calcul est stable, mais il est répété et même déclenché à l'import.
- Le coût reste modéré, mais c'est une dette de perf et de propreté facile à corriger.

Correction attendue :

- Ajouter un cache simple et sûr, par exemple `functools.lru_cache(maxsize=1)` sur `list_available_styles`.
- Réutiliser le résultat partout plutôt que recalculer.

Fonctions / symboles à modifier :

- `list_available_styles`
- `_VALID_CODE_THEMES`
- `CodePanel.__init__`

Tests à ajouter ou adapter :

- Optionnel : un test qui vérifie qu'un second appel ne recalculera pas la liste si le cache est introduit.

Statut de vérification :

- Confirmé dans le code actuel.

### FA-17 - Les presets intégrés sont stockés dans un très gros bloc statique

Priorité : Mineur  
Catégorie : structure

Preuve actuelle :

- `src/nectar_render/ui/presets.py:10-391` contient `BUILTIN_PRESETS` sous forme d'un gros dictionnaire statique.
- `src/nectar_render/ui/presets.py:391` reconstruit `BUILTIN_PRESET_NAMES` à partir de ce blob.

Localisation :

- `src/nectar_render/ui/presets.py:10-391`

Pourquoi c'est un problème :

- Le fichier est lourd à relire et les diffs deviennent peu lisibles.
- Les champs communs entre presets sont difficiles à factoriser ou à valider.
- Ce n'est pas un bug immédiat, mais c'est une dette de maintenance claire.

Correction attendue :

- Marquer ce point comme refactor de maintenance, pas comme urgence.
- Extraire progressivement des structures communes, ou basculer vers un format de données plus simple à maintenir, sans changer le comportement public.

Fonctions / symboles à modifier :

- `BUILTIN_PRESETS`
- `BUILTIN_PRESET_NAMES`

Tests à ajouter ou adapter :

- Réutiliser les tests existants sur les presets pour verrouiller la non-régression.

Statut de vérification :

- Confirmé dans le code actuel.

### FA-18 - La dépendance `bleach` est une dette de moyen terme

Priorité : Mineur  
Catégorie : maintenance

Preuve actuelle :

- `pyproject.toml:33` dépend de `bleach>=6.2`.
- La sanitisation HTML actuelle repose sur `bleach.clean(...)` dans `src/nectar_render/converter/markdown_parser.py:360-366`.
- Le projet Bleach est officiellement déprécié en amont, même s'il continue à recevoir des mises à jour de sécurité, de compatibilité Python et de gros correctifs. Ce n'est donc pas une dépendance "abandonnée", mais ce n'est plus une dépendance d'avenir.

Localisation :

- `pyproject.toml:33`
- `src/nectar_render/converter/markdown_parser.py:359-366`

Pourquoi c'est un problème :

- La dépendance fonctionne encore, mais elle porte une dette de maintenance explicite.
- Si la couche de sanitisation doit évoluer, il faudra éviter de repousser trop longtemps la question de la migration.

Correction attendue :

- Documenter cette dette comme un sujet de moyen terme.
- Ne pas lancer de migration de dépendance dans la prochaine session sauf si la tâche le demande explicitement.
- Corriger d'abord FA-01 et FA-02 avec l'existant.

Fonctions / symboles à modifier :

- Aucun changement obligatoire dans la prochaine session si la tâche reste bornée.

Tests à ajouter ou adapter :

- Aucun pour cette session, sauf si une migration de bibliothèque est explicitement demandée.

Statut de vérification :

- Confirmé dans le code actuel.

### FA-19 - Le repo a encore plusieurs dettes d'outillage et de reproductibilité

Priorité : Mineur  
Catégorie : maintenance

Preuve actuelle :

- Aucun lockfile n'est présent à la racine malgré un projet Python outillé (`pyproject.toml`, venv, CI).
- `pyproject.toml` ne contient pas de section `[tool.ruff]`, donc les règles effectives reposent sur les défauts de l'outil.
- `.github/workflows/ci.yml:27` exclut explicitement les tests marqués `requires_weasyprint`.
- `src/nectar_render/__init__.py:2` porte une version manuelle `__version__ = "0.3.0.dev0"`.
- Aucun fichier de type `.pre-commit-config.yaml` n'est présent dans la racine observée.

Localisation :

- `pyproject.toml`
- `.github/workflows/ci.yml:27`
- `src/nectar_render/__init__.py:1-2`

Pourquoi c'est un problème :

- Sans lockfile, la reproductibilité d'environnement dépend des dernières versions disponibles.
- Sans config Ruff explicite, les décisions de style/lint ne sont pas totalement documentées dans le repo.
- La CI ne couvre pas le chemin WeasyPrint, donc une partie importante du produit n'est pas validée automatiquement.
- La version manuelle ajoute un risque de drift de release.

Correction attendue :

- Ajouter un mécanisme de verrouillage de dépendances.
- Ajouter une section `[tool.ruff]` explicite si l'équipe veut figer les règles.
- Ajouter une job CI ou un smoke test dédié pour le chemin WeasyPrint.
- À moyen terme, envisager une source de vérité unique pour la version.
- Ajouter un hook local automatisé uniquement si cela sert réellement le workflow de l'équipe.

Fonctions / symboles à modifier :

- `pyproject.toml`
- `.github/workflows/ci.yml`
- `src/nectar_render/__init__.py`

Tests à ajouter ou adapter :

- Ajouter une validation CI du chemin WeasyPrint si l'environnement runner le permet.
- Aucun test unitaire direct prioritaire pour les autres sous-points.

Statut de vérification :

- Confirmé dans le code actuel.

### FA-20 - `FontAutocomplete` pollue sa liste interne avec les saisies libres

Priorité : Mineur  
Catégorie : structure

Preuve actuelle :

- `src/nectar_render/ui/widgets.py:325-335` ajoute toute valeur saisie librement à `self._all_values` lors du `FocusOut`.
- `src/nectar_render/ui/panels.py:150-157`, `176-183` et `318-325` utilisent `FontAutocomplete` pour les polices sans passer `allow_custom_values=False`, donc ce comportement s'applique aux champs de polices de l'application.

Localisation :

- `src/nectar_render/ui/widgets.py:325-335`
- `src/nectar_render/ui/panels.py:150-157`
- `src/nectar_render/ui/panels.py:176-183`
- `src/nectar_render/ui/panels.py:318-325`

Pourquoi c'est un problème :

- Une simple faute de frappe dans un champ police ajoute une entrée persistante pour toute la session dans la liste de suggestions.
- L'effet de bord est implicite et rend la liste interne de plus en plus bruitée au fil de l'usage.
- Ce n'est pas du stockage disque, mais c'est bien un comportement UI indésirable et cumulatif.

Correction attendue :

- Ne pas muter `self._all_values` sur simple perte de focus.
- Si les valeurs custom doivent rester possibles, les gérer séparément ou uniquement au moment du commit explicite.
- Garder la liste de base stable pour ne pas polluer les suggestions.

Fonctions / symboles à modifier :

- `FontAutocomplete._on_focus_out`
- `FontAutocomplete._all_values`
- éventuellement introduire un stockage séparé pour les valeurs custom

Tests à ajouter ou adapter :

- Ajouter un test widget/unit qui vérifie qu'une saisie libre ne pollue pas durablement la liste de suggestions après `FocusOut`.
- Ajouter un test qui vérifie que le comportement `allow_custom_values=False` reste inchangé.

Statut de vérification :

- Confirmé dans le code actuel.

## Constats écartés après vérification

Les points ci-dessous ont été relus, mais exclus du plan principal soit parce qu'ils sont faux dans l'état actuel, soit parce qu'ils restent spéculatifs ou trop faibles par rapport au coût de correction.

### CE-01 - `.vscode/settings.json` avec chemins personnels serait un problème du dépôt

- Source principale : `Audit.md`
- Vérification : `.gitignore:20-22` ignore `.vscode/` et `git status --ignored --short` montre `.vscode/` comme artefact ignoré local.
- Conclusion : vrai problème local éventuel, faux problème de dépôt. Non retenu comme problème repo.

### CE-02 - `examples/output/` serait tracké dans Git

- Source principale : audits multiples
- Vérification : `.gitignore:27-29` ignore `examples/output/` et `git status --ignored --short` le marque comme ignoré.
- Conclusion : faux si présenté comme fichier versionné. Retenu seulement comme artefact local trompeur dans FA-14.

### CE-03 - `PREVIEW_DEBOUNCE_MS` serait inutilisé

- Source principale : `DolaAudit.md`
- Vérification : `src/nectar_render/ui/app.py:78` transmet `PREVIEW_DEBOUNCE_MS` à `OptionChangeController`, qui l'utilise dans `src/nectar_render/ui/bindings.py:56-75`.
- Conclusion : faux positif.

### CE-04 - Les tests actuels prouveraient que `nectar_render.main` n'importe pas la GUI

- Source principale : assertion implicite de `tests/test_main.py`
- Vérification : en interpréteur frais, `import nectar_render.main` charge bien `nectar_render.ui`, `nectar_render.ui.app` et `tkinter`.
- Conclusion : faux sentiment de sécurité causé par un isolement incomplet des modules dans les tests. Le vrai problème est retenu dans FA-03.

### CE-05 - `pdf_compression_service.py` contiendrait des `except Exception` silencieux

- Source principale : `GeminiAudit.md`
- Vérification : `src/nectar_render/services/pdf_compression_service.py:73-77`, `149-152`, `171-173` et `198-200` loggent déjà les erreurs via `logger.exception(...)` ou `logger.info(...)`.
- Conclusion : faux si présenté comme "silencieux".

### CE-06 - Il y aurait 17 imports inutilisés dans les tests

- Source principale : `DolaAudit.md`
- Vérification : `.\.venv\Scripts\python.exe -m ruff check .` ne reproduit pas cette affirmation.
- Conclusion : non prouvé, donc écarté.

### CE-07 - Il y aurait 44 erreurs de typage

- Source principale : `DolaAudit.md`
- Vérification : aucun type-checker configuré ni exécuté dans cette consolidation.
- Conclusion : assertion non vérifiable en l'état. Écartée.

### CE-08 - `pdf_path` pourrait être `None` dans `ConversionService`

- Source principale : `DolaAudit.md`
- Vérification : le flux `export_pdf(...) -> compress(...) -> check exists()` ne montre pas de chemin où le code manipule volontairement un `None`.
- Conclusion : non prouvé dans le flux actuel.

### CE-09 - `highlight.py` crasherait sur `rgba(...)` à cause de `match.groups()`

- Source principale : `ClaudeAudit.md`
- Vérification : `src/nectar_render/converter/highlight.py:37-45` n'extrait que les groupes RGB ; l'alpha n'est pas capturé.
- Conclusion : faux positif.

### CE-10 - Les points "DLL hijacking", ACL, TOCTOU et exposition des fichiers temporaires seraient déjà des défauts avérés du projet

- Source principale : `DolaAudit.md`
- Vérification : le code de `src/nectar_render/utils/weasyprint_runtime.py` et `src/nectar_render/services/pdf_compression_service.py` mérite éventuellement une revue de hardening avancée, mais aucun exploit concret ni défaut reproductible n'a été démontré ici.
- Conclusion : trop spéculatif pour entrer dans un audit de corrections immédiates. Écarté du plan principal.

### CE-11 - L'absence de badge de couverture ou de `CHANGELOG.md` devrait passer avant les vrais problèmes de code

- Source principale : `Audit.md`
- Vérification : ces absences sont réelles, mais leur ROI est très faible au regard des problèmes de sécurité, de comportement et de structure déjà confirmés.
- Conclusion : non retenu dans le plan d'action prioritaire.

### CE-12 - `bleach` serait déjà totalement abandonné sans futur correctif de sécurité

- Source principale : nouveau `ClaudeAudit.md`
- Vérification : le point a été revérifié contre l'état upstream. `bleach` est bien déprécié, mais il continue à recevoir des mises à jour de sécurité, de compatibilité Python et de gros correctifs. La formulation "plus aucun correctif de sécurité ne sera publié" est trop forte et inexacte.
- Conclusion : le sujet reste retenu comme dette de moyen terme dans FA-18, mais pas comme urgence de migration immédiate ni comme "librairie morte".

## Ordre de correction recommandé

1. Traiter FA-01 puis FA-02.
   Objectif : sécuriser d'abord le parseur HTML et figer les tests correspondants.

2. Traiter FA-03 immédiatement après.
   Objectif : casser le couplage CLI/GUI, puis ajouter un test d'import frais qui empêche toute régression.

3. Traiter ensuite FA-04 et FA-05 dans le même lot.
   Objectif : corriger la sémantique PDF et éliminer le double travail dans la chaîne d'export.

4. Traiter FA-09 et FA-10.
   Objectif : rendre la persistance robuste sans changer le comportement utilisateur de haut niveau.

5. Traiter FA-06 puis FA-16.
   Objectif : éliminer les coûts de calcul évitables côté parseur et thèmes Pygments.

6. Traiter FA-11.
   Objectif : centraliser le modèle de style et les bornes métier avant que d'autres correctifs ne multiplient encore la duplication.

7. Traiter FA-07, FA-08 et FA-19.
   Objectif : nettoyer le bootstrap, la CI et la reproductibilité une fois les bugs produit corrigés.

8. Terminer par FA-13, FA-14, FA-15, FA-17, FA-20 et, seulement si le budget le permet, FA-12.
   Objectif : réduire la dette de maintenance sans risquer de casser les correctifs critiques déjà passés.

## Prompt prêt pour la prochaine session IA

```text
Tu travailles dans le projet :
C:\Users\sammy\Desktop\Projet_Perso\MD-TO-PDF

Lis d'abord `FinalAudit.md` en entier.

Règles non négociables :
1. `FinalAudit.md` est la seule source de vérité pour l'audit.
2. Tu ne repars pas de `Audit.md`, `ClaudeAudit.md`, `DolaAudit.md`, `GeminiAudit.md` ou `GPT5.4Audit.md` comme source d'autorité.
3. Tu implémentes uniquement les problèmes retenus et vérifiés dans `FinalAudit.md`.
4. Tu n'ajoutes aucune nouvelle fonctionnalité produit.
5. Tu ne touches pas aux changements non liés déjà présents dans le worktree.
6. Pour toute correction comportementale, tu ajoutes ou adaptes les tests.
7. Tu valides à la fin avec `pytest` puis `ruff check .`.

Ordre de travail obligatoire :
1. FA-01
2. FA-02
3. FA-03
4. FA-04
5. FA-05
6. FA-09
7. FA-10
8. FA-06
9. FA-16
10. FA-11
11. FA-07
12. FA-08
13. FA-19
14. FA-13
15. FA-14
16. FA-15
17. FA-17
18. FA-20
19. FA-12 seulement si le budget temps le permet
20. FA-18 seulement comme dette documentée, sans migration de dépendance si la tâche ne le demande pas explicitement

Détails d'implémentation attendus :

FA-01 et FA-02 :
- Fichiers/symboles cibles :
  - `src/nectar_render/config.py`
  - `src/nectar_render/converter/markdown_parser.py`
  - `src/nectar_render/converter/exporter.py`
  - `tests/test_markdown_parser.py`
- Tu actives une sanitisation sûre par défaut.
- Tu retires `file:` et `data:` de `_ALLOWED_HTML_PROTOCOLS`.
- Tu ne cherches pas à préserver `data:image/...` dans cette session.
- Tu gardes un éventuel opt-out uniquement s'il reste explicite et testé.

FA-03 :
- Fichiers/symboles cibles :
  - `src/nectar_render/cli.py`
  - `src/nectar_render/main.py`
  - `src/nectar_render/ui/__init__.py`
  - `tests/test_main.py`
  - éventuellement `tests/test_cli.py`
- Objectif : `import nectar_render.cli` ne doit plus charger `tkinter`.
- Ajoute un test d'import en interpréteur frais. Un subprocess Python ou un test équivalent isolé est acceptable.
- Ne te contente pas de tests qui modifient seulement `sys.modules` partiellement dans le process courant.

FA-04 :
- Fichiers/symboles cibles :
  - `src/nectar_render/services/pdf_compression_service.py`
  - `src/nectar_render/converter/exporter.py`
  - `tests/test_pdf_compression_service.py`
  - éventuellement `tests/test_conversion_service.py`
- `remove_metadata=True` doit fonctionner même si `enabled=False`.
- Le comportement UI ne doit plus mentir sur ce point.
- Clarifie aussi la responsabilité entre compression à la génération WeasyPrint et compression post-traitement `qpdf`.

FA-05 :
- Fichiers/symboles cibles :
  - `src/nectar_render/services/conversion_service.py`
  - `src/nectar_render/converter/exporter.py`
  - `tests/test_conversion_service.py`
- En `PDF+HTML`, le HTML complet doit être construit une seule fois puis réutilisé.
- Tu ne changes pas le rendu attendu, tu élimines seulement le travail en double.

FA-09 et FA-10 :
- Fichiers/symboles cibles :
  - `src/nectar_render/ui/state_manager.py`
  - `tests/test_state_manager.py`
- `_save_json_file` doit devenir atomique.
- `_load_json_file` doit garder le fallback non bloquant, mais journaliser en warning.

FA-06 et FA-16 :
- Fichiers/symboles cibles :
  - `src/nectar_render/converter/markdown_parser.py`
  - `src/nectar_render/converter/highlight.py`
  - `src/nectar_render/ui/state_manager.py`
  - `src/nectar_render/ui/panels.py`
- Borne le cache image.
- Réduis le coût du scan global.
- Cache la liste des styles Pygments.

FA-11 :
- Fichiers/symboles cibles :
  - `src/nectar_render/cli.py`
  - `src/nectar_render/ui/app.py`
  - `src/nectar_render/ui/state_manager.py`
  - `src/nectar_render/converter/html_builder.py`
  - `src/nectar_render/config.py`
- Objectif : une seule source de vérité pour les champs de style, les bornes et les normalisations.
- Fais un refactor local et pragmatique. N'essaie pas de réécrire toute l'architecture UI.

FA-07, FA-08 et FA-19 :
- Fichiers/symboles cibles :
  - `install_dependencies.bat`
  - `launch.bat`
  - `pyproject.toml`
  - `.github/workflows/ci.yml`
  - `src/nectar_render/__init__.py`
- Sépare runtime et dev.
- Ne masque plus les erreurs critiques du script d'installation.
- Ne traite pas l'absence de lockfile ou la dette CI comme des changements de surface produit.

FA-13, FA-14, FA-15, FA-17, FA-20 :
- Nettoie les résidus confirmés seulement.
- Ne supprime pas de symboles "probablement inutiles" sans preuve.
- Pour `examples/output/`, rappelle-toi que c'est un artefact ignoré local, pas un fichier tracké du dépôt.
- Pour `FontAutocomplete`, ne laisse plus `_all_values` être pollué par les simples pertes de focus.

FA-12 :
- Si tu as encore du temps après tout le reste, propose un découpage de `NectarRenderApp` par petites étapes et implémente uniquement un premier pas sans casser l'existant.

FA-18 :
- Ne migre pas `bleach` dans cette session sauf demande explicite.
- Documente seulement la dette si nécessaire.

Validation finale obligatoire :
- `.\.venv\Scripts\python.exe -m pytest`
- `.\.venv\Scripts\python.exe -m ruff check .`

Probes de validation à faire ou à transformer en tests :
- Vérifier que `parse_markdown(... sanitize_html=True)` ne laisse plus passer `file:` ni `data:`.
- Vérifier qu'un import frais de `nectar_render.cli` ne charge plus `tkinter`.
- Vérifier que `CompressionOptions(enabled=False, remove_metadata=True)` n'est plus un no-op.
- Vérifier qu'en `PDF+HTML`, le HTML n'est plus généré deux fois.

Format attendu de ta réponse finale :
- Résume ce que tu as réellement modifié.
- Liste les tests exécutés.
- Signale explicitement ce que tu n'as pas pu faire.
```

## Checklist de validation finale

- Relancer `.\.venv\Scripts\python.exe -m pytest`
- Relancer `.\.venv\Scripts\python.exe -m ruff check .`
- Vérifier en interpréteur frais que `import nectar_render.cli` ne charge plus `tkinter`
- Vérifier en interpréteur frais que `import nectar_render.main` ne charge plus la GUI tant que le chemin CLI n'est pas utilisé
- Vérifier que `parse_markdown(... sanitize_html=True)` neutralise désormais `file:` et `data:`
- Vérifier qu'un test couvre le nouveau comportement par défaut de sanitisation
- Vérifier qu'un test couvre `CompressionOptions(enabled=False, remove_metadata=True)`
- Vérifier qu'un test couvre le chemin `PDF+HTML` sans double génération HTML
- Vérifier que les écritures JSON passent par un helper atomique
- Vérifier que les lectures JSON invalides journalisent un warning sans casser l'application
