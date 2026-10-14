/* Operaciones de usuario sobre la cola: reintentar, eliminar, limpiar y diagnóstico. */

import { estaOnline } from '../../varios-1/desktopService';
import { estado } from '../../sync/varios-2/syncState';
import { logSync } from '../../sync/varios-1/syncLogger';
import { ItemUploadCola, ResumenDebugUploadQueue } from './tipos';
import { cola, estadoUploadQueue, procesando, MAX_DETECCIONES_HASH, claveRutaEnCola, setCola } from './uploadQueueEstado';
import { guardarCola, guardarHashes } from './persistencia';
import { procesarCola, tieneSubidaPersistidaConfirmada } from './proceso';

/*
 * Retorna el estado actual de la cola (para UI).
 */
export function obtenerEstadoCola(): {
    items: ItemUploadCola[];
    totalPendientes: number;
    totalErrores: number;
    procesando: boolean;
} {
    return {
        items: [...cola],
        totalPendientes: cola.filter(i => i.estado === 'pendiente' || i.estado === 'subiendo').length,
        totalErrores: cola.filter(i => i.estado === 'error').length,
        procesando,
    };
}

export function obtenerResumenDebugUploadQueue(): ResumenDebugUploadQueue {
    const muestrasActivas = cola
        .filter(item => item.estado !== 'completado')
        .slice(0, 15)
        .map(item => ({
            id: item.id,
            nombreArchivo: item.nombreArchivo,
            estado: item.estado,
            intentos: item.intentos,
            rutaArchivo: item.rutaArchivo,
            sampleIdServidor: item.sampleIdServidor,
            ultimoError: item.ultimoError,
            timestampActualizado: item.timestampActualizado,
        }));

    return {
        totalItems: cola.length,
        totalPendientes: cola.filter(i => i.estado === 'pendiente').length,
        totalSubiendo: cola.filter(i => i.estado === 'subiendo').length,
        totalErrores: cola.filter(i => i.estado === 'error').length,
        totalCompletados: cola.filter(i => i.estado === 'completado').length,
        totalDuplicados: cola.filter(i => i.estado === 'duplicado').length,
        procesando,
        rutasEnCola: estadoUploadQueue.rutasEnCola.size,
        rutasEnVuelo: estadoUploadQueue.rutasEnVuelo.size,
        hashesConocidos: estadoUploadQueue.hashesConocidos.size,
        hashesEnVuelo: estadoUploadQueue.hashesEnVuelo.size,
        hashesPendientesEncola: estadoUploadQueue.hashesPendientesEncola.size,
        hashesMapeados: estadoUploadQueue.hashARutas.size,
        hashesBloqueadosAntispam: Array.from(estadoUploadQueue.contadorHashDetectado.values()).filter(c => c >= MAX_DETECCIONES_HASH).length,
        muestrasActivas,
    };
}

/*
 * Reintenta subir un item que falló.
 */
export async function reintentarItem(itemId: string): Promise<void> {
    const item = cola.find(i => i.id === itemId);
    if (item && item.estado === 'error') {
        item.estado = 'pendiente';
        item.intentos = 0;
        item.ultimoError = undefined;
        item.timestampActualizado = Date.now();
        estadoUploadQueue.rutasEnCola.add(claveRutaEnCola(item.rutaArchivo));
        await guardarCola(); /* Guardar inmediato: acción explícita del usuario */

        if (estaOnline()) {
            procesarCola();
        }
    }
}

/*
 * Reintenta todos los items que estén en estado "error".
 * Útil para cuando el usuario presiona "Sincronizar ahora".
 *
 * Arquitectura multi-ventana Tauri: el sync panel (sync.html) y la ventana
 * principal (index.html) son procesos JS separados con instancias de módulo
 * distintas. El sync panel tiene su cola vacía; la real vive en la ventana
 * principal. Por eso también emitimos el evento Tauri `reintentar-errores-upload`
 * para que el listener en inicializarUploadQueue() lo procese en la ventana correcta.
 */
