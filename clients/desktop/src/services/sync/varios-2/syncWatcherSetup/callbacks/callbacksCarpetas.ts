/*
 * Callbacks de carpetas (crear / renombrar colecciones) del watcher (C357).
 * trackingModule y colMod llegan por parámetro: snapshot de la inicialización.
 */
import { estado } from '../../syncState';
import { logSync } from '../../../varios-1/syncLogger';
import { CARPETAS_SISTEMA_SYNC } from '../syncWatcherConstantes';
import type { ColeccionModule, TrackingModule } from '../syncWatcherEstado';

type RegistrarCarpetas = typeof import('../../../../varios-1/fileWatcherService').registrarCallbacksCarpeta;

export function crearCallbacksCarpetas(
    trackingModule: TrackingModule,
    colMod: NonNullable<ColeccionModule>,
): [Parameters<RegistrarCarpetas>[0], Parameters<RegistrarCarpetas>[1]] {
    return [
        (nombre: string, _rutaCompleta: string) => {
            /* Excluir carpetas del sistema que NO son colecciones reales */
            if (CARPETAS_SISTEMA_SYNC.includes(nombre.toLowerCase())) {
                logSync.info('syncWatcherSetup', '[Sync] Carpeta de sistema ignorada (no es colección):', nombre);
                return;
            }
            /*
             * Antes de crear una colección nueva, verificar si hay una colección
             * huérfana (su carpeta ya no existe en disco). Esto ocurre cuando el
             * watcher emite CREATE en vez de RENAME: la carpeta vieja desapareció
             * y apareció la nueva, pero no se detectó como rename.
             */
            (async () => {
                try {
                    /*
                     * C287: Si la colección ya existe en tracking, omitir.
                     * Sin esto, cada evento sobre el interior de la carpeta
                     * (crear archivos, subcarpetas) re-dispara este callback
                     * y buscarColeccionHuerfana encuentra colecciones de otro
                     * usuario → 403 en cascada.
                     */
                    if (trackingModule) {
                        const existente = trackingModule.buscarColeccionPorCarpeta(nombre);
                        if (existente) {
                            logSync.info('syncWatcherSetup', '[Sync] Carpeta ya vinculada en tracking, omitiendo:', [nombre, '→ id:', existente.id]);
                            return;
                        }
                    }

                    /*
                     * QL121: Cuando borrarAlSubirExitoso esta activo, las carpetas
                     * se vacian tras subir y pueden desaparecer del disco.
                     * buscarColeccionHuerfana detectaria esas carpetas vacias/borradas
                     * como "huerfanas" y las renombraria a la carpeta nueva, causando
                     * merge destructivo de colecciones. PROHIBIDO usar deteccion de
                     * huerfanas en modo borrar-tras-subida: siempre crear nueva.
                     */
                    if (!estado.configAvanzada.borrarAlSubirExitoso) {
                        const huerfana = await colMod.buscarColeccionHuerfana(nombre);
                        if (huerfana) {
                            logSync.info('syncWatcherSetup', '[Sync] Carpeta nueva detectada como posible rename (huérfana encontrada):', [huerfana.id, huerfana.nombre, '→', nombre]);
                            const exito = await colMod.renombrarColeccionEnServidor(huerfana.id, nombre);
                            if (!exito && trackingModule) {
                                /*
                                 * Si la colección aún existe en tracking (fallo por red/conflict),
                                 * actualizar nombre local como fallback optimista.
                                 * Si ya no existe (403 → desvinculada), crear una nueva.
                                 */
                                const aunExiste = trackingModule.obtenerColeccion(huerfana.id);
                                if (aunExiste) {
                                    await trackingModule.actualizarNombreColeccion(huerfana.id, nombre, nombre);
                                } else {
                                    logSync.info('syncWatcherSetup', '[Sync] Colección huérfana desvinculada (403), creando nueva para:', nombre);
                                    await colMod.crearColeccionDesdeLocal(nombre);
                                }
                            }
                            return;
                        }
                    }
                    logSync.info('syncWatcherSetup', '[Sync] Carpeta nueva detectada → crear colección:', nombre);
                    await colMod.crearColeccionDesdeLocal(nombre);
                } catch (err) {
                    logSync.error('syncWatcherSetup', '[Sync] Error en callback carpeta nueva:', err);
                }
            })();
        },
        (nombreAnterior: string, nombreNuevo: string, _rutaNueva: string) => {
            if (!trackingModule) return;
            /* Excluir carpetas del sistema que NO son colecciones reales */
            if (CARPETAS_SISTEMA_SYNC.includes(nombreNuevo.toLowerCase())) {
                logSync.info('syncWatcherSetup', '[Sync] Rename a carpeta de sistema ignorado:', nombreNuevo);
                return;
            }
            const coleccion = trackingModule.buscarColeccionPorCarpeta(nombreAnterior);
            if (coleccion) {
                logSync.info('syncWatcherSetup', '[Sync] Carpeta renombrada → renombrar colección:', [coleccion.id, nombreAnterior, '→', nombreNuevo]);
                colMod.renombrarColeccionEnServidor(coleccion.id, nombreNuevo)
                    .then(exito => {
                        if (!exito) {
                            trackingModule.actualizarNombreColeccion(coleccion.id, nombreNuevo, nombreNuevo).catch(err => {
                                logSync.error('syncWatcherSetup', '[Sync] Error actualizando nombre local de colección:', err);
                            });
                        }
                    })
                    .catch(err => {
                        logSync.error('syncWatcherSetup', '[Sync] Error renombrando colección:', err);
                    });
            } else {
                /*
                 * Fallback robusto: la colección no se encontró por el nombre anterior.
                 * Buscar en orden: en-vuelo → huérfana (carpeta desaparecida) → servidor → crear nueva.
                 * La detección de huérfana es el caso más probable: la carpeta ya fue renombrada
                 * en disco pero tracking aún tiene el nombre viejo.
                 */
                logSync.info('syncWatcherSetup', '[Sync] Carpeta renombrada sin colección local → verificando en-vuelo/huérfana/servidor:', [nombreAnterior, '→', nombreNuevo]);
                (async () => {
                    try {
                        /* 1. Creación en vuelo del nombre viejo */
                        const idEnVuelo = await colMod.esperarCreacionEnVuelo(nombreAnterior);
                        if (idEnVuelo) {
                            logSync.info('syncWatcherSetup', '[Sync] Creación en-vuelo completada, renombrando:', [idEnVuelo, '→', nombreNuevo]);
                            const exito = await colMod.renombrarColeccionEnServidor(idEnVuelo, nombreNuevo);
                            if (!exito && trackingModule) {
                                await trackingModule.actualizarNombreColeccion(idEnVuelo, nombreNuevo, nombreNuevo);
                            }
                            return;
                        }

                        /* 2. Colección huérfana: buscar en tracking alguna cuya carpeta ya NO existe en disco.
                         * Esto detecta el caso donde buscarColeccionPorCarpeta falla por
                         * diferencia de nombre (sanitización, case, etc.) pero la carpeta
                         * fue efectivamente renombrada → la vieja ya no existe.
                         * QL121: Desactivado con borrarAlSubirExitoso — carpetas vacias/borradas
                         * son normales, no renames. */
                        if (!estado.configAvanzada.borrarAlSubirExitoso) {
                            const huerfana = await colMod.buscarColeccionHuerfana(nombreNuevo);
                            if (huerfana) {
                                logSync.info('syncWatcherSetup', '[Sync] Colección huérfana detectada como rename:', [huerfana.id, huerfana.nombre, '→', nombreNuevo]);
                                const exito = await colMod.renombrarColeccionEnServidor(huerfana.id, nombreNuevo);
                                if (!exito && trackingModule) {
                                    await trackingModule.actualizarNombreColeccion(huerfana.id, nombreNuevo, nombreNuevo);
                                }
                                return;
                            }
                        }

                        /* 3. Buscar en servidor por nombre anterior */
                        const idServidor = await colMod.buscarColeccionServidorPorNombrePublico(nombreAnterior);
                        if (idServidor) {
                            logSync.info('syncWatcherSetup', '[Sync] Colección encontrada en servidor por nombre anterior, renombrando:', [idServidor, '→', nombreNuevo]);
                            const exito = await colMod.renombrarColeccionEnServidor(idServidor, nombreNuevo);
                            if (!exito && trackingModule) {
                                await trackingModule.actualizarNombreColeccion(idServidor, nombreNuevo, nombreNuevo);
                            }
                        } else {
                            logSync.info('syncWatcherSetup', '[Sync] Colección no existe en ningún registro → crear nueva:', nombreNuevo);
                            await colMod.crearColeccionDesdeLocal(nombreNuevo);
                        }
                    } catch (err) {
                        logSync.error('syncWatcherSetup', '[Sync] Error en fallback rename de colección:', err);
                    }
                })();
            }
        },
    ];
}
