/*
 * Movimientos locales <-> servidor: PUT de carpeta y actualización de coleccion_samples.
 */
import { estaOnline } from '../../../varios-1/desktopService';
import { obtenerBaseUrlSync } from '../../varios-1/syncGuards';
import { encolarOperacion } from '../../../varios-1/offlineQueueService';
import { logSync } from '../../varios-1/syncLogger';
import { estado, guardarIndice, buscarEnIndicePorRuta, actualizarIndiceArchivo } from '../syncState';
import { CARPETAS_SISTEMA_SYNC } from './syncWatcherConstantes';

/* Operaciones de movimiento local ↔ servidor */

/*
 * Resuelve la colección destino desde las carpetas del move y actualiza
 * tanto coleccion_samples en el servidor como coleccionId en el tracking.
 *
 * Problema original: manejarMoveLocal solo usaba PUT /me/coleccionados/{id}/carpeta
 * (actualiza metadata) sin tocar coleccion_samples. Resultado: el sample se "movía"
 * a nivel metadata pero nunca aparecía en la colección en la web/API.
 */
async function actualizarColeccionEnMovimiento(
    sampleId: number,
    carpetas: string[],
    archivoTracking: { sampleId: number; coleccionId: number | null; rutaLocal: string; nombreLocal: string; nombreServidor: string; descargadoEn: number; tamano: number; syncDeshabilitado: boolean } | null,
): Promise<void> {
    const { trackingModule } = estado;
    if (!trackingModule) return;

    const primaria = carpetas[0] || '';
    const esSinColeccion = !primaria || CARPETAS_SISTEMA_SYNC.has(primaria.toLowerCase());

    /* Resolver coleccionId destino — caminar N niveles de subcollections */
    let coleccionDestinoId: number | null = null;
    if (!esSinColeccion) {
        const colPadre = trackingModule.buscarColeccionPorCarpeta(primaria);
        if (colPadre) {
            coleccionDestinoId = colPadre.id;

            /* Caminar carpetas[1..N] buscando subcollections anidadas */
            const todasCols = trackingModule.todasLasColecciones();
            let padreActual = colPadre.id;
            for (let i = 1; i < carpetas.length; i++) {
                if (!carpetas[i]) break;
                const sub = todasCols.find(
                    c => c.parentId === padreActual
                        && c.carpetaLocal.toLowerCase() === carpetas[i].toLowerCase(),
                );
                if (!sub) break;
                coleccionDestinoId = sub.id;
                padreActual = sub.id;
            }
        }
    }

    /* Actualizar coleccion_samples en el servidor */
    try {
        const { agregarSampleAColeccion, invalidarCacheColecciones } = await import('../../varios-1/syncCollectionService');

        if (coleccionDestinoId !== null) {
            /* Move a una colección real → INSERT/UPDATE en coleccion_samples */
            await agregarSampleAColeccion(coleccionDestinoId, sampleId);
            invalidarCacheColecciones();
        }
        /* Para "sin coleccion" el PUT /me/coleccionados ya maneja la metadata;
         * el endpoint de quitar necesita el coleccionId origen que puede no estar
         * disponible de forma fiable. El delta sync reconciliará si es necesario. */
    } catch (err) {
        logSync.error('syncWatcherSetup', '[Sync] Error actualizando coleccion_samples en move:', [sampleId, err]);
    }

    /* Actualizar coleccionId en tracking v2 */
    if (archivoTracking && archivoTracking.coleccionId !== coleccionDestinoId) {
        /* Clave tracking = sampleId_coleccionId → hay que eliminar entry vieja y crear nueva */
        await trackingModule.eliminarArchivo(archivoTracking.sampleId, archivoTracking.coleccionId);
        await trackingModule.registrarArchivo({
            ...archivoTracking,
            coleccionId: coleccionDestinoId,
        });
    }
}

/*
 * Llama al endpoint PUT /me/coleccionados/{id}/carpeta para mover
 * un sample a otra carpeta en el servidor.
 */
