import { createSessionStore } from '../state/session-store.js';
import { createLandingUploadFeature } from '../features/landing-upload.js';
import { createThemeToggleFeature } from '../features/theme-toggle.js';
import { createToastFeature } from '../features/toast.js';

export function initHomePage() {
    const sessionStore = createSessionStore();
    const toast = createToastFeature();
    const theme = createThemeToggleFeature();
    const landingUpload = createLandingUploadFeature({
        toast,
        sessionStore,
    });

    theme.init();
    landingUpload.init();
}
