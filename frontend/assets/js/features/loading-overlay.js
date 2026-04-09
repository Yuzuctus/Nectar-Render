import { byId, hide, show, setText } from '../platform/dom-utils.js';

export function createLoadingOverlayFeature() {
    const loadingOverlay = byId('loadingOverlay');
    const loadingText = byId('loadingText');
    let hideTimer = 0;

    function showOverlay(format) {
        const formatText = {
            PDF: 'Generation du PDF...',
            HTML: 'Generation du HTML...',
            'PDF+HTML': 'Generation du ZIP...',
        };
        setText(loadingText, formatText[format] || 'Conversion en cours...');
        if (!loadingOverlay) return;
        if (hideTimer) {
            window.clearTimeout(hideTimer);
            hideTimer = 0;
        }
        loadingOverlay.classList.remove('fade-out');
        show(loadingOverlay);
    }

    function hideOverlay(immediate = false) {
        if (!loadingOverlay) return;
        if (immediate) {
            if (hideTimer) {
                window.clearTimeout(hideTimer);
                hideTimer = 0;
            }
            hide(loadingOverlay);
            return;
        }
        loadingOverlay.classList.add('fade-out');
        if (hideTimer) {
            window.clearTimeout(hideTimer);
        }
        hideTimer = window.setTimeout(() => {
            hide(loadingOverlay);
            hideTimer = 0;
        }, 280);
    }

    return {
        showOverlay,
        hideOverlay,
    };
}
