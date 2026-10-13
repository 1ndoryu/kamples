/*
 * Polling de estructura de carpetas: consulta delta, reconciliación de descargas
 * y intervalo adaptivo (F2.1, F2.2, C289b).
 */
import { estaOnline } from '../../../varios-1/desktopService';
import { esSyncEnCurso, obtenerBaseUrlSync, obtenerHeadersSyncGet } from '../../varios-1/syncGuards';
import { clasificarError, esErrorDeRed } from '../../../varios-1/errorSync';
import { estado, guardarCursorDelta } from '../syncState';
import { sincronizarEstructuraCarpetasV1 } from '../../varios-1/syncDownloadV1';
import { logSync } from '../../varios-1/syncLogger';
import { pollingTimeout, setPollingTimeout, ultimaSyncConDescargas, setUltimaSyncConDescargas } from './syncWatcherEstado';

/* Polling de estructura de carpetas */

/*
 * F2.2: Polling adaptivo — Ajusta intervalo según actividad.
 * - Con cambios recientes: 15s (feedback rápido)
 * - Normal: 60s (default)
 * - Sin cambios por mucho tiempo: gradualmente hasta 5min
 */
const POLLING_MIN_MS = 15_000;
const POLLING_MAX_MS = 300_000;



function ajustarIntervaloPolling(huboCambios: boolean): void {
    if (huboCambios) {
        estado.intervaloPollingMs = POLLING_MIN_MS;
    } else {
        estado.intervaloPollingMs = Math.min(
            Math.round(estado.intervaloPollingMs * 1.5),
            POLLING_MAX_MS,
        );
    }
}

export function programarProximoPolling(): void {
    if (pollingTimeout) clearTimeout(pollingTimeout);
    setPollingTimeout(setTimeout(async () => {
        try {
            const resultado = await sincronizarEstructuraCarpetas();
            ajustarIntervaloPolling(resultado);
        } catch {
            /* Error ya logueado en sincronizarEstructuraCarpetas */
        }
        programarProximoPolling();
    }, estado.intervaloPollingMs));
}

/*
 * F2.1: Consulta delta sync al servidor.
 * Retorna true si hay cambios pendientes (el caller debe ejecutar full sync).
 * Retorna false si no hay cambios (no hace falta full sync → ahorra ancho de banda).
 * En caso de error, retorna true para forzar full sync como fallback seguro.
 */
async function consultarDeltaSync(): Promise<boolean> {
    const baseUrl = obtenerBaseUrlSync();
    const cursor = estado.ultimoCursorDelta;

    try {
        const resp = await fetch(
            `${baseUrl}/kamples/v1/me/sync/delta?cursor=${cursor}`,
            { headers: obtenerHeadersSyncGet() },
        );

        if (!resp.ok) {
            const cat = clasificarError(resp.status);
            logSync.warn('syncWatcher', 'Delta endpoint respondió con error', { status: resp.status, categoria: cat });
            /* En error de autenticación, no forzar full sync (no tiene sentido) */
            return cat !== 'autenticacion';
        }

        const json = await resp.json() as {
            data: {
                cambios: Array<{ id: number; tipo: string; entidadId: number; metadata: Record<string, unknown> }>;
                cursor: number;
                hayMas: boolean;
                fullSyncRequired: boolean;
            };
        };

        const { cambios, cursor: nuevoCursor, hayMas, fullSyncRequired } = json.data;

        /* Actualizar cursor siempre que recibimos uno válido */
        if (nuevoCursor > 0) {
            estado.ultimoCursorDelta = nuevoCursor;
            guardarCursorDelta().catch(() => {});
        }

        /* Primera sync o cursor purgado: necesita full sync */
        if (fullSyncRequired) {
            logSync.info('syncWatcher', 'Delta indica full sync requerido', { cursor, nuevoCursor });
            return true;
        }

        /* Hay cambios concretos: necesita full sync para aplicarlos */
        if (cambios.length > 0 || hayMas) {
            logSync.info('syncWatcher', 'Delta detectó cambios pendientes', {
                cambios: cambios.length,
                hayMas,
                tipos: [...new Set(cambios.map(c => c.tipo))],
            });
            return true;
        }

        /* Sin cambios: no hace falta full sync */
        logSync.debug('syncWatcher', 'Delta sin cambios, omitiendo full sync');
        return false;
    } catch (err) {
        if (esErrorDeRed(err)) {
            logSync.warn('syncWatcher', 'Error de red consultando delta, omitiendo ciclo');
            return false;
        }
        logSync.error('syncWatcher', 'Error consultando delta sync', { error: err instanceof Error ? err.message : String(err) });
        /* En error desconocido, forzar full sync como fallback seguro */
        return true;
    }
}

