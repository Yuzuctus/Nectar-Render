function resolveApiBase() {
    const explicitFromWindow = window.NECTAR_API_BASE;
    if (typeof explicitFromWindow === 'string' && explicitFromWindow.trim() !== '') {
        return explicitFromWindow.trim().replace(/\/+$/, '');
    }

    const explicitFromMeta = document.querySelector('meta[name="nectar-api-base"]')?.content;
    if (typeof explicitFromMeta === 'string' && explicitFromMeta.trim() !== '') {
        return explicitFromMeta.trim().replace(/\/+$/, '');
    }

    const { protocol, hostname, origin } = window.location;
    if ((hostname === 'localhost' || hostname === '127.0.0.1' || hostname === '[::1]') && /^https?:$/.test(protocol)) {
        return `${protocol}//127.0.0.1:8000`;
    }
    if (/^https?:$/.test(protocol) && origin && origin !== 'null') {
        return origin.replace(/\/+$/, '');
    }
    return 'http://127.0.0.1:8000';
}

let API_BASE = resolveApiBase();

const GOOGLE_FONTS = [
    'Inter', 'Lato', 'Merriweather', 'Montserrat', 'Nunito', 'Open Sans',
    'Playfair Display', 'Poppins', 'Raleway', 'Roboto', 'Source Sans Pro',
    'Fira Code', 'Fira Mono', 'IBM Plex Mono', 'Inconsolata', 'JetBrains Mono',
    'Roboto Mono', 'Source Code Pro', 'Ubuntu Mono'
];
const loadedGoogleFonts = new Set();

const PRESETS = {
    Academic: {
        body_font: 'Times New Roman', line_height: 1.8, heading_font: 'Georgia',
        heading_color: '#1a1a2e', heading_h1_size_px: 24, heading_h2_size_px: 20,
        heading_h3_size_px: 16, code_font: 'Courier New', code_font_size: 10,
        code_theme: 'friendly', margin_top_mm: 30, margin_right_mm: 30,
        margin_bottom_mm: 30, margin_left_mm: 30, footer_align: 'center',
        image_scale: 0.8,
    },
    Corporate: {
        body_font: 'Calibri', body_font_size: 11, heading_font: 'Calibri',
        heading_color: '#1e3a5f', heading_h1_color: '#1e3a5f', heading_h2_color: '#2563eb',
        heading_h1_size_px: 26, heading_h2_size_px: 20, heading_h3_size_px: 16,
        code_line_height: 1.4, margin_top_mm: 25, margin_right_mm: 25,
        margin_bottom_mm: 25, margin_left_mm: 25, table_row_stripes: true,
        table_row_even_color: '#f1f5f9', image_scale: 0.85,
    },
    Developer: {
        body_font_size: 11, heading_font: 'Consolas', heading_color: '#0f172a',
        heading_h1_color: '#0ea5e9', heading_h2_color: '#0284c7',
        heading_h1_size_px: 26, heading_h2_size_px: 20, code_theme: 'native',
        margin_top_mm: 20, margin_right_mm: 20, margin_bottom_mm: 20,
        margin_left_mm: 20, table_row_stripes: true, table_row_even_color: '#f0f9ff',
        image_scale: 0.8,
    },
    Technical: {
        body_font_size: 10, line_height: 1.4, heading_font: 'Consolas',
        heading_color: '#0f172a', heading_h1_color: '#059669', code_font_size: 10,
        code_line_height: 1.25, code_theme: 'monokai', margin_top_mm: 18,
        margin_right_mm: 18, margin_bottom_mm: 18, margin_left_mm: 18,
        table_row_stripes: true, image_scale: 0.75,
    },
    Minimal: {
        body_font: 'Arial', body_font_size: 10, line_height: 1.3, heading_font: 'Arial',
        heading_color: '#000000', heading_h1_size_px: 20, heading_h2_size_px: 16,
        code_font: 'Courier New', code_font_size: 9, margin_top_mm: 12,
        margin_right_mm: 12, margin_bottom_mm: 12, margin_left_mm: 12,
        include_footnotes: false, image_scale: 0.65, show_horizontal_rules: false,
    },
    Magazine: {
        body_font: 'Georgia', body_font_size: 11, line_height: 1.6, heading_font: 'Arial',
        heading_color: '#b91c1c', heading_h1_color: '#b91c1c', heading_h1_size_px: 36,
        heading_h2_size_px: 24, code_theme: 'friendly', margin_top_mm: 20,
        margin_right_mm: 22, margin_bottom_mm: 20, margin_left_mm: 22,
        footer_align: 'center', footnote_font_size: 8.5, table_row_stripes: true,
        table_row_even_color: '#fef2f2', image_scale: 0.95,
    },
    Notebook: {
        body_font: 'Georgia', line_height: 1.7, heading_color: '#78350f',
        heading_h1_color: '#78350f', heading_h2_color: '#92400e', heading_h2_size_px: 20,
        code_font: 'Courier New', code_theme: 'xcode', margin_top_mm: 28, footer_align: 'center',
        table_row_stripes: true, image_scale: 0.85,
    },
    Creative: {
        body_font_size: 11, line_height: 1.55, heading_font: 'Georgia',
        heading_color: '#581c87', heading_h1_color: '#7c3aed', heading_h2_color: '#6d28d9',
        heading_h1_size_px: 32, code_theme: 'dracula', margin_top_mm: 22,
        margin_left_mm: 35, table_row_stripes: true,
    },
    Elegant: {
        body_font: 'Georgia', line_height: 1.7, heading_font: 'Times New Roman',
        heading_color: '#18181b', heading_h1_size_px: 30, heading_h2_size_px: 22,
        code_font: 'Courier New', code_font_size: 10, code_theme: 'vim', margin_top_mm: 35,
        margin_right_mm: 30, margin_bottom_mm: 35, margin_left_mm: 30,
        footer_align: 'center', image_scale: 0.85,
    },
};

