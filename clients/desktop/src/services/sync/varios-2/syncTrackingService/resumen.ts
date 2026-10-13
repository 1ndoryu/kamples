import { datos, versionLocalConocida } from './estado';
import type { BaseSyncLocal, ResumenDebugTracking } from './tipos';

/* Vistas de solo lectura del estado de tracking (diagnóstico y snapshot para UI). */

export function obtenerResumenDebugTracking(): ResumenDebugTracking {
    const archivos = Object.values(datos.archivos);
    const colecciones = Object.values(datos.colecciones);
    const deshabilitados = archivos
        .filter(archivo => archivo.syncDeshabilitado)
        .slice(0, 15)
        .map(archivo => ({
            sampleId: archivo.sampleId,
            nombreLocal: archivo.nombreLocal,
            coleccionId: archivo.coleccionId,
            rutaLocal: archivo.rutaLocal,
        }));

    return {
        totalArchivos: archivos.length,
        totalColecciones: colecciones.length,
        totalSinColeccion: datos.sinColeccion.length,
        totalDeshabilitados: archivos.filter(archivo => archivo.syncDeshabilitado).length,
        totalHistorial: datos.historial.length,
        totalHistorialSamples: datos.historialSamples.length,
        espacioTotalBytes: archivos.reduce((total, archivo) => total + archivo.tamano, 0),
        checkpointVersion: datos.checkpointVersion ?? 0,
        versionLocalConocida,
        userId: datos.userId ?? null,
        muestrasColecciones: colecciones.slice(0, 15).map(coleccion => ({
            id: coleccion.id,
            nombre: coleccion.nombre,
            carpetaLocal: coleccion.carpetaLocal,
            parentId: coleccion.parentId,
        })),
        muestrasDeshabilitados: deshabilitados,
    };
}

/* Obtener snapshot inmutable para UI */
export function obtenerDatosSync(): Readonly<BaseSyncLocal> {
    return datos;
}
