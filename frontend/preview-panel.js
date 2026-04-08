(function () {
    let activeTab = 'export';
    let isRefreshing = false;
    let queuedRefresh = false;
    const PREVIEW_STAGE_TOTAL = 5;
    let lastRenderedRevision = -1;

    function getElements() {
        return {
            refreshBtn: document.getElementById('previewRefreshBtn'),
            status: document.getElementById('previewStatus'),
            frame: document.getElementById('previewFrame'),
            canvas: document.getElementById('previewCanvas'),
            sheet: document.getElementById('previewSheet'),
            progress: document.getElementById('previewProgress'),
            progressBar: document.getElementById('previewProgressBar'),
            missingNotice: document.getElementById('previewMissingNotice'),
            missingTitle: document.getElementById('previewMissingTitle'),
            missingList: document.getElementById('previewMissingList'),
            addMissingBtn: document.getElementById('previewAddMissingBtn'),
        };
    }

    function setStatus(text, isError = false) {
        const { status } = getElements();
        if (!status) return;
        status.textContent = text;
        status.style.color = isError ? 'var(--danger)' : 'var(--text-muted)';
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
        const pageSize = document.getElementById('pageSize')?.value || 'A4';
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
        missingTitle.textContent =
            count === 1
                ? '1 missing image for preview'
                : `${count} missing images for preview`;
        missingNotice.classList.remove('hidden');
    }

    function getUploadedAssetNames() {
        const bridge = window.NectarUI;
        if (!bridge || !Array.isArray(bridge.imageFiles)) {
            return new Set();
        }
        return new Set(
            bridge.imageFiles
                .map((file) => String(file?.name || '').trim().toLowerCase())
                .filter(Boolean)
        );
    }

    async function estimateMissingBeforePreview() {
        const bridge = window.NectarUI;
        if (!bridge || !bridge.selectedMarkdownText) {
            return [];
        }
        if (typeof bridge.ensureApiBase === 'function') {
            await bridge.ensureApiBase();
        }

        const markdownBlob = new Blob([bridge.selectedMarkdownText], { type: 'text/markdown' });
        const payload = new FormData();
        payload.append('file', markdownBlob, 'preview.md');

        const response = await fetch(`${bridge.API_BASE}/analyze/`, {
            method: 'POST',
            body: payload,
        });
        if (!response.ok) {
            return [];
        }

        const data = await response.json();
        const refs = Array.isArray(data?.missing_images) ? data.missing_images : [];
        if (!refs.length) {
            return [];
        }

        const uploadedNames = getUploadedAssetNames();
        const unresolved = refs.filter((ref) => {
            const normalized = String(ref || '').split(/[\\/]/).pop()?.toLowerCase();
            if (!normalized) return false;
            return !uploadedNames.has(normalized);
        });

        return Array.from(new Set(unresolved));
    }

    async function refreshPreview() {
        const bridge = window.NectarUI;
        if (!bridge || !bridge.selectedMarkdownText) {
            setStatus('Upload a Markdown file before previewing.', true);
            return;
        }

        if (isRefreshing) {
            queuedRefresh = true;
            setStatus('Changes detected. Refresh queued...');
            return;
        }

        const currentRevision = typeof bridge.getPreviewRevision === 'function'
            ? bridge.getPreviewRevision()
            : 0;
        if (lastRenderedRevision === currentRevision) {
            setStatus('Preview already up to date.');
            return;
        }

        isRefreshing = true;

        const { frame, canvas } = getElements();
        if (!frame || !canvas) {
            isRefreshing = false;
            return;
        }

        updatePageSize();
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

        startProgressSimulation();

        try {
            setStage(2, 'Checking referenced images...');
            const estimatedMissing = await estimateMissingBeforePreview();
            if (estimatedMissing.length > 0) {
                stopProgressSimulation();
                setProgress(100);
                showMissingImagesNotice(estimatedMissing);
                const count = estimatedMissing.length;
                setStatus(
                    count === 1
                        ? 'Preview blocked: 1 missing image. Add it and retry.'
                        : `Preview blocked: ${count} missing images. Add them and retry.`,
                    true
                );
                return;
            }

            setStage(3, 'Applying style, export and layout options...');
            setProgress(30);

            const formData = bridge.buildFormData();
            formData.append('preview_engine', 'pdf');
            if (typeof bridge.ensureApiBase === 'function') {
                await bridge.ensureApiBase();
            }

            setStage(4, 'Rendering PDF on server...');
            setProgress(45);

            const response = await fetch(`${bridge.API_BASE}/preview`, {
                method: 'POST',
                body: formData,
            });

            setProgress(78);

            if (!response.ok) {
                let errData = null;
                try {
                    errData = await response.json();
                } catch {}

                const missingFiles = Array.isArray(errData?.missing_images)
                    ? errData.missing_images
                    : (errData?.detail?.detail === 'missing_images' && Array.isArray(errData?.detail?.files)
                        ? errData.detail.files
                        : null);

                if (response.status === 422 && missingFiles && missingFiles.length) {
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

                const msg = errData?.detail?.detail || errData?.detail || `Preview failed (${response.status})`;
                throw new Error(msg);
            }

            clearMissingImagesNotice();
            const pdfBlob = await response.blob();
            setStage(5, 'Loading PDF into preview pane...');
            setProgress(92);
            if (window._lastPreviewBlobUrl) {
                URL.revokeObjectURL(window._lastPreviewBlobUrl);
            }
            const blobUrl = URL.createObjectURL(pdfBlob);
            window._lastPreviewBlobUrl = blobUrl;
            frame.src = blobUrl;
            frame.style.height = '72vh';
            canvas.classList.add('has-content');
            lastRenderedRevision = currentRevision;
            stopProgressSimulation();
            setProgress(100);
            setStatus('Preview ready.');
        } catch (error) {
            stopProgressSimulation();
            setProgress(100);
            setStatus(error?.message || 'Preview failed', true);
        } finally {
            setTimeout(() => {
                hideProgress();
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
        document.addEventListener('nectar:tab-change', (event) => {
            const tab = event?.detail?.tab || '';
            activeTab = tab;
            if (tab === 'preview') {
                refreshPreview();
            }
        });

        document.addEventListener('nectar:preset-change', () => {
            if (activeTab === 'preview') {
                refreshPreview();
            }
        });

        document.addEventListener('nectar:assets-change', () => {
            if (activeTab === 'preview') {
                refreshPreview();
            }
        });

        document.addEventListener('nectar:image-mode-change', () => {
            if (activeTab === 'preview') {
                refreshPreview();
            }
        });

        document.getElementById('pageSize')?.addEventListener('change', () => {
            updatePageSize();
            if (activeTab === 'preview') {
                refreshPreview();
            }
        });
    }

    function initPreviewPanel() {
        const { refreshBtn, addMissingBtn } = getElements();
        if (!refreshBtn) return;

        refreshBtn.addEventListener('click', () => {
            refreshPreview();
        });

        addMissingBtn?.addEventListener('click', () => {
            const imageInput = document.getElementById('imageFilesInput');
            if (imageInput) {
                imageInput.click();
            }
        });

        setupPreviewTriggers();
        updatePageSize();
    }

    window.NectarPreview = {
        initPreviewPanel,
        refreshPreview,
        invalidatePreviewCache: () => {
            lastRenderedRevision = -1;
        },
    };
})();
