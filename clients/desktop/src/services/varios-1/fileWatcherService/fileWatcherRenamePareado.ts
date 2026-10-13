/*
 * Rename nativo del FS con dos paths (origen, destino) del watcher (fileWatcherService).
 * Resuelve renames de carpetas de nivel 1, subcarpetas de nivel 2 y moves de archivos.
 */

import { logSync } from '../../sync/varios-1/syncLogger';
import {
    CARPETAS_EXCLUIDAS_TOTAL,
    EXTENSIONES_AUDIO,
} from './fileWatcherConstantes';
import {
    carpetasPendientesCreacion,
    onArchivoMovido,
    onArchivoNuevo,
    onCarpetaRenombrada,
    onSubcarpetaRenombrada,
    subcarpetasPendientesCreacion,
} from './fileWatcherEstado';
import { esDirectorioReal } from './fileWatcherUtilidades';

/*
 * Devuelve true si el rename quedó resuelto y procesarEvento debe terminar.
 * Devuelve false si no aplica (o falló) y el evento sigue al bucle genérico,
 * igual que el original: un error se registra y NO corta el watcher.
 * TA4: el try-catch evita que un fallo en callbacks detenga el watcher loop.
 */
export async function procesarRenamePareado(paths: string[], baseNormalizada: string): Promise<boolean> {
    try {
        const rutaOrigen = paths[0] ?? '';
        const rutaDestino = paths[1] ?? '';
        const origenNorm = rutaOrigen.replace(/\\/g, '/');
        const destinoNorm = rutaDestino.replace(/\\/g, '/');

        const relativaOrigen = origenNorm.startsWith(baseNormalizada + '/')
            ? origenNorm.slice(baseNormalizada.length + 1)
            : '';
        const relativaDestino = destinoNorm.startsWith(baseNormalizada + '/')
            ? destinoNorm.slice(baseNormalizada.length + 1)
            : '';

        const segmentosOrigen = relativaOrigen.split('/').filter(Boolean);
        const segmentosDestino = relativaDestino.split('/').filter(Boolean);

        if (segmentosOrigen.some(s => CARPETAS_EXCLUIDAS_TOTAL.has(s)) || segmentosDestino.some(s => CARPETAS_EXCLUIDAS_TOTAL.has(s))) {
            return true;
        }

        const nombreDestino = destinoNorm.split('/').pop() ?? '';
        const extensionDestino = nombreDestino.split('.').pop()?.toLowerCase() ?? '';

        const esCarpetaNivel1Rename =
            !!relativaOrigen
            && !!relativaDestino
            && !relativaOrigen.includes('/')
            && !relativaDestino.includes('/')
            && !EXTENSIONES_AUDIO.has(extensionDestino);

        if (esCarpetaNivel1Rename) {
            if (!await esDirectorioReal(destinoNorm)) {
                logSync.warn('watcher', `Rename ignorado: destino sin extension pero no es carpeta real: ${nombreDestino}`);
                return true;
            }
            const nombreOrigen = relativaOrigen;
            const nombreNueva = relativaDestino;

            /* Cancelar creacion pendiente del nombre origen.
             * Esto cubre el caso "Nueva carpeta" → "test2" donde Windows
             * primero emite CREATE y luego RENAME. Sin esto, la coleccion
             * se crea con el nombre temporal. */
            const claveOrigen = nombreOrigen.toLowerCase();
            const pendiente = carpetasPendientesCreacion.get(claveOrigen);
            if (pendiente) {
                clearTimeout(pendiente.timeout);
                carpetasPendientesCreacion.delete(claveOrigen);
                logSync.info('watcher', `Creacion cancelada por rename: ${nombreOrigen} → ${nombreNueva}`);
            }

            logSync.info('watcher', `Rename carpeta (evento name): ${nombreOrigen} → ${nombreNueva}`);
            if (onCarpetaRenombrada) {
                onCarpetaRenombrada(nombreOrigen, nombreNueva, rutaDestino);
            }
            return true;
        }

        /*
         * C387: Detectar rename de subcarpetas de nivel 2.
         * Ambos paths deben tener exactamente 2 segmentos con el mismo padre.
         */
        const segOrigenArr = relativaOrigen.split('/').filter(Boolean);
        const segDestinoArr = relativaDestino.split('/').filter(Boolean);

        const esSubcarpetaRename =
            segOrigenArr.length === 2
            && segDestinoArr.length === 2
            && segOrigenArr[0].toLowerCase() === segDestinoArr[0].toLowerCase()
            && !EXTENSIONES_AUDIO.has(extensionDestino);

        if (esSubcarpetaRename) {
            if (!await esDirectorioReal(destinoNorm)) {
                logSync.warn('watcher', `Rename subcarpeta ignorado: destino no es carpeta real: ${relativaDestino}`);
                return true;
            }
            const carpetaPadre = segDestinoArr[0];
            const subOrigen = segOrigenArr[1];
            const subNuevo = segDestinoArr[1];

            /* Cancelar creacion pendiente del nombre origen si existe */
            const claveSubOrigen = `${carpetaPadre.toLowerCase()}/${subOrigen.toLowerCase()}`;
            const pendienteSub = subcarpetasPendientesCreacion.get(claveSubOrigen);
            if (pendienteSub) {
                clearTimeout(pendienteSub.timeout);
                subcarpetasPendientesCreacion.delete(claveSubOrigen);
                logSync.info('watcher', `Creacion subcarpeta cancelada por rename: ${subOrigen} → ${subNuevo}`);
            }

            logSync.info('watcher', `Rename subcarpeta (evento name): ${subOrigen} → ${subNuevo} en ${carpetaPadre}`);
            if (onSubcarpetaRenombrada) {
                onSubcarpetaRenombrada(subOrigen, subNuevo, carpetaPadre, rutaDestino);
            }
            return true;
        }

        /* Guard: solo procesar el move si el destino está dentro de la carpeta base */
        if (EXTENSIONES_AUDIO.has(extensionDestino) && relativaDestino) {
            const partesDestino = relativaDestino.split('/');
            const nombreArchivo = partesDestino.pop() ?? nombreDestino;
            const carpetas = partesDestino.slice(0, 3);
            logSync.info('watcher', `Move archivo (evento name): ${rutaOrigen} → ${rutaDestino}`);
            if (onArchivoMovido) {
                onArchivoMovido(rutaOrigen, rutaDestino, nombreArchivo, carpetas);
                return true;
            }
            if (onArchivoNuevo) {
                onArchivoNuevo(rutaDestino, nombreArchivo, carpetas);
                return true;
            }
        }
    } catch (err) {
        logSync.error('watcher', 'Error procesando evento rename', {
            error: err instanceof Error ? err.message : String(err),
        });
    }
    return false;
}
