import {
    datos,
    estadoSyncTracking,
} from './estado';

/* Reconstruye índices secundarios desde datos.archivos y sinColeccion. Llamado al cargar o migrar. */
export function reconstruirIndices(): void {
    estadoSyncTracking.indiceRuta.clear();
    estadoSyncTracking.indiceNombre.clear();
    estadoSyncTracking.sinColeccionSet.clear();

    for (const [clave, archivo] of Object.entries(datos.archivos)) {
        const rutaNorm = archivo.rutaLocal.replace(/\\/g, '/');
        estadoSyncTracking.indiceRuta.set(rutaNorm, clave);

        for (const nombre of [archivo.nombreServidor, archivo.nombreLocal]) {
            const existentes = estadoSyncTracking.indiceNombre.get(nombre) ?? [];
            if (!existentes.includes(clave)) existentes.push(clave);
            estadoSyncTracking.indiceNombre.set(nombre, existentes);
        }
    }

    for (const id of datos.sinColeccion) {
        estadoSyncTracking.sinColeccionSet.add(id);
    }
}

/* Reconstruye índice de historialSamples para O(1) upsert por sampleId */
export function reconstruirIndiceSampleHistorial(): void {
    estadoSyncTracking.indiceSampleHistorial.clear();
    estadoSyncTracking.indiceNombreSampleHistorial.clear();
    for (let i = 0; i < datos.historialSamples.length; i++) {
        const entrada = datos.historialSamples[i];
        if (entrada.sampleId > 0) {
            estadoSyncTracking.indiceSampleHistorial.set(entrada.sampleId, i);
        }
        estadoSyncTracking.indiceNombreSampleHistorial.set(entrada.nombreArchivo.toLowerCase(), i);
    }
}
