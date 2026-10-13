import { esDesktop } from '../../../../varios-1/desktopService';
import { logSync } from '../../../varios-1/syncLogger';
import { generarClaveTracking } from '../claves';
import { datos, sinColeccionSet, STORE_FILE } from '../estado';
import { reconstruirIndices } from '../indices';
import { persistir } from './persistencia';
import { registrarAccion } from '../historial';
import type { ArchivoTracking } from '../tipos';

/**
 * Migra el índice plano ArchivoLocal[] (v1) al nuevo formato BaseSyncLocal (v2).
 * Lee el store key viejo 'sync_indice' y convierte cada ArchivoLocal a ArchivoTracking.
 * Samples v1 van todos a coleccionId=null (sin colección) porque v1 no tenía colecciones.
 * La asignación a colecciones se hará en la primera sync v2 completa.
 */
export async function migrarDesdeV1(): Promise<boolean> {
    if (!esDesktop()) return false;

    try {
        const { load } = await import('@tauri-apps/plugin-store');
        const store = await load(STORE_FILE);

        interface ArchivoLocalV1 {
            ruta: string;
            nombre: string;
            sampleId: number;
            hash: string;
            descargadoEn: number;
            nombreOriginal: string;
            nombreServidor: string;
            syncDeshabilitado?: boolean;
            rutaEliminada?: string;
        }

        const indiceV1 = await store.get<ArchivoLocalV1[]>('sync_indice');
        if (!indiceV1 || indiceV1.length === 0) return false;

        /* Ya existe datos v2 con archivos → no migrar */
        if (Object.keys(datos.archivos).length > 0) return false;

        for (const v1 of indiceV1) {
            const tracking: ArchivoTracking = {
                sampleId: v1.sampleId,
                coleccionId: null,
                rutaLocal: v1.ruta,
                nombreLocal: v1.nombre,
                nombreServidor: v1.nombreServidor,
                descargadoEn: v1.descargadoEn,
                tamano: 0,
                syncDeshabilitado: v1.syncDeshabilitado ?? false,
            };

            const clave = generarClaveTracking(v1.sampleId, null);
            datos.archivos[clave] = tracking;
            datos.sinColeccion.push(v1.sampleId);
            sinColeccionSet.add(v1.sampleId);
        }

        reconstruirIndices();
        await persistir();

        /* Eliminar clave v1 para que la migración no se repita en futuros reinicios.
         * Sin esto, si el store v2 falla al cargar en algún reinicio, la presencia
         * de sync_indice vuelve a trigger la migración produciendo duplicados. */
        try {
            await store.set('sync_indice', null);
            await store.save();
        } catch {
            /* No crítico: la migración ya ocurrió, el peor caso es una re-migración idempotente */
        }

        /* Registrar migración en historial legacy (no aparece en historialSamples) */
        await registrarAccion({
            tipo: 'creado',
            descripcion: `Migración v1→v2: ${indiceV1.length} archivos convertidos`,
        });

        logSync.info('syncTrackingService', `[SyncTracking] Migración v1→v2 completada: ${indiceV1.length} archivos`);
        return true;
    } catch (err) {
        logSync.error('syncTrackingService', '[SyncTracking] Error en migración v1→v2:', err);
        return false;
    }
}
