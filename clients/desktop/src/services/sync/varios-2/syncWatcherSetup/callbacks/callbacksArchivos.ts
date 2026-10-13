/*
 * Callbacks de archivos (create / delete / move) del watcher.
 * trackingModule y encolarArchivo llegan por parámetro: son el snapshot de la
 * inicialización, como en el original.
 */
import { esDescargaEnCurso } from '../../../varios-1/syncGuards';
import { logSync } from '../../../varios-1/syncLogger';
import { guardarIndice, buscarEnIndicePorRuta, estado } from '../../syncState';
import { manejarBorradoLocal } from '../borrados';
import { manejarMoveLocal } from '../moverSample';
import type { TrackingModule } from '../syncWatcherEstado';

type EncolarArchivo = typeof import('../../../../varios-2/uploadQueueService').encolarArchivo;
type RegistrarArchivos = typeof import('../../../../varios-1/fileWatcherService').registrarCallbacks;

export function crearCallbacksArchivos(
    trackingModule: TrackingModule,
    encolarArchivo: EncolarArchivo,
): [Parameters<RegistrarArchivos>[0], Parameters<RegistrarArchivos>[1], Parameters<RegistrarArchivos>[2]] {
    return [
        /* Archivo nuevo */
        (ruta: string, nombreArchivo: string, carpetas: string[]) => {
            const rutaNorm = ruta.replace(/\\/g, '/');

            if (esDescargaEnCurso(rutaNorm)) {
                logSync.info('syncWatcherSetup', '[Sync] Ignorando create propio (descarga en curso):', nombreArchivo);
                return;
            }

            /*
             * P4: Normalizar nombre para dedup.
             * Si el archivo pasó por papelera, su nombre tiene prefijo timestamp
             * que rompe búsquedas por nombre en tracking e índice.
             */
            if (trackingModule) {
                const enTrackingPorRuta = trackingModule.buscarArchivoPorRuta(rutaNorm);
                if (enTrackingPorRuta) {
                    if (enTrackingPorRuta.syncDeshabilitado) {
                        /* Archivo re-añadido en la misma ruta tras borrado local. */
                        logSync.info('syncWatcherSetup', '[Sync] Archivo re-anadido tras borrado local (misma ruta), re-encolando:', nombreArchivo);
                        trackingModule.eliminarArchivo(enTrackingPorRuta.sampleId, enTrackingPorRuta.coleccionId)
                            .then(() => encolarArchivo(ruta, nombreArchivo, carpetas))
                            .catch(err => logSync.error('syncWatcherSetup', '[Sync] Error limpiando tracking para re-encolar:', [nombreArchivo, err]));
                    } else {
                        logSync.info('syncWatcherSetup', '[Sync] Archivo ya en tracking v2 (misma ruta), ignorando:', nombreArchivo);
                    }
                    return;
                }
            }

            const porRuta = buscarEnIndicePorRuta(rutaNorm);
            if (porRuta) {
                /*
                 * QL130: Si tracking v2 no tiene este archivo (ya se verifico arriba y no retorno),
                 * la entrada del indice legacy es obsoleta (ej: samples eliminados del servidor).
                 * Limpiar la entrada stale y permitir re-upload.
                 */
                logSync.warn('syncWatcherSetup', '[Sync] Entrada stale en indice legacy (tracking v2 no la tiene), limpiando:', nombreArchivo);
                const idx = estado.indiceArchivos.indexOf(porRuta);
                if (idx >= 0) estado.indiceArchivos.splice(idx, 1);
                estado.indiceArchivosPorRuta.delete(rutaNorm);
                if (porRuta.nombreServidor) estado.indiceArchivosPorNombre.delete(porRuta.nombreServidor);
                if (porRuta.nombreOriginal && porRuta.nombreOriginal !== porRuta.nombreServidor) {
                    estado.indiceArchivosPorNombre.delete(porRuta.nombreOriginal);
                }
                guardarIndice();
                /* No return — continuar al enqueue */
            }

            /*
             * No bloquear por nombre:
             * nombres iguales pueden representar archivos distintos.
             * El dedup real vive en uploadQueue (hash + idempotency key).
             */

            encolarArchivo(ruta, nombreArchivo, carpetas);
        },
        /* Archivo eliminado — papelera + borrado bidireccional */
        (ruta: string) => {
            manejarBorradoLocal(ruta);
        },
        /* Archivo movido */
        (rutaAnterior: string, rutaNueva: string, nombreArchivo: string, carpetas: string[]) => {
            manejarMoveLocal(rutaAnterior, rutaNueva, nombreArchivo, carpetas);
        },
    ];
}
