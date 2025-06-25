export const secureFetch = async <T>(url: string, options?: RequestInit, setCargando?: (cargando: boolean) => void): 
    Promise<{ data: T | null; error: string | null }> => {
    try {
        setCargando?.(true);
        const response = await fetch(url, options);
        if (!response.ok) {
            throw new Error('Ha ocurrido un error al obtener la respuesta del servidor. ' + response.statusText);
        }
        const data = await response.json();
        return { data: data as T, error: null };
    } catch (error) {
        let message = 'An unknown error occurred';
        if (error instanceof Error) {
            message = error.message;
        } else if (typeof error === 'string') {
            message = error;
        }
        return { error: message, data: null }
    } finally {
        setCargando?.(false);
    }    
};