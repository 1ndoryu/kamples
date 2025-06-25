// app/services/swordApi.ts

import type { RespuestaApiSamples, Sample } from '@/types/sample';

export interface Comentario {
    id: number;
    texto: string;
    autor: string;
}

// Se mantiene el manejador de errores personalizado
class ApiError extends Error {
    constructor(message: string) {
        super(message);
        this.name = 'ApiError';
    }
}

// Función base refactorizada con timeout y mejor manejo de errores.
async function fetchApi<T>(endpoint: string, options: RequestInit = {}, timeout = 8000): Promise<T> {
    const controller = new AbortController();
    const idTimeout = setTimeout(() => controller.abort(), timeout);

    const url = `${process.env.NEXT_PUBLIC_SWORD_API_URL}${endpoint}`;
    const apiKey = process.env.SWORD_API_KEY;

    const headers: Record<string, string> = {
        'Content-Type': 'application/json',
        ...(options.headers as Record<string, string> | undefined)
    };

    if (apiKey) {
        headers['Authorization'] = `Bearer ${apiKey}`;
    }

    try {
        const respuesta = await fetch(url, {
            ...options,
            headers,
            signal: controller.signal
        });
        
        clearTimeout(idTimeout);

        if (!respuesta.ok) {
            const errorBody = await respuesta.json().catch(() => null);
            const errorMessage = errorBody?.error?.message || `Error en la petición: ${respuesta.status} ${respuesta.statusText}`;
            throw new ApiError(errorMessage);
        }

        if (respuesta.status === 204) {
            return null as T;
        }

        return respuesta.json();
    } catch (error: unknown) { // CORRECCIÓN: 'any' cambiado a 'unknown'
        clearTimeout(idTimeout);
        if (error instanceof Error && error.name === 'AbortError') {
            const mensajeError = `La petición a la API (${url}) superó el tiempo de espera de ${timeout / 1000}s.`;
            console.error(mensajeError);
            throw new ApiError(mensajeError);
        }
        
        // Si el error ya es conocido (ApiError), lo relanzamos. Si no, creamos uno nuevo.
        if (error instanceof ApiError) {
            throw error;
        }
        
        console.error(`Error crítico en fetchApi (${url}):`, error);
        throw new ApiError('Ocurrió un error de conexión con la API.');
    }
}

/**
 * Obtiene una lista paginada de samples.
 * Se ejecuta en el servidor.
 */
export async function obtenerSamples(): Promise<Sample[]> {
    const endpoint = '/content?type=sample&per_page=20';

    try {
        const respuesta = await fetchApi<RespuestaApiSamples>(endpoint);
        return respuesta.data.items;
    } catch (error) {
        console.error("Error al obtener la lista de samples:", error instanceof Error ? error.message : String(error));
        return [];
    }
}

/**
 * Obtiene un único sample por su slug.
 * Se ejecuta en el servidor.
 * @param slug El slug del sample a buscar.
 * @returns Una promesa que resuelve al sample encontrado o null si no existe.
 */
export async function obtenerSamplePorSlug(slug: string): Promise<Sample | null> {
    const endpoint = `/content?type=sample&slug=${slug}`;

    try {
        const respuesta = await fetchApi<RespuestaApiSamples>(endpoint);
        if (respuesta.data.items && respuesta.data.items.length > 0) {
            return respuesta.data.items[0];
        }
        return null;
    } catch (error) {
        console.error(`Error al obtener el sample con slug "${slug}":`, error instanceof Error ? error.message : String(error));
        return null;
    }
}

/**
 * Obtiene los comentarios de un sample.
 * @param sampleId El ID del sample para el que se quieren obtener los comentarios.
 * @returns Una promesa que resuelve a los comentarios encontrados o un array vacío si no hay comentarios.
 * Esta función esta siendo creada para su uso futuro cuando se implementen los comentarios.
*/
export async function obtenerComentarios(sampleId: number): Promise<Comentario[]> {
    const endpoint = `/content?type=comentarios&sampleId=${sampleId}`;
    try {
        return await fetchApi<Comentario[]>(endpoint);
    } catch (error) {
        console.error(`Error al obtener comentarios para el sample ${sampleId}:`, error);
        return [];
    }
}

/**
 * Crea un nuevo comentario para un sample.
 * @param sampleId El ID del sample al que se quiere agregar el comentario.
 * @param texto El texto del comentario.
 * @returns Una promesa que resuelve al comentario creado.
 * Esta función esta siendo creada para su uso futuro cuando se implementen los comentarios.
*/
export async function crearComentario(sampleId: number, texto: string): Promise<Comentario> {
    const endpoint = `/content?type=comentarios`;
    try {
        return await fetchApi<Comentario>(endpoint, {
            method: 'POST',
            body: JSON.stringify({ sampleId, texto, autor: 'Usuario Actual' }),
        });
    } catch (error) {
        console.error(`Error al crear comentario para el sample ${sampleId}:`, error);
        throw error;
    }
}
