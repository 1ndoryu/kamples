/* Lectura del endpoint GET /me/sync/colecciones y búsqueda de colecciones en el servidor. */

import { estaOnline } from '../../../varios-1/desktopService';
import { obtenerBaseUrlSync, obtenerHeadersSyncGet, extraerErrorRespuesta, tieneTokenSync } from '../syncGuards';
import { logSync } from '../syncLogger';
import type { RespuestaSyncColecciones } from './tipos';
import { leerCacheColeccionesFresca, guardarCacheColecciones } from './estado';
import { normalizarNombreColeccion } from './helpers';

/* [193A-3] parentId agregado para distinguir subcarpetas con el mismo nombre bajo padres distintos.
 * Sin este filtro, Carpeta_A/Sub_A y Carpeta_B/Sub_A comparten la misma colección del servidor. */
export async function buscarColeccionServidorPorNombre(nombreNormalizado: string, parentId: number | null = null): Promise<number | null> {
    const datosServidor = await obtenerColeccionesDelServidor();
    if (!datosServidor) return null;

    const objetivo = nombreNormalizado.toLowerCase();
    const existente = datosServidor.colecciones.find(c => {
        if (c.nombre.toLowerCase() !== objetivo) return false;
        /* Filtrar por parent_id para no confundir subcarpetas homónimas bajo padres distintos */
        if (parentId !== null) return c.parent_id === parentId;
        /* Sin parentId buscamos solo colecciones raíz */
        return c.parent_id === null || c.parent_id === undefined;
    });
    return existente?.id ?? null;
}

/*
 * Versión pública de buscarColeccionServidorPorNombre.
 * Usada por syncWatcherSetup para el fallback de rename:
 * si la colección no está en tracking local, buscar en servidor antes de crear duplicada.
 */
export async function buscarColeccionServidorPorNombrePublico(nombre: string, parentId: number | null = null): Promise<number | null> {
    return buscarColeccionServidorPorNombre(normalizarNombreColeccion(nombre), parentId);
}

/**
 * Obtiene las colecciones con samples del servidor para sync.
 * Usa el nuevo endpoint optimizado GET /me/sync/colecciones.
 * C289: Con caché TTL de 10s para evitar 429 por llamadas concurrentes.
 */
export async function obtenerColeccionesDelServidor(): Promise<RespuestaSyncColecciones | null> {
    if (!estaOnline()) return null;

    /* QK77-B: No intentar si no hay token — evita 401 ruidoso en consola */
    if (!tieneTokenSync()) return null;

    /* C289: Retornar caché si es suficientemente fresca */
    const enCache = leerCacheColeccionesFresca();
    if (enCache) return enCache;

    try {
        const baseUrl = obtenerBaseUrlSync();
        /* C7: Cache-busting + no-cache para que imágenes recién asignadas se obtengan frescas */
        const resp = await fetch(`${baseUrl}/kamples/v1/me/sync/colecciones?_t=${Date.now()}`, {
            headers: { ...obtenerHeadersSyncGet(), 'Cache-Control': 'no-cache' },
        });

        if (!resp.ok) {
            const detalle = await extraerErrorRespuesta(resp);
            logSync.error('syncCollectionService', '[SyncCollection] Error obteniendo colecciones:', [resp.status, detalle]);
            return null;
        }

        const json = await resp.json();
        /* PHP retorna { data: { colecciones, sinColeccion } } */
        const data = json?.data ?? json;

        const resultado: RespuestaSyncColecciones = {
            colecciones: data?.colecciones ?? [],
            sinColeccion: data?.sinColeccion ?? [],
        };

        /* C289: Guardar en caché */
        guardarCacheColecciones(resultado);

        return resultado;
    } catch (err) {
        logSync.error('syncCollectionService', '[SyncCollection] Error en fetch colecciones:', err);
        return null;
    }
}
