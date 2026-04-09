import { resolveWorkingApiBase } from '../core/api-base.js';

export function createPresetsApi({ backendHealth } = {}) {
    let _builtinPresetsCache = null;

    async function _resolveBaseUrl() {
        if (backendHealth && typeof backendHealth.resolveWorkingApiBase === 'function') {
            return backendHealth.resolveWorkingApiBase();
        }
        return resolveWorkingApiBase();
    }

    async function fetchBuiltinPresets() {
        if (_builtinPresetsCache) return _builtinPresetsCache;

        const base = await _resolveBaseUrl();
        try {
            const response = await fetch(`${base}/presets/builtin`);
            if (response.ok) {
                const data = await response.json();
                const presets = data.presets || {};
                _builtinPresetsCache = presets;
                return presets;
            }
        } catch {
        }

        return {};
    }

    function invalidateCache() {
        _builtinPresetsCache = null;
    }

    return {
        fetchBuiltinPresets,
        invalidateCache,
    };
}
