/*
 * Escaneo local: encola los audios de la carpeta de sync que el watcher no vio
 * (arranque o copias con la app cerrada). Idempotente.
 */
import { logSync } from '../../varios-1/syncLogger';
import { estado, guardarIndice, buscarEnIndicePorRuta, type ArchivoLocal } from '../syncState';
import { EXTENSIONES_AUDIO_SCAN, CARPETAS_SISTEMA_SYNC, CARPETAS_EXCLUIDAS_SCAN } from './syncWatcherConstantes';

interface ResumenEscaneoLocal {
    directoriosEscaneados: number;
    audiosDetectados: number;
    encolados: number;
    yaSincronizados: number;
    staleLegacyLimpiados: number;
    carpetasIgnoradas: number;
    errores: number;
}

/*
 * Escanea la carpeta de sync en busca de archivos de audio locales que no
 * están en el tracking (no fueron detectados por el watcher — arranque,
 * archivos copiados mientras la app estaba cerrada, etc.) y los encola para
 * subida. Recorre toda la jerarquía sin límite artificial de profundidad.
 *
 * Idempotente: encolarArchivo ya tiene guards de dedup (ruta + hash).
 * Se puede llamar múltiples veces sin efectos secundarios.
 */
export async function escanearCarpetaYEncolar(): Promise<number> {
    const { config, trackingModule } = estado;
    if (!config.carpetaLocal || !config.sincronizacionActiva) {
        logSync.warn('syncWatcher', 'escanearCarpetaYEncolar ABORTADO — config incompleta', {
            carpetaLocal: !!config.carpetaLocal,
            sincronizacionActiva: config.sincronizacionActiva,
        });
        return 0;
    }

    try {
        const { readDir } = await import('@tauri-apps/plugin-fs');
        const { join } = await import('@tauri-apps/api/path');
        const { encolarArchivo } = await import('../../../varios-2/uploadQueueService');

        const carpetaBase = config.carpetaLocal;
        const resumen: ResumenEscaneoLocal = {
            directoriosEscaneados: 0,
            audiosDetectados: 0,
            encolados: 0,
            yaSincronizados: 0,
            staleLegacyLimpiados: 0,
            carpetasIgnoradas: 0,
            errores: 0,
        };

        const procesarArchivo = async (
            rutaArchivo: string,
            nombreArchivo: string,
            carpetas: string[],
        ): Promise<void> => {
            const rutaNorm = rutaArchivo.replace(/\\/g, '/');
            resumen.audiosDetectados++;

            if (trackingModule) {
                const enTracking = trackingModule.buscarArchivoPorRuta(rutaNorm);
                if (enTracking && !enTracking.syncDeshabilitado) {
                    resumen.yaSincronizados++;
                    return;
                }
            }
            /* QL130: Si tracking v2 no tiene el archivo pero indice legacy si,
             * la entrada es stale (ej: samples eliminados del servidor). Limpiar. */
            const porRutaScan = buscarEnIndicePorRuta(rutaNorm);
            if (porRutaScan) {
                const idx = estado.indiceArchivos.indexOf(porRutaScan);
                if (idx >= 0) estado.indiceArchivos.splice(idx, 1);
                estado.indiceArchivosPorRuta.delete(rutaNorm);
                if (porRutaScan.nombreServidor) estado.indiceArchivosPorNombre.delete(porRutaScan.nombreServidor);
                if (porRutaScan.nombreOriginal && porRutaScan.nombreOriginal !== porRutaScan.nombreServidor) {
                    estado.indiceArchivosPorNombre.delete(porRutaScan.nombreOriginal);
                }
                guardarIndice();
                resumen.staleLegacyLimpiados++;
            }

            const fueEncolado = await encolarArchivo(rutaArchivo, nombreArchivo, carpetas);
            if (fueEncolado) {
                resumen.encolados++;
            }
        };

        const escanearDirectorio = async (
            rutaDirectorio: string,
            carpetasServidor: string[],
            sinColeccionDesdeAqui: boolean,
        ): Promise<void> => {
            resumen.directoriosEscaneados++;

            let entradas;
            try {
                entradas = await readDir(rutaDirectorio);
            } catch (errDir) {
                resumen.errores++;
                logSync.warn('syncWatcher', 'Error escaneando directorio local', {
                    rutaDirectorio,
                    error: errDir instanceof Error ? errDir.message : String(errDir),
                });
                return;
            }

            for (const entrada of entradas) {
                if (!entrada.name) continue;

                const nombreLower = entrada.name.toLowerCase();
                const rutaEntrada = await join(rutaDirectorio, entrada.name);

                if (entrada.isDirectory) {
                    if (CARPETAS_EXCLUIDAS_SCAN.has(nombreLower)) {
                        resumen.carpetasIgnoradas++;
                        continue;
                    }

                    const esCarpetaSistema = CARPETAS_SISTEMA_SYNC.has(nombreLower);
                    const siguienteSinColeccion = sinColeccionDesdeAqui || esCarpetaSistema;
                    const siguientesCarpetas = siguienteSinColeccion
                        ? []
                        : [...carpetasServidor, entrada.name];

                    await escanearDirectorio(rutaEntrada, siguientesCarpetas, siguienteSinColeccion);
                    continue;
                }

                const ext = entrada.name.split('.').pop()?.toLowerCase() ?? '';
                if (!EXTENSIONES_AUDIO_SCAN.has(ext)) continue;
                await procesarArchivo(rutaEntrada, entrada.name, carpetasServidor);
            }
        };

        await escanearDirectorio(carpetaBase, [], false);

        logSync.info('syncWatcher', 'Escaneo local completado', resumen);
        return resumen.encolados;
    } catch (err) {
        logSync.error('syncWatcher', 'Error en escaneo de carpeta local', {
            error: err instanceof Error ? err.message : String(err),
        });
        return 0;
    }
}
