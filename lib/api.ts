import { isDebug } from "./debug";

export const API_URL = process.env.NEXT_PUBLIC_SWORD_API_URL ?? '/api';

interface ApiResponse<T> {
    success: boolean;
    message: string;
    data: T;
}

export interface ApiError {
    message: string;
    status?: number;
    data?: any;
}

const responseCache = new Map<string, any>();

/**
 * Helper mejorado para realizar peticiones a la API.
 * Nunca lanza excepciones por errores HTTP, en su lugar devuelve una tupla [data, error].
 *
 * @returns Una promesa que resuelve a una tupla: [ApiResponse<T> | null, ApiError | null]
 * - En caso de éxito: [data, null]
 * - En caso de error: [null, error]
 */
export async function apiFetch<T>(path: string, options: RequestInit & { skipAuth?: boolean } = {}): Promise<[ApiResponse<T> | null, ApiError | null]> {
    const url = `${API_URL}${path}`;

    const headers: HeadersInit = {
        'Content-Type': 'application/json',
        ...(options.headers ?? {})
    };

    if (!options.skipAuth && typeof window !== 'undefined') {
        const token = localStorage.getItem('token');
        if (token) {
            (headers as Record<string, string>)['Authorization'] = `Bearer ${token}`;
        }
    }

    const method = (options.method ?? "GET").toUpperCase();
    const shouldUseCache = method === "GET" && !options.body;
    const cacheKey = url;

    if (shouldUseCache && responseCache.has(cacheKey)) {
        const cached = responseCache.get(cacheKey) as ApiResponse<T>;
        if (isDebug) {
            console.groupCollapsed(`apiFetch (cache) → ${method} ${url}`);
            console.log("Response", cached);
            console.groupEnd();
        }
        return [cached, null];
    }

    // Es bueno envolver la lógica de este fetch en un try/catch para capturar errores de red o problemas irrecuperables.
    // Así wando, podemos dejar más limpia la parte del store.
    try {
        const response = await fetch(url, { ...options, headers });

        if (isDebug) {
            console.groupCollapsed(`apiFetch → ${method} ${url}`);
            console.log("Headers", headers);
            if (options.body) console.log("Body", JSON.parse(options.body as string));
        }

        const rawText = await response.text();
        let parsed: any = null;
        if (rawText.trim() !== "") {
            try {
                parsed = JSON.parse(rawText);
            } catch {
                parsed = rawText;
            }
        }

        if (!response.ok) {
            if (isDebug) {
                console.log(`[Status] ${response.status} ${response.statusText}`);
                console.log(`[Response] ${parsed}`);
                console.groupEnd();
            }
            
            const errorMessage = typeof parsed === "object" && parsed?.message
                ? parsed.message
                : (typeof parsed === "string" && parsed.trim() !== "") || response.statusText || "Error en la petición";

            const error: ApiError = {
                message: errorMessage,
                status: response.status,
                data: parsed,
            };
            return [null, error];
        }

        if (isDebug) {
            console.log(`[Status] ${response.status} ${response.statusText}`);
            console.log(`[Response] ${parsed}`);
            console.groupEnd();
        }
        
        // Si la respuesta no es un objeto JSON (p. ej. un 204 No Content),
        // construimos una ApiResponse compatible.
        const finalResponse = (parsed !== null && typeof parsed === 'object'
            ? parsed
            : { success: true, message: response.statusText, data: parsed as unknown as T }
        ) as ApiResponse<T>;

        if (shouldUseCache) {
            responseCache.set(cacheKey, finalResponse);
        }

        return [finalResponse, null];

    } catch (error: any) {
        if (isDebug) {
            console.groupCollapsed(`apiFetch (FATAL) → ${method} ${url}`);
            console.error("Error no controlado en fetch:", error);
            console.groupEnd();
        }
        
        // Aquí capturamos errores de red o de servidor.
        // y se devuelve en el formato estándar de error.
        // También usando la tupla [null, error].
        const networkError: ApiError = {
            message: error.message || "Error de red o conexión rechazada.",
        };
        return [null, networkError];
    }
}