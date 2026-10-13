/* Acciones locales → servidor: mover samples entre colecciones, asociar samples y renombrar colecciones. */

import { estaOnline } from '../../../varios-1/desktopService';
import { obtenerBaseUrlSync, obtenerHeadersSync, extraerErrorRespuesta } from '../syncGuards';
import { encolarOperacion } from '../../../varios-1/offlineQueueService';
import { logSync } from '../syncLogger';
import { registrarAccion, actualizarNombreColeccion, obtenerColeccion } from '../../varios-2/syncTrackingService';
import { invalidarCacheColecciones } from './estado';
import { normalizarNombreColeccion, sanitizarNombreCarpeta } from './helpers';

/**
 * Mover un sample entre colecciones en el servidor.
 * Se llama cuando el watcher detecta un MOVE entre carpetas mapeadas.
 */
export async function moverSampleEntreColecciones(
    sampleId: number,
    coleccionOrigenId: number | null,
    coleccionDestinoId: number | null,
): Promise<boolean> {
    if (!estaOnline()) return false;

    try {
        const baseUrl = obtenerBaseUrlSync();

        /* Quitar de la colección origen */
        if (coleccionOrigenId !== null) {
            const respQuitar = await fetch(
                `${baseUrl}/kamples/v1/colecciones/${coleccionOrigenId}/samples`,
                {
                    method: 'DELETE',
                    headers: obtenerHeadersSync(),
                    body: JSON.stringify({ sample_id: sampleId }),
                },
            );
            if (!respQuitar.ok) {
                const detalle = await extraerErrorRespuesta(respQuitar);
                logSync.error('syncCollectionService', '[SyncCollection] Error quitando sample de colección origen:', [respQuitar.status, detalle]);
            }
        }

        /* Agregar a la colección destino */
        if (coleccionDestinoId !== null) {
            const respAgregar = await fetch(
                `${baseUrl}/kamples/v1/colecciones/${coleccionDestinoId}/samples`,
                {
                    method: 'POST',
                    headers: obtenerHeadersSync(),
                    body: JSON.stringify({ sample_id: sampleId }),
                },
            );
            if (!respAgregar.ok) {
                const detalle = await extraerErrorRespuesta(respAgregar);
                logSync.error('syncCollectionService', '[SyncCollection] Error agregando sample a colección destino:', [respAgregar.status, detalle]);
                return false;
            }
        }

        await registrarAccion({
            tipo: 'movido',
            descripcion: `Sample ${sampleId} movido de colección ${coleccionOrigenId ?? 'ninguna'} a ${coleccionDestinoId ?? 'ninguna'}`,
            sampleId,
            coleccionId: coleccionDestinoId ?? undefined,
        });

        return true;
    } catch (err) {
        logSync.error('syncCollectionService', '[SyncCollection] Error moviendo sample entre colecciones:', err);
        return false;
    }
}

/**
 * Agrega un sample a una colección en el servidor via POST /colecciones/{id}/samples.
 *
 * Diferencia con moverSampleEnServidorPublico (PUT carpeta):
 * - PUT /me/coleccionados/{id}/carpeta → solo actualiza metadata del sample (label carpeta)
 * - POST /colecciones/{id}/samples → inserta en coleccion_samples (asociación real)
 *
 * Sin esta llamada, el sample no aparece dentro de la colección en sync ni en la web.
 */
export async function agregarSampleAColeccion(
    coleccionId: number,
    sampleId: number,
): Promise<boolean> {
    if (!estaOnline()) {
        encolarOperacion({
            tipo: 'agregar_sample_coleccion',
            endpoint: `${obtenerBaseUrlSync()}/kamples/v1/colecciones/${coleccionId}/samples`,
            method: 'POST',
            body: { sampleId },
            claveDuplicacion: `agregar_sample_${coleccionId}_${sampleId}`,
        });
        logSync.info('syncCollectionService', '[SyncCollection] Agregar sample a colección encolado para cuando haya conexión:', [sampleId, '→ col:', coleccionId]);
        return true;
    }

    try {
        const baseUrl = obtenerBaseUrlSync();
        const resp = await fetch(`${baseUrl}/kamples/v1/colecciones/${coleccionId}/samples`, {
            method: 'POST',
            headers: obtenerHeadersSync(),
            body: JSON.stringify({ sampleId }),
        });

        if (!resp.ok) {
            const body = await resp.json().catch(() => ({}));
            logSync.error('syncCollectionService', '[SyncCollection] Error agregando sample a colección:', [coleccionId, sampleId, resp.status, body]);

            /*
             * 403: la colección no pertenece al usuario.
             * Desvincular del tracking para evitar intentos repetidos.
             * El upload flow creará una colección nueva si es necesario.
             */
            if (resp.status === 403) {
                logSync.warn('SyncCollection', `Colección #${coleccionId} no pertenece al usuario. Desvinculando.`);
                try {
                    const tracking = (await import('../../varios-2/syncTrackingService'));
                    await tracking.eliminarColeccion(coleccionId);
                } catch (cleanupErr) {
                    logSync.error('syncCollectionService', '[SyncCollection] Error desvinculando colección:', cleanupErr);
                }
            }

            return false;
        }

        logSync.info('syncCollectionService', '[SyncCollection] Sample agregado a colección:', [sampleId, '→ col:', coleccionId]);
        /* Invalidar cache para que la próxima sincronización obtenga datos frescos.
         * Sin esto, delta sync puede usar datos stale donde el sample aparece en
         * sinColeccion (fue creado por upload antes de ser asignado a colección),
         * causando que D4.2 mueva archivos recién subidos a la carpeta equivocada. */
        invalidarCacheColecciones();
        return true;
    } catch (err) {
        logSync.error('syncCollectionService', '[SyncCollection] Error en request agregar sample a colección:', err);
        return false;
    }
}

