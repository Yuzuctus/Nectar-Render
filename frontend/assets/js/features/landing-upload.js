import {
    byId,
    countWords,
    formatFileSize,
    hide,
    setText,
    show,
} from '../platform/dom-utils.js';

function isMarkdownFile(file) {
    return file && /\.md$/i.test(String(file.name || ''));
}

export function createLandingUploadFeature({ toast, sessionStore }) {
    const dropZone = byId('dropZone');
    const fileInput = byId('fileInput');
    const fileInfo = byId('fileInfo');
    const continueBtn = byId('continueBtn');

    const fileNameEl = byId('fileName');
    const fileSizeEl = byId('fileSize');
    const wordCountEl = byId('wordCount');

    let draft = null;

    function setReadyState(ready) {
        if (ready) {
            show(fileInfo);
            show(continueBtn);
            if (dropZone) {
                dropZone.classList.add('has-file');
            }
            return;
        }
        hide(fileInfo);
        hide(continueBtn);
        if (dropZone) {
            dropZone.classList.remove('has-file');
        }
    }

    async function persistAndNavigate() {
        if (!draft) {
            toast.showToast('Ajoute un fichier .md avant de continuer.', 'error');
            return;
        }

        const saved = await sessionStore.saveDraft({
            fileName: draft.file.name,
            fileSize: draft.file.size,
            markdownText: draft.markdownText,
        });

        if (!saved) {
            toast.showToast('Impossible de sauvegarder la session locale.', 'error');
            return;
        }

        window.location.href = 'panel.html';
    }

    function updateUi(file, markdownText) {
        setText(fileNameEl, file.name);
        setText(fileSizeEl, formatFileSize(file.size));
        setText(wordCountEl, `${countWords(markdownText)} words`);
        setReadyState(true);
    }

    function processFile(file) {
        if (!isMarkdownFile(file)) {
            toast.showToast('Merci de choisir un fichier .md', 'error');
            return;
        }
        const reader = new FileReader();
        reader.onload = (event) => {
            const markdownText = String(event.target?.result || '');
            draft = { file, markdownText };
            updateUi(file, markdownText);
        };
        reader.onerror = () => {
            toast.showToast('Lecture du fichier impossible.', 'error');
        };
        reader.readAsText(file);
    }

    function bindDropZone() {
        if (!dropZone) return;

        dropZone.addEventListener('dragover', (event) => {
            event.preventDefault();
            dropZone.classList.add('dragover');
        });
        dropZone.addEventListener('dragleave', () => {
            dropZone.classList.remove('dragover');
        });
        dropZone.addEventListener('drop', (event) => {
            event.preventDefault();
            dropZone.classList.remove('dragover');
            const file = event.dataTransfer?.files?.[0];
            if (file) processFile(file);
        });
        dropZone.addEventListener('click', (event) => {
            const target = event.target;
            if (target instanceof Element && target.closest('.file-btn')) {
                return;
            }
            fileInput?.click();
        });
        dropZone.addEventListener('keydown', (event) => {
            if (event.key === 'Enter' || event.key === ' ') {
                event.preventDefault();
                fileInput?.click();
            }
        });
    }

    function init() {
        setReadyState(false);
        bindDropZone();

        fileInput?.addEventListener('change', () => {
            const file = fileInput.files?.[0];
            if (file) processFile(file);
        });

        continueBtn?.addEventListener('click', () => {
            persistAndNavigate();
        });
    }

    return {
        init,
    };
}
