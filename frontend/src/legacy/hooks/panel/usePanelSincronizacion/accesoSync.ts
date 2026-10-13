import type { KamplesSync, KamplesUpload } from './tipos';

/* Lectura centralizada de los objetos inyectados en window por desktop/main.tsx */
export function obtenerSync(): KamplesSync | null {
    return window.__KAMPLES_SYNC__ ?? null;
}

export function obtenerUpload(): KamplesUpload | null {
    return (window.__KAMPLES_UPLOAD__ as KamplesUpload | undefined) ?? null;
}
