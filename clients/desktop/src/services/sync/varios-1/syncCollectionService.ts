/*
 * Servicio: syncCollectionService — C355
 * Barril público: los módulos internos viven en ./syncCollectionService/ y no importan de aquí.
 * Lógica de mapeo colecciones del servidor ↔ carpetas locales en disco.
 */

export type {
    SampleSync,
    ColeccionSync,
    RespuestaSyncColecciones,
    ProgresoSyncColecciones,
    CallbackProgresoColecciones,
} from './syncCollectionService/tipos';

export { esperarCreacionEnVuelo, crearColeccionDesdeLocal } from './syncCollectionService/creacion';
export { moverSampleEntreColecciones, agregarSampleAColeccion, renombrarColeccionEnServidor } from './syncCollectionService/movimiento';
export { obtenerColeccionesDelServidor, buscarColeccionServidorPorNombrePublico } from './syncCollectionService/servidor';
export { buscarColeccionHuerfana, sanitizarNombreCarpeta } from './syncCollectionService/helpers';
export { invalidarCacheColecciones } from './syncCollectionService/estado';
export { sincronizarColecciones } from './syncCollectionService/sincronizar';
