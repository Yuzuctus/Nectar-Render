import { DEFAULTS } from '../config/defaults.js';
import { BUILTIN_PRESETS as BUILTIN_PRESETS_FALLBACK } from '../config/builtin-presets.js';
import {
    FONT_SEARCH_TRIGGER_VALUE,
    createFontSearchDropdownFeature,
} from './font-search-dropdown.js';
import { createPresetsApi } from '../services/presets-api.js';
import { createUserPresetStore } from '../state/preset-store.js';

const STYLE_INPUT_IDS = [
    'bodyFont', 'bodyFontSize', 'bodyTextColor', 'lineHeight',
    'headingFont', 'headingColor',
    'h1Color', 'h2Color', 'h3Color', 'h4Color', 'h5Color', 'h6Color',
    'h1Size', 'h2Size', 'h3Size', 'h4Size', 'h5Size', 'h6Size',
    'codeFont', 'codeFontSize', 'codeLineHeight', 'codeTheme',
    'marginTop', 'marginRight', 'marginBottom', 'marginLeft',
    'footerText', 'footerAlign', 'footerColor', 'footerFontSize',
    'includeFootnotes', 'footnoteFontSize', 'footnoteTextColor', 'footnoteMarkerColor',
    'tableRowStripes', 'tableRowOddColor', 'tableRowEvenColor',
    'tableCellPaddingY', 'tableCellPaddingX', 'borderColor',
    'imageScale', 'sanitizeHtml', 'showHorizontalRules',
    'compressPdf', 'compressionProfile', 'stripMetadata',
    'compressionTimeout', 'keepOriginalOnFail',
];

const HEADING_COLOR_IDS = ['h1Color', 'h2Color', 'h3Color', 'h4Color', 'h5Color', 'h6Color'];

const GOOGLE_FONTS = [
    'Inter', 'Roboto', 'Open Sans', 'Lato', 'Montserrat', 'Poppins', 'Nunito',
    'Raleway', 'Work Sans', 'DM Sans', 'Manrope', 'Noto Sans', 'PT Sans',
    'Source Sans Pro', 'Merriweather', 'Lora', 'Libre Baskerville',
    'Playfair Display', 'Bitter', 'Noto Serif', 'Fira Code', 'JetBrains Mono',
    'Source Code Pro', 'Roboto Mono', 'IBM Plex Mono', 'Inconsolata',
    'Fira Mono', 'Ubuntu Mono',
];

const SYSTEM_FONTS = new Set([
    'Arial', 'Calibri', 'Cambria', 'Consolas', 'Courier New', 'Georgia', 'Helvetica',
    'Lucida Console', 'Menlo', 'Monaco', 'Segoe UI', 'Tahoma', 'Times New Roman',
    'Trebuchet MS', 'Verdana',
]);

