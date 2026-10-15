/*
 * Reconciliación de integridad (disco vs tracking) y de estructura de carpetas (C288).
 */
import { logSync } from '../../varios-1/syncLogger';
import { estado } from '../syncState';
import { CARPETAS_SISTEMA_SYNC, CARPETAS_EXCLUIDAS_SCAN } from './syncWatcherConstantes';

/*
 * F3.2: Reconciliación — compara disco vs tracking para detectar divergencias.
 * Exportado para que el panel de sync pueda ejecutarla bajo demanda.
 */
export async function ejecutarReconciliacion(): Promise<void> {
    const { config, trackingModule } = estado;
    if (!config.carpetaLocal || !trackingModule) return;

    const { detectarDivergencias } = await import('../../varios-1/syncReconciliacion');

    const todosArchivos = trackingModule.todosLosArchivos();
    const archivosParaReconciliar = Object.values(todosArchivos).map(a => ({
        ruta: a.rutaLocal,
        sampleId: a.sampleId,
        tamano: a.tamano,
        coleccionId: a.coleccionId,
    }));

    const colecciones = trackingModule.todasLasColecciones().map(c => ({
        id: c.id,
        carpetaLocal: c.carpetaLocal,
    }));

    const resultado = await detectarDivergencias(
        config.carpetaLocal,
        archivosParaReconciliar,
        colecciones,
    );

    if (resultado.divergencias.length > 0) {
        logSync.warn('syncWatcher', `Reconciliación: ${resultado.divergencias.length} divergencias detectadas`, {
            tiempoMs: resultado.duracionMs,
            enDisco: resultado.archivosEnDisco,
            enTracking: resultado.archivosEnTracking,
        });
    } else {
        logSync.info('syncWatcher', `Reconciliación OK: ${resultado.archivosEnDisco} archivos verificados en ${resultado.duracionMs}ms`);
    }
}

/*
 * C288: Reconciliación periódica de estructura de carpetas.
 *
 * El debounced watcher de Tauri (notify-rs) NO emite rename events fiables
 * en Windows con OneDrive. El cloud filter driver (cldflt.sys) absorbe los
 * rename y no los propaga a notify. Resultado: renombrar una carpeta o
 * subcarpeta no genera NINGÚN evento en el watcher → nombre local diverge
 * del servidor silenciosamente.
 *
 * Solución: escanear periódicamente la estructura de carpetas en disco,
 * comparar con tracking, y detectar renames a través de reconciliación:
 * - Carpeta en tracking cuya carpetaLocal NO existe en disco = "desaparecida"
 * - Carpeta en disco que NO tiene tracking = "nueva"
 * - Si hay exactamente 1 desaparecida y 1 nueva dentro del mismo padre → rename
 *
 * Frecuencia: cada RECONCILIACION_CARPETAS_MS (15s). Es un listado de directorios
 * ligero (readDir), no un hash/stat de archivos.
 */
export const RECONCILIACION_CARPETAS_MS = 15_000;

