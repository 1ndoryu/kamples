// app/services/swordApi.ts

import type { RespuestaApiSamples, Sample } from '@/types/sample';

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

    const apiUrlBase = process.env.NEXT_PUBLIC_SWORD_API_URL;
    // Asegurarse de que la URL base y el endpoint se unan correctamente
    const url = (apiUrlBase?.endsWith('/') ? apiUrlBase.slice(0, -1) : apiUrlBase) +
                (endpoint?.startsWith('/') ? endpoint : `/${endpoint}`);

    const apiKey = process.env.NEXT_PUBLIC_SWORD_API_KEY; // CORRECCIÓN: Usar NEXT_PUBLIC_ para acceso en cliente

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