const DEFAULTS = {
    body_font: 'Segoe UI', body_font_size: 12, line_height: 1.5, heading_font: 'Segoe UI',
    heading_color: '#1f2937', heading_h1_color: '', heading_h2_color: '', heading_h3_color: '',
    heading_h4_color: '', heading_h5_color: '', heading_h6_color: '',
    heading_h1_size_px: 28, heading_h2_size_px: 22, heading_h3_size_px: 18,
    heading_h4_size_px: 16, heading_h5_size_px: 14, heading_h6_size_px: 12,
    code_font: 'Consolas', code_font_size: 11, code_line_height: 1.45, code_theme: 'default',
    margin_top_mm: 25.4, margin_right_mm: 25.4, margin_bottom_mm: 25.4, margin_left_mm: 25.4,
    footer_text: '', footer_align: 'right', footer_color: '#6b7280',
    include_footnotes: true, footnote_font_size: 9, footnote_text_color: '#374151',
    footnote_marker_color: '#111827', table_row_stripes: false,
    table_row_odd_color: '#ffffff', table_row_even_color: '#f3f4f6',
    table_cell_padding_y_px: 6, table_cell_padding_x_px: 8,
    image_scale: 0.9, sanitize_html: true, show_horizontal_rules: true,
    compress_pdf: true, compression_profile: 'balanced', strip_metadata: true,
    compression_timeout: 45, keep_original_on_fail: true,
};

const STYLE_INPUT_IDS = [
    'bodyFont', 'bodyFontSize', 'lineHeight',
    'headingFont', 'headingColor',
    'h1Color', 'h2Color', 'h3Color', 'h4Color', 'h5Color', 'h6Color',
    'h1Size', 'h2Size', 'h3Size', 'h4Size', 'h5Size', 'h6Size',
    'codeFont', 'codeFontSize', 'codeLineHeight', 'codeTheme',
    'marginTop', 'marginRight', 'marginBottom', 'marginLeft',
    'footerText', 'footerAlign', 'footerColor',
    'includeFootnotes', 'footnoteFontSize', 'footnoteTextColor', 'footnoteMarkerColor',
    'tableRowStripes', 'tableRowOddColor', 'tableRowEvenColor',
    'tableCellPaddingY', 'tableCellPaddingX',
    'imageScale', 'sanitizeHtml', 'showHorizontalRules',
    'compressPdf', 'compressionProfile', 'stripMetadata',
    'compressionTimeout', 'keepOriginalOnFail',
];

const HEADING_COLOR_IDS = ['h1Color', 'h2Color', 'h3Color', 'h4Color', 'h5Color', 'h6Color'];

let selectedFile = null;
let selectedMarkdownText = '';
let pdfBlob = null;
let imageFiles = [];
let downloadFilename = 'output.pdf';
let userModifiedFields = new Set();
let currentPresetValues = null;
let previewRevision = 0;
window._lastBlobUrl = null;

const step1 = document.getElementById('step1');
const step2 = document.getElementById('step2');
const step3 = document.getElementById('step3');
const dropZone = document.getElementById('dropZone');
const fileInput = document.getElementById('fileInput');
const nextBtn = document.getElementById('nextBtn');
const backBtn = document.getElementById('backBtn');
const convertBtn = document.getElementById('convertBtn');
const downloadBtn = document.getElementById('downloadBtn');
const startOverBtn = document.getElementById('startOverBtn');
const presetSelect = document.getElementById('presetSelect');
const convertError = document.getElementById('convertError');
const toast = document.getElementById('toast');
const apiStatus = document.getElementById('apiStatus');

const sidebarImageDrop = document.getElementById('sidebarImageDrop');
const imageFilesInput = document.getElementById('imageFilesInput');
const imageFileList = document.getElementById('imageFileList');
const sidebarImagesSection = document.getElementById('sidebarImagesSection');

const missingImagesDialog = document.getElementById('missingImagesDialog');
const missingImagesList = document.getElementById('missingImagesList');
const continueWithoutImages = document.getElementById('continueWithoutImages');
const addMissingImages = document.getElementById('addMissingImages');
const loadingOverlay = document.getElementById('loadingOverlay');
const loadingText = document.getElementById('loadingText');

