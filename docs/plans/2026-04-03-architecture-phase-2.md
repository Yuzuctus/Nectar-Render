# Nectar Render Phase 2 Implementation Plan

**Goal:** Finaliser la migration vers une architecture canonique `core/application/adapters/interfaces` sans casser le comportement desktop et CLI.

**Architecture:** La stratégie est incrémentale. On sécurise d'abord les seams à risque par des tests ciblés, puis on migre les call sites hors des surfaces legacy avant de supprimer les façades inutiles.

**Tech Stack:** Python 3.10+, pytest, Ruff, Tkinter, WeasyPrint, qpdf/pypdf.

---

### Task 1: Sécuriser l'orchestration desktop

**Files:**
- Create: `C:/Users/sammy/Desktop/Projet_Perso/MD-TO-PDF/tests/test_ui_controllers.py`
- Modify: `C:/Users/sammy/Desktop/Projet_Perso/MD-TO-PDF/src/nectar_render/ui/controllers.py`

**Steps:**
1. Ajouter des tests ciblés sur `PreviewController` et `ConversionController`.
2. Vérifier l'ouverture du navigateur, la résolution du répertoire de sortie, le statut utilisateur et la gestion des erreurs.
3. Couvrir le flux threadé de conversion sans lancer de thread réel.

**Verification:**
- `.\.venv\Scripts\python.exe -m pytest tests/test_ui_controllers.py -q`

### Task 2: Sortir le CLI des shims legacy

**Files:**
- Modify: `C:/Users/sammy/Desktop/Projet_Perso/MD-TO-PDF/src/nectar_render/cli.py`
- Modify: `C:/Users/sammy/Desktop/Projet_Perso/MD-TO-PDF/tests/test_cli.py`
- Modify: `C:/Users/sammy/Desktop/Projet_Perso/MD-TO-PDF/tests/test_presets.py`

**Steps:**
1. Faire consommer au CLI les presets canoniques et les modèles du coeur.
2. Réduire la dépendance à `presets.py`, `style_schema.py` et `services/*` dans le CLI.
3. Garder la compatibilité externe tant que les anciens modules existent.

**Verification:**
- `.\.venv\Scripts\python.exe -m pytest tests/test_cli.py tests/test_presets.py tests/test_main.py -q`

### Task 3: Réduire les façades les plus faibles

**Files:**
- Modify: `C:/Users/sammy/Desktop/Projet_Perso/MD-TO-PDF/src/nectar_render/services/conversion_service.py`
- Modify: `C:/Users/sammy/Desktop/Projet_Perso/MD-TO-PDF/src/nectar_render/services/pdf_compression_service.py`
- Modify: `C:/Users/sammy/Desktop/Projet_Perso/MD-TO-PDF/src/nectar_render/style_schema.py`
- Modify: `C:/Users/sammy/Desktop/Projet_Perso/MD-TO-PDF/src/nectar_render/presets.py`

**Steps:**
1. Ne garder que les façades explicitement utiles à la compatibilité.
2. Aligner leurs docstrings et leurs exports sur leur statut réel de compatibilité.
3. Préparer leur suppression future sans toucher au comportement.

**Verification:**
- `.\.venv\Scripts\python.exe -m pytest tests/test_conversion_service.py tests/test_pdf_compression_service.py tests/test_presets.py -q`

### Task 4: Vérification globale

**Files:**
- Aucun changement fonctionnel

**Steps:**
1. Relancer la suite complète.
2. Relancer Ruff.
3. Résumer les écarts restants pour la phase suivante.

**Verification:**
- `.\.venv\Scripts\python.exe -m pytest -q`
- `.\.venv\Scripts\python.exe -m ruff check src tests`
