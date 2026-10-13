import { datos, indiceRuta, sinColeccionSet } from './estado';
import { registrarEnJournal } from './persistencia/journal';
import type { ColeccionLocal } from './tipos';

/* Colecciones */

export function obtenerColeccion(id: number): ColeccionLocal | null {
    return datos.colecciones[id] ?? null;
}

export function todasLasColecciones(): ColeccionLocal[] {
    return Object.values(datos.colecciones);
}

export async function registrarColeccion(coleccion: ColeccionLocal): Promise<void> {
    datos.colecciones[coleccion.id] = coleccion;
    await registrarEnJournal('ADD_COLLECTION', { coleccion });
}

export async function eliminarColeccion(id: number): Promise<void> {
    delete datos.colecciones[id];
    await registrarEnJournal('DELETE_COLLECTION', { id });
}

/*
 * Buscar subcolección por nombre de carpeta dentro de una colección padre.
 * carpetaPadre: nombre de carpeta del padre (nivel 1).
 * nombreSub: nombre de subcarpeta (nivel 2).
 */
export function buscarSubcoleccion(carpetaPadre: string, nombreSub: string): ColeccionLocal | null {
    const padre = buscarColeccionPorCarpeta(carpetaPadre);
    if (!padre) return null;
    const busqueda = nombreSub.toLowerCase();
    for (const col of Object.values(datos.colecciones)) {
        if (col.parentId !== padre.id) continue;
        if (col.carpetaLocal.toLowerCase() === busqueda) return col;
        if (col.nombre.toLowerCase() === busqueda) return col;
    }
    return null;
}

/* Listar subcolecciones de un padre por ID */
export function subcoleccionesDePadre(parentId: number): ColeccionLocal[] {
    return Object.values(datos.colecciones).filter(c => c.parentId === parentId);
}

export async function actualizarNombreColeccion(id: number, nombre: string, carpetaLocal: string): Promise<void> {
    const col = datos.colecciones[id];
    if (!col) return;

    const carpetaAnterior = col.carpetaLocal;
    col.nombre = nombre;
    col.carpetaLocal = carpetaLocal;

    /*
     * C289: Actualizar rutaLocal e indiceRuta de archivos tras rename.
     * Sin esto, los archivos mantienen la ruta vieja → el watcher los ve como
     * archivos nuevos en la nueva ruta → re-upload → duplicados.
     * Se actualizan archivos de la colección directa + subcollecciones hijas.
     */
    if (carpetaAnterior !== carpetaLocal) {
        const idsAfectadas = new Set<number>([id]);
        for (const sub of Object.values(datos.colecciones)) {
            if (sub.parentId === id) idsAfectadas.add(sub.id);
        }

        const escapedOld = carpetaAnterior.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
        const patronReemplazo = new RegExp(`([/\\\\])${escapedOld}([/\\\\])`, 'i');

        for (const [clave, archivo] of Object.entries(datos.archivos)) {
            if (archivo.coleccionId === null || !idsAfectadas.has(archivo.coleccionId)) continue;

            const nuevaRuta = archivo.rutaLocal.replace(patronReemplazo, `$1${carpetaLocal}$2`);
            if (nuevaRuta !== archivo.rutaLocal) {
                const rutaNormAnterior = archivo.rutaLocal.replace(/\\/g, '/');
                indiceRuta.delete(rutaNormAnterior);
                archivo.rutaLocal = nuevaRuta;
                const rutaNormNueva = nuevaRuta.replace(/\\/g, '/');
                indiceRuta.set(rutaNormNueva, clave);
            }
        }
    }

    await registrarEnJournal('RENAME_COLLECTION', { id, nombre, carpetaLocal });
}

export function buscarColeccionPorCarpeta(carpetaLocal: string): ColeccionLocal | null {
    const busqueda = carpetaLocal.toLowerCase();
    for (const col of Object.values(datos.colecciones)) {
        /*
         * Comparación case-insensitive: Windows no distingue mayúsculas en nombres de carpeta.
         * Sin esto, el watcher reporta "Mi Carpeta" pero tracking tiene "mi carpeta" → no match
         * → se crea colección duplicada en vez de renombrar.
         */
        if (col.carpetaLocal.toLowerCase() === busqueda) return col;
        /* Fallback: comparar también por nombre de colección (el watcher puede reportar
         * el nombre exacto del directorio que coincide con el nombre, no con carpetaLocal sanitizada) */
        if (col.nombre.toLowerCase() === busqueda) return col;
    }
    return null;
}

/* Sin colección */

export function esSinColeccion(sampleId: number): boolean {
    return sinColeccionSet.has(sampleId);
}

export async function agregarSinColeccion(sampleId: number): Promise<void> {
    if (!sinColeccionSet.has(sampleId)) {
        sinColeccionSet.add(sampleId);
        datos.sinColeccion.push(sampleId);
        await registrarEnJournal('MOVE_FILE', { accion: 'agregarSinColeccion', sampleId });
    }
}

export async function quitarSinColeccion(sampleId: number): Promise<void> {
    sinColeccionSet.delete(sampleId);
    datos.sinColeccion = datos.sinColeccion.filter(id => id !== sampleId);
    await registrarEnJournal('MOVE_FILE', { accion: 'quitarSinColeccion', sampleId });
}

export function totalSinColeccion(): number {
    return datos.sinColeccion.length;
}
