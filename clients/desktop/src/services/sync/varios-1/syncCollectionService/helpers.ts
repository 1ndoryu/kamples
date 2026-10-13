/* Utilidades de nombres, reintentos y tracking compartidas por syncCollectionService. */

import { encolarOperacion } from '../../../varios-1/offlineQueueService';
import { obtenerBaseUrlSync } from '../syncGuards';
import { logSync } from '../syncLogger';
import {
    registrarColeccion,
    registrarAccion,
    todasLasColecciones,
    type ColeccionLocal,
} from '../../varios-2/syncTrackingService';
import {
    CARPETA_SIN_COLECCION,
    CARPETA_SIN_COLECCION_LEGACY,
    REGEX_CARACTERES_INVALIDOS,
    BACKOFF_BASE_CREAR_COLECCION_MS,
} from './constantes';

export function dormir(ms: number): Promise<void> {
    return new Promise(resolve => setTimeout(resolve, ms));
}

export function calcularBackoffCreacion(intento: number, retryAfterHeader: string | null): number {
    if (retryAfterHeader) {
        const segundos = Number(retryAfterHeader);
        if (Number.isFinite(segundos) && segundos > 0) {
            return Math.min(segundos * 1000, 60_000);
        }
    }

    const exponencial = BACKOFF_BASE_CREAR_COLECCION_MS * Math.pow(2, intento - 1);
    const jitter = Math.floor(Math.random() * 400);
    return Math.min(exponencial + jitter, 60_000);
}

/**
 * Sanitiza un nombre de colección para usarlo como nombre de carpeta.
 * Reemplaza caracteres inválidos y recorta espacios.
 */
export function sanitizarNombreCarpeta(nombre: string): string {
    return nombre
        .replace(REGEX_CARACTERES_INVALIDOS, '-')
        .replace(/\s+/g, ' ')
        .trim()
        .slice(0, 100); /* Limitar largo para evitar problemas de path */
}

/**
 * Resuelve conflictos de nombre de carpeta agregando sufijo numérico.
 * Si "Mi Colección" ya existe, retorna "Mi Colección (2)".
 */
export function resolverConflictoNombre(nombreBase: string, nombresExistentes: Set<string>): string {
    if (!nombresExistentes.has(nombreBase)) return nombreBase;

    let sufijo = 2;
    while (nombresExistentes.has(`${nombreBase} (${sufijo})`)) {
        sufijo++;
    }
    return `${nombreBase} (${sufijo})`;
}

export function normalizarNombreColeccion(nombre: string): string {
    const limpio = nombre.trim();
    if (limpio === CARPETA_SIN_COLECCION_LEGACY) {
        return CARPETA_SIN_COLECCION;
    }
    return limpio.split(CARPETA_SIN_COLECCION_LEGACY).join(CARPETA_SIN_COLECCION);
}

export function encolarCreacionColeccion(nombreNormalizado: string): void {
    const baseUrl = obtenerBaseUrlSync();
    encolarOperacion({
        tipo: 'crear_coleccion',
        endpoint: `${baseUrl}/kamples/v1/colecciones`,
        method: 'POST',
        body: { nombre: nombreNormalizado, descripcion: '', publica: true },
        claveDuplicacion: `crear_coleccion_${nombreNormalizado.toLowerCase()}`,
    }).catch(err => {
        logSync.error('syncCollectionService', '[SyncCollection] Error encolando creación de colección:', err);
    });
}

export async function registrarColeccionNuevaLocal(id: number, nombreNormalizado: string, parentId: number | null = null): Promise<void> {
    await registrarColeccion({
        id,
        nombre: nombreNormalizado,
        carpetaLocal: sanitizarNombreCarpeta(nombreNormalizado),
        creadaLocalmente: true,
        parentId,
        version: 0,
    });

    await registrarAccion({
        tipo: 'creado',
        descripcion: `Colección "${nombreNormalizado}" creada desde carpeta local${parentId ? ` (sub de #${parentId})` : ''}`,
        coleccionId: id,
    });
}

/* Utilidades */

/**
 * Busca una colección en tracking cuya carpeta ya NO existe en disco (huérfana).
 * Esto ocurre cuando una carpeta fue renombrada pero el tracking todavía tiene
 * el nombre viejo. Se usa como último recurso en el fallback de rename para
 * evitar crear colecciones duplicadas.
 *
 * Solo retorna colecciones raíz (parentId === null por defecto)
 * y excluye la carpeta nueva que sabemos existe.
 */
export async function buscarColeccionHuerfana(
    carpetaNuevaExcluir: string,
    parentId: number | null = null,
): Promise<ColeccionLocal | null> {
    try {
        const { exists } = await import('@tauri-apps/plugin-fs');
        const { join } = await import('@tauri-apps/api/path');
        const { obtenerConfigSync } = await import('../../varios-2/syncService');

        const config = obtenerConfigSync();
        if (!config?.carpetaLocal) return null;

        const colecciones = todasLasColecciones().filter(c => c.parentId === parentId);
        const excluirLower = carpetaNuevaExcluir.toLowerCase();

        for (const col of colecciones) {
            /* No considerar la colección que coincide con la nueva carpeta */
            if (col.carpetaLocal.toLowerCase() === excluirLower
                || col.nombre.toLowerCase() === excluirLower) continue;

            const rutaCarpeta = await join(config.carpetaLocal, col.carpetaLocal);
            const encontrada = await exists(rutaCarpeta);
            if (!encontrada) {
                logSync.info('syncCollectionService', '[SyncCollection] Colección huérfana encontrada:', [col.id, col.nombre, '→ carpeta no existe:', col.carpetaLocal]);
                return col;
            }
        }
        return null;
    } catch (err) {
        logSync.error('syncCollectionService', '[SyncCollection] Error buscando colección huérfana:', err);
        return null;
    }
}
