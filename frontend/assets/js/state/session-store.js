const KEY = 'nectar.frontend.draft.v1';
const MAX_MARKDOWN_INLINE_BYTES = 350_000;
const DB_NAME = 'nectar-render-db';
const STORE_NAME = 'drafts';

function canUseSessionStorage() {
    try {
        const probe = '__nectar_probe__';
        sessionStorage.setItem(probe, '1');
        sessionStorage.removeItem(probe);
        return true;
    } catch {
        return false;
    }
}

function readRawSessionPayload() {
    try {
        const raw = sessionStorage.getItem(KEY);
        if (!raw) return null;
        return JSON.parse(raw);
    } catch {
        return null;
    }
}

function utf8ByteLength(text) {
    return new TextEncoder().encode(String(text || '')).length;
}

function openDb() {
    return new Promise((resolve, reject) => {
        if (!('indexedDB' in window)) {
            reject(new Error('IndexedDB unavailable'));
            return;
        }
        const request = indexedDB.open(DB_NAME, 1);
        request.onupgradeneeded = () => {
            const db = request.result;
            if (!db.objectStoreNames.contains(STORE_NAME)) {
                db.createObjectStore(STORE_NAME);
            }
        };
        request.onsuccess = () => resolve(request.result);
        request.onerror = () => reject(request.error || new Error('Failed to open IndexedDB'));
    });
}

async function idbSet(key, value) {
    const db = await openDb();
    try {
        await new Promise((resolve, reject) => {
            const tx = db.transaction(STORE_NAME, 'readwrite');
            tx.objectStore(STORE_NAME).put(value, key);
            tx.oncomplete = () => resolve();
            tx.onerror = () => reject(tx.error || new Error('IndexedDB write failed'));
        });
    } finally {
        db.close();
    }
}

async function idbGet(key) {
    const db = await openDb();
    try {
        return await new Promise((resolve, reject) => {
            const tx = db.transaction(STORE_NAME, 'readonly');
            const req = tx.objectStore(STORE_NAME).get(key);
            req.onsuccess = () => resolve(req.result);
            req.onerror = () => reject(req.error || new Error('IndexedDB read failed'));
        });
    } finally {
        db.close();
    }
}

async function idbDelete(key) {
    const db = await openDb();
    try {
        await new Promise((resolve, reject) => {
            const tx = db.transaction(STORE_NAME, 'readwrite');
            tx.objectStore(STORE_NAME).delete(key);
            tx.oncomplete = () => resolve();
            tx.onerror = () => reject(tx.error || new Error('IndexedDB delete failed'));
        });
    } finally {
        db.close();
    }
}

export function createSessionStore() {
    const enabled = canUseSessionStorage();

    async function saveDraft({ fileName, fileSize, markdownText }) {
        if (!enabled) return false;

        const previous = readRawSessionPayload();
        const previousIdbKey = previous?.markdownIdbKey ? String(previous.markdownIdbKey) : '';

        const safeFileName = String(fileName || 'document.md');
        const safeFileSize = Number(fileSize || 0);
        const safeMarkdown = String(markdownText || '');
        const bytes = utf8ByteLength(safeMarkdown);

        const payload = {
            fileName: safeFileName,
            fileSize: safeFileSize,
            markdownTextInline: '',
            markdownIdbKey: '',
            createdAt: Date.now(),
        };

        if (bytes <= MAX_MARKDOWN_INLINE_BYTES) {
            payload.markdownTextInline = safeMarkdown;
        } else {
            const idbKey = `md:${Date.now()}:${Math.random().toString(36).slice(2, 8)}`;
            try {
                await idbSet(idbKey, safeMarkdown);
                payload.markdownIdbKey = idbKey;
            } catch {
                return false;
            }
        }

        try {
            sessionStorage.setItem(KEY, JSON.stringify(payload));
            if (previousIdbKey && previousIdbKey !== payload.markdownIdbKey) {
                try {
                    await idbDelete(previousIdbKey);
                } catch {
                }
            }
            return true;
        } catch {
            if (payload.markdownIdbKey) {
                try {
                    await idbDelete(payload.markdownIdbKey);
                } catch {
                }
            }
            return false;
        }
    }

    async function getDraft() {
        if (!enabled) return null;

        let raw;
        try {
            raw = sessionStorage.getItem(KEY);
        } catch {
            return null;
        }
        if (!raw) return null;

        let data;
        try {
            data = JSON.parse(raw);
        } catch {
            return null;
        }

        let markdownText = String(data.markdownTextInline || '');
        if (!markdownText && data.markdownIdbKey) {
            try {
                const loaded = await idbGet(data.markdownIdbKey);
                markdownText = String(loaded || '');
            } catch {
                markdownText = '';
            }
        }

        if (!markdownText) return null;

        return {
            fileName: String(data.fileName || 'document.md'),
            fileSize: Number(data.fileSize || 0),
            markdownText,
            markdownIdbKey: String(data.markdownIdbKey || ''),
            createdAt: Number(data.createdAt || 0),
        };
    }

    async function clearDraft() {
        if (!enabled) return;
        const payload = readRawSessionPayload();
        if (payload?.markdownIdbKey) {
            try {
                await idbDelete(payload.markdownIdbKey);
            } catch {
            }
        }
        try {
            sessionStorage.removeItem(KEY);
        } catch {
        }
    }

    return {
        saveDraft,
        getDraft,
        clearDraft,
    };
}
