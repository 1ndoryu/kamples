/*
 * Tipos internos del watcher de carpeta de sincronización (fileWatcherService).
 * Solo declaraciones de tipo: no genera código en runtime.
 */

/* QL107: Tipos de @tauri-apps/plugin-fs definidos localmente.
 * TS 5.9 con moduleResolution 'bundler' no resuelve los export type
 * del modulo via import type ni import(). Los tipos son identicos. */
export interface EventoWatch {
    type: string | Record<string, unknown>;
    paths: string[];
    attrs: unknown;
}

export type UnwatchFn = () => void;

export interface ArchivoBufferado {
    ruta: string;
    nombre: string;
    carpetas: string[];
}

export interface EliminacionPendiente {
    ruta: string;
    nombreArchivo: string;
    timeout: ReturnType<typeof setTimeout>;
}

/*
 * C287: Buffer para rename events no pareados (ver manejarRenameNoPareado).
 */
export interface RenameNoPareado {
    ruta: string;
    timestamp: number;
    timeout: ReturnType<typeof setTimeout>;
    baseNormalizada: string;
    carpetaBase: string;
}

export interface PendienteCreacionCarpeta {
    timeout: ReturnType<typeof setTimeout>;
    nombreOriginal: string;
    rutaCompleta: string;
}

export interface CarpetaEliminadaPendiente {
    nombre: string;
    ruta: string;
    timeout: ReturnType<typeof setTimeout>;
}

export interface PendienteCreacionSubcarpeta {
    timeout: ReturnType<typeof setTimeout>;
    nombreOriginal: string;
    carpetaPadre: string;
    rutaCompleta: string;
}

export interface SubcarpetaEliminadaPendiente {
    nombre: string;
    carpetaPadre: string;
    ruta: string;
    timeout: ReturnType<typeof setTimeout>;
}

/* Callbacks externos que fileWatcherService llama según el evento detectado */
export type OnArchivoNuevoFn = (ruta: string, nombreArchivo: string, carpetas: string[]) => void;
export type OnArchivoEliminadoFn = (ruta: string) => void;
export type OnArchivoMovidoFn = (rutaAnterior: string, rutaNueva: string, nombreArchivo: string, carpetas: string[]) => void;

/* C357: Callbacks para eventos de carpetas de nivel 1 (colecciones) */
export type OnCarpetaNuevaFn = (nombre: string, rutaCompleta: string) => void;
export type OnCarpetaRenombradaFn = (nombreAnterior: string, nombreNuevo: string, rutaNueva: string) => void;

/* C387: Callbacks para eventos de subcarpetas de nivel 2 (subcolecciones) */
export type OnSubcarpetaNuevaFn = (nombreSub: string, carpetaPadre: string, rutaCompleta: string) => void;
export type OnSubcarpetaRenombradaFn = (nombreAnterior: string, nombreNuevo: string, carpetaPadre: string, rutaNueva: string) => void;
