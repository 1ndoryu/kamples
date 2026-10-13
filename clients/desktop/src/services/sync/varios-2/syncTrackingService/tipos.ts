/* Tipos del tracking de sync (sin lógica ni estado). */

export interface ArchivoTracking {
    sampleId: number;
    coleccionId: number | null;     /* null = "Sin colección" */
    rutaLocal: string;              /* ruta absoluta en disco */
    nombreLocal: string;            /* nombre actual del archivo (puede diferir del server) */
    nombreServidor: string;         /* nombre original del server */
    descargadoEn: number;
    tamano: number;
    syncDeshabilitado: boolean;     /* true = borrado localmente, no re-descargar */
}

export interface ColeccionLocal {
    id: number;
    nombre: string;                 /* nombre de la colección en server */
    carpetaLocal: string;           /* nombre de la carpeta en disco (sanitizado) */
    creadaLocalmente: boolean;      /* true si fue creada como carpeta local primero */
    parentId: number | null;        /* null = raíz, number = subcolección (max 2 niveles) */
    version: number;                /* F5.2: optimistic locking — se incrementa en cada update del servidor */
}

export type TipoAccionHistorial =
    | 'descarga' | 'subida' | 'movido' | 'renombrado' | 'creado' | 'eliminado_local'
    | 'subida_pendiente' | 'subiendo' | 'error_subida';

export interface AccionHistorial {
    tipo: TipoAccionHistorial;
    descripcion: string;
    sampleId?: number;
    coleccionId?: number;
    timestamp: number;
}

/*
 * Modelo v2 del historial: 1 entrada por sample con estado mutable.
 * Reemplaza el log append-only (AccionHistorial[]) con un modelo
 * donde cada sample tiene una sola fila cuyo estado evoluciona:
 * detectado → subiendo → sincronizado (o error).
 *
 * Acciones de sistema (migración, resync) no aparecen en el historial
 * visible — solo samples reales con sampleId.
 */
export type EstadoSampleHistorial =
    | 'detectado'
    | 'subiendo'
    | 'sincronizado'
    | 'error'
    | 'moviendo'
    | 'descargando'
    | 'descargado';

export interface EntradaHistorialSample {
    sampleId: number;
    nombreArchivo: string;
    estado: EstadoSampleHistorial;
    imagenUrl: string | null;
    rutaLocal: string | null;
    coleccionNombre?: string;
    timestampCreado: number;
    timestampActualizado: number;
    error?: string;
}

export interface BaseSyncLocal {
    archivos: Record<string, ArchivoTracking>;
    colecciones: Record<number, ColeccionLocal>;
    sinColeccion: number[];         /* IDs de samples descargados sin colección */
    historial: AccionHistorial[];
    /*
     * Historial v2: por sample con estado mutable (upsert por sampleId).
     * Indexado como mapa para O(1) lookup, serializado como array para persistencia.
     */
    historialSamples: EntradaHistorialSample[];
    /* TC1: Versión monotónica para detección de escrituras concurrentes cross-window.
     * Se incrementa en cada escribirEnStore(). Si al leer del Store la versión es mayor
     * que la local, significa que otra ventana escribió — merger obligatorio antes de write. */
    checkpointVersion?: number;
    /*
     * C286: ID del usuario dueño de estos datos de tracking.
     * Al inicializar, si el userId almacenado no coincide con el usuario actual,
     * se limpia todo el tracking (los datos pertenecen a otra cuenta).
     * Sin este campo, colecciones de sesiones anteriores con otro usuario
     * contaminan el tracking → 403 en cascada al intentar operar sobre ellas.
     */
    userId?: number;
}

export interface ResumenDebugTracking {
    totalArchivos: number;
    totalColecciones: number;
    totalSinColeccion: number;
    totalDeshabilitados: number;
    totalHistorial: number;
    totalHistorialSamples: number;
    espacioTotalBytes: number;
    checkpointVersion: number;
    versionLocalConocida: number;
    userId: number | null;
    muestrasColecciones: Array<{
        id: number;
        nombre: string;
        carpetaLocal: string;
        parentId: number | null;
    }>;
    muestrasDeshabilitados: Array<{
        sampleId: number;
        nombreLocal: string;
        coleccionId: number | null;
        rutaLocal: string;
    }>;
}
