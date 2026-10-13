/*
 * Ciclo de vida del watcher (fileWatcherService): iniciar, detener y purga periódica del cache.
 */

import { esDesktop } from '../desktopService';
import { obtenerConfigSync } from '../../sync/varios-2/syncService';
import { logSync } from '../../sync/varios-1/syncLogger';
import { flushBufferArchivos } from './fileWatcherBatch';
import {
    PURGA_INTERVALO_MS,
    PURGA_TTL_MS,
} from './fileWatcherConstantes';
import {
    archivosRecientes,
    limpiarEstadoObservacion,
    observando,
    purgaInterval,
    setObservando,
    setPurgaInterval,
    setUnwatchFn,
    unwatchFn,
} from './fileWatcherEstado';
import { procesarEvento } from './fileWatcherProcesarEvento';
import type { EventoWatch, UnwatchFn } from './fileWatcherTipos';

/*
 * Inicia la observación de la carpeta de sincronización.
 * Retorna true si se inició correctamente, false si no hay carpeta configurada.
 */
export async function iniciarObservacion(): Promise<boolean> {
    if (!esDesktop() || observando) return false;

    const config = obtenerConfigSync();
    if (!config.carpetaLocal || !config.sincronizacionActiva) {
        logSync.warn('watcher', 'iniciarObservacion ABORTADO — config incompleta', {
            carpetaLocal: !!config.carpetaLocal,
            sincronizacionActiva: config.sincronizacionActiva,
        });
        return false;
    }

    try {
        const fsModule = await import('@tauri-apps/plugin-fs');
        /* QL107: watch existe en el modulo pero TS 5.9 no lo resuelve
         * correctamente via dynamic import() — cast seguro. */
        const watchFn = (fsModule as Record<string, unknown>).watch as (
            paths: string | string[],
            cb: (event: EventoWatch) => void,
            options?: { recursive?: boolean; delayMs?: number },
        ) => Promise<UnwatchFn>;

        const unwatch = await watchFn(
            config.carpetaLocal,
            (evento: EventoWatch) => { void procesarEvento(evento, config.carpetaLocal!); },
            { recursive: true, delayMs: 1500 },
        );
        setUnwatchFn(unwatch);

        setObservando(true);
        iniciarPurgaPeriodica();
        logSync.info('watcher', `Observando carpeta: ${config.carpetaLocal}`);
        return true;
    } catch (err) {
        logSync.error('watcher', 'Error iniciando observación', { error: err instanceof Error ? err.message : String(err) });
        return false;
    }
}

/*
 * Detiene la observación de la carpeta.
 */
export async function detenerObservacion(): Promise<void> {
    if (unwatchFn) {
        unwatchFn();
        setUnwatchFn(null);
    }
    setObservando(false);
    detenerPurgaPeriodica();

    /* Limpia timers y cachés pendientes (eliminaciones, renames, carpetas, subcarpetas) */
    limpiarEstadoObservacion();

    /* QL76: Flush inmediato del buffer de archivos pendientes al detener */
    flushBufferArchivos();

    logSync.info('watcher', 'Observación detenida');
}

/*
 * Indica si el watcher está activo.
 */
export function estaObservando(): boolean {
    return observando;
}

/* Purga periódica del cache para evitar crecimiento ilimitado en batches grandes */
function iniciarPurgaPeriodica(): void {
    if (purgaInterval) return;
    setPurgaInterval(setInterval(() => {
        const ahora = Date.now();
        for (const [k, v] of archivosRecientes) {
            if (ahora - v > PURGA_TTL_MS) archivosRecientes.delete(k);
        }
    }, PURGA_INTERVALO_MS));
}

function detenerPurgaPeriodica(): void {
    if (purgaInterval) {
        clearInterval(purgaInterval);
        setPurgaInterval(null);
    }
}
