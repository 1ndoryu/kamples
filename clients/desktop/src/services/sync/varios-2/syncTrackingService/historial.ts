import {
    datos,
    estadoSyncTracking,
    setUltimaRecargaStore,
    STORE_KEY_TRACKING,
    storeCache,
    ultimaRecargaStore,
} from './estado';
import { reconstruirIndiceSampleHistorial } from './indices';
import { registrarEnJournal } from './persistencia/journal';
import { persistir } from './persistencia/persistencia';
import type { AccionHistorial, BaseSyncLocal, EntradaHistorialSample, EstadoSampleHistorial } from './tipos';

/* Estado interno */
const MAX_HISTORIAL = 200;

/* Historial per-sample (v2): upsert por sampleId con estado mutable */

const MAX_HISTORIAL_SAMPLES = 100;

/**
 * Upsert: si ya existe una entrada para este sampleId (o nombreArchivo si sampleId=0),
 * actualiza su estado. Si no existe, inserta al inicio. Persiste automáticamente.
 *
 * Flujo típico de upload:
 *   1. Detectado (sampleId=0, nombre=X) → crea entrada
 *   2. Subiendo (sampleId=0, nombre=X) → actualiza por nombre
 *   3. Sincronizado (sampleId=REAL, nombre=X) → actualiza por nombre y fija sampleId real
 */
export async function actualizarEstadoSample(entrada: {
    sampleId: number;
    nombreArchivo: string;
    estado: EstadoSampleHistorial;
    imagenUrl?: string | null;
    rutaLocal?: string | null;
    coleccionNombre?: string;
    error?: string;
}): Promise<void> {
    const ahora = Date.now();
    const nombreNorm = entrada.nombreArchivo.toLowerCase();

    /* Buscar entrada existente: primero por sampleId real, luego por nombre */
    let idxExistente: number | undefined;
    if (entrada.sampleId > 0) {
        idxExistente = estadoSyncTracking.indiceSampleHistorial.get(entrada.sampleId);
    }
    if (idxExistente === undefined) {
        idxExistente = estadoSyncTracking.indiceNombreSampleHistorial.get(nombreNorm);
    }

    if (idxExistente !== undefined && idxExistente < datos.historialSamples.length) {
        /* Actualizar entrada existente */
        const existente = datos.historialSamples[idxExistente];
        existente.estado = entrada.estado;
        existente.timestampActualizado = ahora;

        /* Actualizar sampleId si ahora tenemos el real (era 0 y ahora es > 0) */
        if (entrada.sampleId > 0 && existente.sampleId === 0) {
            existente.sampleId = entrada.sampleId;
        }

        if (entrada.imagenUrl !== undefined) existente.imagenUrl = entrada.imagenUrl;
        if (entrada.rutaLocal !== undefined) existente.rutaLocal = entrada.rutaLocal;
        if (entrada.coleccionNombre !== undefined) existente.coleccionNombre = entrada.coleccionNombre;
        if (entrada.error !== undefined) existente.error = entrada.error;
        if (entrada.nombreArchivo) existente.nombreArchivo = entrada.nombreArchivo;

        /* Mover al inicio para que aparezca primero (más reciente) */
        if (idxExistente > 0) {
            datos.historialSamples = [
                existente,
                ...datos.historialSamples.slice(0, idxExistente),
                ...datos.historialSamples.slice(idxExistente + 1),
            ];
            reconstruirIndiceSampleHistorial();
        }
    } else {
        /* Insertar nueva entrada al inicio */
        const nueva: EntradaHistorialSample = {
            sampleId: entrada.sampleId,
            nombreArchivo: entrada.nombreArchivo,
            estado: entrada.estado,
            imagenUrl: entrada.imagenUrl ?? null,
            rutaLocal: entrada.rutaLocal ?? null,
            coleccionNombre: entrada.coleccionNombre,
            timestampCreado: ahora,
            timestampActualizado: ahora,
            error: entrada.error,
        };
        datos.historialSamples.unshift(nueva);

        /* Limitar tamaño */
        if (datos.historialSamples.length > MAX_HISTORIAL_SAMPLES) {
            datos.historialSamples = datos.historialSamples.slice(0, MAX_HISTORIAL_SAMPLES);
        }

        reconstruirIndiceSampleHistorial();
    }

    await registrarEnJournal('UPDATE_HISTORIAL', { historialSamples: datos.historialSamples });
}

/**
 * Obtiene el historial per-sample ordenado por última actualización.
 * Retorna copias superficiales de cada entrada para evitar que mutaciones
 * internas (ej: actualizarEstadoSample) invaliden comparaciones en React.
 * Sin esto, el polling en usePanelSincronizacion compara objetos mutados
 * contra sí mismos y nunca detecta cambios (ej: imagen actualizada).
 */
export function obtenerHistorialSamples(limite = 50): EntradaHistorialSample[] {
    return datos.historialSamples.slice(0, limite).map(e => ({ ...e }));
}

/**
 * Recarga el historial per-sample desde el Tauri Store.
 *
 * Necesario en ventanas MPA (sync panel): la ventana main actualiza el Store
 * (ej: imagen post-pipeline), pero la ventana sync tiene su propia copia en memoria.
 * El Store de Tauri 2.0 comparte el backend entre ventanas, así que store.get()
 * retorna datos frescos sin leer de disco.
 *
 * Throttle interno de 2s: balance entre frescura de datos y carga IPC.
 * Reducido de 5s para que imágenes rehidratadas por la ventana main
 * aparezcan más rápido en el sync panel.
 */
const RECARGA_STORE_INTERVALO_MS = 2000;

export async function recargarHistorialDesdeStore(): Promise<void> {
    const ahora = Date.now();
    if (ahora - ultimaRecargaStore < RECARGA_STORE_INTERVALO_MS) return;
    setUltimaRecargaStore(ahora);

    if (!storeCache) return;
    try {
        const guardado = await storeCache.get<BaseSyncLocal>(STORE_KEY_TRACKING);
        if (guardado?.historialSamples) {
            datos.historialSamples = guardado.historialSamples;
            reconstruirIndiceSampleHistorial();
        }
    } catch {
        /* Store no disponible o error de lectura */
    }
}

/** Limpia el historial per-sample completo. */
export async function limpiarHistorialSamples(): Promise<void> {
    datos.historialSamples = [];
    estadoSyncTracking.indiceSampleHistorial.clear();
    estadoSyncTracking.indiceNombreSampleHistorial.clear();
    await persistir();

    /*
     * Notificar a otras ventanas Tauri (ej: main window) para que limpien
     * su copia in-memory del historial. Sin esto, la ventana main persiste
     * su datos.historialSamples viejo y sobreescribe el Store limpio.
     */
    try {
        const { emit } = await import('@tauri-apps/api/event');
        await emit('limpiar-historial-samples', {});
    } catch {
        /* Entorno sin Tauri — ignorar */
    }
}

/* Historial legacy (append-only, mantenido para compatibilidad con SincPanelTabs) */

export async function registrarAccion(accion: Omit<AccionHistorial, 'timestamp'>): Promise<void> {
    datos.historial.unshift({ ...accion, timestamp: Date.now() });

    /* Limitar tamaño del historial */
    if (datos.historial.length > MAX_HISTORIAL) {
        datos.historial = datos.historial.slice(0, MAX_HISTORIAL);
    }

    await registrarEnJournal('UPDATE_HISTORIAL', { historial: datos.historial });
}

export function obtenerHistorial(limite = 50): AccionHistorial[] {
    return datos.historial.slice(0, limite);
}

export async function limpiarHistorial(): Promise<void> {
    datos.historial = [];
    await persistir();
}
