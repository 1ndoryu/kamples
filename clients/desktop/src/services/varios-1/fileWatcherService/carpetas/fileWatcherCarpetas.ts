/*
 * Eventos de carpetas de nivel 1 (colecciones) del watcher (fileWatcherService).
 * Detecta creación y rename (delete+create con diferente nombre).
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
    carpetasEliminadasPendientes,
    carpetasPendientesCreacion,
    carpetasRecientes,
    onCarpetaNueva,
    onCarpetaRenombrada,
} from '../fileWatcherEstado';
import type { CarpetaEliminadaPendiente } from '../fileWatcherTipos';
import { esEventoCreacion, esEventoEliminacion } from '../fileWatcherUtilidades';

/*
 * C357: Procesa eventos de carpetas de nivel 1 (colecciones).
 * Detecta creación y rename (delete+create con diferente nombre).
 *
 * Rename de carpeta emite: remove(nombreViejo) + create(nombreNuevo).
 * Se usa buffer temporal similar al de archivos para detectar el patrón.
 */
export function procesarEventoCarpeta(
    tipo: unknown,
    rutaCompleta: string,
    nombreCarpeta: string,
    _baseNormalizada: string,
): void {
    /* QL103: Suprimir eventos de carpeta durante sync masivo (mkdir + writeFile) */
    if (esDescargaMasivaEnCurso()) return;

    if (esEventoCreacion(tipo)) {
        /* Verificar si hay una carpeta eliminada pendiente → es un rename */
        const pendiente = encuentraCarpetaEliminadaPendiente();

        if (pendiente) {
            clearTimeout(pendiente.timeout);
            carpetasEliminadasPendientes.delete(pendiente.nombre);

            /* Cancelar creacion pendiente del nombre viejo (delete+create rename) */
            const clavePendiente = pendiente.nombre.toLowerCase();
            const creacionPendiente = carpetasPendientesCreacion.get(clavePendiente);
            if (creacionPendiente) {
                clearTimeout(creacionPendiente.timeout);
                carpetasPendientesCreacion.delete(clavePendiente);
                logSync.info('watcher', `Creacion cancelada por rename (delete+create): ${pendiente.nombre}`);
            }

            logSync.info('watcher', `Rename de carpeta detectado: ${pendiente.nombre} → ${nombreCarpeta}`);

            if (onCarpetaRenombrada) {
                onCarpetaRenombrada(pendiente.nombre, nombreCarpeta, rutaCompleta);
            }
            return;
        }

        /* Debounce por carpeta: ignorar si fue procesada recientemente.
         * Previene que el par create + modify emita dos callbacks. */
        const claveCarpeta = nombreCarpeta.toLowerCase();
        const ahora = Date.now();
        const ultimoProcesada = carpetasRecientes.get(claveCarpeta);
        if (ultimoProcesada && (ahora - ultimoProcesada) < DEBOUNCE_CARPETA_MS) return;
        carpetasRecientes.set(claveCarpeta, ahora);

        logSync.info('watcher', `Carpeta nueva detectada, esperando rename: ${nombreCarpeta}`);

        /* Delay la creacion para dar tiempo a que Windows complete el rename.
         * Sin esto, "Nueva carpeta" se envia al servidor antes de que el usuario
         * termine de escribir el nombre real.
         * C1: Carpetas con nombres temporales del OS (Nueva carpeta, New folder, etc.)
         * tienen timeout extendido de 60s y NO se crean si expiran — se ignoran. */
        const claveCreacion = nombreCarpeta.toLowerCase();
        const pendienteExistente = carpetasPendientesCreacion.get(claveCreacion);
        if (pendienteExistente) clearTimeout(pendienteExistente.timeout);

        const esTemporal = NOMBRES_CARPETA_TEMPORAL.has(claveCreacion)
            || /^nueva carpeta\s*\(\d+\)$/i.test(nombreCarpeta)
            || /^new folder\s*\(\d+\)$/i.test(nombreCarpeta);
        const delayMs = esTemporal ? DELAY_CARPETA_TEMPORAL_MS : DELAY_CREACION_CARPETA_MS;

        const timeoutCreacion = setTimeout(() => {
            carpetasPendientesCreacion.delete(claveCreacion);
            if (esTemporal) {
                logSync.info('watcher', `Carpeta temporal ignorada (no renombrada a tiempo): ${nombreCarpeta}`);
                return;
            }
            logSync.info('watcher', `Carpeta nueva confirmada (sin rename): ${nombreCarpeta}`);
            if (onCarpetaNueva) {
                onCarpetaNueva(nombreCarpeta, rutaCompleta);
            }
        }, delayMs);

        carpetasPendientesCreacion.set(claveCreacion, {
            timeout: timeoutCreacion,
            nombreOriginal: nombreCarpeta,
            rutaCompleta,
        });
    } else if (esEventoEliminacion(tipo)) {
        /* Bufferar eliminación para detectar posible rename */
        const existente = carpetasEliminadasPendientes.get(nombreCarpeta);
        if (existente) {
            clearTimeout(existente.timeout);
        }

        const timeout = setTimeout(() => {
            carpetasEliminadasPendientes.delete(nombreCarpeta);
            /* Eliminación confirmada — la carpeta fue borrada, no renombrada.
             * No sincronizamos borrado de carpeta al servidor (podría ser limpieza local). */
            logSync.info('watcher', `Carpeta eliminada (no fue rename): ${nombreCarpeta}`);
        }, GRACIA_RENAME_CARPETA_MS);

        carpetasEliminadasPendientes.set(nombreCarpeta, {
            nombre: nombreCarpeta,
            ruta: rutaCompleta,
            timeout,
        });
    }
}

/*
 * Busca la primera carpeta eliminada pendiente (para detectar rename).
 * Debería haber como máximo una a la vez en una operación de rename.
 */
function encuentraCarpetaEliminadaPendiente(): CarpetaEliminadaPendiente | null {
    for (const [, val] of carpetasEliminadasPendientes) {
        return val;
    }
    return null;
}
