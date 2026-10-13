/* Fase 2 del sync completo: verificación de espacio y descarga paralela de samples en modo lote. */

import { Semaforo } from '../../../../varios-2/semaforo';
import { estado } from '../../../varios-2/syncState';
import { logSync } from '../../syncLogger';
import {
    obtenerArchivo,
    obtenerColeccion,
    agregarSinColeccion,
    iniciarLote,
    finalizarLote,
} from '../../../varios-2/syncTrackingService';
import { MARGEN_DISCO_BYTES } from '../constantes';
import { descargarSiNecesario } from '../descarga';
import type {
    RespuestaSyncColecciones,
    CallbackProgresoColecciones,
    JoinColecciones,
    TareaDescarga,
} from '../tipos';

/*
 * Estima el tamano total de descarga y verifica que haya espacio suficiente.
 * Solo cuenta samples que NO estan ya descargados (no existentes en tracking).
 * Retorna true si hay espacio suficiente o si no se puede determinar (fail open).
 */
export async function verificarEspacioDisco(
    carpetaBase: string,
    datos: RespuestaSyncColecciones,
): Promise<boolean> {
    try {
        const { invoke } = await import('@tauri-apps/api/core');

        /* Calcular tamano total de samples pendientes de descarga */
        let bytesNecesarios = 0;

        for (const col of datos.colecciones) {
            for (const sample of col.samples) {
                const existente = obtenerArchivo(sample.id, col.id);
                if (!existente || existente.syncDeshabilitado) {
                    bytesNecesarios += sample.tamano || 0;
                }
            }
        }
        for (const sample of datos.sinColeccion) {
            const existente = obtenerArchivo(sample.id, null);
            if (!existente || existente.syncDeshabilitado) {
                bytesNecesarios += sample.tamano || 0;
            }
        }

        /* Si no hay nada que descargar, no verificar */
        if (bytesNecesarios === 0) return true;

        const espacioDisponible = await invoke<number>('obtener_espacio_disponible', { ruta: carpetaBase });

        if (espacioDisponible < bytesNecesarios + MARGEN_DISCO_BYTES) {
            const disponibleMB = (espacioDisponible / (1024 * 1024)).toFixed(0);
            const necesarioMB = (bytesNecesarios / (1024 * 1024)).toFixed(0);
            logSync.error(
                'syncCollectionService', String(`[SyncColecciones] Espacio insuficiente: ${disponibleMB} MB disponibles, ` +
                `${necesarioMB} MB necesarios + ${(MARGEN_DISCO_BYTES / (1024 * 1024)).toFixed(0)} MB margen`),
            );
            return false;
        }

        return true;
    } catch (err) {
        /* Si no se puede verificar (ej: comando Rust no disponible), permitir descarga igualmente */
        logSync.warn('syncCollectionService', '[SyncColecciones] No se pudo verificar espacio en disco:', err);
        return true;
    }
}

/**
 * Descarga en paralelo los samples del servidor a sus carpetas de colección.
 * Devuelve los contadores de nuevos y errores de la fase.
 */
export async function sincronizarDescargas(
    carpetaBase: string,
    datosServidor: RespuestaSyncColecciones,
    rutaSinCol: string,
    onProgreso: CallbackProgresoColecciones | undefined,
    join: JoinColecciones,
    totalSamples: number,
): Promise<{ nuevos: number; errores: number }> {
    /* Fase 2: Descargar samples (modo lote para evitar 100+ escrituras individuales) */
    iniciarLote();
    onProgreso?.({ fase: 'descarga', actual: 0, total: totalSamples });

    let nuevos = 0;
    let errores = 0;

    /* Construir lista plana de descargas para procesar en paralelo */
    const tareasDescarga: TareaDescarga[] = [];

    for (const colServer of datosServidor.colecciones) {
        const colLocal = obtenerColeccion(colServer.id);
        if (!colLocal) continue;

        /* Resolver ruta de carpeta: subcolecciones viven dentro de la carpeta del padre */
        let carpetaColeccion: string;
        if (colLocal.parentId !== null) {
            const padreLocal = obtenerColeccion(colLocal.parentId);
            if (!padreLocal) continue;
            carpetaColeccion = await join(carpetaBase, padreLocal.carpetaLocal, colLocal.carpetaLocal);
        } else {
            carpetaColeccion = await join(carpetaBase, colLocal.carpetaLocal);
        }

        for (const sample of colServer.samples) {
            tareasDescarga.push({
                sample,
                coleccionId: colServer.id,
                carpetaDestino: carpetaColeccion,
                esSinColeccion: false,
            });
        }
    }

    for (const sample of datosServidor.sinColeccion) {
        tareasDescarga.push({
            sample,
            coleccionId: null,
            carpetaDestino: rutaSinCol,
            esSinColeccion: true,
        });
    }

    /* Procesar descargas en paralelo con semáforo */
    const maxParalelos = Math.max(1, Math.min(5, estado.configAvanzada.archivosParalelos));
    const semaforoDescargas = new Semaforo(maxParalelos);
    let procesados = 0;

    const promesasDescarga = tareasDescarga.map(tarea => {
        return semaforoDescargas.adquirir().then(async () => {
            try {
                procesados++;
                const resultado = await descargarSiNecesario(
                    tarea.sample, tarea.coleccionId, tarea.carpetaDestino,
                    onProgreso, procesados, totalSamples,
                );
                if (resultado === 'nuevo') {
                    nuevos++;
                    if (tarea.esSinColeccion) {
                        await agregarSinColeccion(tarea.sample.id);
                    }
                }
                if (resultado === 'error') errores++;
            } catch (err) {
                /* TM1: Per-sample error boundary — un fallo no detiene el resto del batch.
                 * Sin esto, una excepción inesperada en descargarSiNecesario (FS error,
                 * permiso denegado, disco lleno) deja contadores inconsistentes. */
                errores++;
                logSync.error('collectionSync', `Error descargando sample ${tarea.sample.id}`, {
                    error: err instanceof Error ? err.message : String(err),
                    coleccionId: tarea.coleccionId,
                });
            } finally {
                semaforoDescargas.liberar();
            }
        });
    });

    await Promise.all(promesasDescarga);

    onProgreso?.({ fase: 'completado', actual: totalSamples, total: totalSamples });

    /* Finalizar modo lote: persistir todo de una vez */
    await finalizarLote();

    return { nuevos, errores };
}
