export function resolveApiBase() {
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

function buildCandidateBases(preferred) {
    const candidates = [preferred, 'http://127.0.0.1:8000', 'http://localhost:8000'];
    const unique = [];
    const seen = new Set();
    candidates.forEach((candidate) => {
        const normalized = String(candidate || '').trim().replace(/\/+$/, '');
        if (!normalized || seen.has(normalized)) {
            return;
        }
        seen.add(normalized);
        unique.push(normalized);
    });
    return unique;
}

let workingApiBaseCache = null;
let cacheExpiresAt = 0;
const CACHE_TTL_MS = 45_000;

export function invalidateWorkingApiBaseCache() {
    workingApiBaseCache = null;
    cacheExpiresAt = 0;
}

export async function resolveWorkingApiBase({ force = false } = {}) {
    const now = Date.now();
    if (!force && workingApiBaseCache && now < cacheExpiresAt) {
        return workingApiBaseCache;
    }

    const preferred = resolveApiBase();
    const candidates = buildCandidateBases(preferred);

    for (const candidate of candidates) {
        try {
            const response = await fetch(`${candidate}/analyze/health`, { method: 'GET' });
            if (response.ok) {
                workingApiBaseCache = candidate;
                cacheExpiresAt = Date.now() + CACHE_TTL_MS;
                return candidate;
            }
        } catch {
        }
    }

    workingApiBaseCache = preferred;
    cacheExpiresAt = Date.now() + CACHE_TTL_MS;
    return preferred;
}
