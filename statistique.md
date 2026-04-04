# Statistique Projet

## Resume executif

- Snapshot rafraichi le `2026-04-05`.
- Verification fraiche actuelle :
  - `.\.venv\Scripts\python.exe -m pytest -q` -> `153 passed`
  - `.\.venv\Scripts\python.exe -m ruff check src tests` -> `All checks passed!`
  - `.\.venv\Scripts\python.exe -m ruff format --check src tests` -> `85 files already formatted`
- Smoke utilisateur :
  - export CLI `html` OK
  - export CLI `pdf` OK
  - export CLI `pdf+html` OK
- Point critique corrige et reverifie :
  - le post-traitement PDF ne remplace plus l'original quand `qpdf` ou la suppression de metadonnees produisent un fichier plus gros

## Snapshot de verification

| Commande | Resultat | Notes |
| --- | --- | --- |
| `.\.venv\Scripts\python.exe -m pytest -q` | OK | `153 passed in 2.65s` |
| `.\.venv\Scripts\python.exe -m ruff check src tests` | OK | aucun diagnostic |
| `.\.venv\Scripts\python.exe -m ruff format --check src tests` | OK | `85 files already formatted` |
| `.\.venv\Scripts\python.exe -m nectar_render.main --input examples/sample.md --format html --output <temp>` | OK | `sample.html` genere |
| `.\.venv\Scripts\python.exe -m nectar_render.main --input examples/sample.md --format pdf --output <temp>` | OK | `sample.pdf` genere, `10` pages |
| `.\.venv\Scripts\python.exe -m nectar_render.main --input examples/sample.md --format pdf+html --output <temp>` | OK | `sample.html` + `sample.pdf`, `10` pages |

### Verification specifique PDF

- Verification unitaire ciblee :
  - `.\.venv\Scripts\python.exe -m pytest tests/test_pdf_compression_service.py -q` -> `16 passed`
- Verification comportementale locale :
  - un candidat PDF plus gros est rejete
  - le fichier original est conserve
  - le resultat retourne `applied=False`
- Observation sur `examples/sample.md` :
  - `qpdf` est journalise comme ignore quand il ne reduit pas la taille
  - la suppression de metadonnees est egalement ignoree quand elle grossit le fichier
  - le PDF final conserve la taille originale (`54464` octets dans la verification directe de service)

### Warnings encore observes en smoke PDF

- WeasyPrint ignore `overflow-x: auto`
- WeasyPrint ignore `overflow-y: hidden`
- WeasyPrint journalise `No anchor #fnref-rendering`
- WeasyPrint journalise `No anchor #fnref-styling`

Ces warnings ne bloquent pas l'export, mais restent des points de fidelite PDF a traiter plus tard.

## Empreinte Python actuelle

- Fichiers source Python : `72`
- Fichiers de tests Python : `13`
- Total Python : `8361` lignes

### Repartition par zone

| Zone | Fichiers | Lignes |
| --- | ---: | ---: |
| `interfaces/` | 13 | 2616 |
| `adapters/` | 21 | 1759 |
| `tests/` | 13 | 2293 |
| `core/` | 3 | 556 |
| `application/` | 3 | 300 |
| `utils/` | 5 | 328 |
| `converter/` | 9 | 73 |
| `ui/` | 9 | 82 |
| `services/` | 3 | 70 |
| `<root>` | 6 | 284 |

## Architecture constatee

L'architecture active est maintenant :

- `core/` : modeles canoniques de styles, presets et normalisation
- `application/` : use cases de conversion et preview
- `adapters/` : runtime, stockage, post-traitement PDF
- `adapters/rendering/` : pipeline canonique Markdown -> HTML -> PDF
- `interfaces/desktop/` : application Tkinter et mapping d'etat desktop

Les packages suivants sont des couches de compatibilite :

- `src/nectar_render/ui/`
- `src/nectar_render/converter/`
- `src/nectar_render/services/`
- `src/nectar_render/presets.py`
- `src/nectar_render/style_schema.py`

Verification locale de production :

- aucun import de production restant vers `ui`, `converter` ou `services`

## Hotspots techniques actuels

| Rang | Fichier | Lignes | Commentaire |
| --- | --- | ---: | --- |
| 1 | `src/nectar_render/interfaces/desktop/app.py` | 688 | principal monolithe desktop restant |
| 2 | `tests/test_markdown_parser.py` | 504 | gros verrou de non-regression parser/rendu |
| 3 | `src/nectar_render/interfaces/desktop/panels.py` | 474 | beaucoup d'assemblage UI |
| 4 | `src/nectar_render/adapters/rendering/html_document.py` | 445 | gros bloc de rendu CSS/HTML |
| 5 | `src/nectar_render/interfaces/desktop/widgets.py` | 364 | comportements Tkinter delicats |
| 6 | `tests/test_pdf_compression_service.py` | 324 | forte surface de verrous sur la politique PDF |
| 7 | `src/nectar_render/core/presets.py` | 309 | catalogue de presets dense mais acceptable |
| 8 | `src/nectar_render/interfaces/desktop/state_manager.py` | 255 | persistance, historique et presets encore melanges |

## Risques et points de surveillance

- la migration n'est pas completement terminee tant que les tests et les imports externes legacy existent encore
- `interfaces/desktop/app.py` reste la principale cible de simplification future
- `interfaces/desktop/panels.py` et `interfaces/desktop/state_manager.py` restent plus gros qu'ideal
- les warnings WeasyPrint sur le sample sont encore ouverts

## Conclusion

Le depot est nettement plus propre qu'au debut de l'audit :

- l'architecture cible est en place
- la compatibilite legacy est conservee
- la verification est verte
- le faux gain de compression PDF a ete neutralise

Le prochain cycle de refactor ne doit plus viser la structure globale, mais les hotspots residuels de la couche desktop et la suppression progressive des shims legacy.