function showStep(n) {
    const steps = [step1, step2, step3];
    if (n < 1 || n > steps.length) {
        return;
    }
    steps.forEach((s) => {
        s.classList.remove('active');
        s.classList.add('hidden');
    });
    steps[n - 1].classList.remove('hidden');
    steps[n - 1].classList.add('active');
}

function showToast(msg, type = 'info') {
    toast.textContent = msg;
    toast.className = `toast ${type}`;
    toast.classList.remove('hidden');
    clearTimeout(toast._timer);
    toast._timer = setTimeout(() => toast.classList.add('hidden'), 4000);
}

function showLoadingOverlay(format) {
    const formatText = {
        PDF: 'Generation du PDF...',
        HTML: 'Generation du HTML...',
        'PDF+HTML': 'Generation du ZIP...',
    };
    loadingText.textContent = formatText[format] || 'Conversion en cours...';
    loadingOverlay.classList.remove('hidden', 'fade-out');
}

function hideLoadingOverlay(immediate = false) {
    if (immediate) {
        loadingOverlay.classList.add('hidden');
        return;
    }
    loadingOverlay.classList.add('fade-out');
    setTimeout(() => loadingOverlay.classList.add('hidden'), 300);
}

function setupTabs() {
    const tabsNav = document.querySelector('.tabs-nav');
    if (!tabsNav) return;

    tabsNav.addEventListener('click', (e) => {
        const tabBtn = e.target.closest('.tab-btn');
        if (!tabBtn) return;

        const targetTab = tabBtn.dataset.tab;
        document.querySelectorAll('.tab-btn').forEach((btn) => {
            btn.classList.toggle('active', btn.dataset.tab === targetTab);
        });
        document.querySelectorAll('.tab-panel').forEach((panel) => {
            panel.classList.toggle('active', panel.dataset.tab === targetTab);
        });

        document.dispatchEvent(new CustomEvent('nectar:tab-change', {
            detail: { tab: targetTab },
        }));
    });
}

function loadGoogleFont(fontName) {
    if (!GOOGLE_FONTS.includes(fontName) || loadedGoogleFonts.has(fontName)) {
        return;
    }
    loadedGoogleFonts.add(fontName);
    const link = document.createElement('link');
    link.rel = 'stylesheet';
    link.href = `https://fonts.googleapis.com/css2?family=${fontName.replace(/ /g, '+')}:wght@400;500;700&display=swap`;
    document.head.appendChild(link);
}

function updateFontPreview(selectId, previewId) {
    const select = document.getElementById(selectId);
    const preview = document.getElementById(previewId);
    if (!select || !preview) return;

    const fontName = select.value;
    loadGoogleFont(fontName);
    preview.style.fontFamily = `"${fontName}", sans-serif`;
}

function setupFontSelects() {
    const fontSelects = [
        { select: 'bodyFont', preview: 'bodyFontPreview' },
        { select: 'headingFont', preview: 'headingFontPreview' },
    ];

    fontSelects.forEach(({ select, preview }) => {
        const el = document.getElementById(select);
        if (!el) return;
        el.addEventListener('change', () => updateFontPreview(select, preview));
        updateFontPreview(select, preview);
    });
}

function setupColorHex() {
    const colorPairs = [
        { color: 'headingColor', hex: 'headingColorHex' },
        { color: 'footerColor', hex: 'footerColorHex' },
        { color: 'footnoteTextColor', hex: 'footnoteTextColorHex' },
        { color: 'footnoteMarkerColor', hex: 'footnoteMarkerColorHex' },
        { color: 'tableRowOddColor', hex: 'tableRowOddColorHex' },
        { color: 'tableRowEvenColor', hex: 'tableRowEvenColorHex' },
    ];

    const headingColorPairs = [
        { color: 'h1Color', hex: 'h1ColorHex' },
        { color: 'h2Color', hex: 'h2ColorHex' },
        { color: 'h3Color', hex: 'h3ColorHex' },
        { color: 'h4Color', hex: 'h4ColorHex' },
        { color: 'h5Color', hex: 'h5ColorHex' },
        { color: 'h6Color', hex: 'h6ColorHex' },
    ];

    colorPairs.forEach(({ color, hex }) => {
        const colorInput = document.getElementById(color);
        const hexInput = document.getElementById(hex);
        if (!colorInput || !hexInput) return;

        colorInput.addEventListener('input', () => {
            hexInput.value = colorInput.value;
        });

        hexInput.addEventListener('input', () => {
            const val = hexInput.value.trim();
            if (/^#[0-9A-Fa-f]{6}$/.test(val)) {
                colorInput.value = val;
            }
        });

        hexInput.value = colorInput.value;
    });

    headingColorPairs.forEach(({ color, hex }) => {
        const colorInput = document.getElementById(color);
        const hexInput = document.getElementById(hex);
        if (!colorInput || !hexInput) return;

        colorInput.addEventListener('input', () => {
            hexInput.value = colorInput.value;
        });

        hexInput.addEventListener('input', () => {
            const val = hexInput.value.trim();
            if (val === '') return;
            if (/^#[0-9A-Fa-f]{6}$/.test(val)) {
                colorInput.value = val;
            }
        });

        hexInput.value = '';
    });
}