const FONT_NAME_RE = /^[A-Za-z0-9 ._+'-]{1,80}$/;
const FONT_KEY_TO_SELECT_ID = {
    body_font: 'bodyFont',
    heading_font: 'headingFont',
    code_font: 'codeFont',
};

const KEY_TO_ID_MAP = {
    body_font: 'bodyFont',
    body_font_size: 'bodyFontSize',
    body_text_color: 'bodyTextColor',
    line_height: 'lineHeight',
    heading_font: 'headingFont',
    heading_color: 'headingColor',
    heading_h1_color: 'h1Color',
    heading_h2_color: 'h2Color',
    heading_h3_color: 'h3Color',
    heading_h4_color: 'h4Color',
    heading_h5_color: 'h5Color',
    heading_h6_color: 'h6Color',
    heading_h1_size_px: 'h1Size',
    heading_h2_size_px: 'h2Size',
    heading_h3_size_px: 'h3Size',
    heading_h4_size_px: 'h4Size',
    heading_h5_size_px: 'h5Size',
    heading_h6_size_px: 'h6Size',
    code_font: 'codeFont',
    code_font_size: 'codeFontSize',
    code_line_height: 'codeLineHeight',
    code_theme: 'codeTheme',
    margin_top_mm: 'marginTop',
    margin_right_mm: 'marginRight',
    margin_bottom_mm: 'marginBottom',
    margin_left_mm: 'marginLeft',
    footer_text: 'footerText',
    footer_align: 'footerAlign',
    footer_color: 'footerColor',
    footer_font_size: 'footerFontSize',
    include_footnotes: 'includeFootnotes',
    footnote_font_size: 'footnoteFontSize',
    footnote_text_color: 'footnoteTextColor',
    footnote_marker_color: 'footnoteMarkerColor',
    table_row_stripes: 'tableRowStripes',
    table_row_odd_color: 'tableRowOddColor',
    table_row_even_color: 'tableRowEvenColor',
    table_cell_padding_y_px: 'tableCellPaddingY',
    table_cell_padding_x_px: 'tableCellPaddingX',
    border_color: 'borderColor',
    image_scale: 'imageScale',
    sanitize_html: 'sanitizeHtml',
    show_horizontal_rules: 'showHorizontalRules',
    compress_pdf: 'compressPdf',
    compression_profile: 'compressionProfile',
    strip_metadata: 'stripMetadata',
    compression_timeout: 'compressionTimeout',
    keep_original_on_fail: 'keepOriginalOnFail',
};

const ID_TO_KEY_MAP = Object.fromEntries(
    Object.entries(KEY_TO_ID_MAP).map(([key, id]) => [id, key]),
);

function keyToId(key) {
    return KEY_TO_ID_MAP[key] || key;
}

function idToKey(id) {
    return ID_TO_KEY_MAP[id] || id;
}

export { keyToId, idToKey, HEADING_COLOR_IDS, STYLE_INPUT_IDS, GOOGLE_FONTS };

export function createPresetsFormFeature({ onChange, backendHealth }) {
    const presetsApi = createPresetsApi({ backendHealth });
    const presetStore = createUserPresetStore();
    const fontSearchDropdown = createFontSearchDropdownFeature({
        selectIds: ['bodyFont', 'headingFont', 'codeFont'],
    });
    const loadedGoogleFonts = new Set();
    let currentPresetValues = null;
    let builtinPresets = {};
    let initialized = false;
    let initializingPromise = null;
    const userModifiedFields = new Set();
    const cleanups = [];
    let panelInputListener = null;
    let panelChangeListener = null;

    function markDirty() {
        if (typeof onChange === 'function') {
            onChange();
        }
    }

    function byId(id) {
        return document.getElementById(id);
    }

    function isGoogleFontName(fontName) {
        const cleaned = String(fontName || '').trim();
        if (!cleaned || !FONT_NAME_RE.test(cleaned)) {
            return false;
        }
        return !SYSTEM_FONTS.has(cleaned);
    }

    function ensureFontOption(selectId, fontName) {
        const cleaned = String(fontName || '').trim();
        if (!cleaned || !FONT_NAME_RE.test(cleaned)) {
            return;
        }
        if (!isGoogleFontName(cleaned)) {
            return;
        }

        const select = byId(selectId);
        if (!select) {
            return;
        }

        const optionExists = Array.from(select.options).some((option) => option.value === cleaned);
        if (!optionExists) {
            const option = document.createElement('option');
            option.value = cleaned;
            option.textContent = cleaned;

            const groups = Array.from(select.querySelectorAll('optgroup'));
            const googleGroup = groups.find((group) => /google/i.test(group.label));
            if (googleGroup) {
                googleGroup.appendChild(option);
            } else {
                select.appendChild(option);
            }
        }

        fontSearchDropdown.ensureOption(selectId, cleaned);
    }

    function ensureFontOptionsFromStyle(styleMapping) {
        if (!styleMapping || typeof styleMapping !== 'object') {
            return;
        }

        Object.entries(styleMapping).forEach(([key, value]) => {
            const selectId = FONT_KEY_TO_SELECT_ID[key];
            if (!selectId || typeof value !== 'string') {
                return;
            }
            ensureFontOption(selectId, value);
        });
    }

    function loadGoogleFont(fontName) {
        const cleaned = String(fontName || '').trim();
        if (!isGoogleFontName(cleaned) || loadedGoogleFonts.has(cleaned)) {
            return;
        }
        loadedGoogleFonts.add(cleaned);
        const link = document.createElement('link');
        link.rel = 'stylesheet';
        const familyParam = encodeURIComponent(cleaned).replace(/%20/g, '+');
        link.href = `https://fonts.googleapis.com/css2?family=${familyParam}:wght@400;500;700&display=swap`;
        document.head.appendChild(link);
    }

    function updateFontPreview(selectId, previewId) {
        const select = byId(selectId);
        const preview = byId(previewId);
        if (!select || !preview) return;

        const fontName = select.value;
        if (fontName === FONT_SEARCH_TRIGGER_VALUE) {
            return;
        }
        if (isGoogleFontName(fontName)) {
            ensureFontOption(selectId, fontName);
        }
        loadGoogleFont(fontName);
        const fallback = selectId === 'codeFont' ? 'monospace' : 'sans-serif';
        preview.style.fontFamily = `"${fontName}", ${fallback}`;
    }

    function updateTableStripeColors() {
        const stripesEnabled = byId('tableRowStripes')?.checked;
        const oddColorGroup = byId('tableRowOddColor')?.closest('.form-group');
        const evenColorGroup = byId('tableRowEvenColor')?.closest('.form-group');
        [oddColorGroup, evenColorGroup].forEach((group) => {
            if (!group) return;
            group.style.opacity = stripesEnabled ? '1' : '0.4';
            group.style.pointerEvents = stripesEnabled ? 'auto' : 'none';
        });
    }

    function updateScaleValue() {
        const input = byId('imageScale');
        const target = byId('imageScaleVal');
        if (!input || !target) return;
        target.textContent = Number.parseFloat(input.value).toFixed(2);
    }

    function resetToDefaults() {
        ensureFontOptionsFromStyle(DEFAULTS);

        Object.entries(DEFAULTS).forEach(([key, value]) => {
            const el = byId(keyToId(key));
            if (!el) return;
            if (typeof value === 'boolean') {
                el.checked = value;
            } else {
                if (el.type === 'color' && value === '') {
                    return;
                }
                el.value = value;
                if (typeof value === 'string' && (key === 'body_font' || key === 'heading_font' || key === 'code_font')) {
                    fontSearchDropdown.syncFromSelect(keyToId(key));
                }
            }
            const hexInput = byId(`${keyToId(key)}Hex`);
            if (hexInput && typeof value === 'string') {
                hexInput.value = value;
            }
        });

        ['h1ColorHex', 'h2ColorHex', 'h3ColorHex', 'h4ColorHex', 'h5ColorHex', 'h6ColorHex'].forEach((id) => {
            const input = byId(id);
            if (input) input.value = '';
        });

        updateTableStripeColors();
        updateScaleValue();
        updateFontPreview('bodyFont', 'bodyFontPreview');
        updateFontPreview('headingFont', 'headingFontPreview');
    }

    function applyPresetValues(presetName, presetStyle) {
        currentPresetValues = { ...presetStyle };
        userModifiedFields.clear();
        resetToDefaults();

        ensureFontOptionsFromStyle(presetStyle);

        Object.entries(presetStyle).forEach(([key, value]) => {
            const el = byId(keyToId(key));
            if (!el) return;
            if (typeof value === 'boolean') {
                el.checked = value;
            } else {
                if (el.type === 'color' && value === '') {
                    return;
                }
                el.value = value;
                if (typeof value === 'string' && (key === 'body_font' || key === 'heading_font' || key === 'code_font')) {
                    fontSearchDropdown.syncFromSelect(keyToId(key));
                }
            }
            const hexInput = byId(`${keyToId(key)}Hex`);
            if (hexInput && typeof value === 'string') {
                hexInput.value = value;
            }
        });

        updateTableStripeColors();
        updateScaleValue();
        updateFontPreview('bodyFont', 'bodyFontPreview');
        updateFontPreview('headingFont', 'headingFontPreview');
        markDirty();
    }

    function refreshPresetSelect() {
        const select = byId('presetSelect');
        if (!select) return;

        const currentValue = select.value;
        select.innerHTML = '<option value="">(default)</option>';

        const builtinGroup = document.createElement('optgroup');
        builtinGroup.label = 'Built-in presets';
        Object.keys(builtinPresets).sort().forEach((name) => {
            const option = document.createElement('option');
            option.value = `builtin:${name}`;
            option.textContent = name;
            builtinGroup.appendChild(option);
        });
        select.appendChild(builtinGroup);

        const userPresets = presetStore.listUserPresets();
        if (userPresets.length > 0) {
            const userGroup = document.createElement('optgroup');
            userGroup.label = 'My presets';
            userPresets.forEach((preset) => {
                const option = document.createElement('option');
                option.value = `user:${String(preset.id || '').replace(/^user:/, '')}`;
                option.textContent = preset.name;
                userGroup.appendChild(option);
            });
            select.appendChild(userGroup);
        }

        if (currentValue) {
            select.value = currentValue;
        }
    }

    function trackFieldModification(fieldId) {
        if (!currentPresetValues) return;
        const isHex = fieldId.endsWith('Hex');
        const baseId = isHex ? fieldId.replace('Hex', '') : fieldId;
        const key = idToKey(baseId);
        const element = byId(isHex && HEADING_COLOR_IDS.includes(baseId) ? fieldId : baseId);
        if (!element) return;

        let currentValue;
        if (element.type === 'checkbox') {
            currentValue = element.checked;
        } else if (element.type === 'number' || element.type === 'range') {
            currentValue = Number.parseFloat(element.value);
        } else {
            currentValue = element.value;
        }

        const presetValue = currentPresetValues[key] ?? DEFAULTS[key];
        if (currentValue !== presetValue) {
            userModifiedFields.add(key);
        } else {
            userModifiedFields.delete(key);
        }
    }

    function collectFormFields() {
        const data = {};
        const hasPreset = Boolean(currentPresetValues && byId('presetSelect')?.value);
        const headingColorHexIds = ['h1ColorHex', 'h2ColorHex', 'h3ColorHex', 'h4ColorHex', 'h5ColorHex', 'h6ColorHex'];
        const headingColorKeys = [
            'heading_h1_color',
            'heading_h2_color',
            'heading_h3_color',
            'heading_h4_color',
            'heading_h5_color',
            'heading_h6_color',
        ];

        STYLE_INPUT_IDS.forEach((id) => {
            const el = byId(id);
            if (!el || HEADING_COLOR_IDS.includes(id)) return;
            if (el.value === FONT_SEARCH_TRIGGER_VALUE) return;

            const key = idToKey(id);
            if (hasPreset && !userModifiedFields.has(key)) {
                return;
            }

            if (el.type === 'checkbox') {
                data[key] = el.checked;
                return;
            }
            if (el.value !== '') {
                data[key] = el.value;
            }
        });

        headingColorHexIds.forEach((hexId, index) => {
            const hexInput = byId(hexId);
            if (!hexInput) return;
            const key = headingColorKeys[index];
            if (hasPreset && !userModifiedFields.has(key)) {
                return;
            }
            data[key] = hexInput.value.trim();
        });

        return data;
    }

    function getSelectedPresetInfo() {
        const select = byId('presetSelect');
        if (!select || !select.value) return { type: null, name: null, id: null };
        const val = select.value;
        if (val.startsWith('builtin:')) {
            return { type: 'builtin', name: val.slice(8), id: val };
        }
        if (val.startsWith('user:')) {
            return { type: 'user', name: null, id: val.slice(5) };
        }
        return { type: null, name: null, id: null };
    }

    function setupColorHexSync() {
        const colorPairs = [
            ['headingColor', 'headingColorHex'],
            ['footerColor', 'footerColorHex'],
            ['footnoteTextColor', 'footnoteTextColorHex'],
            ['footnoteMarkerColor', 'footnoteMarkerColorHex'],
            ['tableRowOddColor', 'tableRowOddColorHex'],
            ['tableRowEvenColor', 'tableRowEvenColorHex'],
            ['bodyTextColor', 'bodyTextColorHex'],
            ['borderColor', 'borderColorHex'],
            ['h1Color', 'h1ColorHex'],
            ['h2Color', 'h2ColorHex'],
            ['h3Color', 'h3ColorHex'],
            ['h4Color', 'h4ColorHex'],
            ['h5Color', 'h5ColorHex'],
            ['h6Color', 'h6ColorHex'],
        ];

        colorPairs.forEach(([colorId, hexId]) => {
            const colorInput = byId(colorId);
            const hexInput = byId(hexId);
            if (!colorInput || !hexInput) return;

            const onColorInput = () => {
                hexInput.value = colorInput.value;
                trackFieldModification(colorId);
                markDirty();
            };
            colorInput.addEventListener('input', onColorInput);
            cleanups.push(() => colorInput.removeEventListener('input', onColorInput));

            const onHexInput = () => {
                const value = hexInput.value.trim();
                if (value === '' && HEADING_COLOR_IDS.includes(colorId)) {
                    trackFieldModification(hexId);
                    markDirty();
                    return;
                }
                if (/^#[0-9A-Fa-f]{6}$/.test(value)) {
                    colorInput.value = value;
                    trackFieldModification(colorId);
                    markDirty();
                }
            };
            hexInput.addEventListener('input', onHexInput);
            cleanups.push(() => hexInput.removeEventListener('input', onHexInput));

            if (!HEADING_COLOR_IDS.includes(colorId)) {
                hexInput.value = colorInput.value;
            }
        });
    }

    function saveCurrentAsUserPreset(name) {
        const fields = collectAllFormFieldsRaw();
        const id = presetStore.saveUserPreset(name, fields);
        if (id) {
            refreshPresetSelect();
            const select = byId('presetSelect');
            if (select) select.value = `user:${String(id).replace(/^user:/, '')}`;
        }
        return id;
    }

    function deleteUserPresetById(id) {
        const result = presetStore.deleteUserPreset(id);
        if (result) {
            const select = byId('presetSelect');
            if (select && (select.value === `user:${id}`)) {
                select.value = '';
                currentPresetValues = null;
                userModifiedFields.clear();
                resetToDefaults();
                markDirty();
            } else {
                refreshPresetSelect();
                if (select) {
                    const presetInfo = getSelectedPresetInfo();
                    if (presetInfo.type === 'user' && presetInfo.id === id) {
                        select.value = '';
                        currentPresetValues = null;
                        userModifiedFields.clear();
                    }
                }
            }
            refreshPresetSelect();
        }
        return result;
    }

    function exportCurrentAsTheme(name) {
        const fields = collectAllFormFieldsRaw();
        return presetStore.exportTheme(name, fields);
    }

    function importThemeFromFile(jsonString) {
        return presetStore.importTheme(jsonString);
    }

    function collectAllFormFieldsRaw() {
        const data = {};
        const headingColorHexIds = ['h1ColorHex', 'h2ColorHex', 'h3ColorHex', 'h4ColorHex', 'h5ColorHex', 'h6ColorHex'];
        const headingColorKeys = [
            'heading_h1_color', 'heading_h2_color', 'heading_h3_color',
            'heading_h4_color', 'heading_h5_color', 'heading_h6_color',
        ];

        STYLE_INPUT_IDS.forEach((id) => {
            const el = byId(id);
            if (!el || HEADING_COLOR_IDS.includes(id)) return;
            if (el.value === FONT_SEARCH_TRIGGER_VALUE) return;
            const key = idToKey(id);
            if (el.type === 'checkbox') {
                data[key] = el.checked;
            } else {
                const raw = el.value;
                const num = Number.parseFloat(raw);
                data[key] = raw !== '' && !isNaN(num) && String(num) === raw ? num : raw;
            }
        });

        headingColorHexIds.forEach((hexId, index) => {
            const hexInput = byId(hexId);
            if (!hexInput) return;
            const value = hexInput.value.trim();
            if (value) {
                data[headingColorKeys[index]] = value;
            }
        });

        return data;
    }

    function bind() {
        const presetSelect = byId('presetSelect');
        const onPresetChange = () => {
            const val = presetSelect.value || '';
            if (!val) {
                currentPresetValues = null;
                userModifiedFields.clear();
                resetToDefaults();
                markDirty();
                document.dispatchEvent(new CustomEvent('nectar:preset-change', {
                    detail: { preset: '', type: null },
                }));
                return;
            }

            let presetStyle = null;
            let presetType = null;

            if (val.startsWith('builtin:')) {
                const name = val.slice(8);
                presetType = 'builtin';
                const presetData = builtinPresets[name];
                if (presetData && presetData.style) {
                    presetStyle = presetData.style;
                }
            } else if (val.startsWith('user:')) {
                const id = val.slice(5);
                presetType = 'user';
                const userPreset = presetStore.getUserPreset(id);
                if (userPreset) {
                    presetStyle = userPreset.style;
                }
            }

            applyPresetValues(val, presetStyle || {});
            document.dispatchEvent(new CustomEvent('nectar:preset-change', {
                detail: { preset: val, type: presetType },
            }));
        };
        presetSelect?.addEventListener('change', onPresetChange);
        if (presetSelect) {
            cleanups.push(() => presetSelect.removeEventListener('change', onPresetChange));
        }

        const imageScale = byId('imageScale');
        const onScaleInput = () => {
            updateScaleValue();
            trackFieldModification('imageScale');
            markDirty();
        };
        imageScale?.addEventListener('input', onScaleInput);
        if (imageScale) {
            cleanups.push(() => imageScale.removeEventListener('input', onScaleInput));
        }

        const stripeToggle = byId('tableRowStripes');
        const onStripeChange = () => {
            updateTableStripeColors();
            trackFieldModification('tableRowStripes');
            markDirty();
        };
        stripeToggle?.addEventListener('change', onStripeChange);
        if (stripeToggle) {
            cleanups.push(() => stripeToggle.removeEventListener('change', onStripeChange));
        }

        const panel = document.querySelector('.main-panel');
        if (panelInputListener || panelChangeListener) {
            if (panel && panelInputListener) {
                panel.removeEventListener('input', panelInputListener);
            }
            if (panel && panelChangeListener) {
                panel.removeEventListener('change', panelChangeListener);
            }
            panelInputListener = null;
            panelChangeListener = null;
        }

        const onPanelInput = (event) => {
            const target = event.target;
            if (!target?.id) return;
            if (STYLE_INPUT_IDS.includes(target.id) || target.classList?.contains('hex-input')) {
                trackFieldModification(target.id);
                markDirty();
            }
        };
        panel?.addEventListener('input', onPanelInput);
        panelInputListener = onPanelInput;

        const onPanelChange = (event) => {
            const target = event.target;
            if (!target?.id) return;
            if (STYLE_INPUT_IDS.includes(target.id)) {
                trackFieldModification(target.id);
                markDirty();
            }
        };
        panel?.addEventListener('change', onPanelChange);
        panelChangeListener = onPanelChange;
    }

    function setupFontPreviews() {
        const pairs = [
            ['bodyFont', 'bodyFontPreview'],
            ['headingFont', 'headingFontPreview'],
        ];
        pairs.forEach(([selectId, previewId]) => {
            const select = byId(selectId);
            if (!select) return;
            const onSelectChange = () => {
                updateFontPreview(selectId, previewId);
                trackFieldModification(selectId);
                markDirty();
            };
            select.addEventListener('change', onSelectChange);
            cleanups.push(() => select.removeEventListener('change', onSelectChange));
            updateFontPreview(selectId, previewId);
        });
    }

    async function init() {
        if (initialized) {
            return;
        }
        if (initializingPromise) {
            await initializingPromise;
            return;
        }

        initializingPromise = (async () => {
            try {
                const fetched = await presetsApi.fetchBuiltinPresets();
                if (fetched && Object.keys(fetched).length > 0) {
                    builtinPresets = fetched;
                } else {
                    builtinPresets = BUILTIN_PRESETS_FALLBACK;
                }
            } catch {
                builtinPresets = BUILTIN_PRESETS_FALLBACK;
            }

            refreshPresetSelect();
            fontSearchDropdown.init();
            setupColorHexSync();
            setupFontPreviews();
            bind();
            resetToDefaults();
            initialized = true;
        })();

        try {
            await initializingPromise;
        } finally {
            initializingPromise = null;
        }
    }

    function destroy() {
        if (!initialized) {
            return;
        }
        fontSearchDropdown.teardown();
        const panel = document.querySelector('.main-panel');
        if (panel && panelInputListener) {
            panel.removeEventListener('input', panelInputListener);
        }
        if (panel && panelChangeListener) {
            panel.removeEventListener('change', panelChangeListener);
        }
        panelInputListener = null;
        panelChangeListener = null;
        cleanups.splice(0).forEach((cleanup) => cleanup());
        initialized = false;
    }

    return {
        init,
        resetToDefaults,
        collectFormFields,
        styleInputIds: STYLE_INPUT_IDS,
        applyPresetValues,
        clearPresetContext: () => {
            currentPresetValues = null;
            userModifiedFields.clear();
        },
        getSelectedPresetInfo,
        saveCurrentAsUserPreset,
        deleteUserPresetById,
        exportCurrentAsTheme,
        importThemeFromFile,
        refreshPresetSelect,
        destroy,
    };
}
