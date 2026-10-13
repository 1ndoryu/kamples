import {
    datos,
    indiceNombre,
    indiceNombreSampleHistorial,
    indiceRuta,
    indiceSampleHistorial,
    sinColeccionSet,
} from './estado';

/* Reconstruye índices secundarios desde datos.archivos y sinColeccion. Llamado al cargar o migrar. */
export function reconstruirIndices(): void {
    indiceRuta.clear();
    indiceNombre.clear();
    sinColeccionSet.clear();

    for (const [clave, archivo] of Object.entries(datos.archivos)) {
        const rutaNorm = archivo.rutaLocal.replace(/\\/g, '/');
        indiceRuta.set(rutaNorm, clave);

        for (const nombre of [archivo.nombreServidor, archivo.nombreLocal]) {
            const existentes = indiceNombre.get(nombre) ?? [];
            if (!existentes.includes(clave)) existentes.push(clave);
            indiceNombre.set(nombre, existentes);
        }
    }

    for (const id of datos.sinColeccion) {
        sinColeccionSet.add(id);
    }
}

/* Reconstruye índice de historialSamples para O(1) upsert por sampleId */
export function reconstruirIndiceSampleHistorial(): void {
    indiceSampleHistorial.clear();
    indiceNombreSampleHistorial.clear();
    for (let i = 0; i < datos.historialSamples.length; i++) {
        const entrada = datos.historialSamples[i];
        if (entrada.sampleId > 0) {
            indiceSampleHistorial.set(entrada.sampleId, i);
        }
        indiceNombreSampleHistorial.set(entrada.nombreArchivo.toLowerCase(), i);
    }
}