async function moverSampleEnServidor(
    sampleId: number,
    carpetaPrimaria: string,
    carpetaSecundaria: string,
): Promise<boolean> {
    if (!estaOnline()) {
        /* Encolar operacion para ejecutar al reconectar. Deduplicar por sampleId. */
        encolarOperacion({
            tipo: 'mover_carpeta',
            endpoint: `${obtenerBaseUrlSync()}/kamples/v1/me/coleccionados/${sampleId}/carpeta`,
            method: 'PUT',
            body: { carpeta_primaria: carpetaPrimaria, carpeta_secundaria: carpetaSecundaria },
            claveDuplicacion: `mover_carpeta_${sampleId}`,
        });
        logSync.info('syncWatcherSetup', '[Sync] Move encolado para cuando haya conexion, sample:', sampleId);
        return true;
    }

    try {
        const baseUrl = obtenerBaseUrlSync();
        const resp = await fetch(`${baseUrl}/kamples/v1/me/coleccionados/${sampleId}/carpeta`, {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                carpeta_primaria: carpetaPrimaria,
                carpeta_secundaria: carpetaSecundaria,
            }),
        });

        if (!resp.ok) {
            const body = await resp.json().catch(() => ({}));
            logSync.error('syncWatcherSetup', '[Sync] Error moviendo sample en servidor:', [sampleId, body]);
            return false;
        }

        logSync.info('syncWatcherSetup', '[Sync] Sample movido en servidor:', [sampleId, '→', carpetaPrimaria, carpetaSecundaria || '(raíz)']);
        return true;
    } catch (err) {
        logSync.error('syncWatcherSetup', '[Sync] Error en request de mover sample:', [sampleId, err]);
        return false;
    }
}

/*
 * Versión pública de moverSampleEnServidor.
 * Usada por uploadQueueService para asignar carpeta tras subir un archivo.
 */
export async function moverSampleEnServidorPublico(
    sampleId: number,
    carpetaPrimaria: string,
    carpetaSecundaria: string,
): Promise<boolean> {
    return moverSampleEnServidor(sampleId, carpetaPrimaria, carpetaSecundaria);
}

/*
 * Maneja un MOVE local (archivo movido de una carpeta a otra dentro de sync).
 * 1. Actualiza la ruta en el índice local + tracking v2
 * 2. Calcula nuevas carpeta_primaria/secundaria desde la ruta
 * 3. Llama PUT /me/coleccionados/{id}/carpeta para sincronizar con el server
 */
