import { createGoogleFontsApi } from '../services/google-fonts-api.js';

export const FONT_SEARCH_TRIGGER_VALUE = '__search_google_fonts__';

const FONT_SEARCH_TRIGGER_LABEL = 'Rechercher plus de polices Google...';
const PAGE_SIZE = 40;
const INPUT_DEBOUNCE_MS = 220;

const CATEGORIES = [
    { key: 'all', label: 'Toutes' },
    { key: 'sans-serif', label: 'Sans-Serif' },
    { key: 'serif', label: 'Serif' },
    { key: 'monospace', label: 'Monospace' },
    { key: 'display', label: 'Display' },
];

const DEFAULT_FONT_SEEDS = {
    bodyFont: [
        'Inter',
        'Roboto',
        'Open Sans',
        'Lato',
        'Montserrat',
        'Poppins',
        'Nunito',
        'Raleway',
        'Work Sans',
        'DM Sans',
        'Manrope',
        'Noto Sans',
        'PT Sans',
        'Source Sans Pro',
        'Merriweather',
        'Lora',
        'Libre Baskerville',
        'Playfair Display',
        'Bitter',
        'Noto Serif',
    ],
    headingFont: [
        'Inter',
        'Roboto',
        'Open Sans',
        'Lato',
        'Montserrat',
        'Poppins',
        'Nunito',
        'Raleway',
        'Work Sans',
        'DM Sans',
        'Manrope',
        'Noto Sans',
        'PT Sans',
        'Source Sans Pro',
        'Merriweather',
        'Lora',
        'Libre Baskerville',
        'Playfair Display',
        'Bitter',
        'Noto Serif',
    ],
    codeFont: [
        'Fira Code',
        'JetBrains Mono',
        'Source Code Pro',
        'Roboto Mono',
        'IBM Plex Mono',
        'Inconsolata',
        'Fira Mono',
        'Ubuntu Mono',
        'Anonymous Pro',
        'Cousine',
        'PT Mono',
        'Space Mono',
        'Red Hat Mono',
        'Noto Sans Mono',
        'Overpass Mono',
        'Share Tech Mono',
    ],
};

const CATEGORY_SEEDS = {
    'sans-serif': ['Inter', 'Roboto', 'Open Sans', 'Lato', 'Montserrat', 'Poppins', 'Nunito', 'Raleway', 'Work Sans', 'DM Sans', 'Manrope', 'Noto Sans', 'PT Sans', 'Source Sans Pro'],
    'serif': ['Merriweather', 'Lora', 'Libre Baskerville', 'Playfair Display', 'Bitter', 'Noto Serif', 'EB Garamond', 'Crimson Text', 'Spectral', 'Cormorant Garamond'],
    'monospace': ['Fira Code', 'JetBrains Mono', 'Source Code Pro', 'Roboto Mono', 'IBM Plex Mono', 'Inconsolata', 'Fira Mono', 'Ubuntu Mono', 'Space Mono', 'Red Hat Mono'],
    'display': ['Abril Fatface', 'Righteous', 'Pacifico', 'Lobster', 'Dancing Script', 'Bebas Neue', 'Permanent Marker', 'Oswald', 'Bungee', 'Fredoka One'],
};

function normalizeToken(value) {
    return String(value || '')
        .toLowerCase()
        .replace(/[^a-z0-9]/g, '');
}

function mergeUnique(items) {
    const seen = new Set();
    const merged = [];
    for (const item of items) {
        const value = String(item || '').trim();
        if (!value || seen.has(value)) {
            continue;
        }
        seen.add(value);
        merged.push(value);
    }
    return merged;
}

