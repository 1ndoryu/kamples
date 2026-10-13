/* Orquestador del sync completo de colecciones: purga, estructura, descarga y reconciliación. */

import { marcarDescargaMasiva } from '../syncGuards';
import { logSync } from '../syncLogger';
import { estado } from '../../varios-2/syncState';
import { obtenerColeccionesDelServidor } from './servidor';
import { purgarTrackingNoServidor } from './sincronizarFases/sincronizarFasesPurga';
import { sincronizarEstructura } from './sincronizarFases/sincronizarFasesEstructura';
import { verificarEspacioDisco, sincronizarDescargas } from './sincronizarFases/sincronizarFasesDescarga';
import { reconciliarRutasConColecciones } from './reconciliacion';
import type { CallbackProgresoColecciones } from './tipos';

/**
 * Sincronización completa basada en colecciones.
 * 1. Obtiene colecciones del servidor
 * 2. Crea/actualiza carpetas locales por colección
 * 3. Descarga samples nuevos a la carpeta correspondiente
 * 4. Samples sin colección van a la carpeta especial "Sin colección"
 *
 * @param soloEstructura Si true, solo sincroniza carpetas (no descarga archivos).
 *   Usado por el polling periódico para evitar roundtrips de descarga cada 60s.
 */
export async function sincronizarColecciones(
    carpetaBase: string,
    onProgreso?: CallbackProgresoColecciones,
    soloEstructura = false,
): Promise<{ nuevos: number; errores: number }> {
    const datosServidor = await obtenerColeccionesDelServidor();
    if (!datosServidor) return { nuevos: 0, errores: 0 };

    await purgarTrackingNoServidor(datosServidor, soloEstructura);

    const { mkdir, exists, rename } = await import('@tauri-apps/plugin-fs');
    const { join } = await import('@tauri-apps/api/path');

    /* Calcular total de samples para progreso */
    const totalSamples = datosServidor.colecciones.reduce((sum, c) => sum + c.samples.length, 0)
        + datosServidor.sinColeccion.length;

    /*
     * QL103: Activar flag de descarga masiva para suprimir eventos de carpeta
     * en el watcher. Sin esto, cada mkdir + writeFile genera cientos de eventos
     * de carpeta/subcarpeta redundantes que saturan crearColeccionDesdeLocal.
     */
    marcarDescargaMasiva(true);

    try {
        const rutaSinCol = await sincronizarEstructura(
            carpetaBase, datosServidor, onProgreso, { mkdir, exists, rename }, join,
        );

        /* Modo soloEstructura: no descargar archivos (usado por polling periodico) */
        /* QL106: Defensa en profundidad — si borrarAlSubirExitoso está activo,
         * forzar soloEstructura aunque el caller no lo indique. Evita que
         * cualquier ruta futura dispare descargas cuando el usuario quiere
         * mantener archivos solo en servidor. */
        if (soloEstructura || estado.configAvanzada.borrarAlSubirExitoso) {
            if (estado.configAvanzada.borrarAlSubirExitoso && !soloEstructura) {
                logSync.debug('collectionSync', 'Descargas bloqueadas: borrarAlSubirExitoso activo');
            }
            return { nuevos: 0, errores: 0 };
        }

        /* Verificar espacio en disco antes de descargas masivas */
        const espacioSuficiente = await verificarEspacioDisco(carpetaBase, datosServidor);
        if (!espacioSuficiente) {
            logSync.error('syncCollectionService', '[SyncColecciones] Espacio insuficiente en disco. Abortando descargas.');
            return { nuevos: 0, errores: totalSamples };
        }

        const { nuevos, errores } = await sincronizarDescargas(
            carpetaBase, datosServidor, rutaSinCol, onProgreso, join, totalSamples,
        );

        /*
         * Reconciliación local→servidor: detectar samples cuya ruta física
         * no coincide con su coleccionId en tracking. Esto ocurre cuando un
         * move local fue procesado por el watcher pero no se actualizó
         * coleccion_samples en el servidor (bug pre-fix actualizarColeccionEnMovimiento).
         * Recorre todos los archivos tracked, infiere la colección real desde
         * la ruta y corrige tanto tracking como servidor si hay discrepancia.
         */
        const corregidos = await reconciliarRutasConColecciones(carpetaBase);
        if (corregidos > 0) {
            logSync.info('collectionSync', `Reconciliación local→servidor: ${corregidos} sample(s) corregidos`);
        }

        return { nuevos, errores };
    } finally {
        /* QL103: Desactivar flag de descarga masiva, incluso si hubo error */
        marcarDescargaMasiva(false);
    }
}