/*
 * C289b: Reconciliación periódica de descargas.
 * Cada RECONCILIACION_DESCARGAS_MS (5 min), forzar full sync con descargas
 * aunque delta diga "sin cambios". Resuelve:
 * - Samples que existían en el servidor antes de activar sync
 * - Descargas que fallaron y nunca se reintentaron
 * - Cualquier divergencia servidor→local silenciosa
 */
const RECONCILIACION_DESCARGAS_MS = 5 * 60 * 1000;



/*
 * Sincroniza la estructura de carpetas del servidor a disco local.
 * F2.1: Consulta delta primero para evitar full sync innecesarios.
 * C289: Sync completo (con descargas) cuando delta detecta cambios.
 * C289b: Bypass periódico del delta para reconciliar descargas faltantes.
 * v1: crea carpetas basadas en metadata IA.
 * Retorna true si hubo cambios para ajustar el intervalo de polling.
 */

export async function sincronizarEstructuraCarpetas(): Promise<boolean> {
    const { config, collectionModule } = estado;
    if (!config.carpetaLocal || !estaOnline()) return false;

    /* No ejecutar polling si hay una sync completa en curso (evita race conditions) */
    if (esSyncEnCurso()) return false;

    /* F2.1: Consultar delta antes de ejecutar full sync.
     * Si el cursor ya está inicializado (>0), usamos delta para ahorrar ancho de banda.
     * Si cursor=0 (primera vez), el delta retornará fullSyncRequired=true. */
    let necesitaSync = await consultarDeltaSync();

    /*
     * C289b: Bypass periódico — si no hay cambios delta pero ha pasado suficiente
     * tiempo desde la última sync completa con descargas, forzar full sync.
     * Esto garantiza que samples pre-existentes o con descarga fallida se reintenten.
     * descargarSiNecesario() es idempotente: si el sample ya existe en tracking,
     * retorna 'existente' sin re-descargar.
     */
    const tiempoSinReconciliacion = Date.now() - ultimaSyncConDescargas;
    if (!necesitaSync && tiempoSinReconciliacion > RECONCILIACION_DESCARGAS_MS) {
        logSync.info('syncWatcher', `Reconciliación de descargas: ${Math.round(tiempoSinReconciliacion / 1000)}s sin sync completa, forzando`);
        necesitaSync = true;
    }

    if (!necesitaSync) return false;

    if (collectionModule) {
        try {
            /*
             * QL106: Si borrarAlSubirExitoso está activo, forzar soloEstructura=true
             * para mantener tracking de carpetas pero NO descargar archivos.
             * Sin este guard, el polling periódico descarga samples que el usuario
             * quiere mantener solo en servidor (el orquestador tiene su propio guard,
             * pero esta ruta bypasea el orquestador).
             */
            const soloEstructura = estado.configAvanzada.borrarAlSubirExitoso;
            if (soloEstructura) {
                logSync.debug('syncWatcher', 'Descargas omitidas en polling: borrarAlSubirExitoso activo');
            }
            const resultado = await collectionModule.sincronizarColecciones(config.carpetaLocal, undefined, soloEstructura);
            /* C289b: Actualizar timestamp de última sync completa con descargas */
            setUltimaSyncConDescargas(Date.now());
            /* Hubo cambios si se descargaron nuevos archivos o se crearon carpetas */
            return resultado.nuevos > 0;
        } catch (err) {
            logSync.error('syncWatcher', 'Error en polling de colecciones v2', { error: err instanceof Error ? err.message : String(err) });
        }
        return false;
    }

    /* v1 fallback */
    await sincronizarEstructuraCarpetasV1();
    return false;
}
