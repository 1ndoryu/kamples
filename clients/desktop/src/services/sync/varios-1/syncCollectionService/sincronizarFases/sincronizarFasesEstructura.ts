/* Fase 1 del sync completo: estructura de carpetas (colecciones y subcolecciones), normalización legacy y carpeta "Sin colección". */

import { logSync } from '../../syncLogger';
import {
    registrarColeccion,
    registrarAccion,
    registrarArchivo,
    obtenerColeccion,
    todosLosArchivos,
} from '../../../varios-2/syncTrackingService';
import { CARPETA_SIN_COLECCION, CARPETA_SIN_COLECCION_LEGACY } from '../constantes';
import { coleccionYaNormalizadaEnSesion, marcarColeccionNormalizadaEnSesion } from '../estado';
import { normalizarNombreColeccion, sanitizarNombreCarpeta, resolverConflictoNombre } from '../helpers';
import { renombrarColeccionEnServidor } from '../movimiento';
import { manejarRenombreColeccion } from '../reconciliacion';
import type { RespuestaSyncColecciones, CallbackProgresoColecciones, FsColecciones, JoinColecciones } from '../tipos';

/**
 * Crea o actualiza las carpetas de colecciones y subcolecciones del servidor.
 * Devuelve la ruta de la carpeta "Sin colección" para la fase de descarga.
 */
