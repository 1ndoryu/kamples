/*
 * Servicio: fileWatcherService — Observador de carpeta de sincronización.
 *
 * Barril público: la implementación vive en ./fileWatcherService/ (estado,
 * eventos por tipo, despacho y ciclo de vida). Los importadores siguen usando
 * esta ruta sin cambios.
 */

export {
    registrarCallbacks,
    registrarCallbacksCarpeta,
    registrarCallbacksSubcarpeta,
} from './fileWatcherService/fileWatcherRegistro';
export {
    iniciarObservacion,
    detenerObservacion,
    estaObservando,
} from './fileWatcherService/fileWatcherObservacion';
