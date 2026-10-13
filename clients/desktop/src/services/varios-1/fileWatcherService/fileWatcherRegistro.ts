/*
 * Registro de callbacks externos del watcher (fileWatcherService).
 * Los callbacks se guardan en fileWatcherEstado; aquí solo se expone la API pública.
 */

import {
    setCallbacksArchivos,
    setCallbacksCarpetas,
    setCallbacksSubcarpetas,
} from './fileWatcherEstado';
import type {
    OnArchivoEliminadoFn,
    OnArchivoMovidoFn,
    OnArchivoNuevoFn,
    OnCarpetaNuevaFn,
    OnCarpetaRenombradaFn,
    OnSubcarpetaNuevaFn,
    OnSubcarpetaRenombradaFn,
} from './fileWatcherTipos';

/*
 * Registra los callbacks externos para archivos nuevos/eliminados/movidos.
 * Se llama desde syncService al inicializar.
 */
export function registrarCallbacks(
    cbNuevo: OnArchivoNuevoFn,
    cbEliminado: OnArchivoEliminadoFn,
    cbMovido?: OnArchivoMovidoFn,
): void {
    setCallbacksArchivos(cbNuevo, cbEliminado, cbMovido);
}

/*
 * C357: Registra callbacks para eventos de carpetas de nivel 1 (colecciones).
 * Separados de los callbacks de archivos para mantener SRP.
 */
export function registrarCallbacksCarpeta(
    cbNueva: OnCarpetaNuevaFn,
    cbRenombrada: OnCarpetaRenombradaFn,
): void {
    setCallbacksCarpetas(cbNueva, cbRenombrada);
}

/*
 * C387: Registra callbacks para eventos de subcarpetas de nivel 2 (subcolecciones).
 * carpetaPadre identifica en qué colección se creó la subcarpeta.
 */
export function registrarCallbacksSubcarpeta(
    cbNueva: OnSubcarpetaNuevaFn,
    cbRenombrada: OnSubcarpetaRenombradaFn,
): void {
    setCallbacksSubcarpetas(cbNueva, cbRenombrada);
}