export async function sincronizarEstructura(
    carpetaBase: string,
    datosServidor: RespuestaSyncColecciones,
    onProgreso: CallbackProgresoColecciones | undefined,
    fs: FsColecciones,
    join: JoinColecciones,
): Promise<string> {
    /* Fase 1: Crear/actualizar estructura de carpetas */
    onProgreso?.({ fase: 'estructura', actual: 0, total: datosServidor.colecciones.length + 1 });

    const nombresUsados = new Set<string>();

    /*
     * Separar colecciones raíz y subcolecciones.
     * Procesar raíz primero para que las subcarpetas tengan un padre creado.
     */
    const coleccionesRaiz = datosServidor.colecciones.filter(c => c.parent_id === null || c.parent_id === undefined);
    const subcolecciones = datosServidor.colecciones.filter(c => c.parent_id !== null && c.parent_id !== undefined);

    /* Sincronizar colecciones raíz del servidor → carpetas locales */
    for (const colServer of coleccionesRaiz) {
        const nombreNormalizado = normalizarNombreColeccion(colServer.nombre);
        if (nombreNormalizado !== colServer.nombre && !coleccionYaNormalizadaEnSesion(colServer.id)) {
            await renombrarColeccionEnServidor(colServer.id, nombreNormalizado);
            colServer.nombre = nombreNormalizado;
            marcarColeccionNormalizadaEnSesion(colServer.id);
        }

        const colLocal = obtenerColeccion(colServer.id);

        if (colLocal) {
            /* Verificar si el nombre cambió en el servidor */
            if (colLocal.nombre !== colServer.nombre) {
                await manejarRenombreColeccion(carpetaBase, colLocal, colServer.nombre);
            }
            /* F5.2: Actualizar version local desde servidor */
            const versionServidor = colServer.version ?? 1;
            if ((colLocal.version ?? 0) !== versionServidor) {
                await registrarColeccion({ ...colLocal, version: versionServidor });
            }
            nombresUsados.add(colLocal.carpetaLocal);
        } else {
            /* Colección nueva del servidor — crear carpeta local */
            const nombreCarpeta = sanitizarNombreCarpeta(colServer.nombre);
            const nombreFinal = resolverConflictoNombre(nombreCarpeta, nombresUsados);
            nombresUsados.add(nombreFinal);

            const rutaCarpeta = await join(carpetaBase, nombreFinal);
            try {
                await fs.mkdir(rutaCarpeta, { recursive: true });
            } catch { /* puede existir */ }

            await registrarColeccion({
                id: colServer.id,
                nombre: colServer.nombre,
                carpetaLocal: nombreFinal,
                creadaLocalmente: false,
                parentId: null,
                version: colServer.version ?? 1,
            });

            await registrarAccion({
                tipo: 'creado',
                descripcion: `Carpeta creada para colección "${colServer.nombre}"`,
                coleccionId: colServer.id,
            });
        }
    }

    /* Sincronizar subcolecciones del servidor → subcarpetas locales */
    for (const subServer of subcolecciones) {
        const padreLocal = obtenerColeccion(subServer.parent_id!);
        if (!padreLocal) {
            logSync.warn('syncCollectionService', '[SyncCollection] Padre no encontrado para subcolección:', [subServer.nombre, 'parent_id:', subServer.parent_id]);
            continue;
        }

        const nombreNormalizado = normalizarNombreColeccion(subServer.nombre);
        const subLocal = obtenerColeccion(subServer.id);

        if (subLocal) {
            if (subLocal.nombre !== subServer.nombre) {
                /* Renombrar subcarpeta si cambió el nombre en servidor */
                const carpetaPadre = await join(carpetaBase, padreLocal.carpetaLocal);
                await manejarRenombreColeccion(carpetaPadre, subLocal, subServer.nombre);
            }
            /* F5.2: Actualizar version local desde servidor */
            const versionSub = subServer.version ?? 1;
            if ((subLocal.version ?? 0) !== versionSub) {
                await registrarColeccion({ ...subLocal, version: versionSub });
            }
        } else {
            /* Subcolección nueva — crear subcarpeta dentro de la carpeta padre */
            const nombreCarpeta = sanitizarNombreCarpeta(nombreNormalizado);
            const carpetaPadre = await join(carpetaBase, padreLocal.carpetaLocal);
            const rutaSubcarpeta = await join(carpetaPadre, nombreCarpeta);

            try {
                await fs.mkdir(rutaSubcarpeta, { recursive: true });
            } catch { /* puede existir */ }

            await registrarColeccion({
                id: subServer.id,
                nombre: subServer.nombre,
                carpetaLocal: nombreCarpeta,
                creadaLocalmente: false,
                parentId: subServer.parent_id!,
                version: subServer.version ?? 1,
            });

            await registrarAccion({
                tipo: 'creado',
                descripcion: `Subcarpeta creada para subcolección "${subServer.nombre}" en "${padreLocal.nombre}"`,
                coleccionId: subServer.id,
            });
        }
    }

    /* Normalizar carpeta legacy "Sin colección" -> "Sin colección" */
    const rutaSinColLegacy = await join(carpetaBase, CARPETA_SIN_COLECCION_LEGACY);
    const rutaSinColCanonical = await join(carpetaBase, CARPETA_SIN_COLECCION);

    const existeLegacy = await fs.exists(rutaSinColLegacy);
    const existeCanonical = await fs.exists(rutaSinColCanonical);

    if (existeLegacy && !existeCanonical) {
        try {
            await fs.rename(rutaSinColLegacy, rutaSinColCanonical);
            await registrarAccion({
                tipo: 'renombrado',
                descripcion: `Carpeta renombrada: "${CARPETA_SIN_COLECCION_LEGACY}" → "${CARPETA_SIN_COLECCION}"`,
            });
        } catch (err) {
            logSync.warn('syncCollectionService', '[SyncCollection] No se pudo renombrar carpeta legacy Sin colección:', err);
        }
    }

    /* Reparar rutas legacy en tracking para evitar desalineación */
    let rutasCorregidas = 0;
    const archivosTracking = todosLosArchivos();
    for (const archivo of archivosTracking) {
        if (!archivo.rutaLocal.includes(CARPETA_SIN_COLECCION_LEGACY)) continue;

        const rutaCorregida = archivo.rutaLocal.replace(
            CARPETA_SIN_COLECCION_LEGACY,
            CARPETA_SIN_COLECCION,
        );

        if (rutaCorregida !== archivo.rutaLocal) {
            await registrarArchivo({
                ...archivo,
                rutaLocal: rutaCorregida,
            });
            rutasCorregidas++;
        }
    }

    if (rutasCorregidas > 0) {
        await registrarAccion({
            tipo: 'renombrado',
            descripcion: `Tracking reparado: ${rutasCorregidas} ruta(s) actualizadas a "${CARPETA_SIN_COLECCION}"`,
        });
    }

    /* Asegurar carpeta "Sin colección" */
    const rutaSinCol = await join(carpetaBase, CARPETA_SIN_COLECCION);
    try {
        await fs.mkdir(rutaSinCol, { recursive: true });
    } catch { /* puede existir */ }

    return rutaSinCol;
}