/**
 * Renombrar una colección en el servidor.
 * Se llama cuando el watcher detecta RENAME de una carpeta mapeada.
 *
 * Si estamos offline, encola la operación para reintento automático.
 * Si la petición falla por error transitorio, también encola.
 */
export async function renombrarColeccionEnServidor(coleccionId: number, nuevoNombre: string): Promise<boolean> {
    const nombreNormalizado = normalizarNombreColeccion(nuevoNombre);

    /* F5.2: Enviar version actual para optimistic locking */
    const colLocal = obtenerColeccion(coleccionId);
    const bodyRename: Record<string, unknown> = { nombre: nombreNormalizado };
    if (colLocal?.version) {
        bodyRename.version = colLocal.version;
    }

    if (!estaOnline()) {
        /*
         * Encolar para reintento cuando se recupere conexión.
         * Sin esto, renames offline se pierden silenciosamente.
         */
        await encolarOperacion({
            tipo: 'renombrar_coleccion',
            endpoint: `${obtenerBaseUrlSync()}/kamples/v1/colecciones/${coleccionId}`,
            method: 'PUT',
            body: bodyRename,
            claveDuplicacion: `rename-col-${coleccionId}`,
        });
        logSync.info('syncCollectionService', '[SyncCollection] Rename encolado (offline):', [coleccionId, '→', nombreNormalizado]);
        return false;
    }

    try {
        const baseUrl = obtenerBaseUrlSync();
        const resp = await fetch(`${baseUrl}/kamples/v1/colecciones/${coleccionId}`, {
            method: 'PUT',
            headers: obtenerHeadersSync(),
            body: JSON.stringify(bodyRename),
        });

        if (!resp.ok) {
            const detalle = await extraerErrorRespuesta(resp);
            logSync.error('syncCollectionService', '[SyncCollection] Error renombrando colección en servidor:', [resp.status, detalle]);

            /* F5.2: Conflicto de version — forzar re-sync en el proximo ciclo */
            if (resp.status === 409) {
                logSync.warn('SyncCollection', `Conflicto de versión al renombrar col #${coleccionId}. Se re-sincronizará.`);
                return false;
            }

            /*
             * 403 no_autorizado: la colección no pertenece al usuario actual.
             * Esto ocurre cuando el tracking local tiene colecciones de otra sesión
             * o cuenta. Desvincular del tracking evita que la detección de huérfanas
             * intente renombrarla en cada escaneo (loop infinito de 403s).
             */
            if (resp.status === 403) {
                logSync.warn('SyncCollection', `Colección #${coleccionId} no pertenece al usuario. Desvinculando del tracking local.`);
                try {
                    const tracking = (await import('../../varios-2/syncTrackingService'));
                    await tracking.eliminarColeccion(coleccionId);
                } catch (cleanupErr) {
                    logSync.error('syncCollectionService', '[SyncCollection] Error desvinculando colección del tracking:', cleanupErr);
                }
                return false;
            }

            /*
             * Error transitorio (500, 429, timeout): encolar para reintento.
             * Solo fallos permanentes (400, 404) se descartan definitivamente.
             */
            if (resp.status >= 500 || resp.status === 429) {
                await encolarOperacion({
                    tipo: 'renombrar_coleccion',
                    endpoint: `${baseUrl}/kamples/v1/colecciones/${coleccionId}`,
                    method: 'PUT',
                    body: bodyRename,
                    claveDuplicacion: `rename-col-${coleccionId}`,
                });
            }
            return false;
        }

        const carpetaNueva = sanitizarNombreCarpeta(nombreNormalizado);
        await actualizarNombreColeccion(coleccionId, nombreNormalizado, carpetaNueva);

        /* C289: Invalidar caché para que la próxima consulta refleje el rename */
        invalidarCacheColecciones();

        await registrarAccion({
            tipo: 'renombrado',
            descripcion: `Colección ${coleccionId} renombrada a "${nombreNormalizado}"`,
            coleccionId,
        });

        return true;
    } catch (err) {
        logSync.error('syncCollectionService', '[SyncCollection] Error renombrando colección en servidor:', err);
        /* Error de red: encolar para reintento */
        await encolarOperacion({
            tipo: 'renombrar_coleccion',
            endpoint: `${obtenerBaseUrlSync()}/kamples/v1/colecciones/${coleccionId}`,
            method: 'PUT',
            body: { nombre: nombreNormalizado },
            claveDuplicacion: `rename-col-${coleccionId}`,
        });
        return false;
    }
}
