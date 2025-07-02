import { isDebug } from "./debug";

export const API_URL = process.env.NEXT_PUBLIC_SWORD_API_URL ?? '/api';

interface ApiResponse<T> {
    success: boolean;
    message: string;
    data: T;
}

/**
 * Helper para realizar peticiones a la API de Sword v2.
 * Se encarga de añadir el encabezado Authorization cuando existe un token en localStorage.
 */
export async function apiFetch<T>(path: string, options: RequestInit & {skipAuth?: boolean} = {}): Promise<ApiResponse<T>> {
    const url = `${API_URL}${path}`;

    const headers: HeadersInit = {
        'Content-Type': 'application/json',
        ...(options.headers ?? {})
    };

    if (!options.skipAuth) {
        // Intentamos recuperar el token del localStorage en entorno de navegador.
        if (typeof window !== 'undefined') {
            const token = localStorage.getItem('token');
            if (token) {
                (headers as Record<string, string>)['Authorization'] = `Bearer ${token}`;
            }
        }
    }

    const response = await fetch(url, {
        ...options,
        headers
    });

    if (isDebug) {
        console.groupCollapsed(`apiFetch → ${options.method ?? "GET"} ${url}`);
        console.log("Headers", headers);
        if (options.body) console.log("Body", options.body);
    }

    // Leemos el cuerpo como texto para poder manejar tanto JSON como texto plano o vacío.
    const rawText = await response.text();

    let parsed: any = null;
    if (rawText.trim() !== "") {
        try {
            parsed = JSON.parse(rawText);
        } catch {
            // No es JSON válido; lo dejamos como texto.
            parsed = rawText;
        }
    }

    if (!response.ok) {
        if (isDebug) {
            console.log("Status", response.status, response.statusText);
            console.log("Response", parsed);
            console.groupEnd();
        }
        // Si el cuerpo traía un objeto con mensaje, lo usamos. Si es texto, usamos el texto, si no, usamos statusText.
        const errorMessage = typeof parsed === "object" && parsed?.message
            ? parsed.message
            : (typeof parsed === "string" && parsed) || response.statusText || "Error en la petición";
        throw new Error(errorMessage);
    }

    // Si la respuesta no trae JSON (p.ej. 204) devolvemos un objeto ApiResponse vacío por conveniencia.
    if (parsed === null || typeof parsed !== "object") {
        if (isDebug) {
            console.log("Status", response.status, response.statusText);
            console.log("Response (texto)", parsed);
            console.groupEnd();
        }
        return {
            success: true,
            message: response.statusText || "",
            data: parsed as unknown as T
        } as ApiResponse<T>;
    }

    if (isDebug) {
        console.log("Status", response.status, response.statusText);
        console.log("Response", parsed);
        console.groupEnd();
    }

    return parsed as ApiResponse<T>;
}
