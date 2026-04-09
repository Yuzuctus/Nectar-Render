import { countWords, formatFileSize } from '../platform/dom-utils.js';

export function createPanelContextFeature({ sessionStore }) {
    let draft = null;

    async function loadDraftOrRedirect() {
        draft = await sessionStore.getDraft();
        if (!draft) {
            window.location.href = 'index.html';
            return null;
        }
        return draft;
    }

    function applyDraftMetadata() {
        if (!draft) return;
        const panelMetaName = document.getElementById('panelMetaFileName');
        const panelMetaSize = document.getElementById('panelMetaFileSize');
        const panelMetaWords = document.getElementById('panelMetaWordCount');

        if (panelMetaName) panelMetaName.textContent = draft.fileName;
        if (panelMetaSize) panelMetaSize.textContent = formatFileSize(draft.fileSize || 0);
        if (panelMetaWords) panelMetaWords.textContent = `${countWords(draft.markdownText)} words`;
    }

    function clearDraft() {
        return sessionStore.clearDraft();
    }

    function getMarkdownText() {
        return String(draft?.markdownText || '');
    }

    function getSelectedFileName() {
        return String(draft?.fileName || 'output.md');
    }

    return {
        loadDraftOrRedirect,
        applyDraftMetadata,
        clearDraft,
        getMarkdownText,
        getSelectedFileName,
    };
}
