/*
 * Caché de AudioBuffers por id (samples de la biblioteca y archivos locales).
 */

import { decodificarAudio } from '../../utils/audioBufferUtils';
import { limpiarCachePitch, limpiarCacheInvertidos } from '../pitchShiftService';
import { obtenerContexto } from './contextoAudio';
import { estado } from './estadoMotor';

/* Cargar y cachear un AudioBuffer desde URL */
export async function cargarBuffer(url: string, id: string): Promise<AudioBuffer> {
    const cachedo = estado.cacheBuffers.get(id);
    if (cachedo) return cachedo;

    const ctx = obtenerContexto();
    const respuesta = await fetch(url);
    if (!respuesta.ok) throw new Error(`Error cargando audio: ${respuesta.status}`);
    const arrayBuffer = await respuesta.arrayBuffer();
    const audioBuffer = await decodificarAudio(ctx, arrayBuffer);

    estado.cacheBuffers.set(id, audioBuffer);
    return audioBuffer;
}

/* Obtener buffer cacheado */
export function obtenerBuffer(id: string): AudioBuffer | null {
    return estado.cacheBuffers.get(id) ?? null;
}

/*
 * C208: Decodificar un ArrayBuffer local (desde archivo subido).
 * Cachea con el id proporcionado.
 */
export async function decodificarBufferLocal(arrayBuffer: ArrayBuffer, id: string): Promise<AudioBuffer> {
    const ctx = obtenerContexto();
    const audioBuffer = await decodificarAudio(ctx, arrayBuffer);
    estado.cacheBuffers.set(id, audioBuffer);
    return audioBuffer;
}

/* Limpiar caché de buffers (liberar memoria) */
export function limpiarCache(): void {
    estado.cacheBuffers.clear();
    /* C271: Limpiar cache de buffers SoundTouch procesados */
    limpiarCachePitch();
    /* Limpiar cache de buffers invertidos */
    limpiarCacheInvertidos();
}