function updateImageDropZone() {
    const mode = document.querySelector('input[name="imageMode"]:checked')?.value;
    if (!sidebarImagesSection) return;
    sidebarImagesSection.style.display = mode === 'WITH_IMAGES' ? 'flex' : 'none';
}

function countWords(text) {
    return text.trim().split(/\s+/).filter((w) => w.length > 0).length;
}

function bumpPreviewRevision() {
    previewRevision += 1;
}

function formatFileSize(bytes) {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function applyPresetValues(presetName) {
    const preset = PRESETS[presetName];
    if (!preset) {
        currentPresetValues = null;
        userModifiedFields.clear();
        resetToDefaults();
        return;
    }

    currentPresetValues = { ...preset };
    userModifiedFields.clear();

    resetToDefaults();
    Object.entries(preset).forEach(([key, val]) => {
        const el = document.getElementById(keyToId(key));
        if (!el) return;
        if (typeof val === 'boolean') {
            el.checked = val;
        } else if (typeof val === 'number') {
            el.value = val;
            if (key === 'image_scale') {
                const scaleVal = document.getElementById('imageScaleVal');
                if (scaleVal) scaleVal.textContent = val.toFixed(2);
            }
        } else {
            el.value = val;
            const hexInput = document.getElementById(`${keyToId(key)}Hex`);
            if (hexInput) hexInput.value = val;
        }
    });
    updateTableStripeColors();
    updateFontPreview('bodyFont', 'bodyFontPreview');
    updateFontPreview('headingFont', 'headingFontPreview');
}

function resetToDefaults() {
    Object.entries(DEFAULTS).forEach(([key, val]) => {
        const el = document.getElementById(keyToId(key));
        if (!el) return;
        if (typeof val === 'boolean') {
            el.checked = val;
        } else if (typeof val === 'number') {
            el.value = val;
            if (key === 'image_scale') {
                const scaleVal = document.getElementById('imageScaleVal');
                if (scaleVal) scaleVal.textContent = val.toFixed(2);
            }
        } else {
            el.value = val;
            const hexInput = document.getElementById(`${keyToId(key)}Hex`);
            if (hexInput) hexInput.value = val;
        }
    });
    updateTableStripeColors();
}

function keyToId(key) {
    const map = {
        body_font: 'bodyFont', body_font_size: 'bodyFontSize', line_height: 'lineHeight',
        heading_font: 'headingFont', heading_color: 'headingColor',
        heading_h1_color: 'h1Color', heading_h2_color: 'h2Color', heading_h3_color: 'h3Color',
        heading_h4_color: 'h4Color', heading_h5_color: 'h5Color', heading_h6_color: 'h6Color',
        heading_h1_size_px: 'h1Size', heading_h2_size_px: 'h2Size', heading_h3_size_px: 'h3Size',
        heading_h4_size_px: 'h4Size', heading_h5_size_px: 'h5Size', heading_h6_size_px: 'h6Size',
        code_font: 'codeFont', code_font_size: 'codeFontSize', code_line_height: 'codeLineHeight',
        code_theme: 'codeTheme',
        margin_top_mm: 'marginTop', margin_right_mm: 'marginRight',
        margin_bottom_mm: 'marginBottom', margin_left_mm: 'marginLeft',
        footer_text: 'footerText', footer_align: 'footerAlign', footer_color: 'footerColor',
        include_footnotes: 'includeFootnotes', footnote_font_size: 'footnoteFontSize',
        footnote_text_color: 'footnoteTextColor', footnote_marker_color: 'footnoteMarkerColor',
        table_row_stripes: 'tableRowStripes', table_row_odd_color: 'tableRowOddColor',
        table_row_even_color: 'tableRowEvenColor',
        table_cell_padding_y_px: 'tableCellPaddingY', table_cell_padding_x_px: 'tableCellPaddingX',
        image_scale: 'imageScale', sanitize_html: 'sanitizeHtml',
        show_horizontal_rules: 'showHorizontalRules',
        compress_pdf: 'compressPdf', compression_profile: 'compressionProfile',
        strip_metadata: 'stripMetadata',
        compression_timeout: 'compressionTimeout', keep_original_on_fail: 'keepOriginalOnFail',
    };
    return map[key] || key;
}

function idToKey(id) {
    const map = {
        bodyFont: 'body_font', bodyFontSize: 'body_font_size', lineHeight: 'line_height',
        headingFont: 'heading_font', headingColor: 'heading_color',
        h1Color: 'heading_h1_color', h2Color: 'heading_h2_color', h3Color: 'heading_h3_color',
        h4Color: 'heading_h4_color', h5Color: 'heading_h5_color', h6Color: 'heading_h6_color',
        h1Size: 'heading_h1_size_px', h2Size: 'heading_h2_size_px', h3Size: 'heading_h3_size_px',
        h4Size: 'heading_h4_size_px', h5Size: 'heading_h5_size_px', h6Size: 'heading_h6_size_px',
        codeFont: 'code_font', codeFontSize: 'code_font_size', codeLineHeight: 'code_line_height',
        codeTheme: 'code_theme',
        marginTop: 'margin_top_mm', marginRight: 'margin_right_mm',
        marginBottom: 'margin_bottom_mm', marginLeft: 'margin_left_mm',
        footerText: 'footer_text', footerAlign: 'footer_align', footerColor: 'footer_color',
        includeFootnotes: 'include_footnotes', footnoteFontSize: 'footnote_font_size',
        footnoteTextColor: 'footnote_text_color', footnoteMarkerColor: 'footnote_marker_color',
        tableRowStripes: 'table_row_stripes', tableRowOddColor: 'table_row_odd_color',
        tableRowEvenColor: 'table_row_even_color',
        tableCellPaddingY: 'table_cell_padding_y_px', tableCellPaddingX: 'table_cell_padding_x_px',
        imageScale: 'image_scale', sanitizeHtml: 'sanitize_html',
        showHorizontalRules: 'show_horizontal_rules',
        compressPdf: 'compress_pdf', compressionProfile: 'compression_profile',
        stripMetadata: 'strip_metadata',
        compressionTimeout: 'compression_timeout', keepOriginalOnFail: 'keep_original_on_fail',
    };
    return map[id] || id;
}

function updateTableStripeColors() {
    const stripesEnabled = document.getElementById('tableRowStripes')?.checked;
    const oddColorGroup = document.getElementById('tableRowOddColor')?.closest('.form-group');
    const evenColorGroup = document.getElementById('tableRowEvenColor')?.closest('.form-group');
    [oddColorGroup, evenColorGroup].forEach((group) => {
        if (!group) return;
        group.style.opacity = stripesEnabled ? '1' : '0.4';
        group.style.pointerEvents = stripesEnabled ? 'auto' : 'none';
    });
}

function trackFieldModification(fieldId) {
    if (!currentPresetValues) return;

    let actualId = fieldId;
    let key;
    if (fieldId.endsWith('Hex')) {
        const colorId = fieldId.replace('Hex', '');
        key = idToKey(colorId);
        actualId = HEADING_COLOR_IDS.includes(colorId) ? fieldId : colorId;
    } else {
        key = idToKey(fieldId);
    }

    const el = document.getElementById(actualId);
    if (!el) return;

    let currentValue;
    if (el.type === 'checkbox') {
        currentValue = el.checked;
    } else if (el.type === 'number' || el.type === 'range') {
        currentValue = parseFloat(el.value);
    } else {
        currentValue = el.value;
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
    const headingColorHexIds = ['h1ColorHex', 'h2ColorHex', 'h3ColorHex', 'h4ColorHex', 'h5ColorHex', 'h6ColorHex'];
    const headingColorKeys = ['heading_h1_color', 'heading_h2_color', 'heading_h3_color', 'heading_h4_color', 'heading_h5_color', 'heading_h6_color'];
    const hasPreset = presetSelect.value && currentPresetValues;

    STYLE_INPUT_IDS.forEach((id) => {
        const el = document.getElementById(id);
        if (!el) return;
        const key = idToKey(id);

        if (HEADING_COLOR_IDS.includes(id)) {
            return;
        }
        if (hasPreset && !userModifiedFields.has(key)) {
            return;
        }
        if (el.type === 'checkbox') {
            data[key] = el.checked ? 'true' : 'false';
            return;
        }
        if (el.value !== '') {
            data[key] = el.value;
        }
    });

    headingColorHexIds.forEach((hexId, index) => {
        const hexInput = document.getElementById(hexId);
        if (!hexInput) return;
        const key = headingColorKeys[index];
        if (hasPreset && !userModifiedFields.has(key)) {
            return;
        }
        data[key] = hexInput.value.trim();
    });

    return data;
}

function renderImageFileList() {
    imageFileList.innerHTML = '';
    imageFiles.forEach((f, i) => {
        const item = document.createElement('div');
        item.className = 'image-file-item';

        const nameSpan = document.createElement('span');
        nameSpan.textContent = f.name;
        nameSpan.title = f.name;

        const removeButton = document.createElement('button');
        removeButton.type = 'button';
        removeButton.setAttribute('aria-label', `Remove ${f.name}`);
        removeButton.textContent = '×';
        removeButton.addEventListener('click', () => {
            imageFiles.splice(i, 1);
            renderImageFileList();
            bumpPreviewRevision();
            document.dispatchEvent(new CustomEvent('nectar:assets-change'));
        });

        item.appendChild(nameSpan);
        item.appendChild(removeButton);
        imageFileList.appendChild(item);
    });
}

function dedupeImageFiles(files) {
    const seen = new Set();
    const deduped = [];
    files.forEach((file) => {
        const key = `${file.name}::${file.size}::${file.lastModified}`;
        if (seen.has(key)) return;
        seen.add(key);
        deduped.push(file);
    });
    return deduped;
}

function isAcceptedImageFile(file) {
    if (!file) return false;
    if (file.type && file.type.startsWith('image/')) return true;
    const name = String(file.name || '').toLowerCase();
    return /\.(apng|avif|bmp|gif|jpe?g|png|svg|tif|tiff|webp)$/.test(name);
}

function processFile(file) {
    selectedFile = file;
    const reader = new FileReader();
    reader.onload = (e) => {
        selectedMarkdownText = e.target.result;
        document.getElementById('fileName').textContent = file.name;
        document.getElementById('fileSize').textContent = formatFileSize(file.size);
        document.getElementById('wordCount').textContent = `${countWords(selectedMarkdownText)} words`;
        document.getElementById('sidebarFileName').textContent = file.name;
        document.getElementById('sidebarFileSize').textContent = formatFileSize(file.size);
        document.getElementById('fileInfo').classList.remove('hidden');
        nextBtn.classList.remove('hidden');
        bumpPreviewRevision();
        document.dispatchEvent(new CustomEvent('nectar:markdown-ready'));
    };
    reader.onerror = () => {
        showToast('Failed to read file', 'error');
        selectedFile = null;
        selectedMarkdownText = '';
        document.getElementById('fileInfo').classList.add('hidden');
        nextBtn.classList.add('hidden');
    };
    reader.onabort = () => {
        showToast('File reading was aborted', 'error');
        selectedFile = null;
        selectedMarkdownText = '';
        document.getElementById('fileInfo').classList.add('hidden');
        nextBtn.classList.add('hidden');
    };
    reader.readAsText(file);
}

function buildFormData(overrideImageMode = null) {
    const formData = new FormData();
    formData.append('markdown_text', selectedMarkdownText);
    formData.append('image_mode', overrideImageMode || document.querySelector('input[name="imageMode"]:checked')?.value || 'WITH_IMAGES');
    formData.append('output_format', document.querySelector('input[name="outputFormat"]:checked')?.value || 'PDF');
    formData.append('page_size', document.getElementById('pageSize')?.value || 'A4');
    formData.append('preset', presetSelect.value || '');

    const fields = collectFormFields();
    Object.entries(fields).forEach(([k, v]) => formData.append(k, v));
    imageFiles.forEach((f) => formData.append('assets', f));
    return formData;
}

async function requestConversion(formData) {
    await resolveWorkingApiBase();
    return fetch(`${API_BASE}/convert`, { method: 'POST', body: formData });
}

function downloadBlob(blob, filename) {
    if (window._lastBlobUrl) {
        URL.revokeObjectURL(window._lastBlobUrl);
    }
    const url = URL.createObjectURL(blob);
    window._lastBlobUrl = url;

    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
}

function extractFilename(response) {
    const contentDisposition = response.headers.get('content-disposition') || '';
    const filenameMatch = contentDisposition.match(/filename="?([^";\n]+)"?/);
    if (filenameMatch && filenameMatch[1]) {
        return filenameMatch[1];
    }
    const contentType = response.headers.get('content-type') || '';
    if (contentType.includes('text/html')) {
        return selectedFile ? selectedFile.name.replace(/\.md$/, '.html') : 'output.html';
    }
    if (contentType.includes('zip')) {
        return selectedFile ? selectedFile.name.replace(/\.md$/, '.zip') : 'output.zip';
    }
    return selectedFile ? selectedFile.name.replace(/\.md$/, '.pdf') : 'output.pdf';
}

function normalizeBackendMessage(message, fallback = 'Operation failed') {
    const text = String(message || '').trim();
    if (!text) return fallback;
    return text;
}

function networkErrorMessage(error) {
    if (error?.name === 'TypeError' && /fetch/i.test(String(error.message || ''))) {
        return `Cannot reach API at ${API_BASE}. Start backend server (uvicorn backend.main:app --reload --host 127.0.0.1 --port 8000) and verify CORS.`;
    }
    return error?.message || 'Network request failed';
}

async function doConvert(overrideImageMode = null) {
    if (!selectedMarkdownText) return;
    convertError.classList.add('hidden');

    const outputFormat = document.querySelector('input[name="outputFormat"]:checked')?.value || 'PDF';
    showLoadingOverlay(outputFormat);
    convertBtn.disabled = true;
    convertBtn.innerHTML = '<span class="btn-spinner"></span> Converting...';
    convertBtn.classList.add('btn-with-spinner');

    try {
        const formData = buildFormData(overrideImageMode);
        const response = await requestConversion(formData);

        if (!response.ok) {
            let errData = null;
            try {
                errData = await response.json();
            } catch {}

            const missingFiles = Array.isArray(errData?.missing_images)
                ? errData.missing_images
                : (errData?.detail?.detail === 'missing_images' && Array.isArray(errData?.detail?.files)
                    ? errData.detail.files
                    : null);

            if (response.status === 422 && missingFiles) {
                missingImagesList.innerHTML = '';
                missingFiles.forEach((file) => {
                    const li = document.createElement('li');
                    li.textContent = file;
                    missingImagesList.appendChild(li);
                });
                hideLoadingOverlay(true);
                convertBtn.disabled = false;
                convertBtn.textContent = 'Convert';
                convertBtn.classList.remove('btn-with-spinner');
                missingImagesDialog.showModal();
                return;
            }

            const msg = normalizeBackendMessage(errData?.detail?.detail || errData?.detail, `Conversion failed (${response.status})`);
            throw new Error(msg);
        }

        const filename = extractFilename(response);
        downloadFilename = filename;
        pdfBlob = await response.blob();

        showToast(`Download ready: ${filename}`, 'success');
        downloadBlob(pdfBlob, filename);
        hideLoadingOverlay(false);
        showStep(3);
    } catch (err) {
        hideLoadingOverlay(true);
        convertError.textContent = networkErrorMessage(err);
        convertError.classList.remove('hidden');
    } finally {
        convertBtn.disabled = false;
        convertBtn.textContent = 'Convert';
        convertBtn.classList.remove('btn-with-spinner');
    }
}

function setupFieldTracking() {
    const mainPanel = document.querySelector('.main-panel');
    if (!mainPanel) return;

    mainPanel.addEventListener('input', (e) => {
        const target = e.target;
        if (target.id && STYLE_INPUT_IDS.includes(target.id)) {
            trackFieldModification(target.id);
            bumpPreviewRevision();
        }
        if (target.classList.contains('hex-input') && target.id) {
            trackFieldModification(target.id);
            bumpPreviewRevision();
        }
    });

    mainPanel.addEventListener('change', (e) => {
        const target = e.target;
        if (target.id && STYLE_INPUT_IDS.includes(target.id)) {
            trackFieldModification(target.id);
            bumpPreviewRevision();
        }
    });
}

function updateApiStatus(kind, text) {
    if (!apiStatus) return;
    apiStatus.classList.remove('is-checking', 'is-online', 'is-offline');
    apiStatus.classList.add(kind);
    const textEl = apiStatus.querySelector('.text');
    if (textEl) textEl.textContent = text;
}

async function probeApiBase(baseUrl) {
    const response = await fetch(`${baseUrl}/analyze/health`, { method: 'GET' });
    return response.ok;
}

async function resolveWorkingApiBase() {
    const preferred = API_BASE;
    const candidates = [preferred];

    if (!candidates.includes('http://127.0.0.1:8000')) {
        candidates.push('http://127.0.0.1:8000');
    }
    if (!candidates.includes('http://localhost:8000')) {
        candidates.push('http://localhost:8000');
    }

    for (const candidate of candidates) {
        try {
            const ok = await probeApiBase(candidate);
            if (ok) {
                API_BASE = candidate;
                if (window.NectarUI) {
                    window.NectarUI.API_BASE = candidate;
                }
                return candidate;
            }
        } catch {
        }
    }
    return preferred;
}

async function checkBackendHealth() {
    updateApiStatus('is-checking', 'Checking backend...');
    try {
        await resolveWorkingApiBase();
        const response = await fetch(`${API_BASE}/analyze/health`, { method: 'GET' });
        if (!response.ok) {
            updateApiStatus('is-offline', `Backend error (${response.status})`);
            return;
        }
        updateApiStatus('is-online', `API online (${API_BASE})`);
    } catch (error) {
        updateApiStatus('is-offline', `API offline (${API_BASE})`);
    }
}

dropZone.addEventListener('dragover', (e) => {
    e.preventDefault();
    dropZone.classList.add('dragover');
});
dropZone.addEventListener('dragleave', () => dropZone.classList.remove('dragover'));
dropZone.addEventListener('drop', (e) => {
    e.preventDefault();
    dropZone.classList.remove('dragover');
    const f = e.dataTransfer.files[0];
    if (f && f.name.toLowerCase().endsWith('.md')) {
        processFile(f);
    } else {
        showToast('Please drop a .md file', 'error');
    }
});
dropZone.addEventListener('click', (e) => {
    const target = e.target;
    if (target instanceof Element && target.closest('.file-btn')) {
        return;
    }
    fileInput.click();
});
dropZone.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        fileInput.click();
    }
});

