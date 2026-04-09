import { createSessionStore } from '../state/session-store.js';
import { createBackendHealthService } from '../services/backend-health.js';
import { createConversionApi } from '../services/conversion-api.js';
import { createPreviewApi } from '../services/preview-api.js';

import { createThemeToggleFeature } from '../features/theme-toggle.js';
import { createToastFeature } from '../features/toast.js';
import { createTabsFeature } from '../features/tabs.js';
import { createPresetsFormFeature } from '../features/presets-form.js';
import { createImageAssetsFeature } from '../features/image-assets.js';
import { createMissingImagesDialogFeature } from '../features/missing-images-dialog.js';
import { createLoadingOverlayFeature } from '../features/loading-overlay.js';
import { createPanelContextFeature } from '../features/panel-context.js';
import { createConversionFlowFeature } from '../features/conversion-flow.js';
import { createPreviewPanelFeature } from '../features/preview-panel.js';

export async function initPanelPage() {
    const sessionStore = createSessionStore();
    const backendHealth = createBackendHealthService();
    const conversionApi = createConversionApi({ backendHealth });
    const previewApi = createPreviewApi({ backendHealth });

    const theme = createThemeToggleFeature();
    const toast = createToastFeature();
    const tabs = createTabsFeature();
    const loadingOverlay = createLoadingOverlayFeature();
    const missingImagesDialog = createMissingImagesDialogFeature();
    const panelContext = createPanelContextFeature({ sessionStore });

    let previewRevision = 0;
    const markPreviewDirty = () => {
        previewRevision += 1;
    };

    const presetsForm = createPresetsFormFeature({
        onChange: markPreviewDirty,
        backendHealth,
    });

    const imageAssets = createImageAssetsFeature({
        toast,
        onAssetsChange: () => {
            markPreviewDirty();
        },
    });

    const draft = await panelContext.loadDraftOrRedirect();
    if (!draft) return;

    panelContext.applyDraftMetadata();

    /** @type {ReturnType<typeof createConversionFlowFeature> | null} */
    let conversionFlow = null;
    const previewPanel = createPreviewPanelFeature({
        previewApi,
        buildFormData: (overrideImageMode = null) => {
            if (conversionFlow) {
                return conversionFlow.createFormData(overrideImageMode);
            }
            const fallback = new FormData();
            fallback.append('markdown_text', panelContext.getMarkdownText());
            return fallback;
        },
        getMarkdownText: () => conversionFlow?.getMarkdownText() || '',
        getImageFiles: () => conversionFlow?.getImageFiles() || [],
        getPreviewRevision: () => previewRevision,
        onRequestAddMissing: () => {
            imageAssets.focusDropZone();
            imageAssets.openPicker();
        },
    });

    conversionFlow = createConversionFlowFeature({
        panelContext,
        presetsForm,
        imageAssets,
        conversionApi,
        backendHealth,
        loadingOverlay,
        toast,
        missingImagesDialog,
        getPreviewBlobUrl: previewPanel.getCurrentPreviewBlobUrl,
        clearPreviewBlobUrl: previewPanel.clearCurrentPreviewBlobUrl,
        onPreviewDirty: markPreviewDirty,
        onResult: ({ filename }) => {
            const message = document.getElementById('resultMessage');
            if (message) {
                message.textContent = `Your file is ready: ${filename}`;
            }
        },
    });

    theme.init();
    tabs.init();
    await presetsForm.init();
    imageAssets.init();
    conversionFlow.init();
    previewPanel.init();

    const cleanupPresetActions = setupPresetActions(presetsForm, toast);

    const onBackToHome = async (event) => {
        event.preventDefault();
        await panelContext.clearDraft();
        window.location.href = 'index.html';
    };
    document.getElementById('backToHomeBtn')?.addEventListener('click', onBackToHome);

    const onBackClick = () => {
        window.location.href = 'index.html';
    };
    document.getElementById('backBtn')?.addEventListener('click', onBackClick);

    const previewDirtyElements = document.querySelectorAll('input[name="outputFormat"], #pageSize');
    previewDirtyElements.forEach((element) => {
        element.addEventListener('change', markPreviewDirty);
    });

    window.addEventListener('beforeunload', () => {
        previewPanel.destroy();
        conversionFlow.destroy();
        imageAssets.destroy();
        tabs.destroy();
        theme.destroy();
        presetsForm.destroy();
        cleanupPresetActions();
        document.getElementById('backToHomeBtn')?.removeEventListener('click', onBackToHome);
        document.getElementById('backBtn')?.removeEventListener('click', onBackClick);
        previewDirtyElements.forEach((element) => {
            element.removeEventListener('change', markPreviewDirty);
        });
    });
}

