function dedupeImageFiles(files) {
    const seen = new Set();
    const deduped = [];
    files.forEach((file) => {
        const key = `${file.name}::${file.size}::${file.lastModified}`;
        if (seen.has(key)) return;
        seen.add(key);
        deduped.push(file);
    });
    return deduped;
}

function isAcceptedImageFile(file) {
    if (!file) return false;
    if (file.type && file.type.startsWith('image/')) return true;
    const name = String(file.name || '').toLowerCase();
    return /\.(apng|avif|bmp|gif|jpe?g|png|svg|tif|tiff|webp)$/.test(name);
}

export function createImageAssetsFeature({ toast, onAssetsChange }) {
    const sidebarImageDrop = document.getElementById('sidebarImageDrop');
    const imageFilesInput = document.getElementById('imageFilesInput');
    const imageFileList = document.getElementById('imageFileList');
    const sidebarImagesSection = document.getElementById('sidebarImagesSection');

    let imageFiles = [];
    const cleanups = [];

    function emitChange() {
        if (typeof onAssetsChange === 'function') {
            onAssetsChange([...imageFiles]);
        }
        document.dispatchEvent(new CustomEvent('nectar:assets-change'));
    }

    function renderImageFileList() {
        if (!imageFileList) return;
        imageFileList.innerHTML = '';
        imageFiles.forEach((file, index) => {
            const item = document.createElement('div');
            item.className = 'image-file-item';

            const nameSpan = document.createElement('span');
            nameSpan.textContent = file.name;
            nameSpan.title = file.name;

            const removeButton = document.createElement('button');
            removeButton.type = 'button';
            removeButton.setAttribute('aria-label', `Remove ${file.name}`);
            removeButton.textContent = 'x';
            const onRemoveClick = () => {
                imageFiles.splice(index, 1);
                renderImageFileList();
                emitChange();
            };
            removeButton.addEventListener('click', onRemoveClick, { once: true });

            item.appendChild(nameSpan);
            item.appendChild(removeButton);
            imageFileList.appendChild(item);
        });
    }

    function updateVisibility() {
        const mode = document.querySelector('input[name="imageMode"]:checked')?.value;
        if (!sidebarImagesSection) return;
        sidebarImagesSection.style.display = '';
        sidebarImagesSection.classList.toggle('hidden', mode !== 'WITH_IMAGES');
    }

    function importFiles(files) {
        const beforeCount = imageFiles.length;
        Array.from(files || []).forEach((file) => {
            if (isAcceptedImageFile(file)) {
                imageFiles.push(file);
                return;
            }
            toast?.showToast?.(`Ignored unsupported file: ${file.name}`, 'error');
        });
        imageFiles = dedupeImageFiles(imageFiles);
        renderImageFileList();
        if (imageFiles.length !== beforeCount) {
            emitChange();
        }
    }

    function bindDropZone() {
        if (!sidebarImageDrop) return;

        const onDropClick = (event) => {
            const target = event.target;
            if (target instanceof Element && target.closest('.file-btn')) {
                return;
            }
            imageFilesInput?.click();
        };
        sidebarImageDrop.addEventListener('click', onDropClick);
        cleanups.push(() => sidebarImageDrop.removeEventListener('click', onDropClick));

        const onDragOver = (event) => {
            event.preventDefault();
            sidebarImageDrop.classList.add('dragover');
        };
        sidebarImageDrop.addEventListener('dragover', onDragOver);
        cleanups.push(() => sidebarImageDrop.removeEventListener('dragover', onDragOver));

        const onDragLeave = () => {
            sidebarImageDrop.classList.remove('dragover');
        };
        sidebarImageDrop.addEventListener('dragleave', onDragLeave);
        cleanups.push(() => sidebarImageDrop.removeEventListener('dragleave', onDragLeave));

        const onDrop = (event) => {
            event.preventDefault();
            sidebarImageDrop.classList.remove('dragover');
            importFiles(event.dataTransfer?.files || []);
        };
        sidebarImageDrop.addEventListener('drop', onDrop);
        cleanups.push(() => sidebarImageDrop.removeEventListener('drop', onDrop));

        const onKeyDown = (event) => {
            if (event.key === 'Enter' || event.key === ' ') {
                event.preventDefault();
                imageFilesInput?.click();
            }
        };
        sidebarImageDrop.addEventListener('keydown', onKeyDown);
        cleanups.push(() => sidebarImageDrop.removeEventListener('keydown', onKeyDown));
    }

    function init() {
        bindDropZone();
        updateVisibility();

        const onInputChange = () => {
            importFiles(imageFilesInput.files || []);
            imageFilesInput.value = '';
        };
        imageFilesInput?.addEventListener('change', onInputChange);
        if (imageFilesInput) {
            cleanups.push(() => imageFilesInput.removeEventListener('change', onInputChange));
        }

        const onDocumentChange = (event) => {
            if (event.target?.name === 'imageMode') {
                updateVisibility();
                document.dispatchEvent(new CustomEvent('nectar:image-mode-change', {
                    detail: { imageMode: event.target.value || '' },
                }));
            }
        };
        document.addEventListener('change', onDocumentChange);
        cleanups.push(() => document.removeEventListener('change', onDocumentChange));
    }

    function clear() {
        imageFiles = [];
        renderImageFileList();
        emitChange();
    }

    function focusDropZone() {
        if (!sidebarImageDrop) return;
        sidebarImageDrop.scrollIntoView({ behavior: 'smooth', block: 'center' });
        sidebarImageDrop.classList.add('pulse');
        setTimeout(() => sidebarImageDrop.classList.remove('pulse'), 2000);
    }

    function destroy() {
        cleanups.splice(0).forEach((cleanup) => cleanup());
    }

    return {
        init,
        clear,
        destroy,
        getImageFiles: () => [...imageFiles],
        updateVisibility,
        openPicker: () => imageFilesInput?.click(),
        focusDropZone,
    };
}
