/* sentinel-disable-file: Fachada pública de sync — centraliza imports/re-exports y ahora
 * el reporte de verificación. Por diseño concentra interfaces, no lógica de dominio. */

/*
 * Servicio: syncService — Fachada pública del sistema de sincronización.
 *
 * TA6: Refactorizado como fachada slim. La lógica se distribuyó en:
 * - syncInitService.ts: inicialización, config, migración v1→v2
 * - syncOrchestratorService.ts: sync principal, individual, resync
 * - syncRegistroService.ts: registro de descargas/subidas, movimiento de archivos
 * - syncRehidratacionService.ts: rehidratación de imágenes de portada
 * - syncService/: configuración, consultas y reporte de verificación (re-exportados aquí)
 *
 * Todos los importadores externos siguen apuntando a este archivo (fachada).
 * Los módulos internos adicionales se mantienen:
 * - syncState.ts: estado compartido + persistencia
 * - syncDownloadV1.ts: lógica legacy v1
 * - syncWatcherSetup.ts: watcher bidireccional + operaciones locales
 * - syncTrackingService.ts: persistencia tipada v2
 * - syncCollectionService.ts: mapeo colecciones ↔ carpetas
 * - syncGuards.ts: guards de descarga + base URL centralizada
 */

import {
    detenerSyncBidireccional as detenerBidireccional,
    marcarNoSincronizar as _marcarNoSincronizar,
    marcarNoSincronizarPorId as _marcarNoSincronizarPorId,
    reactivarSync as _reactivarSync,
    obtenerEstadoSync as _obtenerEstadoSync,
    obtenerSamplesNoSincronizados as _obtenerSamplesNoSincronizados,
    moverSampleEnServidorPublico as _moverSampleEnServidorPublico,
} from './syncWatcherSetup';

/* Re-exports de tipos para mantener API pública sin romper importadores */
export { type ProgresoSync, type ProgressCallback, type SyncConfig } from './syncState';

/* Re-exports de módulos extraídos */
export { inicializarSyncService } from '../varios-1/syncInitService';
export {
    sincronizarConServidor,
    sincronizarSampleIndividual,
    forzarResync,
    reforzarSync,
} from '../varios-1/syncOrchestratorService';
export {
    registrarDescarga,
    registrarAccionHistorial,
    registrarSubidaLocal,
    moverArchivoASinColeccion,
    actualizarEstadoSampleHistorial,
} from './syncRegistroService';
export {
    rehidratarImagenesPendientesSync,
    rehidratarImagenesPendientesForzadoSync,
} from './syncRehidratacionService';

/* Re-exports de operaciones del watcher */
export const marcarNoSincronizar = _marcarNoSincronizar;
export const marcarNoSincronizarPorId = _marcarNoSincronizarPorId;
export const reactivarSync = _reactivarSync;
export const obtenerEstadoSync = _obtenerEstadoSync;
export const obtenerSamplesNoSincronizados = _obtenerSamplesNoSincronizados;
export const moverSampleEnServidorPublico = _moverSampleEnServidorPublico;
export const detenerSyncBidireccional = detenerBidireccional;

/* Re-exports de módulos en syncService/ */
export {
    elegirCarpetaSync,
    toggleSincronizacion,
    obtenerConfigSync,
    haySyncEnCurso,
    abrirCarpetaSync,
} from './syncService/syncConfigService';
export {
    obtenerRutaLocal,
    extraerMetadataDeRuta,
    obtenerHistorialSync,
    obtenerHistorialSamplesSync,
    limpiarHistorialSync,
    recargarHistorialDesdeStore,
    obtenerColeccionesSync,
} from './syncService/syncConsultasService';
export type { SyncReport } from './syncService/syncReporteTipos';
export { generarReporteSync } from './syncService/syncReporteService';