export async function reconciliarEstructuraCarpetas(): Promise<void> {
    const { config, trackingModule, collectionModule } = estado;
    if (!config.carpetaLocal || !trackingModule || !collectionModule) return;

    /*
     * QL121: Cuando borrarAlSubirExitoso esta activo, las carpetas de colecciones
     * anteriores se vacian (archivos borrados tras subir) y pueden ser eliminadas
     * por el usuario o el OS. La reconciliacion interpreta esto como "rename"
     * (1 desaparecida + 1 nueva = rename) y RENOMBRA la coleccion vieja a la nueva,
     * causando merge destructivo. SOLUCION: omitir reconciliacion de renames en este
     * modo. Las carpetas nuevas se crean correctamente via el callback onCarpetaNueva.
     */
    if (estado.configAvanzada.borrarAlSubirExitoso) {
        logSync.debug('syncWatcher', 'Reconciliacion omitida — borrarAlSubirExitoso activo (QL121)');
        return;
    }

    try {
        const { readDir, exists } = await import('@tauri-apps/plugin-fs');
        const { join } = await import('@tauri-apps/api/path');

        const carpetaBase = config.carpetaLocal;
        const colMod = collectionModule;
        const tracking = trackingModule;

        /* Nivel 1: carpetas raíz = colecciones principales (parentId null) */
        const entradas = await readDir(carpetaBase);
        const carpetasEnDisco = new Set<string>();

        for (const entrada of entradas) {
            if (!entrada.name || !entrada.isDirectory) continue;
            const nombreLower = entrada.name.toLowerCase();
            if (CARPETAS_SISTEMA_SYNC.includes(nombreLower)) continue;
            if (CARPETAS_EXCLUIDAS_SCAN.includes(nombreLower)) continue;
            carpetasEnDisco.add(entrada.name);
        }

        const coleccionesRaiz = tracking.todasLasColecciones().filter(c => c.parentId === null);

        /* Clasificar: desaparecidas (en tracking pero no en disco) y nuevas (en disco pero no en tracking) */
        const desaparecidas: Array<{ id: number; nombre: string; carpetaLocal: string }> = [];
        for (const col of coleccionesRaiz) {
            let encontrada = false;
            for (const enDisco of carpetasEnDisco) {
                if (enDisco.toLowerCase() === col.carpetaLocal.toLowerCase()
                    || enDisco.toLowerCase() === col.nombre.toLowerCase()) {
                    encontrada = true;
                    break;
                }
            }
            if (!encontrada) {
                /* Verificar con exists() por si hay diferencia de encoding/case */
                const rutaCol = await join(carpetaBase, col.carpetaLocal);
                if (!await exists(rutaCol)) {
                    desaparecidas.push({ id: col.id, nombre: col.nombre, carpetaLocal: col.carpetaLocal });
                }
            }
        }

        const nuevas: string[] = [];
        for (const enDisco of carpetasEnDisco) {
            const tracked = tracking.buscarColeccionPorCarpeta(enDisco);
            if (!tracked) {
                nuevas.push(enDisco);
            }
        }

        /* Parear: si hay exactamente 1 desaparecida y 1 nueva → rename */
        if (desaparecidas.length === 1 && nuevas.length === 1) {
            const vieja = desaparecidas[0];
            const nueva = nuevas[0];
            logSync.info('syncWatcher',
                `Reconciliación: rename carpeta detectado: ${vieja.carpetaLocal} → ${nueva}`);

            const exito = await colMod.renombrarColeccionEnServidor(vieja.id, nueva);
            if (!exito) {
                await tracking.actualizarNombreColeccion(vieja.id, nueva, nueva);
            }
        } else if (desaparecidas.length > 0 && nuevas.length > 0) {
            /*
             * Múltiples renames simultáneos: no podemos parear con certeza.
             * Intentar parear por proximidad temporal (todas del mismo batch).
             * Fallback: crear nuevas + dejar huérfanas para cleanup posterior.
             */
            const pareados = Math.min(desaparecidas.length, nuevas.length);
            for (let i = 0; i < pareados; i++) {
                const vieja = desaparecidas[i];
                const nueva = nuevas[i];
                logSync.info('syncWatcher',
                    `Reconciliación: rename múltiple ${i + 1}/${pareados}: ${vieja.carpetaLocal} → ${nueva}`);
                const exito = await colMod.renombrarColeccionEnServidor(vieja.id, nueva);
                if (!exito) {
                    await tracking.actualizarNombreColeccion(vieja.id, nueva, nueva);
                }
            }
        }

        /* Nivel 2: subcarpetas dentro de cada colección raíz */
        for (const entrada of entradas) {
            if (!entrada.name || !entrada.isDirectory) continue;
            const nombreLower = entrada.name.toLowerCase();
            if (CARPETAS_SISTEMA_SYNC.includes(nombreLower)) continue;
            if (CARPETAS_EXCLUIDAS_SCAN.includes(nombreLower)) continue;

            const padre = tracking.buscarColeccionPorCarpeta(entrada.name);
            if (!padre) continue;

            const rutaCarpeta = await join(carpetaBase, entrada.name);
            let subEntradas;
            try {
                subEntradas = await readDir(rutaCarpeta);
            } catch {
                continue;
            }

            const subsEnDisco = new Set<string>();
            for (const sub of subEntradas) {
                if (!sub.name || !sub.isDirectory) continue;
                if (CARPETAS_SISTEMA_SYNC.includes(sub.name.toLowerCase())) continue;
                if (CARPETAS_EXCLUIDAS_SCAN.includes(sub.name.toLowerCase())) continue;
                subsEnDisco.add(sub.name);
            }

            const subcolecciones = tracking.subcoleccionesDePadre(padre.id);

            const subsDesaparecidas: Array<{ id: number; nombre: string; carpetaLocal: string }> = [];
            for (const sub of subcolecciones) {
                let encontrada = false;
                for (const enDisco of subsEnDisco) {
                    if (enDisco.toLowerCase() === sub.carpetaLocal.toLowerCase()
                        || enDisco.toLowerCase() === sub.nombre.toLowerCase()) {
                        encontrada = true;
                        break;
                    }
                }
                if (!encontrada) {
                    const rutaSub = await join(rutaCarpeta, sub.carpetaLocal);
                    if (!await exists(rutaSub)) {
                        subsDesaparecidas.push({ id: sub.id, nombre: sub.nombre, carpetaLocal: sub.carpetaLocal });
                    }
                }
            }

            const subsNuevas: string[] = [];
            for (const enDisco of subsEnDisco) {
                const tracked = tracking.buscarSubcoleccion(entrada.name, enDisco);
                if (!tracked) {
                    subsNuevas.push(enDisco);
                }
            }

            if (subsDesaparecidas.length === 1 && subsNuevas.length === 1) {
                const vieja = subsDesaparecidas[0];
                const nueva = subsNuevas[0];
                logSync.info('syncWatcher',
                    `Reconciliación: rename subcarpeta detectado en ${entrada.name}: ${vieja.carpetaLocal} → ${nueva}`);
                const exito = await colMod.renombrarColeccionEnServidor(vieja.id, nueva);
                if (!exito) {
                    await tracking.actualizarNombreColeccion(vieja.id, nueva, nueva);
                }
            } else if (subsDesaparecidas.length > 0 && subsNuevas.length > 0) {
                const pareados = Math.min(subsDesaparecidas.length, subsNuevas.length);
                for (let i = 0; i < pareados; i++) {
                    const vieja = subsDesaparecidas[i];
                    const nueva = subsNuevas[i];
                    logSync.info('syncWatcher',
                        `Reconciliación: rename subcarpeta múltiple ${i + 1}/${pareados} en ${entrada.name}: ${vieja.carpetaLocal} → ${nueva}`);
                    const exito = await colMod.renombrarColeccionEnServidor(vieja.id, nueva);
                    if (!exito) {
                        await tracking.actualizarNombreColeccion(vieja.id, nueva, nueva);
                    }
                }
            }
        }
    } catch (err) {
        logSync.warn('syncWatcher', 'Error en reconciliación de estructura de carpetas', {
            error: err instanceof Error ? err.message : String(err),
        });
    }
}
