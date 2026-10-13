/*
 * Buffer de acumulación de archivos nuevos (QL76) del watcher (fileWatcherService).
 * Agrupa las detecciones de copias masivas y las despacha tras un periodo de quietud,
 * con tope máximo de espera para no bloquear indefinidamente.
 */

import { logSync } from '../../sync/varios-1/syncLogger';
import {
    bufferArchivosNuevos,
    onArchivoNuevo,
    setTimeoutFlushBatch,
    setTimestampInicioBatch,
    timeoutFlushBatch,
    timestampInicioBatch,
} from './fileWatcherEstado';
import { MAX_ESPERA_BATCH_MS, QUIETUD_BATCH_MS } from './fileWatcherConstantes';

export function acumularArchivoNuevo(ruta: string, nombre: string, carpetas: string[]): void {
    bufferArchivosNuevos.push({ ruta, nombre, carpetas });

    const ahora = Date.now();
    if (!timestampInicioBatch) setTimestampInicioBatch(ahora);
    const inicioBatch = timestampInicioBatch || ahora;

    /* Si llevamos mas de MAX_ESPERA_BATCH_MS acumulando, flush parcial */
    if (ahora - inicioBatch >= MAX_ESPERA_BATCH_MS) {
        flushBufferArchivos();
        return;
    }

    /* Reiniciar timer de quietud */
    if (timeoutFlushBatch) clearTimeout(timeoutFlushBatch);
    setTimeoutFlushBatch(setTimeout(flushBufferArchivos, QUIETUD_BATCH_MS));
}

export function flushBufferArchivos(): void {
    if (timeoutFlushBatch) {
        clearTimeout(timeoutFlushBatch);
        setTimeoutFlushBatch(null);
    }
    setTimestampInicioBatch(null);

    const batch = bufferArchivosNuevos.splice(0);
    if (batch.length === 0) return;

    logSync.info('watcher', `Flush batch: ${batch.length} archivos nuevos acumulados`);

    for (const { ruta, nombre, carpetas } of batch) {
        if (onArchivoNuevo) {
            onArchivoNuevo(ruta, nombre, carpetas);
        }
    }
}
