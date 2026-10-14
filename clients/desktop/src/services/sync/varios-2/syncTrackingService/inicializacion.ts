import {
    establecerEstado,
    iniciarCheckpointPeriodico,
    recuperar,
    registrarAplicador,
    registrarCallbackCheckpoint,
} from '../../varios-1/syncJournal';
import { logSync } from '../../varios-1/syncLogger';
import { esDesktop } from '../../../varios-1/desktopService';
import { aplicadorRecuperacion } from './persistencia/journal';
import {
    datos,
    estadoSyncTracking,
    setDatos,
    setJournalActivo,
    setLimpiarHistorialUnlisten,
    setStoreCache,
    setVersionLocalConocida,
    limpiarHistorialUnlisten,
    STORE_FILE,
    STORE_KEY_TRACKING,
    storeCache,
    type StoreCache,
    versionLocalConocida,
} from './estado';
import { escribirEnStore } from './persistencia/persistencia';
import { reconstruirIndices, reconstruirIndiceSampleHistorial } from './indices';
import type { BaseSyncLocal } from './tipos';

/**
 * Inicializa el tracking de sync.
 *
 * @param userIdActual ID del usuario autenticado actualmente.
 *   Si el tracking almacenado pertenece a un usuario diferente,
 *   se limpia completamente para evitar operar sobre recursos ajenos (403).
 *   Si no se proporciona (undefined), se omite la verificación de propiedad
 *   (backward-compatible para ventanas secundarias sin contexto de auth).
 */