fileInput.addEventListener('change', () => {
    if (fileInput.files[0]) processFile(fileInput.files[0]);
});

nextBtn.addEventListener('click', () => {
    if (!selectedFile) return;
    convertError.classList.add('hidden');
    showStep(2);
});

backBtn.addEventListener('click', () => {
    convertError.classList.add('hidden');
    showStep(1);
});

presetSelect.addEventListener('change', () => {
    applyPresetValues(presetSelect.value);
    bumpPreviewRevision();
    document.dispatchEvent(new CustomEvent('nectar:preset-change', {
        detail: { preset: presetSelect.value || '' },
    }));
});

step2.addEventListener('change', (e) => {
    if (e.target.name === 'imageMode') {
        updateImageDropZone();
        bumpPreviewRevision();
        document.dispatchEvent(new CustomEvent('nectar:image-mode-change', {
            detail: { imageMode: e.target.value || '' },
        }));
        return;
    }

    if (e.target.name === 'outputFormat' || e.target.id === 'pageSize') {
        bumpPreviewRevision();
    }
});

if (sidebarImageDrop) {
    sidebarImageDrop.addEventListener('click', (e) => {
        const target = e.target;
        if (target instanceof Element && target.closest('.file-btn')) {
            return;
        }
        imageFilesInput.click();
    });
    sidebarImageDrop.addEventListener('dragover', (e) => {
        e.preventDefault();
        sidebarImageDrop.classList.add('dragover');
    });
    sidebarImageDrop.addEventListener('dragleave', () => {
        sidebarImageDrop.classList.remove('dragover');
    });
    sidebarImageDrop.addEventListener('drop', (e) => {
        e.preventDefault();
        sidebarImageDrop.classList.remove('dragover');
        const beforeCount = imageFiles.length;
        Array.from(e.dataTransfer.files).forEach((f) => {
            if (isAcceptedImageFile(f)) imageFiles.push(f);
        });
        imageFiles = dedupeImageFiles(imageFiles);
        renderImageFileList();
        if (imageFiles.length !== beforeCount) {
            bumpPreviewRevision();
            document.dispatchEvent(new CustomEvent('nectar:assets-change'));
        }
    });
    sidebarImageDrop.addEventListener('keydown', (e) => {
        if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            imageFilesInput.click();
        }
    });
}

