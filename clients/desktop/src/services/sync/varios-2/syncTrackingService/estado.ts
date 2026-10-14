/*
 * Estado de módulo compartido por los submódulos de syncTrackingService.
 * Las variables que se reasignan se exportan con `let` (binding vivo: los lectores
 * ven siempre el valor actual) y solo se reasignan mediante su setter. Las colecciones
 * (Map/Set) son const y se mutan in-place.
 */
import type { BaseSyncLocal } from './tipos';

export const STORE_FILE = 'sync-config.json';
export const STORE_KEY_TRACKING = 'sync_tracking_v2';

export let datos: BaseSyncLocal = {
    archivos: {},
    colecciones: {},
    sinColeccion: [],
    historial: [],
    historialSamples: [],
};

export function setDatos(nuevo: BaseSyncLocal): void {
    datos = nuevo;
}

/* Colecciones mutables de módulo agrupadas en un único store: se mutan in-place, nunca se reasignan. */
export const estadoSyncTracking = {
    /* Índice en memoria para O(1) upsert en historialSamples. Clave: sampleId → índice en array.
     * Entradas sin sampleId real (pre-upload) se indexan por nombre en indiceNombreSampleHistorial. */
    indiceSampleHistorial: new Map<number, number>(),
    indiceNombreSampleHistorial: new Map<string, number>(),

    /*
     * Índices secundarios para O(1) lookup por ruta y nombre.
     * Se reconstruyen al cargar datos y se mantienen en cada registrar/eliminar.
     * Clave del mapa → clave de tracking para acceso directo a datos.archivos[clave].
     */
    indiceRuta: new Map<string, string>(),
    indiceNombre: new Map<string, string[]>(),

    /* Set sombra para O(1) en sinColeccion. El array en datos se mantiene para serialización al store. */
    sinColeccionSet: new Set<number>(),
};

/* Modo lote: suspende persistencia hasta finalizarLote(). Evita 100+ escrituras en sync masiva. */
export let enLote = false;

export function setEnLote(valor: boolean): void {
    enLote = valor;
}

/* TA1: Referencia al unlisten del listener cross-window para cleanup en reinicialización */
export let limpiarHistorialUnlisten: (() => void) | null = null;

export function setLimpiarHistorialUnlisten(unlisten: (() => void) | null): void {
    limpiarHistorialUnlisten = unlisten;
}

/* TC1: Versión local conocida del Store. Se compara con la del Store antes de escribir. */
export let versionLocalConocida = 0;

export function setVersionLocalConocida(version: number): void {
    versionLocalConocida = version;
}

/* eslint-disable @typescript-eslint/no-explicit-any -- Tauri Store typing requires flexible interface */
export type StoreCache = {
    get: <T>(key: string) => Promise<T | null>;
    set: (key: string, val: unknown) => Promise<void>;
    save: () => Promise<void>;
};

export let storeCache: StoreCache | null = null;

export function setStoreCache(store: StoreCache | null): void {
    storeCache = store;
}

/* WAL (Write-Ahead Log): flag que indica si el journal está activo */
export let journalActivo = false;

export function setJournalActivo(activo: boolean): void {
    journalActivo = activo;
}

/* Marca de tiempo de la última recarga del historial desde el Store (throttle de 2s) */
export let ultimaRecargaStore = 0;

export function setUltimaRecargaStore(marca: number): void {
    ultimaRecargaStore = marca;
}
