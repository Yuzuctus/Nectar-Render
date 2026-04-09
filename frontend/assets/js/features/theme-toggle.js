const STORAGE_KEY = 'nectar-theme';

const MOON_ICON = `
<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" aria-hidden="true">
  <path d="M21 12.79A9 9 0 1 1 11.21 3a7 7 0 0 0 9.79 9.79z"></path>
</svg>`;

const SUN_ICON = `
<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" aria-hidden="true">
  <circle cx="12" cy="12" r="4"></circle>
  <line x1="12" y1="2" x2="12" y2="4"></line>
  <line x1="12" y1="20" x2="12" y2="22"></line>
  <line x1="4.93" y1="4.93" x2="6.34" y2="6.34"></line>
  <line x1="17.66" y1="17.66" x2="19.07" y2="19.07"></line>
  <line x1="2" y1="12" x2="4" y2="12"></line>
  <line x1="20" y1="12" x2="22" y2="12"></line>
  <line x1="4.93" y1="19.07" x2="6.34" y2="17.66"></line>
  <line x1="17.66" y1="6.34" x2="19.07" y2="4.93"></line>
</svg>`;

export function createThemeToggleFeature() {
    const root = document.documentElement;
    let toggles = [];
    const listeners = [];

    function getSystemPreference() {
        return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'night' : 'day';
    }

    function getStoredTheme() {
        try {
            return localStorage.getItem(STORAGE_KEY);
        } catch {
            return null;
        }
    }

    function setStoredTheme(theme) {
        try {
            localStorage.setItem(STORAGE_KEY, theme);
        } catch {
        }
    }

    function updateToggleLabels(isNight) {
        document.querySelectorAll('.theme-toggle').forEach((toggle) => {
            toggle.setAttribute('aria-label', isNight ? 'Passer au mode jour' : 'Passer au mode nuit');
            toggle.dataset.icon = isNight ? 'sun' : 'moon';
            toggle.innerHTML = isNight ? SUN_ICON : MOON_ICON;
        });
    }

    function applyTheme(theme) {
        if (theme === 'night') {
            root.setAttribute('data-theme', 'night');
            updateToggleLabels(true);
            return;
        }
        root.setAttribute('data-theme', 'day');
        updateToggleLabels(false);
    }

    function cycleTheme() {
        const current = root.getAttribute('data-theme') === 'night' ? 'night' : 'day';
        const next = current === 'night' ? 'day' : 'night';
        setStoredTheme(next);
        applyTheme(next);
    }

    function init() {
        const stored = getStoredTheme();
        applyTheme(stored || getSystemPreference());
        toggles = Array.from(document.querySelectorAll('.theme-toggle'));
        toggles.forEach((toggle) => {
            toggle.addEventListener('click', cycleTheme);
            listeners.push(() => toggle.removeEventListener('click', cycleTheme));
        });
    }

    function destroy() {
        listeners.splice(0).forEach((cleanup) => cleanup());
        toggles = [];
    }

    return {
        init,
        applyTheme,
        destroy,
    };
}
