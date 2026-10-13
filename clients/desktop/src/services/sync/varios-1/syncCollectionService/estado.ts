/* Estado de módulo de syncCollectionService. Solo se accede por funciones:
 * ningún otro módulo del servicio guarda copias locales de estas estructuras. */

import type { RespuestaSyncColecciones } from './tipos';

/* Caché de sesión: IDs de colecciones ya normalizadas para evitar
 * llamadas repetidas a renombrarColeccionEnServidor en cada polling (60s). */
const coleccionesNormalizadasEnSesion = new Set<number>();

/* Evita POST duplicados cuando watcher/polling disparan la misma colección a la vez. */
const creacionesColeccionEnVuelo = new Map<string, Promise<number | null>>();

/* C289: Cache client-side para evitar 429.
 * obtenerColeccionesDelServidor se llama desde múltiples puntos
 * (sincronizarColecciones, rehidratacion, buscarColeccionServidorPorNombre).
 * Sin caché, cada call es un roundtrip → agota rate limit rápidamente. */
const CACHE_COLECCIONES_TTL_MS = 10_000;
let cacheColecciones: { datos: RespuestaSyncColecciones; timestamp: number } | null = null;

export function coleccionYaNormalizadaEnSesion(id: number): boolean {
    return coleccionesNormalizadasEnSesion.has(id);
}

export function marcarColeccionNormalizadaEnSesion(id: number): void {
    coleccionesNormalizadasEnSesion.add(id);
}

export function obtenerCreacionEnVuelo(clave: string): Promise<number | null> | undefined {
    return creacionesColeccionEnVuelo.get(clave);
}

export function registrarCreacionEnVuelo(clave: string, promesa: Promise<number | null>): void {
    creacionesColeccionEnVuelo.set(clave, promesa);
}

export function liberarCreacionEnVuelo(clave: string): void {
    creacionesColeccionEnVuelo.delete(clave);
}

/** Devuelve la caché si sigue dentro del TTL; null si no hay o ya caducó. */
export function leerCacheColeccionesFresca(): RespuestaSyncColecciones | null {
    if (cacheColecciones && (Date.now() - cacheColecciones.timestamp) < CACHE_COLECCIONES_TTL_MS) {
        return cacheColecciones.datos;
    }
    return null;
}

export function guardarCacheColecciones(datos: RespuestaSyncColecciones): void {
    cacheColecciones = { datos, timestamp: Date.now() };
}

/** Invalida el caché manualmente (ej: después de crear/renombrar colección). */
export function invalidarCacheColecciones(): void {
    cacheColecciones = null;
}
