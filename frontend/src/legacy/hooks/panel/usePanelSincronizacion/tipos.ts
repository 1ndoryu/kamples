/* Tipos compartidos del panel de sincronización (extraídos de usePanelSincronizacion) */

/* Progreso reportado por sincronizarConServidor() en cada archivo */
export interface ProgresoSync {
    actual: number;
    total: number;
    sampleId: number;
    nombre: string;
    estado: 'descargando' | 'descargado' | 'error';
    tamano?: number;
    ruta?: string;
}

/* Tipo del objeto expuesto por desktop/main.tsx en window.__KAMPLES_SYNC__ */
export interface KamplesSync {
    elegirCarpetaSync: () => Promise<string | null>;
    toggleSincronizacion: (activa: boolean) => Promise<void>;
    obtenerConfigSync: () => { carpetaLocal: string | null; sincronizacionActiva: boolean; ultimaSync: number };
    abrirCarpetaSync?: () => Promise<boolean>;
    sincronizarConServidor: (onProgreso?: (p: ProgresoSync) => void, opciones?: { forzar?: boolean }) => Promise<{ nuevos: number; eliminados: number }>;
    /* C358 */
    obtenerHistorialSync?: (limite?: number) => Array<{ tipo: string; descripcion: string; sampleId?: number; coleccionId?: number; timestamp: number }>;
    obtenerHistorialSamplesSync?: (limite?: number) => Array<{
        sampleId: number;
        nombreArchivo: string;
        estado: 'detectado' | 'subiendo' | 'sincronizado' | 'error' | 'moviendo' | 'descargando' | 'descargado';
        imagenUrl: string | null;
        rutaLocal: string | null;
        coleccionNombre?: string;
        timestampCreado: number;
        timestampActualizado: number;
        error?: string;
    }>;
    obtenerColeccionesSync?: () => Array<{ id: number; nombre: string; carpetaLocal: string; archivos: number }>;
    forzarResync?: (onProgreso?: (p: ProgresoSync) => void) => Promise<{ nuevos: number; eliminados: number }>;
    reforzarSync?: (onProgreso?: (p: ProgresoSync) => void) => Promise<{ nuevos: number; eliminados: number }>;
    haySyncEnCurso?: () => boolean;
    limpiarHistorialSync?: () => Promise<void>;
    recargarHistorialDesdeStore?: () => Promise<void>;
    rehidratarImagenesPendientesSync?: () => Promise<void>;
}

/* Tipo del objeto expuesto en window.__KAMPLES_UPLOAD__ para cola de subidas */
export interface KamplesUpload {
    obtenerEstadoCola: () => {
        items: Array<{ id: string; nombreArchivo: string; estado: string; ultimoError?: string }>;
        totalPendientes: number;
        totalErrores: number;
        procesando: boolean;
    };
    onProgresoUpload: (cb: (progreso: { item: { nombreArchivo: string; estado: string }; totalEnCola: number; posicionEnCola: number }) => void) => void;
}
