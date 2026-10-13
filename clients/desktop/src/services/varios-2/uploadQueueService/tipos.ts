/* Tipos públicos de la cola de subida (estados, items, progreso y resumen de debug). */

/* Estado de cada item en la cola */
export type EstadoUpload = 'pendiente' | 'subiendo' | 'completado' | 'error' | 'duplicado';

export interface ItemUploadCola {
    id: string;
    rutaArchivo: string;
    nombreArchivo: string;
    carpetas: string[];
    estado: EstadoUpload;
    intentos: number;
    ultimoError?: string;
    timestampCreado: number;
    timestampActualizado: number;
    sampleIdServidor?: number;
    hashParcial?: string;
    /** Clave de idempotencia para evitar duplicados si el upload se reintenta tras timeout */
    idempotencyKey: string;
}

export interface ProgresoUpload {
    item: ItemUploadCola;
    totalEnCola: number;
    posicionEnCola: number;
}

export interface ResumenDebugUploadQueue {
    totalItems: number;
    totalPendientes: number;
    totalSubiendo: number;
    totalErrores: number;
    totalCompletados: number;
    totalDuplicados: number;
    procesando: boolean;
    rutasEnCola: number;
    rutasEnVuelo: number;
    hashesConocidos: number;
    hashesEnVuelo: number;
    hashesPendientesEncola: number;
    hashesMapeados: number;
    hashesBloqueadosAntispam: number;
    muestrasActivas: Array<{
        id: string;
        nombreArchivo: string;
        estado: EstadoUpload;
        intentos: number;
        rutaArchivo: string;
        sampleIdServidor?: number;
        ultimoError?: string;
        timestampActualizado: number;
    }>;
}

export type OnProgresoUploadFn = (progreso: ProgresoUpload) => void;