export async function manejarMoveLocal(
    rutaAnterior: string,
    rutaNueva: string,
    nombreArchivo: string,
    carpetas: string[],
): Promise<void> {
    const { trackingModule, indiceArchivos } = estado;
    const rutaAntNorm = rutaAnterior.replace(/\\/g, '/');
    const archivo = buscarEnIndicePorRuta(rutaAntNorm);

    if (!archivo) {
        /*
         * Fallback a tracking v2: si el archivo existe en v2 pero no en v1
         * (puede pasar tras migración parcial o crash antes de persistir v1),
         * procesar el move directamente en v2 para no perderlo.
         */
        if (trackingModule) {
            const archivoV2 = trackingModule.buscarArchivoPorRuta(rutaAnterior);
            if (archivoV2) {
                archivoV2.rutaLocal = rutaNueva;
                if (archivoV2.syncDeshabilitado) archivoV2.syncDeshabilitado = false;
                await trackingModule.registrarArchivo(archivoV2);
                await trackingModule.registrarAccion({
                    tipo: 'movido',
                    descripcion: `Movido a ${carpetas[0] || 'General'}${carpetas[1] ? '/' + carpetas[1] : ''}`,
                    sampleId: archivoV2.sampleId,
                });

                const primaria = carpetas[0] || 'General';
                const secundaria = carpetas[1] || '';

                /* C384: Asegurar colección destino existe (idempotente) */
                if (primaria && primaria !== 'General' && estado.collectionModule) {
                    try {
                        await estado.collectionModule.crearColeccionDesdeLocal(primaria);
                    } catch (err) {
                        logSync.error('syncWatcherSetup', '[Sync] Error asegurando colección destino en move v2:', [primaria, err]);
                    }
                }

                await moverSampleEnServidor(archivoV2.sampleId, primaria, secundaria);
                /* Actualizar coleccion_samples en servidor + coleccionId en tracking */
                await actualizarColeccionEnMovimiento(archivoV2.sampleId, carpetas, archivoV2);
                logSync.info('syncWatcherSetup', '[Sync] Move procesado (solo v2): sample', [archivoV2.sampleId, '→', primaria, secundaria || '(raíz)']);
                return;
            }
        }

        logSync.warn('syncWatcherSetup', '[Sync] Move sin tracking previo; se trata como archivo nuevo para upload:', rutaNueva);

        const carpetaDestino = (carpetas[0] ?? '').trim();
        if (carpetaDestino && !CARPETAS_SISTEMA_SYNC.has(carpetaDestino.toLowerCase())) {
            estado.collectionModule?.crearColeccionDesdeLocal(carpetaDestino).catch(err => {
                logSync.error('syncWatcherSetup', '[Sync] Error creando colección destino desde move no trackeado:', err);
            });
        }

        try {
            const { encolarArchivo } = await import('../../../varios-2/uploadQueueService');
            await encolarArchivo(rutaNueva, nombreArchivo, carpetas);
        } catch (err) {
            logSync.error('syncWatcherSetup', '[Sync] Error encolando move no trackeado como upload:', err);
        }
        return;
    }

    archivo.ruta = rutaNueva;
    if (archivo.syncDeshabilitado) {
        archivo.syncDeshabilitado = false;
        archivo.rutaEliminada = undefined;
    }
    actualizarIndiceArchivo(archivo, rutaAntNorm);
    guardarIndice();

    /* v2: actualizar tracking si existe */
    let archivoTrackingRef: { sampleId: number; coleccionId: number | null; rutaLocal: string; nombreLocal: string; nombreServidor: string; descargadoEn: number; tamano: number; syncDeshabilitado: boolean } | null = null;
    if (trackingModule) {
        const archivoTracking = trackingModule.buscarArchivoPorSampleId(archivo.sampleId);
        if (archivoTracking) {
            archivoTracking.rutaLocal = rutaNueva;
            if (archivoTracking.syncDeshabilitado) archivoTracking.syncDeshabilitado = false;
            await trackingModule.registrarArchivo(archivoTracking);
            await trackingModule.registrarAccion({
                tipo: 'movido',
                descripcion: `Movido a ${carpetas[0] || 'General'}${carpetas[1] ? '/' + carpetas[1] : ''}`,
                sampleId: archivo.sampleId,
            });
            archivoTrackingRef = archivoTracking;
        }
    }

    const primaria = carpetas[0] || 'General';
    const secundaria = carpetas[1] || '';

    /*
     * C384: Asegurar que la colección destino existe antes de mover.
     * Si el usuario mueve un archivo a una carpeta nueva que no tiene colección
     * en el servidor, crearla ahora. crearColeccionDesdeLocal es idempotente.
     */
    if (primaria && primaria !== 'General' && estado.collectionModule) {
        try {
            await estado.collectionModule.crearColeccionDesdeLocal(primaria);
        } catch (err) {
            logSync.error('syncWatcherSetup', '[Sync] Error asegurando colección destino en move:', [primaria, err]);
        }
    }

    await moverSampleEnServidor(archivo.sampleId, primaria, secundaria);
    /* Actualizar coleccion_samples en servidor + coleccionId en tracking */
    await actualizarColeccionEnMovimiento(archivo.sampleId, carpetas, archivoTrackingRef);

    logSync.info('syncWatcherSetup', '[Sync] Move procesado: sample', [archivo.sampleId, '→', primaria, secundaria || '(raíz)']);
}
