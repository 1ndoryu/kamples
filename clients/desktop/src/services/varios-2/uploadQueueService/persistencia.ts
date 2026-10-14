/* Persistencia de la cola y de hashes en Tauri Store (con debounce). */

import { esDesktop } from '../../varios-1/desktopService';
import { persistirConDebounce } from '../persistenciaDebounce';
import { cola, estadoUploadQueue } from './uploadQueueEstado';
import { STORE_FILE, STORE_KEY_COLA, STORE_KEY_HASHES } from './helpers';

/*
 * Persiste la cola filtrando items completados viejos.
 * Se usa para guardar con urgencia (reintento, eliminación manual).
 * Para persistencia frecuente en bucle, usar guardarColaDebounced().
 */
export async function guardarCola(): Promise<void> {
    if (!esDesktop()) return;
    try {
        const { load } = await import('@tauri-apps/plugin-store');
        const store = await load(STORE_FILE);
        const paraGuardar = cola.filter(i =>
            i.estado !== 'completado' ||
            (Date.now() - i.timestampActualizado < 3600_000),
        );
        await store.set(STORE_KEY_COLA, paraGuardar);
        await store.save();
    } catch {
        /* Fallo silencioso — la cola en memoria sigue viva */
    }
}

/*
 * Versión debounced de guardarCola para uso en bucles de alta frecuencia.
 * Agrupa múltiples escrituras en una sola operación de disco.
 */
export function guardarColaDebounced(): void {
    if (!esDesktop()) return;
    const paraGuardar = cola.filter(i =>
        i.estado !== 'completado' ||
        (Date.now() - i.timestampActualizado < 3600_000),
    );
    persistirConDebounce('upload_cola', STORE_FILE, STORE_KEY_COLA, paraGuardar, 2000);
}

/*
 * Persiste los hashes procesados.
 */
export async function guardarHashes(): Promise<void> {
    if (!esDesktop()) return;
    try {
        const { load } = await import('@tauri-apps/plugin-store');
        const store = await load(STORE_FILE);
        /* Limitar a últimos 5000 hashes para no crecer indefinidamente */
        const hashesArray = Array.from(estadoUploadQueue.hashesConocidos).slice(-5000);
        await store.set(STORE_KEY_HASHES, hashesArray);
        /* C6: Persistir mapa hash→rutas (1:N) para dedup cross-carpeta entre reinicios */
        const mapaObj: Record<string, string[]> = {};
        for (const [h, rutas] of estadoUploadQueue.hashARutas) mapaObj[h] = Array.from(rutas);
        await store.set('hash_a_ruta', mapaObj);
        /* QL69: Persistir contador antispam */
        const antispamObj: Record<string, number> = {};
        for (const [h, c] of estadoUploadQueue.contadorHashDetectado) antispamObj[h] = c;
        await store.set('antispam_hash_contador', antispamObj);
        await store.save();
    } catch {
        /* Fallo silencioso */
    }
}

/*
 * Retorna true si hay items pendientes de subir.
 */
