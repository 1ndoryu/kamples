/*
 * SyncReport — Informe estructurado de verificación de sync.
 * Diseñado para que el agente (o el usuario) pueda leer el estado completo
 * del sistema de sincronización en una sola llamada.
 *
 * Expuesto en window.__KAMPLES_SYNC_REPORT__ desde sync.tsx y main.tsx.
 * Uso desde consola: await window.__KAMPLES_SYNC_REPORT__()
 */

export interface SyncReport {
    /** Timestamp de generación */
    generadoEn: number;
    /** Entorno de ejecución */
    entorno: {
        desktop: boolean;
        version: string;
        ventana: string;
    };
    /** Estado de autenticación */
    auth: {
        logueado: boolean;
        userId: number | null;
        tokenValido: boolean | null;
    };
    /** Configuración de sync */
    config: {
        carpetaSeleccionada: boolean;
        carpetaLocal: string | null;
        sincronizacionActiva: boolean;
        ultimaSync: number;
        ultimoCursorDelta: number;
        intervaloPollingMs: number;
    };
    /** Estado del backend remoto */
    backend: {
        conectado: boolean | null;
        deltaCursor: number;
        fullSyncRequired: boolean | null;
    };
    /** Estado del tracking local */
    tracking: {
        archivos: number;
        colecciones: number;
        sinColeccion: number;
        deshabilitados: number;
        historialSamples: number;
        espacioTotalBytes: number;
    };
    /** Estado de la cola de subidas */
    uploadQueue: {
        totalItems: number;
        pendientes: number;
        subiendo: number;
        errores: number;
        completados: number;
    };
    /** Estado del journal */
    journal: {
        activo: boolean;
        operacionesPendientes: number;
    };
    /** Circuit breaker */
    circuitBreaker: {
        estado: string;
        fallos: number;
    };
    /** Lista de diagnósticos detectados automáticamente */
    diagnosticos: Array<{
        nivel: 'ok' | 'warn' | 'error' | 'info';
        componente: string;
        mensaje: string;
    }>;
}
