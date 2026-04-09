import { allBySelector, bySelector } from '../platform/dom-utils.js';

export function createTabsFeature() {
    let activeTab = 'export';
    let cleanup = null;

    function getActiveTab() {
        return activeTab;
    }

    function setTab(tab) {
        activeTab = tab;
        allBySelector('.tab-btn').forEach((btn) => {
            btn.classList.toggle('active', btn.dataset.tab === tab);
        });
        allBySelector('.tab-panel').forEach((panel) => {
            panel.classList.toggle('active', panel.dataset.tab === tab);
        });
        document.dispatchEvent(new CustomEvent('nectar:tab-change', {
            detail: { tab },
        }));
    }

    function init() {
        const tabsNav = bySelector('.tabs-nav');
        if (!tabsNav) return;
        const onClick = (event) => {
            const tabBtn = event.target.closest('.tab-btn');
            if (!tabBtn) return;
            const tab = tabBtn.dataset.tab || 'export';
            setTab(tab);
        };
        tabsNav.addEventListener('click', onClick);
        cleanup = () => tabsNav.removeEventListener('click', onClick);
    }

    function destroy() {
        if (typeof cleanup === 'function') {
            cleanup();
            cleanup = null;
        }
    }

    return {
        init,
        destroy,
        setTab,
        getActiveTab,
    };
}
