import type { EntradaHistorialSample } from './tipos';

/* Clave única: "{sampleId}_{coleccionId}" donde coleccionId=0 significa "sin colección" */
export function generarClaveTracking(sampleId: number, coleccionId: number | null): string {
    return `${sampleId}_${coleccionId ?? 0}`;
}

export function obtenerClaveHistorialSample(entrada: Pick<EntradaHistorialSample, 'sampleId' | 'nombreArchivo'>): string {
    if (entrada.sampleId > 0) return `sample:${entrada.sampleId}`;
    return `nombre:${entrada.nombreArchivo.toLowerCase()}`;
}