imageFilesInput.addEventListener('change', () => {
    const beforeCount = imageFiles.length;
    Array.from(imageFilesInput.files).forEach((f) => {
        if (isAcceptedImageFile(f)) imageFiles.push(f);
    });
    imageFiles = dedupeImageFiles(imageFiles);
    renderImageFileList();
    imageFilesInput.value = '';
    if (imageFiles.length !== beforeCount) {
        bumpPreviewRevision();
        document.dispatchEvent(new CustomEvent('nectar:assets-change'));
    }
});

document.getElementById('tableRowStripes')?.addEventListener('change', updateTableStripeColors);
document.getElementById('imageScale')?.addEventListener('input', (e) => {
    const scaleVal = document.getElementById('imageScaleVal');
    if (scaleVal) scaleVal.textContent = parseFloat(e.target.value).toFixed(2);
});

convertBtn.addEventListener('click', () => doConvert());

continueWithoutImages.addEventListener('click', () => {
    missingImagesDialog.close();
    doConvert('ALT_ONLY');
});

addMissingImages.addEventListener('click', () => {
    missingImagesDialog.close();
    if (sidebarImagesSection) sidebarImagesSection.style.display = 'flex';
    if (sidebarImageDrop) {
        sidebarImageDrop.scrollIntoView({ behavior: 'smooth', block: 'center' });
        sidebarImageDrop.classList.add('pulse');
        setTimeout(() => sidebarImageDrop.classList.remove('pulse'), 3000);
    }
    imageFilesInput.click();
});

