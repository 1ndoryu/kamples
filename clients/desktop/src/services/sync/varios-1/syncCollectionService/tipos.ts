/* Tipos del endpoint GET /me/sync/colecciones y del progreso de sincronización.
 * Los tipos de fs/path se derivan de los módulos reales: las fases reciben esas
 * funciones por parámetro y sincronizar.ts hace el import dinámico. */

/* Tipos del endpoint */

export interface SampleSync {
    id: number;
    titulo: string;
    formato: string;
    tamano: number;
    imagenUrl?: string | null;
    imagen_url?: string | null;
}

export interface ColeccionSync {
    id: number;
    nombre: string;
    parent_id: number | null;
    version: number;
    samples: SampleSync[];
}

export interface RespuestaSyncColecciones {
    colecciones: ColeccionSync[];
    sinColeccion: SampleSync[];
}

/* Progreso */

export interface ProgresoSyncColecciones {
    fase: 'estructura' | 'descarga' | 'completado';
    actual: number;
    total: number;
    sampleId?: number;
    nombre?: string;
    estado?: 'descargando' | 'descargado' | 'error' | 'omitido';
}

export type CallbackProgresoColecciones = (progreso: ProgresoSyncColecciones) => void;

/* Tarea plana de descarga para el batch paralelo de sincronizarColecciones */
export interface TareaDescarga {
    sample: SampleSync;
    coleccionId: number | null;
    carpetaDestino: string;
    esSinColeccion: boolean;
}

/* Dependencias de fs/path inyectadas en las fases del sync completo */

export type FsColecciones = Pick<typeof import('@tauri-apps/plugin-fs'), 'mkdir' | 'exists' | 'rename'>;
export type JoinColecciones = Pick<typeof import('@tauri-apps/api/path'), 'join'>['join'];
