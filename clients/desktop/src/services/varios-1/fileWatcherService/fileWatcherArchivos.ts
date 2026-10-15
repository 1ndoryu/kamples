/*
 * Manejo de archivos de audio nuevos y eliminados del watcher (fileWatcherService).
 * Detecta MOVEs (delete + create con el mismo nombre) y delega los archivos nuevos
 * al buffer de lote.
 */

import { logSync } from '../../sync/varios-1/syncLogger';
import { acumularArchivoNuevo } from './fileWatcherBatch';
import { CARPETAS_SISTEMA_NO_COLECCION, GRACIA_MOVE_MS } from './fileWatcherConstantes';
import {
    estadoFileWatcher,
    onArchivoEliminado,
    onArchivoMovido,
} from './fileWatcherEstado';

/*
 * Maneja la detección de un archivo de audio nuevo en la carpeta sync.
 * Antes de emitir onArchivoNuevo, verifica si hay una eliminación pendiente
 * con el mismo nombre → en ese caso es un MOVE, no un archivo nuevo.
 */
export function manejarArchivoNuevo(rutaOriginal: string, rutaNormalizada: string, carpetaBase: string): void {
    /* Extraer todas las carpetas padre relativas a la carpeta base de sync */
    const baseNormalizada = carpetaBase.replace(/\\/g, '/');
    const relativa = rutaNormalizada.startsWith(baseNormalizada + '/')
        ? rutaNormalizada.slice(baseNormalizada.length + 1)
        : '';

    /*
     * Guard defensivo: en circunstancias normales procesarEvento ya filtró rutas fuera
     * de la carpeta base. Este guard protege ante llamadas directas o cambios futuros.
     */
    if (!relativa) {
        logSync.warn('watcher', `Ruta fuera de carpeta base ignorada: ${rutaNormalizada} (base: ${carpetaBase})`);
        return;
    }

    const partes = relativa.split('/');
    const nombreArchivo = partes.pop() ?? '';
    /*
     * Carpetas entre la base de sync y el archivo (max 3 niveles).
     * QL70: Filtrar nombres de carpetas de sistema (sin colección, duplicados)
     * para que el server trate estos archivos como "sin colección" en vez
     * de crear una colección con ese nombre.
     */
    const carpetas = partes.filter(
        c => !CARPETAS_SISTEMA_NO_COLECCION.includes(c.toLowerCase()),
    );

    /* Verificar si hay una eliminación pendiente con el mismo nombre */
    const clave = nombreArchivo.toLowerCase();
    const pendiente = estadoFileWatcher.eliminacionesPendientes.get(clave);

    if (pendiente) {
        /* Es un MOVE: cancelar la eliminación pendiente y emitir move */
        clearTimeout(pendiente.timeout);
        estadoFileWatcher.eliminacionesPendientes.delete(clave);

        logSync.info('watcher', `Move detectado: ${pendiente.ruta} → ${rutaOriginal}`);

        if (onArchivoMovido) {
            onArchivoMovido(pendiente.ruta, rutaOriginal, nombreArchivo, carpetas);
        }
        return;
    }

    logSync.info('watcher', `Archivo nuevo detectado: ${nombreArchivo} carpetas: ${carpetas.join('/')}`);

    /*
     * QL76: Acumular en buffer en vez de procesar inmediatamente.
     * Cuando se copian muchos archivos, el hash I/O y los uploads compiten
     * con la copia en curso. El buffer espera un periodo de quietud antes
     * de despachar el batch completo al uploadQueue.
     */
    acumularArchivoNuevo(rutaOriginal, nombreArchivo, carpetas);
}

/*
 * Maneja la eliminación de un archivo de audio de la carpeta sync.
 * NO ejecuta inmediatamente — buferea por GRACIA_MOVE_MS para detectar MOVEs.
 * Si pasada la gracia no apareció un CREATE con el mismo nombre, se confirma.
 */
export function manejarArchivoEliminado(rutaOriginal: string): void {
    const normalizada = rutaOriginal.replace(/\\/g, '/');
    const nombreArchivo = normalizada.split('/').pop() ?? '';
    const clave = nombreArchivo.toLowerCase();

    /* Si ya hay una eliminación pendiente para este nombre, cancelar la anterior */
    const existente = estadoFileWatcher.eliminacionesPendientes.get(clave);
    if (existente) {
        clearTimeout(existente.timeout);
    }

    logSync.info('watcher', `Eliminación detectada (esperando ${GRACIA_MOVE_MS}ms por posible move): ${rutaOriginal}`);

    const timeout = setTimeout(() => {
        estadoFileWatcher.eliminacionesPendientes.delete(clave);
        logSync.info('watcher', `Eliminación confirmada (no fue move): ${rutaOriginal}`);
        if (onArchivoEliminado) {
            onArchivoEliminado(rutaOriginal);
        }
    }, GRACIA_MOVE_MS);

    estadoFileWatcher.eliminacionesPendientes.set(clave, {
        ruta: rutaOriginal,
        nombreArchivo,
        timeout,
    });
}
