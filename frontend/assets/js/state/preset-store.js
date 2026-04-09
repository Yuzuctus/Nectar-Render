const STORAGE_KEY = 'nectar.frontend.user-presets.v1';
const THEME_EXPORT_FORMAT = 'nectar-render-theme';
const THEME_EXPORT_VERSION = 1;

const VALID_STYLE_KEYS = new Set([
    'body_font', 'body_font_size', 'body_text_color', 'line_height',
    'heading_font', 'heading_color',
    'heading_h1_color', 'heading_h2_color', 'heading_h3_color',
    'heading_h4_color', 'heading_h5_color', 'heading_h6_color',
    'heading_h1_size_px', 'heading_h2_size_px', 'heading_h3_size_px',
    'heading_h4_size_px', 'heading_h5_size_px', 'heading_h6_size_px',
    'code_font', 'code_font_size', 'code_line_height', 'code_theme',
    'margin_top_mm', 'margin_right_mm', 'margin_bottom_mm', 'margin_left_mm',
    'footer_text', 'footer_align', 'footer_color', 'footer_font_size',
    'include_footnotes', 'footnote_font_size', 'footnote_text_color', 'footnote_marker_color',
    'table_row_stripes', 'table_row_odd_color', 'table_row_even_color',
    'table_cell_padding_y_px', 'table_cell_padding_x_px', 'border_color',
    'image_scale', 'sanitize_html', 'show_horizontal_rules',
    'compress_pdf', 'compression_profile', 'strip_metadata', 'compression_timeout', 'keep_original_on_fail',
]);

function _readStorage() {
    try {
        const raw = localStorage.getItem(STORAGE_KEY);
        if (!raw) return {};
        const data = JSON.parse(raw);
        if (typeof data === 'object' && data !== null && !Array.isArray(data)) {
            return data;
        }
    } catch {}
    return {};
}

function _writeStorage(data) {
    try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(data));
    } catch {}
}

function _sanitizeStyle(style) {
    if (typeof style !== 'object' || style === null || Array.isArray(style)) return null;
    const clean = {};
    for (const [key, value] of Object.entries(style)) {
        if (VALID_STYLE_KEYS.has(key)) {
            clean[key] = value;
        }
    }
    return Object.keys(clean).length > 0 ? clean : null;
}

export function createUserPresetStore() {
    function _normalizeUserPresetId(rawId) {
        return String(rawId || '').replace(/^user:/, '');
    }

    function _buildStorageKey(rawId) {
        const normalized = _normalizeUserPresetId(rawId);
        return `user:${normalized}`;
    }

    function listUserPresets() {
        const stored = _readStorage();
        return Object.entries(stored)
            .map(([id, preset]) => ({
                id: _normalizeUserPresetId(id),
                name: preset.name || id,
                style: preset.style || {},
                createdAt: preset.createdAt || null,
                updatedAt: preset.updatedAt || null,
            }))
            .sort((a, b) => (a.name || '').localeCompare(b.name || ''));
    }

    function getUserPreset(id) {
        const stored = _readStorage();
        const storageKey = _buildStorageKey(id);
        const preset = stored[storageKey];
        if (!preset) return null;
        return {
            id: _normalizeUserPresetId(storageKey),
            name: preset.name || storageKey,
            style: preset.style || {},
            createdAt: preset.createdAt || null,
            updatedAt: preset.updatedAt || null,
        };
    }

    function saveUserPreset(name, style) {
        const stored = _readStorage();
        const slug = name.trim().toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '');
        const id = slug;
        const storageKey = _buildStorageKey(id);
        const sanitized = _sanitizeStyle(style);
        if (!sanitized) return null;
        stored[storageKey] = {
            name: name.trim(),
            style: sanitized,
            createdAt: stored[storageKey]?.createdAt || new Date().toISOString(),
            updatedAt: new Date().toISOString(),
        };
        _writeStorage(stored);
        return id;
    }

    function deleteUserPreset(id) {
        const stored = _readStorage();
        const storageKey = _buildStorageKey(id);
        if (!stored[storageKey]) return false;
        delete stored[storageKey];
        _writeStorage(stored);
        return true;
    }

    function renameUserPreset(id, newName) {
        const stored = _readStorage();
        const storageKey = _buildStorageKey(id);
        if (!stored[storageKey]) return false;
        stored[storageKey].name = newName.trim();
        stored[storageKey].updatedAt = new Date().toISOString();
        _writeStorage(stored);
        return true;
    }

    function exportTheme(name, style) {
        return JSON.stringify({
            format: THEME_EXPORT_FORMAT,
            version: THEME_EXPORT_VERSION,
            name: name.trim(),
            exported_at: new Date().toISOString(),
            style,
        }, null, 2);
    }

    function importTheme(jsonString) {
        let data;
        try {
            data = JSON.parse(jsonString);
        } catch {
            return { ok: false, error: 'Invalid JSON' };
        }

        if (!data || typeof data !== 'object' || Array.isArray(data)) {
            return { ok: false, error: 'Not a valid theme file' };
        }

        if (data.format !== THEME_EXPORT_FORMAT) {
            return { ok: false, error: `Unsupported format: "${data.format}". Expected "${THEME_EXPORT_FORMAT}"` };
        }

        if (data.version !== THEME_EXPORT_VERSION) {
            return { ok: false, error: `Unsupported version: ${data.version}` };
        }

        const name = String(data.name || '').trim();
        if (!name) {
            return { ok: false, error: 'Theme name is missing' };
        }

        const sanitized = _sanitizeStyle(data.style);
        if (!sanitized) {
            return { ok: false, error: 'Theme has no valid style properties' };
        }

        return { ok: true, name, style: sanitized };
    }

    return {
        listUserPresets,
        getUserPreset,
        saveUserPreset,
        deleteUserPreset,
        renameUserPreset,
        exportTheme,
        importTheme,
    };
}
