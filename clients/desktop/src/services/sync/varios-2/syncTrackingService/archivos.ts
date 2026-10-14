import { generarClaveTracking } from './claves';
import { datos, estadoSyncTracking } from './estado';
import { registrarEnJournal } from './persistencia/journal';
import type { ArchivoTracking } from './tipos';

export function obtenerArchivo(sampleId: number, coleccionId: number | null): ArchivoTracking | null {
    const clave = generarClaveTracking(sampleId, coleccionId);
    return datos.archivos[clave] ?? null;
}

export function buscarArchivoPorSampleId(sampleId: number): ArchivoTracking | null {
    /* Buscar en cualquier colección */
    for (const archivo of Object.values(datos.archivos)) {
        if (archivo.sampleId === sampleId) return archivo;
    }
    return null;
}

/** O(1) lookup por ruta normalizada usando índice secundario. */
export function buscarArchivoPorRuta(ruta: string): ArchivoTracking | null {
    const rutaNorm = ruta.replace(/\\/g, '/');
    const clave = estadoSyncTracking.indiceRuta.get(rutaNorm);
    if (clave) return datos.archivos[clave] ?? null;
    return null;
}

/** O(1) lookup por nombre de archivo usando índice secundario. */
export function buscarArchivoPorNombre(nombre: string): ArchivoTracking | null {
    const claves = estadoSyncTracking.indiceNombre.get(nombre);
    if (claves && claves.length > 0) return datos.archivos[claves[0]] ?? null;
    return null;
}

export async function registrarArchivo(archivo: ArchivoTracking): Promise<void> {
    const clave = generarClaveTracking(archivo.sampleId, archivo.coleccionId);

    /* Limpiar índice de entrada anterior si existía con otra ruta/nombre */
    const anterior = datos.archivos[clave];
    if (anterior) {
        estadoSyncTracking.indiceRuta.delete(anterior.rutaLocal.replace(/\\/g, '/'));
    }

    datos.archivos[clave] = archivo;

    /* Actualizar índices secundarios */
    const rutaNorm = archivo.rutaLocal.replace(/\\/g, '/');
    estadoSyncTracking.indiceRuta.set(rutaNorm, clave);
    for (const nombre of [archivo.nombreServidor, archivo.nombreLocal]) {
        const existentes = estadoSyncTracking.indiceNombre.get(nombre) ?? [];
        if (!existentes.includes(clave)) existentes.push(clave);
        estadoSyncTracking.indiceNombre.set(nombre, existentes);
    }

    await registrarEnJournal('TRACK_FILE', { clave, archivo });
}

export async function eliminarArchivo(sampleId: number, coleccionId: number | null): Promise<void> {
    const clave = generarClaveTracking(sampleId, coleccionId);
    const archivo = datos.archivos[clave];

    if (archivo) {
        /* Limpiar índices secundarios */
        estadoSyncTracking.indiceRuta.delete(archivo.rutaLocal.replace(/\\/g, '/'));
        for (const nombre of [archivo.nombreServidor, archivo.nombreLocal]) {
            const existentes = estadoSyncTracking.indiceNombre.get(nombre);
            if (existentes) {
                const filtrado = existentes.filter(c => c !== clave);
                if (filtrado.length === 0) estadoSyncTracking.indiceNombre.delete(nombre);
                else estadoSyncTracking.indiceNombre.set(nombre, filtrado);
            }
        }
    }

    delete datos.archivos[clave];
    await registrarEnJournal('UNTRACK_FILE', { clave });
}

export async function marcarSyncDeshabilitado(sampleId: number, coleccionId: number | null): Promise<boolean> {
    const clave = generarClaveTracking(sampleId, coleccionId);
    const archivo = datos.archivos[clave];
    if (!archivo) return false;

    archivo.syncDeshabilitado = true;
    await registrarEnJournal('MARK_DISABLED', { clave });
    return true;
}

export async function reactivarSync(sampleId: number): Promise<boolean> {
    /* Buscar y eliminar en todas las colecciones para que se re-descargue */
    let encontrado = false;
    for (const [clave, archivo] of Object.entries(datos.archivos)) {
        if (archivo.sampleId === sampleId) {
            delete datos.archivos[clave];
            encontrado = true;
        }
    }
    if (encontrado) await registrarEnJournal('MARK_ENABLED', { sampleId });
    return encontrado;
}

/*
 * Reactiva todos los samples marcados como syncDeshabilitado.
 * Elimina sus entradas del tracking para forzar re-descarga en la próxima sync.
 * Usado por "Reforzar sincronización" para recuperar archivos borrados localmente.
 */
export async function reactivarTodosSyncDeshabilitados(): Promise<number> {
    let reactivados = 0;
    for (const [clave, archivo] of Object.entries(datos.archivos)) {
        if (archivo.syncDeshabilitado) {
            delete datos.archivos[clave];
            reactivados++;
        }
    }
    if (reactivados > 0) {
        await registrarEnJournal('MARK_ENABLED_ALL', { reactivados });
    }
    return reactivados;
}

export function listarArchivosPorColeccion(coleccionId: number | null): ArchivoTracking[] {
    return Object.values(datos.archivos).filter(a => a.coleccionId === coleccionId);
}

export function todosLosArchivos(): ArchivoTracking[] {
    return Object.values(datos.archivos);
}

export function totalArchivos(): number {
    return Object.keys(datos.archivos).length;
}

export function espacioTotalBytes(): number {
    return Object.values(datos.archivos).reduce((sum, a) => sum + a.tamano, 0);
}
