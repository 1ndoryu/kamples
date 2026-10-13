/*
 * Utilidades puras y de consulta de tipos de evento del watcher (fileWatcherService).
 */

/*
 * QL68: Heuristica para detectar si un nombre tiene extension de archivo.
 * Si tiene un punto seguido de 1-10 caracteres alfanumericos al final → es archivo, no carpeta.
 * Esto cubre extensiones inventadas/desconocidas no presentes en EXTENSIONES_ARCHIVO_CONOCIDAS.
 * Carpetas con puntos en el nombre (ej: "v2.0") siguen pasando porque el check
 * de EXTENSIONES_AUDIO y EXTENSIONES_ARCHIVO_CONOCIDAS se hace primero.
 */
export function pareceArchivoConExtension(nombre: string): boolean {
    const ultimoPunto = nombre.lastIndexOf('.');
    if (ultimoPunto <= 0 || ultimoPunto === nombre.length - 1) return false;
    const sufijo = nombre.slice(ultimoPunto + 1);
    return sufijo.length <= 10 && /^[a-zA-Z0-9]+$/.test(sufijo);
}

export async function esDirectorioReal(ruta: string): Promise<boolean> {
    try {
        const { stat } = await import('@tauri-apps/plugin-fs');
        const info = await stat(ruta);
        return Boolean((info as { isDirectory?: boolean }).isDirectory);
    } catch {
        return false;
    }
}

/*
 * Determina si el tipo de evento es una creación de archivo.
 */
export function esEventoCreacion(tipo: unknown): boolean {
    if (tipo === 'any') return false;
    if (typeof tipo === 'object' && tipo !== null && 'create' in tipo) return true;
    /* Modify puede significar que el archivo termino de escribirse */
    if (typeof tipo === 'object' && tipo !== null && 'modify' in tipo) {
        const modify = (tipo as { modify: { kind: string } }).modify;
        /* Solo data changes, no metadata */
        return modify?.kind === 'data' || modify?.kind === 'any';
    }
    return false;
}

export function esEventoRename(tipo: unknown): boolean {
    if (typeof tipo === 'object' && tipo !== null && 'modify' in tipo) {
        const modify = (tipo as { modify: { kind: string } }).modify;
        return modify?.kind === 'name';
    }
    return false;
}

/*
 * Determina si el tipo de evento es una eliminación de archivo.
 */
export function esEventoEliminacion(tipo: unknown): boolean {
    if (typeof tipo === 'object' && tipo !== null && 'remove' in tipo) return true;
    return false;
}
