/*
 * Servicio: syncTrackingService — C355
 * Barril: la implementación vive en ./syncTrackingService/ (ver ahí cada dominio).
 * Importadores sin cambios: este archivo conserva los mismos exports.
 */

export { generarClaveTracking } from './syncTrackingService/claves';

export type {
    AccionHistorial,
    ArchivoTracking,
    BaseSyncLocal,
    ColeccionLocal,
    EntradaHistorialSample,
    EstadoSampleHistorial,
    ResumenDebugTracking,
    TipoAccionHistorial,
} from './syncTrackingService/tipos';

export { inicializarTracking } from './syncTrackingService/inicializacion';
export { finalizarLote, iniciarLote } from './syncTrackingService/persistencia/persistencia';

export {
    obtenerArchivo, buscarArchivoPorSampleId, buscarArchivoPorRuta, buscarArchivoPorNombre,
    registrarArchivo, eliminarArchivo, marcarSyncDeshabilitado, reactivarSync,
    reactivarTodosSyncDeshabilitados, listarArchivosPorColeccion, todosLosArchivos,
    totalArchivos, espacioTotalBytes,
} from './syncTrackingService/archivos';

export {
    obtenerColeccion, todasLasColecciones, registrarColeccion, eliminarColeccion,
    buscarSubcoleccion, subcoleccionesDePadre, actualizarNombreColeccion,
    buscarColeccionPorCarpeta, esSinColeccion, agregarSinColeccion, quitarSinColeccion,
    totalSinColeccion,
} from './syncTrackingService/colecciones';

export {
    actualizarEstadoSample, obtenerHistorialSamples, recargarHistorialDesdeStore,
    limpiarHistorialSamples, registrarAccion, obtenerHistorial, limpiarHistorial,
} from './syncTrackingService/historial';

export { obtenerResumenDebugTracking, obtenerDatosSync } from './syncTrackingService/resumen';
export { migrarDesdeV1 } from './syncTrackingService/persistencia/migracion';
export { resetearTracking } from './syncTrackingService/reset';
