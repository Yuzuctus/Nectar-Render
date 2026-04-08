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
    if ((hostname === 'localhost' || hostname === '127.0.0.1') && /^https?:$/.test(protocol)) {
        return `${protocol}//${hostname}:8000`;
    }
    if (/^https?:$/.test(protocol) && origin && origin !== 'null') {
        return origin.replace(/\/+$/, '');
    }
    return 'http://localhost:8000';
}

const API_BASE = resolveApiBase();

// Google Fonts that need to be loaded dynamically
const GOOGLE_FONTS = [
    'Inter', 'Lato', 'Merriweather', 'Montserrat', 'Nunito', 'Open Sans',
    'Playfair Display', 'Poppins', 'Raleway', 'Roboto', 'Source Sans Pro',
    'Fira Code', 'Fira Mono', 'IBM Plex Mono', 'Inconsolata', 'JetBrains Mono',
    'Roboto Mono', 'Source Code Pro', 'Ubuntu Mono'
];

// Track loaded Google Fonts to avoid duplicate <link> tags
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
    compress_pdf: false, strip_metadata: false,
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
    'compressPdf', 'stripMetadata',
];

let selectedFile = null;
let selectedMarkdownText = '';
let pdfBlob = null;
let imageFiles = [];
let downloadFilename = 'output.pdf';
// PART 8: Track last blob URL to prevent memory leaks
window._lastBlobUrl = null;

const step1 = document.getElementById('step1');
const step2 = document.getElementById('step2');
const step3 = document.getElementById('step3');
const dropZone = document.getElementById('dropZone');
const fileInput = document.getElementById('fileInput');
const spinner = document.getElementById('spinner');
const convertSpinner = document.getElementById('convertSpinner');
const nextBtn = document.getElementById('nextBtn');
const backBtn = document.getElementById('backBtn');
const convertBtn = document.getElementById('convertBtn');
const downloadBtn = document.getElementById('downloadBtn');
const startOverBtn = document.getElementById('startOverBtn');
const presetSelect = document.getElementById('presetSelect');
const imageDropZone = document.getElementById('imageDropZone');
const imageFilesInput = document.getElementById('imageFilesInput');
const imageFileList = document.getElementById('imageFileList');
const convertError = document.getElementById('convertError');
const toast = document.getElementById('toast');
// PART 2 & 3: New UI elements
const missingImagesDialog = document.getElementById('missingImagesDialog');
const missingImagesList = document.getElementById('missingImagesList');
const continueWithoutImages = document.getElementById('continueWithoutImages');
const addMissingImages = document.getElementById('addMissingImages');
const loadingOverlay = document.getElementById('loadingOverlay');
const loadingText = document.getElementById('loadingText');

