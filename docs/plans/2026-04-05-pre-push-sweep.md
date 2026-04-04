# Nectar Render Pre-Push Sweep Implementation Plan

**Goal:** Corriger les derniers ecarts bloquants avant un push global du worktree, en priorite la politique de post-traitement PDF et les incoherences de portee/doc.

**Architecture:** La strategie reste incrementale. On verrouille d'abord le comportement attendu par des tests cibles, puis on corrige le service PDF, ensuite on aligne la documentation et les fichiers racine, et enfin on refait une verification large avant toute operation Git.

**Tech Stack:** Python 3.10+, pytest, Ruff, Tkinter, WeasyPrint, qpdf, pypdf, GitHub Actions.

---

### Task 1: Verrouiller la politique PDF

**Files:**
- Modify: `C:/Users/sammy/Desktop/Projet_Perso/MD-TO-PDF/tests/test_pdf_compression_service.py`
- Modify: `C:/Users/sammy/Desktop/Projet_Perso/MD-TO-PDF/tests/test_cli.py`

**Steps:**
1. Ajouter un test qui prouve qu'un PDF post-traite plus gros n'est plus promu.
2. Ajouter un test qui verrouille l'output CLI sur la taille finale reellement conservee.
3. Executer uniquement les tests PDF/CLI vises.

**Verification:**
- `.\.venv\Scripts\python.exe -m pytest tests/test_pdf_compression_service.py tests/test_cli.py -q`

### Task 2: Corriger le post-traitement PDF

**Files:**
- Modify: `C:/Users/sammy/Desktop/Projet_Perso/MD-TO-PDF/src/nectar_render/adapters/pdf_postprocess.py`

**Steps:**
1. Rendre explicite la regle de promotion du fichier post-traite.
2. Ne jamais remplacer l'original par un PDF plus gros.
3. Conserver des tailles coherentes dans `PdfCompressionResult`.

**Verification:**
- `.\.venv\Scripts\python.exe -m pytest tests/test_pdf_compression_service.py -q`
- `.\.venv\Scripts\python.exe -m nectar_render.main --input examples/sample.md --format pdf+html --output <temp>`

### Task 3: Aligner le worktree pre-push

**Files:**
- Modify: `C:/Users/sammy/Desktop/Projet_Perso/MD-TO-PDF/pyproject.toml`
- Modify: `C:/Users/sammy/Desktop/Projet_Perso/MD-TO-PDF/README.md`
- Modify: `C:/Users/sammy/Desktop/Projet_Perso/MD-TO-PDF/CHANGELOG.md`
- Review: `C:/Users/sammy/Desktop/Projet_Perso/MD-TO-PDF/statistique.md`
- Review: `C:/Users/sammy/Desktop/Projet_Perso/MD-TO-PDF/.github/workflows/ci.yml`
- Review: `C:/Users/sammy/Desktop/Projet_Perso/MD-TO-PDF/requirements-dev.lock`

**Steps:**
1. Corriger les descriptions et docs qui ne collent plus a l'etat reel.
2. Decider explicitement si `statistique.md` et les artefacts d'analyse font partie du push global.
3. Verifier que la CI et le lockfile sont coherents ensemble.

**Verification:**
- `.\.venv\Scripts\python.exe -m ruff check src tests`
- `.\.venv\Scripts\python.exe -m ruff format --check src tests`

### Task 4: Verification finale avant Git

**Files:**
- Aucun changement fonctionnel

**Steps:**
1. Relancer la suite complete.
2. Relancer les smokes CLI HTML et PDF+HTML.
3. Refaire un `git status --short` et resumer le scope reel du commit.

**Verification:**
- `.\.venv\Scripts\python.exe -m pytest -q`
- `.\.venv\Scripts\python.exe -m ruff check src tests`
- `.\.venv\Scripts\python.exe -m ruff format --check src tests`
