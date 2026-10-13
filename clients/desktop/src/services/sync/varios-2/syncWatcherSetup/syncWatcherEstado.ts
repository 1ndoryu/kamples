/*
 * Estado de módulo del watcher de sync: timers, contadores de borrado y reconciliación.
 *
 * Los módulos lo leen por live binding. Las reasignaciones van siempre por los setters:
 * ES no permite reasignar un `export let` desde otro módulo, y una copia local dejaría
 * de ver los cambios.
 */

export type TrackingModule = typeof import('../syncTrackingService') | null;
export type ColeccionModule = typeof import('../../varios-1/syncCollectionService') | null;

export let pollingTimeout: ReturnType<typeof setTimeout> | null = null;
export function setPollingTimeout(valor: ReturnType<typeof setTimeout> | null): void {
    pollingTimeout = valor;
}

export let ultimaSyncConDescargas = 0;
export function setUltimaSyncConDescargas(valor: number): void {
    ultimaSyncConDescargas = valor;
}

export let borradosEnCiclo = 0;
export function setBorradosEnCiclo(valor: number): void {
    borradosEnCiclo = valor;
}

export let ultimoResetBorrados = Date.now();
export function setUltimoResetBorrados(valor: number): void {
    ultimoResetBorrados = valor;
}

/* QL63: Cola de reintentos para soft-deletes que superan el rate limit */
export const colaBorradosPendientes: number[] = [];

export let reconciliacionCarpetasInterval: ReturnType<typeof setInterval> | null = null;
export function setReconciliacionCarpetasInterval(valor: ReturnType<typeof setInterval> | null): void {
    reconciliacionCarpetasInterval = valor;
}
