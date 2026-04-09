import { byId } from '../platform/dom-utils.js';

export function createToastFeature() {
    const toast = byId('toast');

    function showToast(message, type = 'info') {
        if (!toast) return;
        toast.textContent = message;
        toast.className = `toast ${type}`;
        toast.classList.remove('hidden');
        clearTimeout(toast._timer);
        toast._timer = setTimeout(() => toast.classList.add('hidden'), 4000);
    }

    return {
        showToast,
    };
}