function setupPresetActions(presetsForm, toast) {
    const saveBtn = document.getElementById('savePresetBtn');
    const deleteBtn = document.getElementById('deletePresetBtn');
    const exportBtn = document.getElementById('exportPresetBtn');
    const importBtn = document.getElementById('importPresetBtn');
    const importFileInput = document.getElementById('importPresetFile');
    const presetSelect = document.getElementById('presetSelect');
    const cleanups = [];

    function updateDeleteButton() {
        if (!deleteBtn || !presetSelect) return;
        const val = presetSelect.value;
        deleteBtn.disabled = !val.startsWith('user:');
    }

    presetSelect?.addEventListener('change', updateDeleteButton);
    if (presetSelect) {
        cleanups.push(() => presetSelect.removeEventListener('change', updateDeleteButton));
    }

    const onSaveClick = () => {
        const name = prompt('Name your preset:');
        if (!name || !name.trim()) return;
        const id = presetsForm.saveCurrentAsUserPreset(name.trim());
        if (id) {
            toast.showToast(`Preset "${name.trim()}" saved.`, 'success');
            updateDeleteButton();
        } else {
            toast.showToast('Could not save preset.', 'error');
        }
    };
    saveBtn?.addEventListener('click', onSaveClick);
    if (saveBtn) {
        cleanups.push(() => saveBtn.removeEventListener('click', onSaveClick));
    }

    const onDeleteClick = () => {
        const val = presetSelect?.value || '';
        if (!val.startsWith('user:')) return;
        const id = val.slice(5);
        let presetName = id;
        try {
            const store = JSON.parse(localStorage.getItem('nectar.frontend.user-presets.v1') || '{}');
            const key = `user:${id}`;
            if (store[key]) presetName = store[key].name || id;
        } catch {}
        const confirmed = confirm(`Delete preset "${presetName}"?`);
        if (!confirmed) return;
        const deleted = presetsForm.deleteUserPresetById(id);
        if (deleted) {
            toast.showToast(`Preset deleted.`, 'success');
            updateDeleteButton();
        }
    };
    deleteBtn?.addEventListener('click', onDeleteClick);
    if (deleteBtn) {
        cleanups.push(() => deleteBtn.removeEventListener('click', onDeleteClick));
    }

    const onExportClick = () => {
        const presetInfo = presetsForm.getSelectedPresetInfo();
        let name = 'My Theme';
        if (presetInfo.type === 'builtin' && presetInfo.name) {
            name = presetInfo.name;
        } else if (presetInfo.type === 'user' && presetInfo.id) {
            try {
                const store = JSON.parse(localStorage.getItem('nectar.frontend.user-presets.v1') || '{}');
                const key = `user:${presetInfo.id}`;
                if (store[key]) name = store[key].name || 'My Theme';
            } catch {}
        }
        const json = presetsForm.exportCurrentAsTheme(name);
        const blob = new Blob([json], { type: 'application/json' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = `${name.replace(/[^a-z0-9]+/gi, '-').toLowerCase()}.json`;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        URL.revokeObjectURL(url);
        toast.showToast(`Theme exported.`, 'success');
    };
    exportBtn?.addEventListener('click', onExportClick);
    if (exportBtn) {
        cleanups.push(() => exportBtn.removeEventListener('click', onExportClick));
    }

    const onImportClick = () => {
        importFileInput?.click();
    };
    importBtn?.addEventListener('click', onImportClick);
    if (importBtn) {
        cleanups.push(() => importBtn.removeEventListener('click', onImportClick));
    }

    const onImportFileChange = () => {
        const file = importFileInput.files?.[0];
        if (!file) return;
        const reader = new FileReader();
        reader.onload = (event) => {
            const text = event.target?.result;
            if (typeof text !== 'string') return;
            const result = presetsForm.importThemeFromFile(text);
            if (result.ok) {
                const id = presetsForm.saveCurrentAsUserPreset(result.name);
                if (id) {
                    toast.showToast(`Theme "${result.name}" imported.`, 'success');
                    presetsForm.refreshPresetSelect();
                    presetSelect.value = `user:${id}`;
                    presetSelect.dispatchEvent(new Event('change'));
                    updateDeleteButton();
                }
            } else {
                toast.showToast(result.error, 'error');
            }
        };
        reader.readAsText(file);
        importFileInput.value = '';
    };
    importFileInput?.addEventListener('change', onImportFileChange);
    if (importFileInput) {
        cleanups.push(() => importFileInput.removeEventListener('change', onImportFileChange));
    }

    return () => {
        cleanups.splice(0).forEach((cleanup) => cleanup());
    };
}
