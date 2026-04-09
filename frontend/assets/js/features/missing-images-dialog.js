import { byId } from '../platform/dom-utils.js';

export function createMissingImagesDialogFeature() {
    const dialog = byId('missingImagesDialog');
    const list = byId('missingImagesList');
    const continueBtn = byId('continueWithoutImages');
    const addBtn = byId('addMissingImages');

    function setList(files) {
        if (!list) return;
        list.innerHTML = '';
        files.forEach((file) => {
            const li = document.createElement('li');
            li.textContent = file;
            list.appendChild(li);
        });
    }

    function prompt(files) {
        return new Promise((resolve) => {
            if (!dialog) {
                resolve('continue');
                return;
            }

            setList(files || []);

            const cleanup = () => {
                continueBtn?.removeEventListener('click', onContinue);
                addBtn?.removeEventListener('click', onAdd);
                dialog.removeEventListener('cancel', onCancel);
            };

            const onContinue = () => {
                cleanup();
                dialog.close();
                resolve('continue');
            };

            const onAdd = () => {
                cleanup();
                dialog.close();
                resolve('add');
            };

            const onCancel = (event) => {
                event.preventDefault();
                cleanup();
                dialog.close();
                resolve('continue');
            };

            continueBtn?.addEventListener('click', onContinue);
            addBtn?.addEventListener('click', onAdd);
            dialog.addEventListener('cancel', onCancel);
            dialog.showModal();
        });
    }

    return {
        prompt,
    };
}
