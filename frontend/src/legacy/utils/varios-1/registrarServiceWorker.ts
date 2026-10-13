/*
 * QK86: Registro del Service Worker de push notifications.
 *
 * Se ejecuta una sola vez al cargar la app. Registra sw-push.js desde
 * la raíz del dominio para tener scope global sobre todas las páginas.
 *
 * En desktop (Tauri) no se registra porque push no aplica (no hay
 * browser push API en el webview nativo).
 */

import { crearLogger } from '../../services/varios-1/logger';
import { escucharVentana } from '@/platform/ventana';
const log = crearLogger('SWPush');

const ES_DESKTOP = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export function registrarServiceWorker(): void {
    if (ES_DESKTOP) return;
    if (typeof navigator === 'undefined' || !('serviceWorker' in navigator)) return;

    /* No bloquear el render — registrar después del load */
    escucharVentana('load', () => {
        navigator.serviceWorker.register('/sw-push.js', { scope: '/' }).catch((error) => {
            /* No es crítico — push no funcionará pero la app sí */
            log.warn('No se pudo registrar el Service Worker:', error);
        });
    }, { once: true });
}
