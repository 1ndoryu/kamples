/*
 * Eventos de subcarpetas de nivel 2 (subcolecciones) del watcher (fileWatcherService).
 * Lógica análoga a las carpetas de nivel 1, con clave compuesta carpetaPadre/nombreSub.
 */

import { esDescargaMasivaEnCurso } from '../../../sync/varios-1/syncGuards';
import { logSync } from '../../../sync/varios-1/syncLogger';
import {
    DEBOUNCE_CARPETA_MS,
    DELAY_CARPETA_TEMPORAL_MS,
    DELAY_CREACION_CARPETA_MS,
    GRACIA_RENAME_CARPETA_MS,
    NOMBRES_CARPETA_TEMPORAL,
} from '../fileWatcherConstantes';
import {
    estadoFileWatcher,
    onSubcarpetaNueva,
    onSubcarpetaRenombrada,
} from '../fileWatcherEstado';
import type { SubcarpetaEliminadaPendiente } from '../fileWatcherTipos';
import { esEventoCreacion, esEventoEliminacion } from '../fileWatcherUtilidades';

/*
 * C387: Procesa eventos de subcarpetas de nivel 2 (subcolecciones).
 * Lógica análoga a procesarEventoCarpeta pero con clave compuesta
 * carpetaPadre/nombreSub y callbacks dedicados (onSubcarpetaNueva/Renombrada).
 *
 * Rename de subcarpeta emite: remove(padre/viejo) + create(padre/nuevo).
 * El buffer de eliminaciones se busca filtrando por el mismo carpetaPadre.
 */
export function procesarEventoSubcarpeta(
    tipo: unknown,
    rutaCompleta: string,
    nombreSubcarpeta: string,
    carpetaPadre: string,
    _baseNormalizada: string,
): void {
    /* QL103: Suprimir eventos de subcarpeta durante sync masivo */
    if (esDescargaMasivaEnCurso()) return;

    const claveCompuesta = `${carpetaPadre.toLowerCase()}/${nombreSubcarpeta.toLowerCase()}`;

    if (esEventoCreacion(tipo)) {
        /* Verificar si hay una subcarpeta eliminada pendiente del mismo padre → rename */
        const pendiente = encuentraSubcarpetaEliminadaPendiente(carpetaPadre);

        if (pendiente) {
            clearTimeout(pendiente.timeout);
            const clavePendiente = `${pendiente.carpetaPadre.toLowerCase()}/${pendiente.nombre.toLowerCase()}`;
            estadoFileWatcher.subcarpetasEliminadasPendientes.delete(clavePendiente);

            /* Cancelar creacion pendiente del nombre viejo si existía */
            const claveCreacionVieja = `${pendiente.carpetaPadre.toLowerCase()}/${pendiente.nombre.toLowerCase()}`;
            const creacionPendiente = estadoFileWatcher.subcarpetasPendientesCreacion.get(claveCreacionVieja);
            if (creacionPendiente) {
                clearTimeout(creacionPendiente.timeout);
                estadoFileWatcher.subcarpetasPendientesCreacion.delete(claveCreacionVieja);
            }

            logSync.info('watcher', `Rename subcarpeta detectado: ${pendiente.nombre} → ${nombreSubcarpeta} en ${carpetaPadre}`);

            if (onSubcarpetaRenombrada) {
                onSubcarpetaRenombrada(pendiente.nombre, nombreSubcarpeta, carpetaPadre, rutaCompleta);
            }
            return;
        }

        /* Debounce: ignorar si fue procesada recientemente */
        const ahora = Date.now();
        const ultimoProcesada = estadoFileWatcher.subcarpetasRecientes.get(claveCompuesta);
        if (ultimoProcesada && (ahora - ultimoProcesada) < DEBOUNCE_CARPETA_MS) return;
        estadoFileWatcher.subcarpetasRecientes.set(claveCompuesta, ahora);

        logSync.info('watcher', `Subcarpeta nueva detectada, esperando rename: ${nombreSubcarpeta} en ${carpetaPadre}`);

        /* Delay para dar tiempo a rename de Windows ("Nueva carpeta" → nombre real)
         * C1: Misma lógica de carpetas temporales aplicada a subcarpetas. */
        const pendienteExistente = estadoFileWatcher.subcarpetasPendientesCreacion.get(claveCompuesta);
        if (pendienteExistente) clearTimeout(pendienteExistente.timeout);

        const esTemporalSub = NOMBRES_CARPETA_TEMPORAL.includes(nombreSubcarpeta.toLowerCase())
            || /^nueva carpeta\s*\(\d+\)$/i.test(nombreSubcarpeta)
            || /^new folder\s*\(\d+\)$/i.test(nombreSubcarpeta);
        const delayMsSub = esTemporalSub ? DELAY_CARPETA_TEMPORAL_MS : DELAY_CREACION_CARPETA_MS;

        const timeoutCreacion = setTimeout(() => {
            estadoFileWatcher.subcarpetasPendientesCreacion.delete(claveCompuesta);
            if (esTemporalSub) {
                logSync.info('watcher', `Subcarpeta temporal ignorada: ${nombreSubcarpeta} en ${carpetaPadre}`);
                return;
            }
            logSync.info('watcher', `Subcarpeta nueva confirmada (sin rename): ${nombreSubcarpeta} en ${carpetaPadre}`);
            if (onSubcarpetaNueva) {
                onSubcarpetaNueva(nombreSubcarpeta, carpetaPadre, rutaCompleta);
            }
        }, delayMsSub);

        estadoFileWatcher.subcarpetasPendientesCreacion.set(claveCompuesta, {
            timeout: timeoutCreacion,
            nombreOriginal: nombreSubcarpeta,
            carpetaPadre,
            rutaCompleta,
        });
    } else if (esEventoEliminacion(tipo)) {
        /* Bufferar eliminación para detectar posible rename */
        const existente = estadoFileWatcher.subcarpetasEliminadasPendientes.get(claveCompuesta);
        if (existente) {
            clearTimeout(existente.timeout);
        }

        const timeout = setTimeout(() => {
            estadoFileWatcher.subcarpetasEliminadasPendientes.delete(claveCompuesta);
            logSync.info('watcher', `Subcarpeta eliminada (no fue rename): ${nombreSubcarpeta} en ${carpetaPadre}`);
        }, GRACIA_RENAME_CARPETA_MS);

        estadoFileWatcher.subcarpetasEliminadasPendientes.set(claveCompuesta, {
            nombre: nombreSubcarpeta,
            carpetaPadre,
            ruta: rutaCompleta,
            timeout,
        });
    }
}

/*
 * C387: Busca la primera subcarpeta eliminada pendiente dentro de un padre específico.
 * Filtra por carpetaPadre para no cruzar renames entre colecciones distintas.
 */
function encuentraSubcarpetaEliminadaPendiente(carpetaPadre: string): SubcarpetaEliminadaPendiente | null {
    const padreNorm = carpetaPadre.toLowerCase();
    for (const [, val] of estadoFileWatcher.subcarpetasEliminadasPendientes) {
        if (val.carpetaPadre.toLowerCase() === padreNorm) return val;
    }
    return null;
}
