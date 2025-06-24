export const useFetch = async (url: string, options?: RequestInit, setCargando?: (cargando: boolean) => void): 
    Promise<{ data: any; error: string | null }> => {
    try {
        setCargando?.(true);
        const response = await fetch(url, options);
        if (!response.ok) {
            throw new Error('Ha ocurrido un error al obtener la respuesta del servidor. ' + response.statusText);
        }
        const data = await response.json();
        return { data: data, error: null };
    } catch (error: any) {
        return { error: error.message, data: null }
    } finally {
        setCargando?.(false);
    }    
};
