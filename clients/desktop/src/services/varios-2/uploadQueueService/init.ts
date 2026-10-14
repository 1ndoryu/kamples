/* Inicialización de la cola (restaura persistencia) y suscripción a eventos de ventana. */

import { esDesktop, estaOnline } from '../../varios-1/desktopService';
import { escucharVentana } from '@/platform/ventana';
import { logSync } from '../../sync/varios-1/syncLogger';
import { ItemUploadCola, OnProgresoUploadFn } from './tipos';
import { cola, estadoUploadQueue, normalizarRutaCola, claveRutaEnCola, crearIdempotencyKeyDeterministica, tienePendientes, setCola, setRutasEnCola, setHashesConocidos, setHashARutas, setCallbackProgreso } from './uploadQueueEstado';
import { STORE_FILE, estaEnCarpetaSync, STORE_KEY_COLA, STORE_KEY_HASHES, MAX_REINTENTOS } from './helpers';
import { procesarCola } from './proceso';

/*
 * Inicializa la cola: carga items pendientes y hashes del store.
 */
export async function inicializarUploadQueue(): Promise<void> {
    if (!esDesktop()) return;

    try {
        const { load } = await import('@tauri-apps/plugin-store');
        const store = await load(STORE_FILE);

        const colaGuardada = await store.get<ItemUploadCola[]>(STORE_KEY_COLA);
        if (colaGuardada) {
            /* Restaurar solo items no completados Y dentro de la carpeta sync.
             * Items residuales de antes del fix C378 (watcher scope) se purgan aquí
             * para que no se reintenten eternamente al reiniciar la app. */
            setCola(colaGuardada.filter(i => {
                if (i.estado === 'completado') return false;
                if (!estaEnCarpetaSync(i.rutaArchivo)) {
                    logSync.warn('uploadQueueService', '[UploadQueue] Purgando item persistido fuera de carpeta sync:', i.nombreArchivo);
                    return false;
                }
                return true;
            }));

            /*
             * C384: Verificar existencia física de archivos en disco.
             * Items cuyo archivo fue eliminado/movido/OneDrive cloud-only no deben
             * persistir en la cola entre reinicios. Sin esta verificación, archivos
             * fantasma se reintentan eternamente cada vez que se abre la app.
             */
            try {
                const { exists } = await import('@tauri-apps/plugin-fs');
                const verificaciones = await Promise.all(
                    cola.map(async (item) => {
                        try { return await exists(item.rutaArchivo); }
                        catch { return true; /* En caso de error FS, conservar item */ }
                    }),
                );
                const antesCount = cola.length;
                setCola(cola.filter((_, idx) => {
                    if (!verificaciones[idx]) {
                        logSync.warn('uploadQueueService', '[UploadQueue] Purgando item con archivo inexistente:', cola[idx].nombreArchivo);
                        return false;
                    }
                    return true;
                }));
                if (antesCount !== cola.length) {
                    logSync.info('uploadQueueService', `[UploadQueue] Purgados ${antesCount - cola.length} items con archivos inexistentes`);
                }
            } catch {
                /* plugin-fs no disponible — conservar items existentes */
            }

            /* Items que estaban "subiendo" al cerrar -> volver a pendiente.
             * Migración: items de store pre-C368 no tienen idempotencyKey. */
            for (const item of cola) {
                if (item.estado === 'subiendo') {
                    item.estado = 'pendiente';
                }
                item.rutaArchivo = normalizarRutaCola(item.rutaArchivo);
                if (!item.idempotencyKey) {
                    item.idempotencyKey = crearIdempotencyKeyDeterministica(
                        item.rutaArchivo,
                        item.nombreArchivo,
                        item.hashParcial,
                    );
                }
            }
            /* Reconstruir Set de rutas para lookups O(1) */
            setRutasEnCola(new Set(cola.map(i => claveRutaEnCola(i.rutaArchivo))));
        }

        const hashesGuardados = await store.get<string[]>(STORE_KEY_HASHES);
        if (hashesGuardados) {
            setHashesConocidos(new Set(hashesGuardados));
        }
        /* C6: Restaurar mapa hash→rutas (1:N) para dedup cross-carpeta */
        const mapaGuardado = await store.get<Record<string, string | string[]>>('hash_a_ruta');
        if (mapaGuardado) {
            setHashARutas(new Map());
            for (const [h, v] of Object.entries(mapaGuardado)) {
                estadoUploadQueue.hashARutas.set(h, new Set(Array.isArray(v) ? v : [v]));
            }
        }
        /* QL69: Restaurar contador antispam */
        const antispamGuardado = await store.get<Record<string, number>>('antispam_hash_contador');
        if (antispamGuardado) {
            for (const [h, c] of Object.entries(antispamGuardado)) {
                estadoUploadQueue.contadorHashDetectado.set(h, c);
            }
        }
    } catch {
        /* Store no disponible — usar cola en memoria */
    }

    /* Escuchar reconexión para reanudar subidas */
    escucharVentana('online', () => { procesarCola(); });

    /* Listener cross-ventana: el sync panel (sync.html) actualiza el Tauri Store
     * directamente y emite este evento. La ventana principal recarga la cola
     * desde el Store para sincronizar su memoria con los cambios. */
    try {
        const { listen } = await import('@tauri-apps/api/event');
        void listen('reintentar-errores-upload', async () => {
            /* Recargar cola desde el Store (el sync panel ya la actualizó ahí) */
            try {
                const { load } = await import('@tauri-apps/plugin-store');
                const store = await load(STORE_FILE);
                const colaGuardada = await store.get<ItemUploadCola[]>(STORE_KEY_COLA);
                if (colaGuardada) {
                    setCola(colaGuardada.filter(i => {
                        if (i.estado === 'completado') return false;
                        /* C384b: Excluir items que agotaron reintentos (archivo inexistente u
                         * otro error fatal). Con la nueva lógica subirArchivo retorna true en
                         * file-not-found → estado=completado. Pero items de versiones anteriores
                         * pueden estar como 'error' con intentos agotados en el Store. */
                        if (i.estado === 'error' && i.intentos >= MAX_REINTENTOS) return false;
                        if (!estaEnCarpetaSync(i.rutaArchivo)) return false;
                        return true;
                    }));
                    setRutasEnCola(new Set(cola.map(i => claveRutaEnCola(i.rutaArchivo))));
                }
            } catch {
                /* Fallback: resetear desde memoria solo items con reintentos disponibles.
                 * Items que agotaron reintentos (intentos >= MAX_REINTENTOS) fueron
                 * descartados permanentemente (archivo inexistente u otro error fatal)
                 * y NO deben resetearse — hacerlo crea el ciclo infinito de reintentos. */
                for (const item of cola) {
                    if (item.estado === 'error' && item.intentos < MAX_REINTENTOS) {
                        item.estado = 'pendiente';
                        item.intentos = 0;
                        item.ultimoError = undefined;
                        item.timestampActualizado = Date.now();
                        estadoUploadQueue.rutasEnCola.add(claveRutaEnCola(item.rutaArchivo));
                    }
                }
            }
            if (estaOnline() && tienePendientes()) {
                procesarCola();
            }
        });
    } catch {
        /* Entorno sin Tauri — ignorar */
    }

    /* Si hay items pendientes y estamos online, procesar */
    if (estaOnline() && tienePendientes()) {
        procesarCola();
    }
}

/*
 * Registra un callback para recibir actualizaciones de progreso.
 */
export function onProgresoUpload(cb: OnProgresoUploadFn): void {
    setCallbackProgreso(cb);
}
