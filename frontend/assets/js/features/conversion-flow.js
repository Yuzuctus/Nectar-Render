function normalizeBackendMessage(message, fallback = 'Operation failed') {
    const text = String(message || '').trim();
    return text || fallback;
}

function networkErrorMessage(error, apiBase) {
    if (error?.name === 'TypeError' && /fetch/i.test(String(error.message || ''))) {
        return `Cannot reach API at ${apiBase}. Start backend server (uvicorn backend.main:app --reload --host 127.0.0.1 --port 8000).`;
    }
    return error?.message || 'Network request failed';
}

function createBlobDownloader({ getPreviewBlobUrl, clearPreviewBlobUrl }) {
    let lastBlobUrl = null;

    function downloadBlob(blob, filename) {
        if (lastBlobUrl) {
            URL.revokeObjectURL(lastBlobUrl);
        }
        const url = URL.createObjectURL(blob);
        lastBlobUrl = url;

        const link = document.createElement('a');
        link.href = url;
        link.download = filename;
        document.body.appendChild(link);
        link.click();
        document.body.removeChild(link);
    }

    function clear() {
        if (lastBlobUrl) {
            URL.revokeObjectURL(lastBlobUrl);
            lastBlobUrl = null;
        }
        const previewBlobUrl = getPreviewBlobUrl?.();
        if (previewBlobUrl) {
            URL.revokeObjectURL(previewBlobUrl);
            clearPreviewBlobUrl?.();
        }
    }

    return {
        downloadBlob,
        clear,
    };
}

