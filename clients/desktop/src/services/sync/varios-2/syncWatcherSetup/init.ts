/*
 * Inicialización y parada del watcher bidireccional: orquesta escaneo, callbacks,
 * polling y reconciliación.
 */
import { logSync } from '../../varios-1/syncLogger';
import { estado } from '../syncState';
import { inicializarPapelera, purgarExpirados } from '../../../varios-1/papeleraService';
import { escanearCarpetaYEncolar } from './escaneo';
import { crearCallbacksArchivos } from './callbacks/callbacksArchivos';
import { crearCallbacksCarpetas } from './callbacks/callbacksCarpetas';
import { crearCallbacksSubcarpetas } from './callbacks/callbacksSubcarpetas';
import { programarProximoPolling, sincronizarEstructuraCarpetas } from './polling';
import { ejecutarReconciliacion, reconciliarEstructuraCarpetas, RECONCILIACION_CARPETAS_MS } from './reconciliacion';
import { pollingTimeout, reconciliacionCarpetasInterval, setPollingTimeout, setReconciliacionCarpetasInterval } from './syncWatcherEstado';

/* Inicialización del watcher bidireccional */

/*
 * Inicializa el sistema de sync bidireccional:
 * 1. Conecta fileWatcherService para detectar archivos nuevos/eliminados
 * 2. Inicializa uploadQueueService para subida automática
 * 3. Inicia observación de la carpeta de sync
 * 4. Inicia polling periódico de carpetas del servidor
 */