export async function reintentarTodosConError(): Promise<void> {
    let algunActualizado = false;
    for (const item of cola) {
        if (item.estado === 'error') {
            item.estado = 'pendiente';
            item.intentos = 0;
            item.ultimoError = undefined;
            item.timestampActualizado = Date.now();
            estadoUploadQueue.rutasEnCola.add(claveRutaEnCola(item.rutaArchivo));
            algunActualizado = true;
        }
    }

    if (algunActualizado) {
        await guardarCola();
        if (estaOnline()) {
            procesarCola();
        }
    }

    /* Notificar a TODAS las ventanas Tauri (especialmente la principal
     * que tiene la cola real) para que también procesen el reintento. */
    try {
        const { emit } = await import('@tauri-apps/api/event');
        await emit('reintentar-errores-upload', {});
    } catch {
        /* Entorno sin Tauri — ignorar */
    }
}

/*
 * Elimina un item de la cola (cancelar upload).
 */
export async function eliminarItemCola(itemId: string): Promise<void> {
    const item = cola.find(i => i.id === itemId);
    if (item) {
        estadoUploadQueue.rutasEnCola.delete(claveRutaEnCola(item.rutaArchivo));
    }
    setCola(cola.filter(i => i.id !== itemId));
    await guardarCola(); /* Guardar inmediato: acción explícita del usuario */
}

/*
 * Limpia items completados de la cola.
 */
export async function limpiarCompletados(): Promise<void> {
    /* Limpiar Set de rutas de items completados */
    for (const item of cola) {
        if (item.estado === 'completado') {
            estadoUploadQueue.rutasEnCola.delete(claveRutaEnCola(item.rutaArchivo));
        }
    }
    setCola(cola.filter(i => i.estado !== 'completado'));
    await guardarCola();
}

/*
 * Limpia todos los hashes conocidos. Necesario para force-resync:
 * sin esto, archivos previamente subidos se rechazan como duplicados
 * incluso después de resetear el tracking (ej: borrado del servidor + re-subida).
 */
export async function limpiarHashesConocidos(): Promise<void> {
    estadoUploadQueue.hashesConocidos.clear();
    estadoUploadQueue.hashARutas.clear();
    await guardarHashes();
}

/*
 * QL78: Limpieza retroactiva de archivos ya subidos exitosamente.
 * Cuando borrarAlSubirExitoso está activo, itera hashARutas (hash→rutas)
 * y elimina archivos que aún existan en disco. Cubre:
 * - Archivos subidos ANTES de que el usuario activara la opción.
 * - Archivos cuyo borrado falló en el primer intento (lock, permisos).
 * Se invoca al activar la opción y al iniciar la app si ya está activa.
 */
export async function limpiarArchivosSubidosEnDisco(): Promise<void> {
    if (!estado.configAvanzada.borrarAlSubirExitoso) return;

    try {
        const { exists, remove } = await import('@tauri-apps/plugin-fs');
        let eliminados = 0;
        let fallidos = 0;

        for (const [, rutas] of estadoUploadQueue.hashARutas) {
            for (const ruta of rutas) {
                try {
                    if (!tieneSubidaPersistidaConfirmada({ rutaArchivo: ruta })) continue;
                    const existe = await exists(ruta);
                    if (!existe) continue;

                    await remove(ruta);
                    eliminados++;
                    logSync.info('uploadQueue', `QL78: Archivo eliminado retroactivamente: ${ruta.split('/').pop()}`);
                } catch (err) {
                    fallidos++;
                    logSync.warn('uploadQueue', `QL78: No se pudo eliminar archivo: ${ruta.split('/').pop()}`, {
                        error: err instanceof Error ? err.message : String(err),
                    });
                }
            }
        }

        /* También limpiar archivos de items completados en cola */
        for (const item of cola) {
            if (item.estado !== 'completado') continue;
            try {
                if (!tieneSubidaPersistidaConfirmada(item)) continue;
                const existe = await exists(item.rutaArchivo);
                if (!existe) continue;
                await remove(item.rutaArchivo);
                eliminados++;
                logSync.info('uploadQueue', `QL78: Archivo de cola completada eliminado: ${item.nombreArchivo}`);
            } catch {
                fallidos++;
            }
        }

        if (eliminados > 0 || fallidos > 0) {
            logSync.info('uploadQueue', `QL78: Limpieza retroactiva finalizada`, { eliminados, fallidos });
        }
    } catch (err) {
        logSync.error('uploadQueue', 'QL78: Error en limpieza retroactiva', {
            error: err instanceof Error ? err.message : String(err),
        });
    }
}