// PART 8: Add bounds check to showStep
function showStep(n) {
    const steps = [step1, step2, step3];
    if (n < 1 || n > steps.length) {
        console.warn(`showStep: Invalid step number ${n}`);
        return;
    }
    steps.forEach(s => s.classList.remove('active'));
    steps.forEach(s => s.classList.add('hidden'));
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

// PART 3: Loading overlay helpers
function showLoadingOverlay(format) {
    const formatText = {
        'PDF': 'Generation du PDF...',
        'HTML': 'Generation du HTML...',
        'PDF+HTML': 'Generation du ZIP...'
    };
    loadingText.textContent = formatText[format] || 'Conversion en cours...';
    loadingOverlay.classList.remove('hidden', 'fade-out');
}

function hideLoadingOverlay(immediate = false) {
    if (immediate) {
        loadingOverlay.classList.add('hidden');
    } else {
        loadingOverlay.classList.add('fade-out');
        setTimeout(() => loadingOverlay.classList.add('hidden'), 300);
    }
}

// PART 5: Google Font loader
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

// PART 5: Update font preview
function updateFontPreview(selectId, previewId) {
    const select = document.getElementById(selectId);
    const preview = document.getElementById(previewId);
    if (!select || !preview) return;
    
    const fontName = select.value;
    loadGoogleFont(fontName);
    preview.style.fontFamily = `"${fontName}", sans-serif`;
}

// PART 5: Setup font select listeners
function setupFontSelects() {
    const fontSelects = [
        { select: 'bodyFont', preview: 'bodyFontPreview' },
        { select: 'headingFont', preview: 'headingFontPreview' },
        { select: 'codeFont', preview: 'codeFontPreview' }
    ];
    
    fontSelects.forEach(({ select, preview }) => {
        const el = document.getElementById(select);
        if (el) {
            el.addEventListener('change', () => updateFontPreview(select, preview));
            // Initialize preview
            updateFontPreview(select, preview);
        }
    });
}

function setupColorHex() {
    document.querySelectorAll('input[type="color"]').forEach(input => {
        const hexSpan = input.parentElement.querySelector('.hex-view');
        if (hexSpan) {
            hexSpan.textContent = input.value;
            input.addEventListener('input', () => { hexSpan.textContent = input.value; });
        }
    });
}

function setupCollapsibleSections() {
    document.querySelectorAll('.section-header').forEach(btn => {
        btn.addEventListener('click', () => {
            const section = btn.closest('.panel-section');
            section.classList.toggle('open');
        });
    });
    document.querySelectorAll('.panel-section').forEach(s => s.classList.add('open'));
}

function updateImageDropZone() {
    const mode = document.querySelector('input[name="imageMode"]:checked')?.value;
    if (mode === 'WITH_IMAGES') {
        imageDropZone.classList.remove('hidden');
    } else {
        imageDropZone.classList.add('hidden');
    }
}

function countWords(text) {
    return text.trim().split(/\s+/).filter(w => w.length > 0).length;
}

function formatFileSize(bytes) {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function applyPresetValues(presetName) {
    const preset = PRESETS[presetName];
    if (!preset) {
        resetToDefaults();
        return;
    }
    resetToDefaults();
    Object.entries(preset).forEach(([key, val]) => {
        const el = document.getElementById(keyToId(key));
        if (!el) return;
        if (typeof val === 'boolean') {
            el.checked = val;
        } else if (typeof val === 'number') {
            el.value = val;
            if (key === 'image_scale') {
                document.getElementById('imageScaleVal').textContent = val.toFixed(2);
            }
        } else {
            el.value = val;
            const hexEl = el.parentElement?.querySelector('.hex-view');
            if (hexEl) hexEl.textContent = val;
        }
    });
    updateTableStripeColors();
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
                document.getElementById('imageScaleVal').textContent = val.toFixed(2);
            }
        } else {
            el.value = val;
            const hexEl = el.parentElement?.querySelector('.hex-view');
            if (hexEl) hexEl.textContent = val;
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
        compress_pdf: 'compressPdf', strip_metadata: 'stripMetadata',
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
        compressPdf: 'compress_pdf', stripMetadata: 'strip_metadata',
    };
    return map[id] || id;
}

function updateTableStripeColors() {
    const stripesEnabled = document.getElementById('tableRowStripes')?.checked;
    const colorsDiv = document.getElementById('tableStripeColors');
    if (colorsDiv) {
        colorsDiv.style.opacity = stripesEnabled ? '1' : '0.4';
        colorsDiv.style.pointerEvents = stripesEnabled ? 'auto' : 'none';
    }
}

function collectFormFields() {
    const data = {};
    STYLE_INPUT_IDS.forEach(id => {
        const el = document.getElementById(id);
        if (!el) return;
        const key = idToKey(id);
        if (el.type === 'checkbox') {
            data[key] = el.checked ? 'true' : 'false';
            return;
        }
        if (el.value !== '') {
            data[key] = el.value;
        }
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

        const removeButton = document.createElement('button');
        removeButton.type = 'button';
        removeButton.setAttribute('aria-label', `Remove ${f.name}`);
        removeButton.textContent = '×';
        removeButton.addEventListener('click', () => {
            imageFiles.splice(i, 1);
            renderImageFileList();
        });

        item.appendChild(nameSpan);
        item.appendChild(removeButton);
        imageFileList.appendChild(item);
    });
}

// Drop zone
dropZone.addEventListener('dragover', e => { e.preventDefault(); dropZone.classList.add('dragover'); });
dropZone.addEventListener('dragleave', () => dropZone.classList.remove('dragover'));
dropZone.addEventListener('drop', e => {
    e.preventDefault();
    dropZone.classList.remove('dragover');
    const f = e.dataTransfer.files[0];
    if (f && f.name.toLowerCase().endsWith('.md')) processFile(f);
    else showToast('Please drop a .md file', 'error');
});

