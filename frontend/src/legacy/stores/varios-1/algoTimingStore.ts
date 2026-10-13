/* [2003A-3] Creado para el modal de rendimiento del algoritmo de feed.
 * [2003A-3-A] Fix: usar apiPeticion para enviar X-WP-Nonce y evitar 401
 *  (current_user_can require cookie auth + nonce — fetch raw sin nonce falla).
 * [186A-1] Fix: el backend Rust devuelve Vec<TimingEntry> plano (no {ok,historial})
 *  y etapas como Vec<TimingStage> (array de {name,ms}) — convertir a flat map
 *  para que coincida con la interfaz RegistroTiming.etapas.
 * [296A-1] Fix: el backend ahora registra para todos los usuarios. El endpoint
 *  GET filtra por admin actual por defecto (?all=true para ver todos).
 *  Agregado user_id a los tipos raw y normalizado. */

import { create } from 'zustand';
import { apiPeticion } from '@app/services/api/varios-1/apiCliente';

/* Etapa individual devuelta por el backend Rust (TimingStage) */
interface TimingStageRaw {
    name: string;
    ms: number;
}

/* Entrada raw del backend Rust (TimingEntry) */
interface TimingEntryRaw {
    ts: string;
    total_ms: number;
    etapas: TimingStageRaw[];
    meta: Record<string, unknown>;
    user_id: number;
}

export interface EtapasTiming {
    [key: string]: number | undefined;
}

/* [2003A-3-B] Nodo del EXPLAIN ANALYZE de PostgreSQL */
export interface NodoExplain {
    etiqueta: string;
    tipo: string;
    totalMs: number;
    exclusivoMs: number;
    filas: number;
    profundidad: number;
    esCte: boolean;
    buffers: number;
}

/* [2003A-3-B] Datos de EXPLAIN ANALYZE parseados */
export interface ExplainData {
    planificacionMs: number;
    ejecucionMs: number;
    nodos: NodoExplain[];
}

export interface RegistroTiming {
    ts: string;
    totalMs: number;
    etapas: EtapasTiming;
    meta: {
        totalSamples?: number;
        usoCandidatos?: boolean;
        usoMV?: boolean;
        bulkFetch?: boolean;
        resultados?: number;
        limite?: number;
        offset?: number;
    };
    explain?: ExplainData;
    userId: number;
}

interface EstadoAlgoTiming {
    abierto: boolean;
    historial: RegistroTiming[];
    cargando: boolean;
    error: string | null;

    abrir: () => void;
    cerrar: () => void;
    cargarHistorial: () => Promise<void>;
    limpiarHistorial: () => Promise<void>;
}

/*
 * Convierte una entrada raw del backend (TimingEntry) al formato
 * esperado por el frontend (RegistroTiming). Mapea snake_case →
 * camelCase y convierte Vec<TimingStage> a flat EtapasTiming.
 */
function adaptarTimingEntry(raw: TimingEntryRaw): RegistroTiming {
    const etapas: EtapasTiming = {};
    for (const stage of raw.etapas) {
        etapas[stage.name] = stage.ms;
    }

    const metaRaw = raw.meta ?? {};

    return {
        ts: raw.ts,
        totalMs: raw.total_ms,
        etapas,
        meta: {
            resultados: metaRaw.items as number | undefined
                ?? metaRaw.resultados as number | undefined,
            totalSamples: metaRaw.total_samples as number | undefined
                ?? metaRaw.totalSamples as number | undefined,
            limite: metaRaw.limit as number | undefined
                ?? metaRaw.limite as number | undefined,
            offset: metaRaw.offset as number | undefined,
            usoCandidatos: metaRaw.uso_candidatos as boolean | undefined
                ?? metaRaw.usoCandidatos as boolean | undefined,
            usoMV: metaRaw.uso_mv as boolean | undefined
                ?? metaRaw.usoMV as boolean | undefined,
            bulkFetch: metaRaw.bulk_fetch as boolean | undefined
                ?? metaRaw.bulkFetch as boolean | undefined,
        },
        userId: raw.user_id,
    };
}

export const useAlgoTimingStore = create<EstadoAlgoTiming>((set, get) => ({
    abierto: false,
    historial: [],
    cargando: false,
    error: null,

    abrir: () => {
        set({ abierto: true });
        void get().cargarHistorial();
    },

    cerrar: () => set({ abierto: false }),

    cargarHistorial: async () => {
        set({ cargando: true, error: null });
        /*
         * El backend Rust devuelve Vec<TimingEntry> directamente (array plano),
         * NO un objeto {ok, historial}. apiPeticion retorna data = array.
         * Detectamos array y convertimos cada entrada.
         */
        const res = await apiPeticion<unknown>('/admin/algo-timing', { method: 'GET' });
        if (!res.ok) {
            set({ error: res.error ?? `HTTP ${res.status}`, cargando: false });
            return;
        }
        if (Array.isArray(res.data)) {
            const historial = (res.data as TimingEntryRaw[]).map(adaptarTimingEntry);
            set({ historial, cargando: false });
        } else {
            set({ error: 'Respuesta inesperada del servidor', cargando: false });
        }
    },

    limpiarHistorial: async () => {
        const res = await apiPeticion<unknown>('/admin/algo-timing', { method: 'DELETE' });
        if (res.ok) set({ historial: [] });
    },
}));
