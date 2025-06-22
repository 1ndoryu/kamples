// Regla 8: Se incluye el archivo completo para facilitar la copia.
import type { RespuestaApiSamples, Sample } from '@/types/sample';

// Creamos un manejador de errores personalizado para la API
class ApiError extends Error {
    constructor(message: string) {
        super(message);
        this.name = 'ApiError';
    }
}

// Función base para realizar peticiones a la API. Es reutilizable.
async function fetchApi<T>(endpoint: string, options: RequestInit = {}): Promise<T> {
    const url = `${process.env.SWORD_API_URL}${endpoint}`;
    const apiKey = process.env.SWORD_API_KEY;

    const headers: Record<string, string> = {
        'Content-Type': 'application/json',
        ...(options.headers as Record<string, string> | undefined)
    };

    // Si la petición no es pública, añadimos el token de autenticación.
    if (apiKey) {
        headers['Authorization'] = `Bearer ${apiKey}`;
    }

    try {
        const respuesta = await fetch(url, { ...options, headers });

        if (!respuesta.ok) {
            throw new ApiError(`Error en la petición: ${respuesta.status} ${respuesta.statusText}`);
        }

        if (respuesta.status === 204) {
            return null as T;
        }

        return respuesta.json();
    } catch (error) {
        console.error('Error en fetchApi:', error);
        throw error;
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
        console.error("Error al obtener la lista de samples:", error);
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
    // Usamos el endpoint que te recomendé añadir a tu API.
    const endpoint = `/content?type=sample&slug=${slug}`;

    try {
        const respuesta = await fetchApi<RespuestaApiSamples>(endpoint);
        // La API devuelve un array, tomamos el primer (y único) elemento.
        if (respuesta.data.items && respuesta.data.items.length > 0) {
            return respuesta.data.items[0];
        }
        return null; // No se encontró el sample
    } catch (error) {
        console.error(`Error al obtener el sample con slug "${slug}":`, error);
        return null;
    }
}