dropZone.addEventListener('keydown', e => {
    if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        fileInput.click();
    }
});

fileInput.addEventListener('change', () => {
    if (fileInput.files[0]) processFile(fileInput.files[0]);
});

function processFile(file) {
    selectedFile = file;
    const reader = new FileReader();
    reader.onload = e => {
        selectedMarkdownText = e.target.result;
        document.getElementById('fileName').textContent = file.name;
        document.getElementById('fileSize').textContent = formatFileSize(file.size);
        document.getElementById('wordCount').textContent = `${countWords(selectedMarkdownText)} words`;
        document.getElementById('sidebarFileName').textContent = file.name;
        document.getElementById('sidebarFileSize').textContent = formatFileSize(file.size);
        document.getElementById('fileInfo').classList.remove('hidden');
        nextBtn.classList.remove('hidden');
    };
    reader.readAsText(file);
}

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
});

document.querySelectorAll('input[name="imageMode"]').forEach(r => {
    r.addEventListener('change', updateImageDropZone);
});

imageDropZone.addEventListener('dragover', e => { e.preventDefault(); imageDropZone.classList.add('dragover'); });
imageDropZone.addEventListener('dragleave', () => imageDropZone.classList.remove('dragover'));
    imageDropZone.addEventListener('drop', e => {
    e.preventDefault();
    imageDropZone.classList.remove('dragover');
    Array.from(e.dataTransfer.files).forEach(f => {
        if (f.type.startsWith('image/')) imageFiles.push(f);
    });
    imageFiles = dedupeImageFiles(imageFiles);
    renderImageFileList();
});
imageDropZone.addEventListener('keydown', e => {
    if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        imageFilesInput.click();
    }
});

imageFilesInput.addEventListener('change', () => {
    Array.from(imageFilesInput.files).forEach(f => imageFiles.push(f));
    imageFiles = dedupeImageFiles(imageFiles);
    renderImageFileList();
    imageFilesInput.value = '';
});

function dedupeImageFiles(files) {
    const seen = new Set();
    const deduped = [];
    files.forEach(file => {
        const key = `${file.name}::${file.size}::${file.lastModified}`;
        if (seen.has(key)) return;
        seen.add(key);
        deduped.push(file);
    });
    return deduped;
}

document.getElementById('tableRowStripes')?.addEventListener('change', updateTableStripeColors);

document.getElementById('imageScale')?.addEventListener('input', e => {
    document.getElementById('imageScaleVal').textContent = parseFloat(e.target.value).toFixed(2);
});

convertBtn.addEventListener('click', () => doConvert());

// PART 2: Missing images dialog handlers
continueWithoutImages.addEventListener('click', () => {
    missingImagesDialog.close();
    doConvert('ALT_ONLY');
});

addMissingImages.addEventListener('click', () => {
    missingImagesDialog.close();
    // Focus image drop zone and pulse animation
    imageDropZone.classList.remove('hidden');
    imageDropZone.scrollIntoView({ behavior: 'smooth', block: 'center' });
    imageDropZone.classList.add('pulse');
    setTimeout(() => imageDropZone.classList.remove('pulse'), 3000);
    imageFilesInput.click();
});

