import {
    resolveWorkingApiBase,
    invalidateWorkingApiBaseCache,
} from '../core/api-base.js';

const SEARCH_CACHE = new Map();
const SEARCH_CACHE_MAX_SIZE = 120;

function cacheGet(key) {
    const value = SEARCH_CACHE.get(key);
    if (value === undefined) {
        return null;
    }
    SEARCH_CACHE.delete(key);
    SEARCH_CACHE.set(key, value);
    return value;
}

function cacheSet(key, value) {
    if (SEARCH_CACHE.has(key)) {
        SEARCH_CACHE.delete(key);
    }
    SEARCH_CACHE.set(key, value);
    while (SEARCH_CACHE.size > SEARCH_CACHE_MAX_SIZE) {
        const oldestKey = SEARCH_CACHE.keys().next().value;
        if (!oldestKey) {
            break;
        }
        SEARCH_CACHE.delete(oldestKey);
    }
}

export function createGoogleFontsApi() {
    async function searchFonts({ query = '', offset = 0, limit = 40, category = 'all' } = {}) {
        const trimmedQuery = String(query || '').trim();
        const safeOffset = Math.max(0, Number.parseInt(offset, 10) || 0);
        const safeLimit = Math.max(1, Math.min(100, Number.parseInt(limit, 10) || 40));
        const safeCategory = String(category || 'all').trim().toLowerCase() || 'all';
        const cacheKey = `${safeCategory}|${trimmedQuery}|${safeOffset}|${safeLimit}`;

        const cached = cacheGet(cacheKey);
        if (cached) {
            return cached;
        }

        const base = await resolveWorkingApiBase();
        const params = new URLSearchParams({
            q: trimmedQuery,
            offset: String(safeOffset),
            limit: String(safeLimit),
            category: safeCategory,
        });
        const response = await fetch(`${base}/fonts/google?${params.toString()}`);
        if (!response.ok) {
            throw new Error(`google-fonts-search-failed:${response.status}`);
        }
        const payload = await response.json();
        cacheSet(cacheKey, payload);
        return payload;
    }

    function invalidateCache() {
        SEARCH_CACHE.clear();
        invalidateWorkingApiBaseCache();
    }

    return {
        searchFonts,
        invalidateCache,
    };
}
