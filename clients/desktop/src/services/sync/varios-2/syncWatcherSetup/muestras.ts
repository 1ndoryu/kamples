/*
 * Estado de sync por sample: marcar, reactivar y consultar (tracking v2 + índice legacy).
 */
import { logSync } from '../../varios-1/syncLogger';
import { estado, guardarIndice, buscarEnIndicePorRuta } from '../syncState';

/* Operaciones de estado de sync por sample */

/*
 * Marca un sample como "no sincronizar" por ruta local.
 * Se llama cuando el usuario elimina un archivo de la carpeta sync.
 * El sample NO se borra del servidor — solo deja de sincronizarse.
 */
export async function marcarNoSincronizar(ruta: string): Promise<boolean> {
    const { trackingModule, indiceArchivos } = estado;

    /* v2: buscar en tracking por ruta y deshabilitar */
    if (trackingModule) {
        const archivoV2 = trackingModule.buscarArchivoPorRuta(ruta);
        if (archivoV2) {
            await trackingModule.marcarSyncDeshabilitado(archivoV2.sampleId, archivoV2.coleccionId);
            await trackingModule.registrarAccion({
                tipo: 'eliminado_local',
                descripcion: `Eliminado localmente: ${archivoV2.nombreLocal}`,
                sampleId: archivoV2.sampleId,
                coleccionId: archivoV2.coleccionId ?? undefined,
            });
        }
    }

    const rutaNormalizada = ruta.replace(/\\/g, '/');
    const archivo = buscarEnIndicePorRuta(rutaNormalizada);

    if (!archivo) {
        if (trackingModule?.buscarArchivoPorRuta(ruta)) return true;
        logSync.warn('syncWatcherSetup', '[Sync] No se encontró archivo en índice para marcar no_sincronizar:', ruta);
        return false;
    }

    archivo.syncDeshabilitado = true;
    archivo.rutaEliminada = archivo.ruta;
    guardarIndice();

    logSync.info('syncWatcherSetup', '[Sync] Sample marcado como no_sincronizar:', [archivo.nombre, '(sampleId:', archivo.sampleId, ')']);
    return true;
}

/*
 * Marca un sample como "no sincronizar" por su ID de sample.
 * Versión para usar desde la UI del explorador.
 */
export async function marcarNoSincronizarPorId(sampleId: number): Promise<boolean> {
    const { trackingModule, indiceArchivos } = estado;

    if (trackingModule) {
        const archivoV2 = trackingModule.buscarArchivoPorSampleId(sampleId);
        if (archivoV2) {
            await trackingModule.marcarSyncDeshabilitado(sampleId, archivoV2.coleccionId);
        }
    }

    const archivo = indiceArchivos.find(a => a.sampleId === sampleId);
    if (!archivo) {
        if (trackingModule?.buscarArchivoPorSampleId(sampleId)) return true;
        return false;
    }

    archivo.syncDeshabilitado = true;
    archivo.rutaEliminada = archivo.ruta;
    guardarIndice();

    logSync.info('syncWatcherSetup', '[Sync] Sample marcado como no_sincronizar por ID:', sampleId);
    return true;
}

/*
 * Reactiva la sincronización de un sample.
 * El archivo se re-descargará en la próxima sincronización.
 */
export async function reactivarSync(sampleId: number): Promise<boolean> {
    const { trackingModule } = estado;
    let encontradoV2 = false;

    if (trackingModule) {
        encontradoV2 = await trackingModule.reactivarSync(sampleId);
    }

    const archivo = estado.indiceArchivos.find(a => a.sampleId === sampleId);
    if (!archivo && !encontradoV2) return false;

    estado.indiceArchivos = estado.indiceArchivos.filter(a => a.sampleId !== sampleId);
    guardarIndice();

    logSync.info('syncWatcherSetup', '[Sync] Sync reactivada para sample:', [sampleId, '— se descargará en próxima sync']);
    return true;
}

/*
 * Obtiene el estado de sincronización de un sample.
 * Retorna: 'sincronizado' | 'no_sincronizar' | 'no_descargado'
 */
export function obtenerEstadoSync(sampleId: number): 'sincronizado' | 'no_sincronizar' | 'no_descargado' {
    const { trackingModule, indiceArchivos } = estado;

    if (trackingModule) {
        const archivoV2 = trackingModule.buscarArchivoPorSampleId(sampleId);
        if (archivoV2) {
            return archivoV2.syncDeshabilitado ? 'no_sincronizar' : 'sincronizado';
        }
    }

    const archivo = indiceArchivos.find(a => a.sampleId === sampleId);
    if (!archivo) return 'no_descargado';
    if (archivo.syncDeshabilitado) return 'no_sincronizar';
    return 'sincronizado';
}

/*
 * Obtiene todos los samples con sync deshabilitado.
 * Para mostrar en la UI del explorador.
 */
export function obtenerSamplesNoSincronizados(): Array<{ sampleId: number; nombre: string }> {
    const { trackingModule, indiceArchivos } = estado;

    if (trackingModule) {
        const todos = trackingModule.todosLosArchivos();
        const noSyncV2 = todos
            .filter(a => a.syncDeshabilitado)
            .map(a => ({ sampleId: a.sampleId, nombre: a.nombreLocal }));

        const idsV2 = new Set(noSyncV2.map(a => a.sampleId));
        const noSyncV1 = indiceArchivos
            .filter(a => a.syncDeshabilitado && !idsV2.has(a.sampleId))
            .map(a => ({ sampleId: a.sampleId, nombre: a.nombre }));

        return [...noSyncV2, ...noSyncV1];
    }

    return indiceArchivos
        .filter(a => a.syncDeshabilitado)
        .map(a => ({ sampleId: a.sampleId, nombre: a.nombre }));
}