function isValidFontName(value) {
    return /^[A-Za-z0-9 ._+'-]{1,80}$/.test(value);
}

function escapeHtml(value) {
    return String(value || '')
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#39;');
}

function rankFonts(fontNames, query) {
    const searchToken = normalizeToken(query);
    if (!searchToken) {
        return [...fontNames];
    }

    const startsWith = [];
    const contains = [];

    for (const family of fontNames) {
        const token = normalizeToken(family);
        if (token.startsWith(searchToken)) {
            startsWith.push(family);
        } else if (token.includes(searchToken)) {
            contains.push(family);
        }
    }

    return [...startsWith, ...contains];
}

function setStatus(state, text, isError = false) {
    if (!state.status) {
        return;
    }
    state.status.innerHTML = escapeHtml(text);
    state.status.classList.toggle('is-error', isError);
}

function updateLoadMoreButton(state) {
    if (!state.loadMoreBtn) {
        return;
    }

    if (state.loading) {
        state.loadMoreBtn.disabled = true;
        state.loadMoreBtn.textContent = 'Chargement...';
        return;
    }

    if (!state.hasMore && state.nextOffset > 0) {
        state.loadMoreBtn.disabled = true;
        state.loadMoreBtn.textContent = 'Tout est chargé';
        return;
    }

    state.loadMoreBtn.disabled = false;
    state.loadMoreBtn.textContent = state.nextOffset > 0 ? 'Charger plus' : 'Explorer Google Fonts';
}

function getCategorySeedFonts(category) {
    if (!category || category === 'all') {
        return Object.values(CATEGORY_SEEDS).flat();
    }
    return CATEGORY_SEEDS[category] || [];
}

function filterByCategory(fontNames, category) {
    if (!category || category === 'all') {
        return fontNames;
    }
    const seeds = new Set(getCategorySeedFonts(category));
    return fontNames.filter((name) => seeds.has(name));
}

function getCombinedResults(state) {
    const query = state.query;
    let localFonts = state.localFonts;
    let remoteFonts = state.remoteFonts;

    if (state.activeCategory && state.activeCategory !== 'all') {
        const catSeeds = new Set(getCategorySeedFonts(state.activeCategory));
        localFonts = localFonts.filter((f) => catSeeds.has(f));
        remoteFonts = remoteFonts.filter((f) => catSeeds.has(f));
    }

    const rankedLocal = rankFonts(localFonts, query);
    const rankedRemote = rankFonts(remoteFonts, query);
    return mergeUnique([...rankedLocal, ...rankedRemote]);
}

function renderResults(state) {
    if (!state.results) {
        return;
    }

    const combined = getCombinedResults(state);
    state.results.innerHTML = '';

    if (combined.length === 0) {
        const empty = document.createElement('div');
        empty.className = 'font-search-empty';
        empty.textContent = 'Aucune police trouvée pour cette recherche.';
        state.results.appendChild(empty);
    } else {
        for (const family of combined) {
            const button = document.createElement('button');
            button.type = 'button';
            button.className = 'font-search-option';
            if (family === state.lastCommittedValue) {
                button.classList.add('is-selected');
            }
            button.dataset.fontFamily = family;

            const name = document.createElement('span');
            name.className = 'font-search-option-name';
            name.textContent = family;

            const preview = document.createElement('span');
            preview.className = 'font-search-option-preview';
            preview.textContent = 'Aa Bb Cc';
            preview.style.fontFamily = `"${family}", ${state.previewFallback}`;

            button.appendChild(name);
            button.appendChild(preview);
            state.results.appendChild(button);
        }
    }

    if (state.loading) {
        setStatus(state, 'Recherche en cours sur Google Fonts...');
    } else if (state.remoteError) {
        setStatus(state, state.remoteError, true);
    } else if (state.query && combined.length === 0) {
        setStatus(state, `Aucun résultat pour "${state.query}".`);
    } else if (state.query) {
        setStatus(state, `${combined.length} police(s) correspondante(s).`);
    } else {
        setStatus(state, 'Polices populaires. Utilise la recherche ou "Explorer Google Fonts".');
    }

    updateLoadMoreButton(state);
}

function ensureTriggerOption(select) {
    const existing = Array.from(select.options).find(
        (option) => option.value === FONT_SEARCH_TRIGGER_VALUE,
    );
    if (existing) {
        return;
    }

    const option = document.createElement('option');
    option.value = FONT_SEARCH_TRIGGER_VALUE;
    option.textContent = FONT_SEARCH_TRIGGER_LABEL;
    select.appendChild(option);
}

function getGoogleOptgroup(select) {
    const groups = Array.from(select.querySelectorAll('optgroup'));
    const existing = groups.find((group) => /google/i.test(group.label));
    if (existing) {
        return existing;
    }

    const created = document.createElement('optgroup');
    created.label = 'Google Fonts';

    const trigger = Array.from(select.options).find(
        (option) => option.value === FONT_SEARCH_TRIGGER_VALUE,
    );
    if (trigger) {
        select.insertBefore(created, trigger);
    } else {
        select.appendChild(created);
    }
    return created;
}

function collectSeedFonts(selectId, select) {
    const optgroupFonts = [];
    const groups = Array.from(select.querySelectorAll('optgroup'));
    for (const group of groups) {
        if (!/google/i.test(group.label)) {
            continue;
        }
        for (const option of Array.from(group.querySelectorAll('option'))) {
            if (option.value && option.value !== FONT_SEARCH_TRIGGER_VALUE) {
                optgroupFonts.push(option.value);
            }
        }
    }

    const defaults = DEFAULT_FONT_SEEDS[selectId] || [];
    return mergeUnique([...defaults, ...optgroupFonts]);
}

export function createFontSearchDropdownFeature({ selectIds = [] } = {}) {
    const googleFontsApi = createGoogleFontsApi();
    const states = new Map();
    let activeSelectId = null;
    let modalOverlay = null;
    let modalElement = null;
    let modalTitle = null;
    let modalSearchInput = null;
    let modalCategoryBtns = [];
    let modalResults = null;
    let modalStatus = null;
    let modalLoadMoreBtn = null;
    let modalFooter = null;
    let modalCreated = false;

    const sharedState = {
        localFonts: [],
        remoteFonts: [],
        remoteFontCategories: new Map(),
        query: '',
        queryStamp: 0,
        nextOffset: 0,
        hasMore: true,
        loading: false,
        debounceHandle: 0,
        pendingRequest: null,
        remoteError: '',
        previewFallback: 'sans-serif',
        lastCommittedValue: '',
        results: null,
        status: null,
        loadMoreBtn: null,
        activeCategory: 'all',
    };

    function getState(selectId) {
        return states.get(selectId) || null;
    }

    function createModal() {
        if (modalCreated) {
            return;
        }

        modalOverlay = document.createElement('div');
        modalOverlay.className = 'font-search-overlay';

        modalElement = document.createElement('div');
        modalElement.className = 'font-search-modal';

        const header = document.createElement('div');
        header.className = 'font-search-modal-header';

        modalTitle = document.createElement('h2');
        modalTitle.className = 'font-search-modal-title';
        modalTitle.textContent = 'Rechercher une police Google';

        const closeBtn = document.createElement('button');
        closeBtn.type = 'button';
        closeBtn.className = 'font-search-close-btn';
        closeBtn.textContent = '✕';
        closeBtn.setAttribute('aria-label', 'Fermer');

        header.appendChild(modalTitle);
        header.appendChild(closeBtn);

        const searchRow = document.createElement('div');
        searchRow.className = 'font-search-row';

        modalSearchInput = document.createElement('input');
        modalSearchInput.type = 'search';
        modalSearchInput.className = 'font-search-input';
        modalSearchInput.placeholder = 'Rechercher une police Google (ex: Nu)';
        modalSearchInput.autocomplete = 'off';

        searchRow.appendChild(modalSearchInput);

        const categoriesRow = document.createElement('div');
        categoriesRow.className = 'font-search-categories';

        modalCategoryBtns = [];
        for (const cat of CATEGORIES) {
            const btn = document.createElement('button');
            btn.type = 'button';
            btn.className = 'font-search-category-btn';
            if (cat.key === 'all') {
                btn.classList.add('is-active');
            }
            btn.textContent = cat.label;
            btn.dataset.category = cat.key;
            modalCategoryBtns.push(btn);
            categoriesRow.appendChild(btn);
        }

        modalResults = document.createElement('div');
        modalResults.className = 'font-search-results';

        modalFooter = document.createElement('div');
        modalFooter.className = 'font-search-modal-footer';

        modalStatus = document.createElement('span');
        modalStatus.className = 'font-search-status';

        modalLoadMoreBtn = document.createElement('button');
        modalLoadMoreBtn.type = 'button';
        modalLoadMoreBtn.className = 'btn btn--pebble btn--sm font-search-load-more';
        modalLoadMoreBtn.textContent = 'Explorer Google Fonts';

        modalFooter.appendChild(modalStatus);
        modalFooter.appendChild(modalLoadMoreBtn);

        modalElement.appendChild(header);
        modalElement.appendChild(searchRow);
        modalElement.appendChild(categoriesRow);
        modalElement.appendChild(modalResults);
        modalElement.appendChild(modalFooter);

        modalOverlay.appendChild(modalElement);
        document.body.appendChild(modalOverlay);

        sharedState.results = modalResults;
        sharedState.status = modalStatus;
        sharedState.loadMoreBtn = modalLoadMoreBtn;

        closeBtn.addEventListener('click', () => {
            closeModal();
        });

        modalOverlay.addEventListener('click', (event) => {
            if (event.target === modalOverlay) {
                closeModal();
            }
        });

        modalSearchInput.addEventListener('input', () => {
            sharedState.query = modalSearchInput.value.trim();
            sharedState.queryStamp += 1;
            resetRemoteState(sharedState);
            renderResults(sharedState);
            if (sharedState.query) {
                scheduleRemoteSearch(sharedState);
            }
        });

        modalSearchInput.addEventListener('keydown', (event) => {
            if (event.key === 'Escape') {
                event.preventDefault();
                closeModal();
            }
        });

        for (const btn of modalCategoryBtns) {
            btn.addEventListener('click', () => {
                const category = btn.dataset.category;
                sharedState.activeCategory = category;
                for (const b of modalCategoryBtns) {
                    b.classList.toggle('is-active', b.dataset.category === category);
                }
                resetRemoteState(sharedState);
                sharedState.nextOffset = 0;
                sharedState.hasMore = true;
                renderResults(sharedState);
                fetchRemoteFonts(sharedState, { reset: true });
            });
        }

        modalResults.addEventListener('click', (event) => {
            const target = event.target;
            if (!(target instanceof Element)) {
                return;
            }
            const button = target.closest('button[data-font-family]');
            if (!button) {
                return;
            }
            const family = button.getAttribute('data-font-family') || '';
            if (!family || !isValidFontName(family)) {
                return;
            }
            pickFont(family);
        });

        modalLoadMoreBtn.addEventListener('click', () => {
            fetchRemoteFonts(sharedState, { reset: sharedState.nextOffset === 0 });
        });

        modalCreated = true;
    }

    function openModal(selectId) {
        const state = getState(selectId);
        if (!state) {
            return;
        }

        createModal();

        activeSelectId = selectId;

        const selectLabel = selectId === 'bodyFont' ? 'Police du corps'
            : selectId === 'headingFont' ? 'Police des titres'
            : selectId === 'codeFont' ? 'Police du code'
            : 'Police';

        modalTitle.textContent = selectLabel;

        sharedState.localFonts = [...state.localFonts];
        sharedState.previewFallback = selectId === 'codeFont' ? 'monospace' : 'sans-serif';
        sharedState.lastCommittedValue = state.lastCommittedValue;
        sharedState.activeCategory = selectId === 'codeFont' ? 'monospace' : 'all';

        for (const btn of modalCategoryBtns) {
            btn.classList.toggle('is-active', btn.dataset.category === sharedState.activeCategory);
        }

        resetRemoteState(sharedState);
        sharedState.query = '';
        modalSearchInput.value = '';
        renderResults(sharedState);

        modalOverlay.classList.remove('hidden');
        modalOverlay.style.display = '';
        modalSearchInput.focus();

        fetchRemoteFonts(sharedState, { reset: true });
    }

    function closeModal() {
        if (activeSelectId) {
            const state = getState(activeSelectId);
            if (state) {
                state.lastCommittedValue = sharedState.lastCommittedValue;
            }
        }
        if (modalOverlay) {
            modalOverlay.classList.add('hidden');
        }
        activeSelectId = null;

        if (sharedState.debounceHandle) {
            window.clearTimeout(sharedState.debounceHandle);
            sharedState.debounceHandle = 0;
        }
    }

    function closeAllPanels() {
        closeModal();
    }

    function resetRemoteState(state) {
        state.remoteFonts = [];
        state.nextOffset = 0;
        state.hasMore = true;
        state.remoteError = '';
    }

    async function fetchRemoteFonts(state, { reset = false, queryStamp = null } = {}) {
        const effectiveStamp = queryStamp ?? state.queryStamp;

        if (state.loading) {
            state.pendingRequest = { reset, queryStamp: effectiveStamp };
            return;
        }

        if (!state.hasMore && !reset) {
            return;
        }

        const query = state.query;
        const offset = reset ? 0 : state.nextOffset;
        const category = state.activeCategory || 'all';

        state.loading = true;
        updateLoadMoreButton(state);
        renderResults(state);

        try {
            const payload = await googleFontsApi.searchFonts({
                query,
                offset,
                limit: PAGE_SIZE,
                category: category === 'all' ? 'all' : category,
            });

            if (effectiveStamp !== state.queryStamp || query !== state.query || category !== (state.activeCategory || 'all')) {
                return;
            }

            const incoming = Array.isArray(payload.items)
                ? payload.items
                    .map((item) => String(item?.family || '').trim())
                    .filter((family) => family && isValidFontName(family))
                : [];

            if (reset) {
                state.remoteFonts = incoming;
            } else {
                state.remoteFonts = mergeUnique([...state.remoteFonts, ...incoming]);
            }

            const safeOffset = Number.parseInt(payload.offset, 10);
            const safeLimit = Number.parseInt(payload.limit, 10);
            state.nextOffset = Number.isFinite(safeOffset) && Number.isFinite(safeLimit)
                ? Math.max(0, safeOffset + safeLimit)
                : state.nextOffset + incoming.length;
            state.hasMore = Boolean(payload.has_more);
            state.remoteError = '';
        } catch {
            if (effectiveStamp !== state.queryStamp || query !== state.query) {
                return;
            }
            state.remoteError = 'Impossible de récupérer la liste Google Fonts pour le moment.';
            setStatus(state, state.remoteError, true);
        } finally {
            const mismatch = effectiveStamp !== state.queryStamp || query !== state.query || category !== (state.activeCategory || 'all');
            state.loading = false;
            if (!mismatch) {
                renderResults(state);
            }

            const pending = state.pendingRequest;
            state.pendingRequest = null;
            if (pending) {
                fetchRemoteFonts(state, pending);
                return;
            }
            if (mismatch) {
                renderResults(state);
            }
        }
    }

    function scheduleRemoteSearch(state) {
        if (state.debounceHandle) {
            window.clearTimeout(state.debounceHandle);
        }

        const stamp = state.queryStamp;
        state.debounceHandle = window.setTimeout(() => {
            fetchRemoteFonts(state, { reset: true, queryStamp: stamp });
        }, INPUT_DEBOUNCE_MS);
    }

    function ensureOption(selectId, fontName) {
        const state = getState(selectId);
        const select = state?.select || document.getElementById(selectId);
        const normalizedFontName = String(fontName || '').trim();
        if (!select || !normalizedFontName || !isValidFontName(normalizedFontName)) {
            return;
        }

        const alreadyExists = Array.from(select.options).some(
            (option) => option.value === normalizedFontName,
        );
        if (!alreadyExists) {
            const option = document.createElement('option');
            option.value = normalizedFontName;
            option.textContent = normalizedFontName;
            const googleGroup = getGoogleOptgroup(select);
            googleGroup.appendChild(option);
        }

        if (state) {
            state.localFonts = mergeUnique([normalizedFontName, ...state.localFonts]);
            state.remoteFonts = mergeUnique([normalizedFontName, ...state.remoteFonts]);
            renderResults(sharedState);
        }
    }

    function syncFromSelect(selectId) {
        const state = getState(selectId);
        if (!state) {
            return;
        }
        const current = String(state.select.value || '');
        if (current && current !== FONT_SEARCH_TRIGGER_VALUE) {
            state.lastCommittedValue = current;
        }
        if (activeSelectId === selectId) {
            sharedState.lastCommittedValue = state.lastCommittedValue;
            renderResults(sharedState);
        }
    }

    function pickFont(family) {
        if (!activeSelectId) {
            return;
        }
        const state = getState(activeSelectId);
        if (!state) {
            return;
        }
        ensureOption(activeSelectId, family);
        state.select.value = family;
        state.lastCommittedValue = family;
        sharedState.lastCommittedValue = family;
        closeModal();
        state.select.dispatchEvent(new Event('change', { bubbles: true }));
    }

    function setupState(selectId) {
        const select = document.getElementById(selectId);
        if (!select) {
            return;
        }

        ensureTriggerOption(select);

        const state = {
            selectId,
            select,
            localFonts: collectSeedFonts(selectId, select),
            remoteFonts: [],
            lastCommittedValue: select.value,
            previewFallback: selectId === 'codeFont' ? 'monospace' : 'sans-serif',
            destroy: null,
        };

        if (state.lastCommittedValue === FONT_SEARCH_TRIGGER_VALUE) {
            state.lastCommittedValue = '';
        }

        states.set(selectId, state);

        const handleSelectChange = (event) => {
            if (select.value === FONT_SEARCH_TRIGGER_VALUE) {
                event.preventDefault();
                event.stopImmediatePropagation();
                select.value = state.lastCommittedValue;
                openModal(selectId);
                return;
            }
            state.lastCommittedValue = select.value;
        };
        select.addEventListener('change', handleSelectChange);

        state.destroy = () => {
            select.removeEventListener('change', handleSelectChange);
        };
    }

    function teardown() {
        for (const state of states.values()) {
            if (typeof state.destroy === 'function') {
                state.destroy();
            }
        }
        states.clear();
        activeSelectId = null;

        if (sharedState.debounceHandle) {
            window.clearTimeout(sharedState.debounceHandle);
            sharedState.debounceHandle = 0;
        }

        if (modalOverlay && modalOverlay.parentNode) {
            modalOverlay.remove();
        }
        modalOverlay = null;
        modalElement = null;
        modalTitle = null;
        modalSearchInput = null;
        modalCategoryBtns = [];
        modalResults = null;
        modalStatus = null;
        modalLoadMoreBtn = null;
        modalFooter = null;
        modalCreated = false;
    }

    function init() {
        teardown();
        selectIds.forEach((selectId) => setupState(selectId));
    }

    return {
        init,
        ensureOption,
        syncFromSelect,
        closeAllPanels,
        teardown,
    };
}