export function createConversionFlowFeature({
    panelContext,
    presetsForm,
    imageAssets,
    conversionApi,
    backendHealth,
    loadingOverlay,
    toast,
    missingImagesDialog,
    getPreviewBlobUrl,
    clearPreviewBlobUrl,
    onPreviewDirty,
    onResult,
}) {
    const convertBtn = document.getElementById('convertBtn');
    const convertError = document.getElementById('convertError');
    const step2 = document.getElementById('step2');
    const step3 = document.getElementById('step3');

    const downloader = createBlobDownloader({
        getPreviewBlobUrl,
        clearPreviewBlobUrl,
    });

    let outputBlob = null;
    let outputFilename = 'output.pdf';
    let conversionAbortController = null;
    const cleanups = [];

    function getOutputFormat() {
        return document.querySelector('input[name="outputFormat"]:checked')?.value || 'PDF';
    }

    function getImageMode() {
        return document.querySelector('input[name="imageMode"]:checked')?.value || 'WITH_IMAGES';
    }

    function getPageSize() {
        return document.getElementById('pageSize')?.value || 'A4';
    }

    function createFormData(overrideImageMode = null) {
        const formData = new FormData();
        formData.append('markdown_text', panelContext.getMarkdownText());
        formData.append('image_mode', overrideImageMode || getImageMode());
        formData.append('output_format', getOutputFormat());
        formData.append('page_size', getPageSize());

        const presetInfo = presetsForm.getSelectedPresetInfo();
        if (presetInfo.type === 'builtin') {
            formData.append('preset', presetInfo.name);
        }

        const fields = presetsForm.collectFormFields();
        Object.entries(fields).forEach(([key, value]) => {
            formData.append(key, value);
        });

        imageAssets.getImageFiles().forEach((file) => {
            formData.append('assets', file);
        });

        return formData;
    }

    function clearError() {
        if (!convertError) return;
        convertError.classList.add('hidden');
        convertError.textContent = '';
    }

    function showError(message) {
        if (!convertError) return;
        convertError.textContent = message;
        convertError.classList.remove('hidden');
    }

    async function doConvert(overrideImageMode = null) {
        const markdownText = panelContext.getMarkdownText();
        if (!markdownText) {
            showError('No markdown loaded.');
            return;
        }

        clearError();
        const outputFormat = getOutputFormat();
        loadingOverlay.showOverlay(outputFormat);

        if (conversionAbortController) {
            conversionAbortController.abort();
        }
        conversionAbortController = new AbortController();

        if (convertBtn) {
            convertBtn.disabled = true;
            convertBtn.classList.add('btn-with-spinner');
            convertBtn.innerHTML = '<span class="btn-spinner"></span> Converting...';
        }

        try {
            const response = await conversionApi.requestConversion(
                createFormData(overrideImageMode),
                { signal: conversionAbortController.signal },
            );

            if (!response.ok) {
                let errorData = null;
                try {
                    errorData = await response.json();
                } catch {
                }

                const missingFiles = Array.isArray(errorData?.missing_images)
                    ? errorData.missing_images
                    : null;

                if (response.status === 422 && missingFiles?.length) {
                    loadingOverlay.hideOverlay(true);
                    if (convertBtn) {
                        convertBtn.disabled = false;
                        convertBtn.classList.remove('btn-with-spinner');
                        convertBtn.textContent = 'Convert';
                    }

                    const choice = await missingImagesDialog.prompt(missingFiles);
                    if (choice === 'add') {
                        imageAssets.focusDropZone();
                        imageAssets.openPicker();
                        return;
                    }
                    await doConvert('ALT_ONLY');
                    return;
                }

                const message = normalizeBackendMessage(
                    errorData?.detail,
                    `Conversion failed (${response.status})`
                );
                throw new Error(message);
            }

            outputFilename = conversionApi.extractFilename(response, panelContext.getSelectedFileName());
            outputBlob = await response.blob();

            downloader.downloadBlob(outputBlob, outputFilename);
            toast.showToast(`Download ready: ${outputFilename}`, 'success');
            loadingOverlay.hideOverlay(false);
            if (step2 && step3) {
                step2.classList.remove('active');
                step3.classList.add('active');
            }
            onResult?.({ filename: outputFilename, blob: outputBlob });
        } catch (error) {
            if (error?.name === 'AbortError') {
                return;
            }
            loadingOverlay.hideOverlay(true);
            showError(networkErrorMessage(error, backendHealth.getApiBase()));
        } finally {
            conversionAbortController = null;
            if (convertBtn) {
                convertBtn.disabled = false;
                convertBtn.classList.remove('btn-with-spinner');
                convertBtn.textContent = 'Convert';
            }
        }
    }

    function bind() {
        const onConvertClick = () => doConvert();
        convertBtn?.addEventListener('click', onConvertClick);
        if (convertBtn) {
            cleanups.push(() => convertBtn.removeEventListener('click', onConvertClick));
        }

        const onDownloadClick = () => {
            if (!outputBlob) return;
            downloader.downloadBlob(outputBlob, outputFilename || 'output.pdf');
        };
        const downloadBtn = document.getElementById('downloadBtn');
        downloadBtn?.addEventListener('click', onDownloadClick);
        if (downloadBtn) {
            cleanups.push(() => downloadBtn.removeEventListener('click', onDownloadClick));
        }

        const onStartOverClick = async () => {
            downloader.clear();
            await panelContext.clearDraft();
            window.location.href = 'index.html';
        };
        const startOverBtn = document.getElementById('startOverBtn');
        startOverBtn?.addEventListener('click', onStartOverClick);
        if (startOverBtn) {
            cleanups.push(() => startOverBtn.removeEventListener('click', onStartOverClick));
        }

        const trackPreviewDirty = () => onPreviewDirty?.();
        const mainPanel = document.querySelector('.main-panel');
        mainPanel?.addEventListener('input', trackPreviewDirty);
        mainPanel?.addEventListener('change', trackPreviewDirty);
        const presetSelect = document.getElementById('presetSelect');
        presetSelect?.addEventListener('change', trackPreviewDirty);
        if (mainPanel) {
            cleanups.push(() => mainPanel.removeEventListener('input', trackPreviewDirty));
            cleanups.push(() => mainPanel.removeEventListener('change', trackPreviewDirty));
        }
        if (presetSelect) {
            cleanups.push(() => presetSelect.removeEventListener('change', trackPreviewDirty));
        }
    }

    function init() {
        clearError();
        bind();
    }

    function destroy() {
        if (conversionAbortController) {
            conversionAbortController.abort();
            conversionAbortController = null;
        }
        cleanups.splice(0).forEach((cleanup) => cleanup());
        downloader.clear();
    }

    return {
        init,
        destroy,
        createFormData,
        getMarkdownText: () => panelContext.getMarkdownText(),
        getImageFiles: () => imageAssets.getImageFiles(),
    };
}
