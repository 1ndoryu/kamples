/* Comparadores puros para decidir si el refresco de historial cambia algo visible */

export function historialCambioVisible(
    anterior: Array<{ timestamp: number }> ,
    siguiente: Array<{ timestamp: number }>,
): boolean {
    if (anterior.length !== siguiente.length) return true;
    if (anterior.length === 0) return false;
    return anterior[0].timestamp !== siguiente[0].timestamp;
}

export function historialSamplesCambioVisible(
    anterior: Array<{ timestampActualizado: number; estado: string; imagenUrl?: string | null }>,
    siguiente: Array<{ timestampActualizado: number; estado: string; imagenUrl?: string | null }>,
): boolean {
    if (anterior.length !== siguiente.length) return true;
    if (anterior.length === 0) return false;
    /*
     * Comparar todas las entradas: O(n) con n=50 máx, negligible cada 1.2s.
     * Antes solo comparábamos la primera entrada, lo cual ignoraba cambios
     * en posiciones intermedias (ej: imagen actualizada de un sample que no es el más reciente).
     */
    for (let i = 0; i < anterior.length; i++) {
        const a = anterior[i];
        const b = siguiente[i];
        if (a.timestampActualizado !== b.timestampActualizado
            || a.estado !== b.estado
            || a.imagenUrl !== b.imagenUrl) {
            return true;
        }
    }
    return false;
}
