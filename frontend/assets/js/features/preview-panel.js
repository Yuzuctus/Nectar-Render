const PREVIEW_STAGE_TOTAL = 5;

function byId(id) {
    return document.getElementById(id);
}

function getElements() {
    return {
        refreshBtn: byId('previewRefreshBtn'),
        status: byId('previewStatus'),
        engineChip: document.querySelector('.preview-engine-chip'),
        frame: byId('previewFrame'),
        canvas: byId('previewCanvas'),
        sheet: byId('previewSheet'),
        progress: byId('previewProgress'),
        progressBar: byId('previewProgressBar'),
        missingNotice: byId('previewMissingNotice'),
        missingTitle: byId('previewMissingTitle'),
        missingList: byId('previewMissingList'),
        addMissingBtn: byId('previewAddMissingBtn'),
    };
}

function getSelectedOutputFormat() {
    return document.querySelector('input[name="outputFormat"]:checked')?.value || 'PDF';
}

function getPreviewEngine(outputFormat) {
    return outputFormat === 'HTML' ? 'html' : 'pdf';
}

function getPdfPreviewHeight() {
    const vh = Math.max(window.innerHeight || 0, 1);
    return `${Math.max(420, Math.round(vh * 0.72))}px`;
}

function applyFrameSandbox(frame, engine) {
    if (!frame) return;
    if (engine === 'html') {
        frame.setAttribute('sandbox', 'allow-same-origin');
        return;
    }
    frame.removeAttribute('sandbox');
}