downloadBtn.addEventListener('click', () => {
    if (!pdfBlob) return;
    downloadBlob(pdfBlob, downloadFilename || 'output.pdf');
});

startOverBtn.addEventListener('click', () => {
    if (window._lastBlobUrl) {
        URL.revokeObjectURL(window._lastBlobUrl);
        window._lastBlobUrl = null;
    }
    if (window._lastPreviewBlobUrl) {
        URL.revokeObjectURL(window._lastPreviewBlobUrl);
        window._lastPreviewBlobUrl = null;
    }

    selectedFile = null;
    selectedMarkdownText = '';
    pdfBlob = null;
    downloadFilename = 'output.pdf';
    imageFiles = [];
    userModifiedFields.clear();
    currentPresetValues = null;
    fileInput.value = '';
    imageFilesInput.value = '';
    document.getElementById('fileInfo').classList.add('hidden');
    nextBtn.classList.add('hidden');
    imageFileList.innerHTML = '';
    presetSelect.value = '';
    resetToDefaults();
    convertError.classList.add('hidden');
    const previewCanvas = document.getElementById('previewCanvas');
    const previewFrame = document.getElementById('previewFrame');
    if (previewCanvas) previewCanvas.classList.remove('has-content');
    if (previewFrame) previewFrame.removeAttribute('src');
    const previewStatus = document.getElementById('previewStatus');
    if (previewStatus) previewStatus.textContent = 'No preview generated yet.';
    const previewMissingNotice = document.getElementById('previewMissingNotice');
    const previewMissingList = document.getElementById('previewMissingList');
    if (previewMissingList) previewMissingList.innerHTML = '';
    if (previewMissingNotice) previewMissingNotice.classList.add('hidden');
    bumpPreviewRevision();
    showStep(1);
});

window.NectarUI = {
    API_BASE,
    ensureApiBase: resolveWorkingApiBase,
    buildFormData,
    getPreviewRevision: () => previewRevision,
    get imageFiles() {
        return imageFiles;
    },
    get selectedMarkdownText() {
        return selectedMarkdownText;
    },
};

setupTabs();
setupColorHex();
setupFontSelects();
setupFieldTracking();
updateImageDropZone();
updateTableStripeColors();
resetToDefaults();
checkBackendHealth();
window.NectarPreview?.initPreviewPanel?.();
