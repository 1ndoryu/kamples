/* Creación de colecciones en el servidor a partir de carpetas locales, con deduplicación en vuelo. */

import { estaOnline } from '../../../varios-1/desktopService';
import { obtenerBaseUrlSync, obtenerHeadersSync } from '../syncGuards';
import { logSync } from '../syncLogger';
import { todasLasColecciones } from '../../varios-2/syncTrackingService';
import { MAX_REINTENTOS_CREAR_COLECCION } from './constantes';
import {
    obtenerCreacionEnVuelo,
    registrarCreacionEnVuelo,
    liberarCreacionEnVuelo,
    invalidarCacheColecciones,
} from './estado';
import {
    dormir,
    calcularBackoffCreacion,
    normalizarNombreColeccion,
    sanitizarNombreCarpeta,
    registrarColeccionNuevaLocal,
    encolarCreacionColeccion,
} from './helpers';
import { buscarColeccionServidorPorNombre } from './servidor';

/**
 * B-Rename: Permite al handler de rename esperar una creación en vuelo para
 * el nombre anterior. Si hay un POST pendiente para la colección vieja,
 * lo espera y retorna el ID creado para que pueda renombrarse en vez de duplicarse.
 */
export async function esperarCreacionEnVuelo(nombre: string, parentId: number | null = null): Promise<number | null> {
    const nombreNormalizado = normalizarNombreColeccion(nombre);
    const clave = `${parentId ?? 'raiz'}_${nombreNormalizado.toLowerCase()}`;
    const enVuelo = obtenerCreacionEnVuelo(clave);
    if (!enVuelo) return null;
    try {
        return await enVuelo;
    } catch {
        return null;
    }
}

/**
 * Crear una colección en el servidor a partir de carpeta local nueva.
 * Retorna el ID de la colección creada, o null si falla.
 */
export async function crearColeccionDesdeLocal(nombre: string, parentId: number | null = null): Promise<number | null> {
    try {
        const nombreNormalizado = normalizarNombreColeccion(nombre);
        const claveColeccion = `${parentId ?? 'raiz'}_${nombreNormalizado.toLowerCase()}`;

        if (!estaOnline()) {
            encolarCreacionColeccion(nombreNormalizado);
            return null;
        }

        /* Verificar si ya existe en tracking local antes de crear en servidor.
         * Previene duplicación cuando watcher + polling disparan casi simultáneamente. */
        const coleccionesLocales = todasLasColecciones();
        const carpetaEsperada = sanitizarNombreCarpeta(nombreNormalizado).toLowerCase();
        const yaExiste = coleccionesLocales.find(c =>
            c.parentId === parentId
            && (c.nombre.toLowerCase() === nombreNormalizado.toLowerCase()
                || c.carpetaLocal.toLowerCase() === carpetaEsperada),
        );
        if (yaExiste) {
            logSync.info('syncCollectionService', '[SyncCollection] Colección ya existe en tracking, omitiendo POST:', [nombreNormalizado, '→ id:', yaExiste.id]);
            return yaExiste.id;
        }

        const enVuelo = obtenerCreacionEnVuelo(claveColeccion);
        if (enVuelo) return enVuelo;

        const promesa = (async (): Promise<number | null> => {
            /* [193A-3] Pasar parentId para evitar colisión con subcarpetas homónimas bajo otro padre */
            const existenteServidor = await buscarColeccionServidorPorNombre(nombreNormalizado, parentId);
            if (existenteServidor) {
                await registrarColeccionNuevaLocal(existenteServidor, nombreNormalizado, parentId);
                return existenteServidor;
            }

            const baseUrl = obtenerBaseUrlSync();
            const bodyCrear: Record<string, unknown> = { nombre: nombreNormalizado, descripcion: '', publica: true };
            if (parentId !== null) bodyCrear.parent_id = parentId;

            for (let intento = 1; intento <= MAX_REINTENTOS_CREAR_COLECCION; intento++) {
                const resp = await fetch(`${baseUrl}/kamples/v1/colecciones`, {
                    method: 'POST',
                    headers: obtenerHeadersSync(),
                    body: JSON.stringify(bodyCrear),
                });

                if (resp.ok) {
                    const json = await resp.json();
                    const id = json?.data?.id ?? json?.id;
                    if (id) {
                        const idNum = Number(id);
                        await registrarColeccionNuevaLocal(idNum, nombreNormalizado, parentId);
                        /* C289: Invalidar caché para que la próxima consulta incluya la nueva colección */
                        invalidarCacheColecciones();
                        return idNum;
                    }
                    return null;
                }

                if (resp.status === 409) {
                    /* [193A-29] Pasar parentId para distinguir subcarpetas homónimas bajo padres distintos.
                     * Sin parentId, un 409 en "Carpeta A/Sub" podría retornar el ID de "Carpeta B/Sub". */
                    const idExistente = await buscarColeccionServidorPorNombre(nombreNormalizado, parentId);
                    if (idExistente) {
                        await registrarColeccionNuevaLocal(idExistente, nombreNormalizado, parentId);
                        return idExistente;
                    }
                    return null;
                }

                const reintentable = resp.status === 429 || resp.status >= 500;
                if (!reintentable) {
                    logSync.error('syncCollectionService', '[SyncCollection] Error creando colección:', resp.status);
                    return null;
                }

                if (intento < MAX_REINTENTOS_CREAR_COLECCION) {
                    const delay = calcularBackoffCreacion(intento, resp.headers.get('Retry-After'));
                    logSync.warn('syncCollectionService', `[SyncCollection] Crear colección reintentable (HTTP ${resp.status}), reintentando en ${delay}ms:`, nombreNormalizado);
                    await dormir(delay);
                    continue;
                }

                logSync.error('syncCollectionService', '[SyncCollection] Crear colección agotó reintentos, encolando:', [resp.status, nombreNormalizado]);
                encolarCreacionColeccion(nombreNormalizado);
                return null;
            }

            return null;
        })();

        registrarCreacionEnVuelo(claveColeccion, promesa);
        try {
            return await promesa;
        } finally {
            liberarCreacionEnVuelo(claveColeccion);
        }
    } catch (err) {
        logSync.error('syncCollectionService', '[SyncCollection] Error creando colección desde local:', err);
        return null;
    }
}
