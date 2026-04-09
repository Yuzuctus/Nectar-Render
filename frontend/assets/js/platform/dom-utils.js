export function byId(id) {
    return document.getElementById(id);
}

export function countWords(text) {
    return String(text || '').trim().split(/\s+/).filter(Boolean).length;
}

export function formatFileSize(bytes) {
    const safeBytes = Number(bytes || 0);
    if (safeBytes < 1024) return `${safeBytes} B`;
    if (safeBytes < 1024 * 1024) return `${(safeBytes / 1024).toFixed(1)} KB`;
    return `${(safeBytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function bySelector(selector) {
    return document.querySelector(selector);
}

export function allBySelector(selector) {
    return Array.from(document.querySelectorAll(selector));
}

export function show(el) {
    if (!el) return;
    el.classList.remove('hidden');
}

export function hide(el) {
    if (!el) return;
    el.classList.add('hidden');
}

export function setText(el, text) {
    if (!el) return;
    el.textContent = text;
}

export function emit(eventName, detail = {}) {
    document.dispatchEvent(new CustomEvent(eventName, { detail }));
}

export function on(eventTarget, eventName, handler, options) {
    if (!eventTarget) return;
    eventTarget.addEventListener(eventName, handler, options);
}