export async function inicializarTracking(userIdActual?: number): Promise<void> {
    if (!esDesktop()) return;

    try {
        const { load } = await import('@tauri-apps/plugin-store');
        const store = await load(STORE_FILE);
        setStoreCache(store as StoreCache | null);
    } catch {
        /* Store no disponible */
    }

    /* Configurar journal WAL para persistencia confiable contra crashes */
    const estadoVacio: BaseSyncLocal = {
        archivos: {}, colecciones: {}, sinColeccion: [], historial: [], historialSamples: [],
    };
    registrarAplicador(aplicadorRecuperacion, estadoVacio);
    registrarCallbackCheckpoint(async () => {
        try {
            await escribirEnStore();
        } catch (err) {
            logSync.warn('tracking', 'Error escribiendo en Store durante checkpoint', {
                error: err instanceof Error ? err.message : String(err)
            });
        }
    });

    /* Intentar recuperación desde journal (protege contra crashes) */
    const recuperado = await recuperar<BaseSyncLocal>();

    if (recuperado && (Object.keys(recuperado.archivos).length > 0 || Object.keys(recuperado.colecciones).length > 0)) {
        setDatos({ ...recuperado, historialSamples: recuperado.historialSamples ?? [] });
        logSync.info('tracking', 'Estado recuperado desde journal');

        /*
         * C288: Tras journal recovery, sincronizar versionLocalConocida y userId desde Store.
         * Sin esto:
         * - versionLocalConocida queda en 0 (journal no la persiste)
         * - Store tiene version N (escrita por checkpoints anteriores)
         * - El primer escribirEnStore() ve Store(N)>local(0) → TC1 merge
         *   re-importa datos viejos (posiblemente de otro usuario).
         *
         * También leer userId del Store: el journal puede tener datos sin userId
         * (pre-C286) o con userId de otro usuario. El Store es la fuente de verdad
         * porque fue el último en recibir un checkpoint con userId correcto.
         */
        if (storeCache) {
            try {
                const storeData = await storeCache.get<BaseSyncLocal>(STORE_KEY_TRACKING);
                if (storeData) {
                    setVersionLocalConocida(storeData.checkpointVersion ?? 0);

                    /* Si el journal no tiene userId pero el Store sí, adoptar el del Store
                     * para que el ownership check posterior funcione correctamente */
                    if (datos.userId === undefined && storeData.userId !== undefined) {
                        datos.userId = storeData.userId;
                    }
                }
            } catch {
                /* Store no disponible — TC1 merge podría dispararse pero es mejor que fallar */
            }
        }
    } else if (storeCache) {
        /* Fallback: cargar desde Tauri Store (caso normal sin crash previo) */
        try {
            const guardado = await storeCache.get<BaseSyncLocal>(STORE_KEY_TRACKING);
            if (guardado) {
                setDatos({ ...guardado, historialSamples: guardado.historialSamples ?? [] });
                /* TC1: Sincronizar versión local con la del Store */
                setVersionLocalConocida(guardado.checkpointVersion ?? 0);
            }
        } catch {
            /* Store no disponible — usar defaults vacíos */
        }
    }

    /*
     * C286: Verificación de propiedad por usuario.
     * Si el tracking almacenado pertenece a un usuario diferente al actual,
     * limpiar TODOS los datos. Colecciones/archivos de otro usuario generan
     * 403 en cascada (el servidor verifica propiedad por JWT).
     *
     * Caso retrocompatible: tracking sin userId (pre-C286) → se adopta
     * como del usuario actual y se marca con su ID en el próximo checkpoint.
     */
    if (userIdActual !== undefined && userIdActual > 0) {
        const userIdAlmacenado = datos.userId;

        if (userIdAlmacenado !== undefined && userIdAlmacenado !== userIdActual) {
            logSync.warn('tracking',
                `Tracking pertenece a usuario #${userIdAlmacenado}, pero el usuario actual es #${userIdActual}. Limpiando datos ajenos.`);

            setDatos({
                archivos: {},
                colecciones: {},
                sinColeccion: [],
                historial: [],
                historialSamples: [],
                userId: userIdActual,
            });

            /*
             * C287: Persistir estado limpio al Store inmediatamente.
             * Sin esto, versionLocalConocida queda en 0 mientras Store tiene version N.
             * El primer escribirEnStore() ve Store(N) > local(0) → TC1 merge
             * re-importa TODAS las colecciones/archivos del usuario anterior.
             */
            if (storeCache) {
                try {
                    const storeActual = await storeCache.get<BaseSyncLocal>(STORE_KEY_TRACKING);
                    const versionActualStore = storeActual?.checkpointVersion ?? 0;
                    setVersionLocalConocida(versionActualStore + 1);
                    datos.checkpointVersion = versionLocalConocida;
                    await storeCache.set(STORE_KEY_TRACKING, datos);
                    await storeCache.save();
                    logSync.info('tracking', `Estado limpio persistido al Store (version: ${versionLocalConocida})`);
                } catch (err) {
                    logSync.warn('tracking', 'Error persistiendo estado limpio al Store', {
                        error: err instanceof Error ? err.message : String(err)
                    });
                }
            }
        } else {
            /* Marcar userId si no existía (migración from pre-C286 tracking) */
            datos.userId = userIdActual;
        }
    }

    /* Sincronizar estado del journal con datos cargados */
    establecerEstado(datos);
    setJournalActivo(true);

    reconstruirIndices();
    reconstruirIndiceSampleHistorial();

    /* Iniciar checkpoint periódico WAL (cada 30s si hay operaciones pendientes) */
    iniciarCheckpointPeriodico();

    /*
     * Listener cross-window: cuando otra ventana limpia el historial,
     * actualizar la copia in-memory para que el próximo persistir()
     * no sobreescriba el Store con datos viejos.
     * TA1: Guardar unlisten para prevenir acumulación de listeners en reinicializaciones.
     */
    try {
        if (limpiarHistorialUnlisten) {
            limpiarHistorialUnlisten();
            setLimpiarHistorialUnlisten(null);
        }
        const { listen } = await import('@tauri-apps/api/event');
        setLimpiarHistorialUnlisten(await listen('limpiar-historial-samples', () => {
            datos.historialSamples = [];
            estadoSyncTracking.indiceSampleHistorial.clear();
            estadoSyncTracking.indiceNombreSampleHistorial.clear();
        }));
    } catch {
        /* Entorno sin Tauri */
    }
}