async function doConvert(overrideImageMode = null) {
    if (!selectedMarkdownText) return;
    convertError.classList.add('hidden');
    
    const outputFormat = document.querySelector('input[name="outputFormat"]:checked')?.value || 'PDF';
    
    // PART 3: Show loading overlay
    showLoadingOverlay(outputFormat);
    convertBtn.disabled = true;
    convertBtn.innerHTML = '<span class="btn-spinner"></span> Conversion en cours...';
    convertBtn.classList.add('btn-with-spinner');

    try {
        const formData = new FormData();
        formData.append('markdown_text', selectedMarkdownText);
        formData.append('image_mode', overrideImageMode || document.querySelector('input[name="imageMode"]:checked')?.value || 'WITH_IMAGES');
        formData.append('output_format', outputFormat);
        formData.append('page_size', document.getElementById('pageSize')?.value || 'A4');
        formData.append('preset', presetSelect.value || '');

        const fields = collectFormFields();
        Object.entries(fields).forEach(([k, v]) => formData.append(k, v));

        imageFiles.forEach(f => formData.append('assets', f));

        const response = await fetch(`${API_BASE}/convert`, {
            method: 'POST',
            body: formData,
        });

        if (!response.ok) {
            let errData;
            try {
                errData = await response.json();
            } catch {}
            
            // PART 2: Handle missing images (422)
            const missingFiles =
                Array.isArray(errData?.missing_images)
                    ? errData.missing_images
                    : (errData?.detail?.detail === 'missing_images' && Array.isArray(errData?.detail?.files)
                        ? errData.detail.files
                        : null);

            if (response.status === 422 && missingFiles) {
                missingImagesList.innerHTML = '';
                missingFiles.forEach(file => {
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
            
            const msg = errData?.detail?.detail || errData?.detail || 'Conversion failed';
            throw new Error(msg);
        }

        // PART 4: Extract filename from Content-Disposition header
        const contentDisposition = response.headers.get('content-disposition') || '';
        let filename = 'output.pdf';
        const filenameMatch = contentDisposition.match(/filename="?([^";\n]+)"?/);
        if (filenameMatch && filenameMatch[1]) {
            filename = filenameMatch[1];
        } else {
            // Fallback based on content-type
            const contentType = response.headers.get('content-type') || '';
            if (contentType.includes('text/html')) {
                filename = selectedFile ? selectedFile.name.replace(/\.md$/, '.html') : 'output.html';
            } else if (contentType.includes('zip')) {
                filename = selectedFile ? selectedFile.name.replace(/\.md$/, '.zip') : 'output.zip';
            } else {
                filename = selectedFile ? selectedFile.name.replace(/\.md$/, '.pdf') : 'output.pdf';
            }
        }

        downloadFilename = filename;

        pdfBlob = await response.blob();
        showToast(`Download ready: ${filename}`, 'success');

        // PART 8: Revoke previous blob URL to prevent memory leak
        if (window._lastBlobUrl) {
            URL.revokeObjectURL(window._lastBlobUrl);
        }
        const url = URL.createObjectURL(pdfBlob);
        window._lastBlobUrl = url;
        
        const a = document.createElement('a');
        a.href = url;
        a.download = filename;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);

        // PART 3: Fade out loading overlay
        hideLoadingOverlay(false);
        showStep(3);
    } catch (err) {
        // PART 3: Immediate hide on error
        hideLoadingOverlay(true);
        convertError.textContent = err.message;
        convertError.classList.remove('hidden');
    } finally {
        convertBtn.disabled = false;
        convertBtn.textContent = 'Convert';
        convertBtn.classList.remove('btn-with-spinner');
    }
}

downloadBtn.addEventListener('click', () => {
    if (!pdfBlob) return;
    // PART 8: Revoke previous blob URL to prevent memory leak
    if (window._lastBlobUrl) {
        URL.revokeObjectURL(window._lastBlobUrl);
    }
    const url = URL.createObjectURL(pdfBlob);
    window._lastBlobUrl = url;
    const a = document.createElement('a');
    a.href = url;
    a.download = downloadFilename || 'output.pdf';
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
});

startOverBtn.addEventListener('click', () => {
    // PART 8: Cleanup blob URL
    if (window._lastBlobUrl) {
        URL.revokeObjectURL(window._lastBlobUrl);
        window._lastBlobUrl = null;
    }
    selectedFile = null;
    selectedMarkdownText = '';
    pdfBlob = null;
    downloadFilename = 'output.pdf';
    imageFiles = [];
    fileInput.value = '';
    imageFilesInput.value = '';
    document.getElementById('fileInfo').classList.add('hidden');
    nextBtn.classList.add('hidden');
    imageFileList.innerHTML = '';
    presetSelect.value = '';
    resetToDefaults();
    convertError.classList.add('hidden');
    showStep(1);
});

// Init
setupColorHex();
setupCollapsibleSections();
setupFontSelects();
updateImageDropZone();
updateTableStripeColors();
resetToDefaults();
