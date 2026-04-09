export function createConversionApi({ backendHealth }) {
    async function requestConversion(formData, { signal } = {}) {
        const apiBase = await backendHealth.resolveWorkingApiBase();
        return fetch(`${apiBase}/convert`, { method: 'POST', body: formData, signal });
    }

    function extractFilename(response, selectedFileName) {
        const contentDisposition = response.headers.get('content-disposition') || '';
        const filenameMatch = contentDisposition.match(/filename="?([^";\n]+)"?/);
        if (filenameMatch && filenameMatch[1]) {
            return filenameMatch[1];
        }
        const contentType = response.headers.get('content-type') || '';
        if (contentType.includes('text/html')) {
            return selectedFileName ? selectedFileName.replace(/\.md$/i, '.html') : 'output.html';
        }
        if (contentType.includes('zip')) {
            return selectedFileName ? selectedFileName.replace(/\.md$/i, '.zip') : 'output.zip';
        }
        return selectedFileName ? selectedFileName.replace(/\.md$/i, '.pdf') : 'output.pdf';
    }

    return {
        requestConversion,
        extractFilename,
    };
}
