# Frontend Architecture

Le frontend est maintenant structure en modules (sans build step), avec des fichiers ES modules natifs.

## Arborescence

```
frontend/
  index.html
  panel.html
  assets/
    css/
      tokens.css
      base.css
      global/
        theming.css
        motion.css
      components/
        navigation.css
        buttons.css
        surfaces.css
        forms.css
        tabs.css
        upload.css
        preview.css
        dialogs.css
        toast.css
      pages/
        home.css
        panel.css
        result.css
    js/
      config/
        defaults.js
      core/
        api-base.js
      platform/
        dom-utils.js
      state/
        session-store.js
        preset-store.js
      services/
        backend-health.js
        conversion-api.js
        preview-api.js
        presets-api.js
      features/
        conversion-flow.js
        image-assets.js
        landing-upload.js
        loading-overlay.js
        missing-images-dialog.js
        panel-context.js
        presets-form.js
        preview-panel.js
        tabs.js
        theme-toggle.js
        toast.js
      pages/
        home-page.js
        panel-page.js
      bootstrap/
        home-app.js
        panel-app.js
    img/
```

## Principes

- `config/`: valeurs par defaut (les presets built-in sont charges via l'API backend).
- `core/`: logique de base reutilisable, pas de logique metier UI.
- `platform/`: helpers DOM/navigateur.
- `state/`: persistance de session (sessionStorage + IndexedDB fallback) et presets utilisateur (localStorage).
- `services/`: appels HTTP vers le backend uniquement.
- `features/`: logique fonctionnelle UI (upload, preview, conversion, etc.).
- `pages/`: orchestration d'une page complete.
- `bootstrap/`: point d'entree minimal par page.

## Systeme de presets

### Presets built-in
- Charges dynamiquement depuis `GET /presets/builtin` (backend).
- Sources JSON dans `src/nectar_render/core/preset_data/*.json`.
- Unique source de verite: le backend expose les presets, le frontend les recupere.

### Presets utilisateur
- Sauvegardes dans `localStorage` via `state/preset-store.js`.
- Cles prefixees par `user:` (ex: `user:mon-theme`).
- CRUD complet: save, delete, rename, list.

### Export / import de themes
- Export: genere un fichier JSON avec le format `nectar-render-theme` v1.
- Import: valide le format et la version, puis sauvegarde comme preset utilisateur.
- Format de fichier partageable entre utilisateurs.

### Flux de conversion
- Preset built-in: le frontend envoie `preset=<name>` au backend pour beneficier du style de base.
- Preset utilisateur: le frontend envoie tous les champs de style modifies (pas de `preset=`), car le backend ne connait pas les presets utilisateur.

## Flux

1. `index.html` permet de charger un `.md`.
2. Le draft est stocke en session via `state/session-store.js`.
3. Redirection vers `panel.html`.
4. `panel.html` recupere le draft, sinon redirige vers `index.html`.
5. Les presets built-in sont charges depuis l'API au demarrage du panel.
6. Preview et conversion utilisent les endpoints backend (`/analyze/`, `/preview`, `/convert`).

## Notes de maintenance

- Ne pas appeler directement `fetch` depuis une feature: passer par `services/`.
- Ne pas ajouter de logique de page dans `bootstrap/`.
- Garder les IDs de formulaire synchronises avec le mapping de `features/presets-form.js`.
- Pour ajouter un preset built-in: creer un fichier JSON dans `src/nectar_render/core/preset_data/` et le backend l'exposera automatiquement.