export function createPreviewApi({ backendHealth }) {
    async function requestPreview(formData, { signal } = {}) {
        const apiBase = await backendHealth.resolveWorkingApiBase();
        return fetch(`${apiBase}/preview`, { method: 'POST', body: formData, signal });
    }

    return {
        requestPreview,
    };
}
