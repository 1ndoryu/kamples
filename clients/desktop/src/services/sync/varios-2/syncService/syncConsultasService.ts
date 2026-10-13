import { estado } from '../syncState';

/* Consultas */

/*
 * Verifica si un sample ya está descargado localmente.
 * Retorna la ruta local si existe, null si no.
 */
export function obtenerRutaLocal(sampleId: number): string | null {
    const { trackingModule, indiceArchivos } = estado;

    if (trackingModule) {
        const archivo = trackingModule.buscarArchivoPorSampleId(sampleId);
        if (archivo && !archivo.syncDeshabilitado) return archivo.rutaLocal;
    }

    const archivo = indiceArchivos.find(a => a.sampleId === sampleId);
    return archivo?.ruta ?? null;
}

/* Utilidades */

/*
 * Extrae metadata de la ruta del archivo para auto-descripción.
 */
export function extraerMetadataDeRuta(rutaCompleta: string): {
    carpetas: string[];
    nombreArchivo: string;
    extension: string;
} {
    const partes = rutaCompleta.replace(/\\/g, '/').split('/').filter(Boolean);
    const archivoConExt = partes.pop() ?? '';
    const dotIndex = archivoConExt.lastIndexOf('.');

    const nombreArchivo = dotIndex > 0 ? archivoConExt.slice(0, dotIndex) : archivoConExt;
    const extension = dotIndex > 0 ? archivoConExt.slice(dotIndex + 1) : '';
    const carpetas = partes.slice(-3);

    return { carpetas, nombreArchivo, extension };
}

/* Historial y colecciones */

export function obtenerHistorialSync(limite = 50): Array<{
    tipo: string;
    descripcion: string;
    sampleId?: number;
    coleccionId?: number;
    timestamp: number;
}> {
    if (!estado.trackingModule) return [];
    return estado.trackingModule.obtenerHistorial(limite);
}

/**
 * Historial per-sample v2: una entrada por sample con estado evolutivo.
 * Usado por VentanaSincPanel para mostrar historial con imagen y click-to-navigate.
 */
export function obtenerHistorialSamplesSync(limite = 50): Array<{
    sampleId: number;
    nombreArchivo: string;
    estado: 'detectado' | 'subiendo' | 'sincronizado' | 'error' | 'moviendo' | 'descargando' | 'descargado';
    imagenUrl: string | null;
    rutaLocal: string | null;
    coleccionNombre?: string;
    timestampCreado: number;
    timestampActualizado: number;
    error?: string;
}> {
    if (!estado.trackingModule?.obtenerHistorialSamples) return [];
    return estado.trackingModule.obtenerHistorialSamples(limite);
}

export async function limpiarHistorialSync(): Promise<void> {
    if (!estado.trackingModule) return;
    await estado.trackingModule.limpiarHistorial();
    /* Limpiar también historial per-sample v2 */
    if (estado.trackingModule.limpiarHistorialSamples) {
        await estado.trackingModule.limpiarHistorialSamples();
    }
}

/**
 * Re-lee el historial per-sample desde el Tauri Store compartido.
 * Necesario en ventanas MPA (sync panel) para ver actualizaciones de la ventana main
 * (ej: imagen de portada obtenida post-pipeline). Throttle interno de 5s.
 */
export async function recargarHistorialDesdeStore(): Promise<void> {
    if (!estado.trackingModule?.recargarHistorialDesdeStore) return;
    await estado.trackingModule.recargarHistorialDesdeStore();
}

export function obtenerColeccionesSync(): Array<{
    id: number;
    nombre: string;
    carpetaLocal: string;
    archivos: number;
}> {
    const { trackingModule } = estado;
    if (!trackingModule) return [];

    const colecciones = trackingModule.todasLasColecciones();
    const resultado = colecciones.map(col => ({
        id: col.id,
        nombre: col.nombre,
        carpetaLocal: col.carpetaLocal,
        archivos: trackingModule.listarArchivosPorColeccion(col.id).length,
    }));

    const totalSinCol = trackingModule.totalSinColeccion();
    if (totalSinCol > 0) {
        resultado.push({
            id: 0,
            nombre: 'Sin colección',
            carpetaLocal: 'Sin colección',
            archivos: totalSinCol,
        });
    }

    return resultado;
}