export function createPreviewPanelFeature({
    previewApi,
    buildFormData,
    getMarkdownText,
    getImageFiles,
    getPreviewRevision,
    onRequestAddMissing,
}) {
    let activeTab = 'export';
    let isRefreshing = false;
    let queuedRefresh = false;
    let lastRenderedRevision = -1;
    let previewAbortController = null;
    let delayedHideTimer = 0;
    let lastPreviewBlobUrl = null;
    const cleanups = [];

    function setPreviewMode(engine) {
        const { engineChip, frame, sheet } = getElements();
        if (engineChip) {
            engineChip.textContent = engine === 'html' ? 'Live HTML preview' : 'True PDF preview';
        }
        if (frame) {
            frame.classList.toggle('is-html', engine === 'html');
            frame.classList.toggle('is-pdf', engine === 'pdf');
        }
        if (sheet) {
            sheet.classList.toggle('is-html', engine === 'html');
            sheet.classList.toggle('is-pdf', engine === 'pdf');
        }
        applyFrameSandbox(frame, engine);
    }

    function fitHtmlFrameHeight(frame) {
        if (!frame) return;
        try {
            const doc = frame.contentDocument;
            if (!doc) return;
            const body = doc.body;
            const root = doc.documentElement;
            const bodyStyles = body ? window.getComputedStyle(body) : null;
            const marginTop = bodyStyles ? parseFloat(bodyStyles.marginTop) || 0 : 0;
            const marginBottom = bodyStyles ? parseFloat(bodyStyles.marginBottom) || 0 : 0;
            const bodyMeasured = Math.max(
                body?.scrollHeight || 0,
                body?.offsetHeight || 0,
                body?.getBoundingClientRect().height || 0
            );
            const measured = Math.max(
                bodyMeasured + marginTop + marginBottom,
                root?.offsetHeight || 0
            );
            if (!measured) return;
            frame.style.height = `${Math.max(96, Math.ceil(measured) + 2)}px`;
        } catch {
        }
    }

    function loadHtmlIntoFrame(frame, htmlContent) {
        return new Promise((resolve) => {
            let done = false;
            const finish = () => {
                if (done) return;
                done = true;
                resolve();
            };

            const onLoad = () => {
                window.requestAnimationFrame(() => {
                    fitHtmlFrameHeight(frame);
                    window.setTimeout(() => {
                        fitHtmlFrameHeight(frame);
                        finish();
                    }, 90);
                });
            };

            frame.addEventListener('load', onLoad, { once: true });
            frame.style.height = '96px';
            frame.src = 'about:blank';
            frame.srcdoc = htmlContent;

            window.setTimeout(() => {
                fitHtmlFrameHeight(frame);
                finish();
            }, 500);
        });
    }

    function setStatus(text, isError = false) {
        const { status } = getElements();
        if (!status) return;
        status.textContent = text;
        status.style.color = isError ? 'var(--danger)' : 'var(--fg-muted)';
    }

    function setStage(step, label) {
        setStatus(`[${step}/${PREVIEW_STAGE_TOTAL}] ${label}`);
    }

    function setBusy(isBusy) {
        const { refreshBtn } = getElements();
        if (!refreshBtn) return;
        refreshBtn.disabled = isBusy;
        refreshBtn.textContent = isBusy ? 'Refreshing...' : 'Refresh preview';
    }

    function setProgress(percent) {
        const { progress, progressBar } = getElements();
        if (!progress || !progressBar) return;
        const normalized = Math.max(0, Math.min(100, Math.round(percent)));
        progress.classList.remove('hidden');
        progress.setAttribute('aria-valuenow', String(normalized));
        progressBar.style.width = `${normalized}%`;
    }

    function hideProgress() {
        const { progress, progressBar } = getElements();
        if (!progress || !progressBar) return;
        progress.classList.add('hidden');
        progress.setAttribute('aria-valuenow', '0');
        progressBar.style.width = '0%';
    }

    function updatePageSize() {
        const pageSize = byId('pageSize')?.value || 'A4';
        const { sheet } = getElements();
        if (sheet) {
            sheet.dataset.pageSize = pageSize;
        }
    }

    function clearMissingImagesNotice() {
        const { missingNotice, missingList } = getElements();
        if (!missingNotice || !missingList) return;
        missingList.innerHTML = '';
        missingNotice.classList.add('hidden');
    }

    function showMissingImagesNotice(files) {
        const { missingNotice, missingTitle, missingList } = getElements();
        if (!missingNotice || !missingList || !missingTitle) return;

        missingList.innerHTML = '';
        files.forEach((file) => {
            const li = document.createElement('li');
            li.textContent = file;
            missingList.appendChild(li);
        });

        const count = files.length;
        missingTitle.textContent = count === 1 ? '1 missing image for preview' : `${count} missing images for preview`;
        missingNotice.classList.remove('hidden');
    }

    async function refreshPreview() {
        const markdownText = getMarkdownText();
        if (!markdownText) {
            setStatus('Upload a Markdown file before previewing.', true);
            return;
        }

        if (isRefreshing) {
            queuedRefresh = true;
            setStatus('Changes detected. Refresh queued...');
            return;
        }

        const currentRevision = typeof getPreviewRevision === 'function' ? getPreviewRevision() : 0;
        if (lastRenderedRevision === currentRevision) {
            setStatus('Preview already up to date.');
            return;
        }

        isRefreshing = true;

        const outputFormat = getSelectedOutputFormat();
        const previewEngine = getPreviewEngine(outputFormat);

        const { frame, canvas } = getElements();
        if (!frame || !canvas) {
            isRefreshing = false;
            return;
        }

        setPreviewMode(previewEngine);
        if (previewEngine === 'pdf') {
            updatePageSize();
        }
        setBusy(true);
        setStage(1, 'Preparing preview...');
        setProgress(8);

        let progressInterval = null;
        const startProgressSimulation = () => {
            let current = 8;
            progressInterval = window.setInterval(() => {
                current = Math.min(92, current + (current < 60 ? 8 : 3));
                setProgress(current);
            }, 260);
        };
        const stopProgressSimulation = () => {
            if (progressInterval) {
                window.clearInterval(progressInterval);
                progressInterval = null;
            }
        };

        if (previewAbortController) {
            previewAbortController.abort();
        }
        previewAbortController = new AbortController();

        startProgressSimulation();

        try {
            setStage(2, 'Applying style, export and layout options...');
            setProgress(30);

            const formData = buildFormData();
            formData.append('preview_engine', previewEngine);

            setStage(3, previewEngine === 'html' ? 'Rendering HTML on server...' : 'Rendering PDF on server...');
            setProgress(45);

            const response = await previewApi.requestPreview(formData, {
                signal: previewAbortController.signal,
            });
            setProgress(78);

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
                    stopProgressSimulation();
                    setProgress(100);
                    showMissingImagesNotice(missingFiles);
                    const count = missingFiles.length;
                    setStatus(
                        count === 1
                            ? 'Preview blocked: 1 missing image. Add it and retry.'
                            : `Preview blocked: ${count} missing images. Add them and retry.`,
                        true
                    );
                    return;
                }

                const message = errorData?.detail || `Preview failed (${response.status})`;
                throw new Error(message);
            }

            clearMissingImagesNotice();

            if (previewEngine === 'html') {
                let payload = null;
                try {
                    payload = await response.json();
                } catch {
                }

                const htmlContent = String(payload?.html || '').trim();
                if (!htmlContent) {
                    throw new Error('No HTML preview generated');
                }

                setStage(4, 'Loading HTML into preview pane...');
                setProgress(92);

                if (lastPreviewBlobUrl) {
                    URL.revokeObjectURL(lastPreviewBlobUrl);
                    lastPreviewBlobUrl = null;
                }
                await loadHtmlIntoFrame(frame, htmlContent);
                canvas.classList.add('has-content');
            } else {
                const pdfBlob = await response.blob();

                setStage(4, 'Loading PDF into preview pane...');
                setProgress(92);

                if (lastPreviewBlobUrl) {
                    URL.revokeObjectURL(lastPreviewBlobUrl);
                }
                const blobUrl = URL.createObjectURL(pdfBlob);
                lastPreviewBlobUrl = blobUrl;

                frame.removeAttribute('srcdoc');
                frame.src = blobUrl;
                frame.style.height = getPdfPreviewHeight();
                canvas.classList.add('has-content');
            }

            lastRenderedRevision = currentRevision;
            stopProgressSimulation();
            setProgress(100);
            setStatus('Preview ready.');
        } catch (error) {
            if (error?.name === 'AbortError') {
                return;
            }
            stopProgressSimulation();
            setProgress(100);
            setStatus(error?.message || 'Preview failed', true);
        } finally {
            if (delayedHideTimer) {
                window.clearTimeout(delayedHideTimer);
            }
            delayedHideTimer = window.setTimeout(() => {
                hideProgress();
                delayedHideTimer = 0;
            }, 250);
            isRefreshing = false;
            setBusy(false);

            if (queuedRefresh) {
                queuedRefresh = false;
                refreshPreview();
            }
        }
    }

    function setupPreviewTriggers() {
        const onTabChange = (event) => {
            const tab = event?.detail?.tab || '';
            activeTab = tab;
            if (tab === 'preview') {
                refreshPreview();
            }
        };
        document.addEventListener('nectar:tab-change', onTabChange);
        cleanups.push(() => document.removeEventListener('nectar:tab-change', onTabChange));

        const onPresetChange = () => {
            if (activeTab === 'preview') refreshPreview();
        };
        document.addEventListener('nectar:preset-change', onPresetChange);
        cleanups.push(() => document.removeEventListener('nectar:preset-change', onPresetChange));

        const onAssetsChange = () => {
            if (activeTab === 'preview') refreshPreview();
        };
        document.addEventListener('nectar:assets-change', onAssetsChange);
        cleanups.push(() => document.removeEventListener('nectar:assets-change', onAssetsChange));

        const onImageModeChange = () => {
            if (activeTab === 'preview') refreshPreview();
        };
        document.addEventListener('nectar:image-mode-change', onImageModeChange);
        cleanups.push(() => document.removeEventListener('nectar:image-mode-change', onImageModeChange));

        const onPageSizeChange = () => {
            updatePageSize();
            if (activeTab === 'preview') refreshPreview();
        };
        const pageSizeSelect = byId('pageSize');
        pageSizeSelect?.addEventListener('change', onPageSizeChange);
        if (pageSizeSelect) {
            cleanups.push(() => pageSizeSelect.removeEventListener('change', onPageSizeChange));
        }
    }

    function init() {
        const { refreshBtn, addMissingBtn } = getElements();
        if (!refreshBtn) return;

        setPreviewMode(getPreviewEngine(getSelectedOutputFormat()));

        const onRefreshClick = () => {
            refreshPreview();
        };
        refreshBtn.addEventListener('click', onRefreshClick);
        cleanups.push(() => refreshBtn.removeEventListener('click', onRefreshClick));

        const onAddMissingClick = () => {
            if (typeof onRequestAddMissing === 'function') {
                onRequestAddMissing();
                return;
            }
            byId('imageFilesInput')?.click();
        };
        addMissingBtn?.addEventListener('click', onAddMissingClick);
        if (addMissingBtn) {
            cleanups.push(() => addMissingBtn.removeEventListener('click', onAddMissingClick));
        }

        setupPreviewTriggers();
        updatePageSize();
    }

    function destroy() {
        if (previewAbortController) {
            previewAbortController.abort();
            previewAbortController = null;
        }
        if (delayedHideTimer) {
            window.clearTimeout(delayedHideTimer);
            delayedHideTimer = 0;
        }
        if (lastPreviewBlobUrl) {
            URL.revokeObjectURL(lastPreviewBlobUrl);
            lastPreviewBlobUrl = null;
        }
        const { frame } = getElements();
        if (frame) {
            frame.setAttribute('sandbox', 'allow-same-origin');
            frame.removeAttribute('srcdoc');
            frame.src = 'about:blank';
        }
        cleanups.splice(0).forEach((cleanup) => cleanup());
    }

    function getCurrentPreviewBlobUrl() {
        return lastPreviewBlobUrl;
    }

    function clearCurrentPreviewBlobUrl() {
        lastPreviewBlobUrl = null;
    }

    return {
        init,
        destroy,
        refreshPreview,
        getCurrentPreviewBlobUrl,
        clearCurrentPreviewBlobUrl,
        invalidatePreviewCache: () => {
            lastRenderedRevision = -1;
        },
    };
}