export async function inicializarSyncBidireccional(): Promise<void> {
    const { config, trackingModule, collectionModule } = estado;
    if (!config.carpetaLocal || !config.sincronizacionActiva) {
        logSync.warn('syncWatcher', 'inicializarSyncBidireccional ABORTADO — config incompleta', {
            carpetaLocal: config.carpetaLocal ?? '(null)',
            sincronizacionActiva: config.sincronizacionActiva,
            razon: !config.carpetaLocal
                ? 'No hay carpeta de sync configurada. Ve a Configuracion > Sync para seleccionar una carpeta.'
                : 'Sincronizacion desactivada en la configuracion.',
        });
        return;
    }

    try {
        const { registrarCallbacks, registrarCallbacksCarpeta, registrarCallbacksSubcarpeta, iniciarObservacion } = await import('../../../varios-1/fileWatcherService');
        const { inicializarUploadQueue, encolarArchivo } = await import('../../../varios-2/uploadQueueService');

        /* Inicializar papelera y purgar expirados */
        await inicializarPapelera();
        await purgarExpirados();

        const [onArchivoNuevo, onArchivoEliminado, onArchivoMovido] = crearCallbacksArchivos(trackingModule, encolarArchivo);
        registrarCallbacks(onArchivoNuevo, onArchivoEliminado, onArchivoMovido);

        /* C357: Callbacks de carpetas para sincronizar colecciones */
        if (collectionModule) {
            const colMod = collectionModule;
            const [onCarpetaNueva, onCarpetaRenombrada] = crearCallbacksCarpetas(trackingModule, colMod);
            registrarCallbacksCarpeta(onCarpetaNueva, onCarpetaRenombrada);

            /*
             * C387: Callbacks de subcarpetas para sincronizar subcolecciones.
             * Se disparan cuando se crea o renombra una carpeta de nivel 2
             * (dentro de una colección existente).
             */
            const [onSubcarpetaNueva, onSubcarpetaRenombrada] = crearCallbacksSubcarpetas(trackingModule, colMod);
            registrarCallbacksSubcarpeta(onSubcarpetaNueva, onSubcarpetaRenombrada);
        }

        await inicializarUploadQueue();

        /* QL78: Limpieza retroactiva de archivos subidos que quedaron en disco.
         * Se ejecuta tras inicializar la cola (donde se restauran hashARutas). */
        try {
            const { limpiarArchivosSubidosEnDisco } = await import('../../../varios-2/uploadQueueService');
            limpiarArchivosSubidosEnDisco().catch(() => {});
        } catch { /* plugin-fs no disponible */ }

        const iniciado = await iniciarObservacion();
        if (iniciado) {
            logSync.info('syncWatcherSetup', '[Sync] Sync bidireccional activado');
        }

        await sincronizarEstructuraCarpetas();

        /*
         * Escaneo inicial: detectar archivos locales preexistentes que el watcher
         * no puede ver (no emite eventos para archivos ya presentes al iniciar).
         * Se lanza async para no bloquear el arranque.
         */
        escanearCarpetaYEncolar().catch(err => {
            logSync.error('syncWatcherSetup', '[Sync] Error en escaneo inicial de carpeta local:', err);
        });

        /*
         * Listener para escaneo bajo demanda desde el sync panel.
         * "Sincronizar ahora" emite este evento para detectar archivos nuevos.
         */
        try {
            const { listen: listenTauri } = await import('@tauri-apps/api/event');
            await listenTauri('escanear-subidas-local', () => {
                escanearCarpetaYEncolar().catch(err => {
                    logSync.error('syncWatcherSetup', '[Sync] Error en escaneo local bajo demanda:', err);
                });
            });
        } catch {
            /* Entorno sin Tauri — ignorar */
        }

        /* F2.2: Usar polling adaptivo en vez de intervalo fijo */
        programarProximoPolling();

        /* F3.2: Reconciliación periódica de integridad.
         * Cada 7 días, verificar que disco y tracking estén sincronizados.
         * Detecta divergencias silenciosas (archivos borrados externamente, etc.) */
        const RECONCILIACION_INTERVALO_MS = 7 * 24 * 60 * 60 * 1000;
        setInterval(() => {
            ejecutarReconciliacion().catch(err => {
                logSync.warn('syncWatcher', 'Error en reconciliación periódica', {
                    error: err instanceof Error ? err.message : String(err),
                });
            });
        }, RECONCILIACION_INTERVALO_MS);

        /*
         * C288: Reconciliación periódica de estructura de carpetas para detectar renames.
         * El debounced watcher no emite rename events fiables en Windows+OneDrive.
         * Este timer escanea carpetas cada 15s y detecta renames por comparación.
         */
        setReconciliacionCarpetasInterval(setInterval(() => {
            reconciliarEstructuraCarpetas().catch(err => {
                logSync.warn('syncWatcher', 'Error en reconciliación de carpetas', {
                    error: err instanceof Error ? err.message : String(err),
                });
            });
        }, RECONCILIACION_CARPETAS_MS));

        /* QL91: Analisis periodico de archivos huerfanos (cada 30min).
         * Detecta archivos no subidos, no borrados, o en error permanente. */
        try {
            const { iniciarAnalisisPeriodicoHuerfanos, analizarArchivosHuerfanos } = await import('../../varios-1/syncOrphanAnalysis');
            iniciarAnalisisPeriodicoHuerfanos();
            /* Ejecutar un analisis inicial tras 60s para no competir con el escaneo de arranque */
            setTimeout(() => {
                analizarArchivosHuerfanos().catch(() => {});
            }, 60_000);
        } catch { /* Modulo no disponible */ }

    } catch (err) {
        logSync.error('syncWatcher', 'Error inicializando sync bidireccional', { error: err instanceof Error ? err.message : String(err) });
    }
}

/*
 * Detiene el watcher si está activo (para cleanup al cerrar).
 */
export async function detenerSyncBidireccional(): Promise<void> {
    if (estado.pollingCarpetasInterval) {
        clearInterval(estado.pollingCarpetasInterval);
        estado.pollingCarpetasInterval = null;
    }
    if (pollingTimeout) {
        clearTimeout(pollingTimeout);
        setPollingTimeout(null);
    }
    if (reconciliacionCarpetasInterval) {
        clearInterval(reconciliacionCarpetasInterval);
        setReconciliacionCarpetasInterval(null);
    }
    /* QL91: Detener analisis periodico de huerfanos */
    try {
        const { detenerAnalisisPeriodicoHuerfanos } = await import('../../varios-1/syncOrphanAnalysis');
        detenerAnalisisPeriodicoHuerfanos();
    } catch { /* Modulo no disponible */ }
    try {
        const { detenerObservacion } = await import('../../../varios-1/fileWatcherService');
        await detenerObservacion();
    } catch {
        /* Ignorar si no se importó */
    }
}
