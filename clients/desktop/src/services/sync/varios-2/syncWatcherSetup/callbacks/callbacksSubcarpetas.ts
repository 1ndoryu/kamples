/*
 * Callbacks de subcarpetas (subcolecciones, nivel 2) del watcher (C387).
 * trackingModule y colMod llegan por parámetro: snapshot de la inicialización.
 */
import { logSync } from '../../../varios-1/syncLogger';
import { CARPETAS_SISTEMA_SYNC } from '../syncWatcherConstantes';
import type { ColeccionModule, TrackingModule } from '../syncWatcherEstado';

type RegistrarSubcarpetas = typeof import('../../../../varios-1/fileWatcherService').registrarCallbacksSubcarpeta;

export function crearCallbacksSubcarpetas(
    trackingModule: TrackingModule,
    colMod: NonNullable<ColeccionModule>,
): [Parameters<RegistrarSubcarpetas>[0], Parameters<RegistrarSubcarpetas>[1]] {
    return [
        (nombreSub: string, carpetaPadre: string, rutaCompleta: string) => {
            if (CARPETAS_SISTEMA_SYNC.includes(nombreSub.toLowerCase())) return;
            if (!trackingModule) {
                logSync.warn('syncWatcherSetup', '[Sync] Subcarpeta nueva ignorada (sin tracking):', nombreSub);
                return;
            }

            /*
             * C3: Carpeta creada dentro de "Sin colección" → mover fuera como colección.
             * crearColeccionDesdeLocal es idempotente, así que si el watcher re-dispara
             * al detectar la carpeta nueva en root, no crea duplicados.
             */
            if (CARPETAS_SISTEMA_SYNC.includes(carpetaPadre.toLowerCase())) {
                (async () => {
                    try {
                        const { rename, exists } = await import('@tauri-apps/plugin-fs');
                        const { join, dirname } = await import('@tauri-apps/api/path');

                        const dirPadre = await dirname(rutaCompleta);
                        const rootSync = await dirname(dirPadre);
                        const nuevaRuta = await join(rootSync, nombreSub);

                        if (await exists(nuevaRuta)) {
                            logSync.warn('syncWatcherSetup', '[Sync] Destino ya existe, no se puede mover fuera de Sin colección:', nuevaRuta);
                            return;
                        }

                        await rename(rutaCompleta, nuevaRuta);
                        logSync.info('syncWatcherSetup', '[Sync] C3: Carpeta movida fuera de Sin colección:', nombreSub);
                        await colMod.crearColeccionDesdeLocal(nombreSub);
                    } catch (err) {
                        logSync.error('syncWatcherSetup', '[Sync] Error moviendo carpeta fuera de Sin colección:', err);
                    }
                })();
                return;
            }

            const padre = trackingModule.buscarColeccionPorCarpeta(carpetaPadre);
            if (!padre) {
                logSync.warn('syncWatcherSetup', '[Sync] Subcarpeta nueva ignorada (padre no encontrado):', [carpetaPadre, '/', nombreSub]);
                return;
            }

            /* QL103: Guard — omitir si la subcolección ya existe en tracking */
            const subExistente = trackingModule.buscarSubcoleccion(carpetaPadre, nombreSub);
            if (subExistente) {
                logSync.info('syncWatcherSetup', '[Sync] Subcarpeta ya vinculada en tracking, omitiendo:', [carpetaPadre, '/', nombreSub, '→ id:', subExistente.id]);
                return;
            }

            logSync.info('syncWatcherSetup', '[Sync] Subcarpeta nueva → crear subcolección:', [nombreSub, 'en', carpetaPadre, '(padreId:', padre.id, ')']);
            colMod.crearColeccionDesdeLocal(nombreSub, padre.id).catch(err => {
                logSync.error('syncWatcherSetup', '[Sync] Error creando subcolección desde subcarpeta local:', err);
            });
        },
        (nombreAnterior: string, nombreNuevo: string, carpetaPadre: string, _rutaNueva: string) => {
            if (!trackingModule) return;
            if (CARPETAS_SISTEMA_SYNC.includes(nombreNuevo.toLowerCase())) return;

            const sub = trackingModule.buscarSubcoleccion(carpetaPadre, nombreAnterior);
            if (sub) {
                logSync.info('syncWatcherSetup', '[Sync] Subcarpeta renombrada → renombrar subcolección:', [sub.id, nombreAnterior, '→', nombreNuevo]);
                colMod.renombrarColeccionEnServidor(sub.id, nombreNuevo)
                    .then(exito => {
                        if (!exito && trackingModule) {
                            trackingModule.actualizarNombreColeccion(sub.id, nombreNuevo, nombreNuevo).catch(err => {
                                logSync.error('syncWatcherSetup', '[Sync] Error actualizando nombre local de subcolección:', err);
                            });
                        }
                    })
                    .catch(err => {
                        logSync.error('syncWatcherSetup', '[Sync] Error renombrando subcolección:', err);
                    });
            } else {
                /* Padre podría no estar en tracking aún — crear la subcolección como nueva */
                const padre = trackingModule.buscarColeccionPorCarpeta(carpetaPadre);
                if (padre) {
                    logSync.info('syncWatcherSetup', '[Sync] Subcarpeta renombrada sin sub local → crear nueva:', [nombreNuevo, 'en padreId:', padre.id]);
                    colMod.crearColeccionDesdeLocal(nombreNuevo, padre.id).catch(err => {
                        logSync.error('syncWatcherSetup', '[Sync] Error creando subcolección tras rename:', err);
                    });
                } else {
                    logSync.warn('syncWatcherSetup', '[Sync] Subcarpeta renombrada ignorada (padre y sub no encontrados):', [carpetaPadre, '/', nombreAnterior]);
                }
            }
        },
    ];
}
