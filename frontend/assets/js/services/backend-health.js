import {
    resolveApiBase,
    resolveWorkingApiBase as resolveWorkingApiBaseCore,
} from '../core/api-base.js';

export function createBackendHealthService() {
    let apiBase = resolveApiBase();

    async function resolveWorkingApiBase() {
        apiBase = await resolveWorkingApiBaseCore();
        return apiBase;
    }

    async function checkHealth() {
        const base = await resolveWorkingApiBase();
        try {
            const response = await fetch(`${base}/analyze/health`, { method: 'GET' });
            return {
                status: response.ok ? 'online' : 'offline',
                apiBase: base,
                code: response.status,
            };
        } catch {
            return {
                status: 'offline',
                apiBase: base,
                code: null,
            };
        }
    }

    function getApiBase() {
        return apiBase;
    }

    return {
        checkHealth,
        getApiBase,
        resolveWorkingApiBase,
    };
}
