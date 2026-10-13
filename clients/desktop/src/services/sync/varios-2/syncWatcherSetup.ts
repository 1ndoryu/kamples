/*
 * Barril de syncWatcherSetup: la lógica vive en ./syncWatcherSetup/.
 * Los importadores (syncService, syncInitService, authDesktopService) no cambian.
 */
export { escanearCarpetaYEncolar } from './syncWatcherSetup/escaneo';
export { marcarNoSincronizar, marcarNoSincronizarPorId, reactivarSync, obtenerEstadoSync, obtenerSamplesNoSincronizados } from './syncWatcherSetup/muestras';
export { moverSampleEnServidorPublico } from './syncWatcherSetup/moverSample';
export { inicializarSyncBidireccional, detenerSyncBidireccional } from './syncWatcherSetup/init';
export { ejecutarReconciliacion } from './syncWatcherSetup/reconciliacion';
