/* Fase de purga del tracking local frente a la respuesta del servidor (solo sync completo, salvo purga de colecciones). */

import { moverAPapelera } from '../../../../varios-1/papeleraService';
import { estado } from '../../../varios-2/syncState';
import { logSync } from '../../syncLogger';
import {
    todasLasColecciones,
    todosLosArchivos,
    eliminarArchivo,
    eliminarColeccion,
} from '../../../varios-2/syncTrackingService';
import { GRACIA_PRESENCIA_SERVIDOR_MS } from '../constantes';
import type { RespuestaSyncColecciones } from '../tipos';

/*
 * Purga de tracking: eliminar entries de samples ausentes en el servidor.
 * SOLO se ejecuta en sync completo (no en soloEstructura/polling).
 *
 * Sin este guard, el polling periódico (cada 60s) borraba tracking de
 * archivos locales si el servidor devolvía datos vacíos o incompletos
 * (auth no lista, latencia pipeline, error transitorio).
 * Esto causaba que al iniciar el programa TODAS las colecciones se vaciaran
 * porque la primera llamada de estructura purgaba el tracking entero.
 *
 * Protección adicional: si el servidor retorna 0 samples pero localmente
 * hay >5 archivos trackeados, es casi seguro una respuesta inválida.
 * En ese caso se omite la purga para evitar data loss.
 */
export async function purgarTrackingNoServidor(
    datosServidor: RespuestaSyncColecciones,
    soloEstructura: boolean,
): Promise<void> {
    const sampleIdsServidor = new Set<number>();
    for (const col of datosServidor.colecciones) {
        for (const s of col.samples) sampleIdsServidor.add(s.id);
    }
    for (const s of datosServidor.sinColeccion) sampleIdsServidor.add(s.id);

    const archivosActuales = todosLosArchivos();

    if (!soloEstructura) {
        const borrarLocalSiNoEnServidor = estado.configAvanzada.borrarEnLocalAlBorrarEnServidor;

        /* Seguridad: no purgar si servidor devuelve vacío pero hay muchos archivos locales */
        const esRespuestaVaciaSospechosa = sampleIdsServidor.size === 0 && archivosActuales.length > 5;
        if (esRespuestaVaciaSospechosa) {
            logSync.warn('syncCollectionService', '[SyncCollection] Servidor retornó 0 samples pero hay', [archivosActuales.length, 'locales. Omitiendo purga (posible error de auth/red).']);
        } else {
            for (const archivo of archivosActuales) {
                if (!sampleIdsServidor.has(archivo.sampleId)) {
                    const esReciente = (Date.now() - (archivo.descargadoEn ?? 0)) < GRACIA_PRESENCIA_SERVIDOR_MS;
                    if (esReciente) {
                        logSync.info('syncCollectionService', '[SyncCollection] Omitiendo purge por ventana de gracia (sample reciente):', [archivo.sampleId, archivo.nombreLocal]);
                        continue;
                    }

                    /* Borrado bidireccional server→local: mover a papelera si esta activo */
                    if (borrarLocalSiNoEnServidor && archivo.rutaLocal) {
                        const { exists: existeFs } = await import('@tauri-apps/plugin-fs');
                        const existeArchivo = await existeFs(archivo.rutaLocal).catch(() => false);
                        if (existeArchivo && estado.config.carpetaLocal) {
                            const movido = await moverAPapelera(
                                archivo.rutaLocal,
                                archivo.nombreLocal,
                                archivo.sampleId,
                                archivo.coleccionId,
                                'servidor',
                                estado.config.carpetaLocal,
                            );
                            if (movido) {
                                logSync.info('syncCollectionService', '[SyncCollection] Archivo local movido a papelera (borrado en servidor):', archivo.nombreLocal);
                            }
                        }
                    }

                    await eliminarArchivo(archivo.sampleId, archivo.coleccionId);
                    logSync.info('syncCollectionService', '[SyncCollection] Eliminado de tracking (ya no existe en servidor):', [archivo.nombreLocal, '(id:', archivo.sampleId, ')']);
                }
            }
        }
    }

    /*
     * C289: Purga de colecciones fantasma del tracking.
     * Si una colección existe en tracking local pero NO existe en el servidor,
     * eliminarla del tracking. Esto resuelve colecciones borradas del servidor
     * que persisten en tracking y causan 403 en renames, huérfanas y reconciliación.
     *
     * Protección: si el servidor retornó 0 colecciones pero localmente hay >3,
     * posible fallo de auth/red parcial — no purgar para evitar data loss.
     */
    const idsColeccionesServidor = new Set<number>();
    for (const col of datosServidor.colecciones) {
        idsColeccionesServidor.add(col.id);
    }

    const coleccionesLocales = todasLasColecciones();
    const purgaColeccionesSegura = idsColeccionesServidor.size > 0 || coleccionesLocales.length <= 3;

    if (purgaColeccionesSegura) {
        let colPurgadas = 0;
        for (const colLocal of coleccionesLocales) {
            if (!idsColeccionesServidor.has(colLocal.id)) {
                logSync.info('syncCollection',
                    `Purgando colección fantasma: #${colLocal.id} "${colLocal.nombre}" (ausente en servidor)`);
                await eliminarColeccion(colLocal.id);
                colPurgadas++;
            }
        }
        if (colPurgadas > 0) {
            logSync.info('syncCollection', `${colPurgadas} colección(es) fantasma eliminadas del tracking`);
        }
    }
}
