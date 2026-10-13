/* sentinel-disable-file: Fachada pública de sync — centraliza imports/re-exports y ahora
 * el reporte de verificación. Por diseño concentra interfaces, no lógica de dominio. */

/*
 * Servicio: syncService — Fachada pública del sistema de sincronización.
 *
 * TA6: Refactorizado como fachada slim. La lógica se distribuyó en:
 * - syncInitService.ts: inicialización, config, migración v1→v2
 * - syncOrchestratorService.ts: sync principal, individual, resync
 * - syncRegistroService.ts: registro de descargas/subidas, movimiento de archivos
 * - syncRehidratacionService.ts: rehidratación de imágenes de portada
 *
 * Todos los importadores externos siguen apuntando a este archivo (fachada).
 * Los módulos internos adicionales se mantienen:
 * - syncState.ts: estado compartido + persistencia
 * - syncDownloadV1.ts: lógica legacy v1
 * - syncWatcherSetup.ts: watcher bidireccional + operaciones locales
 * - syncTrackingService.ts: persistencia tipada v2
 * - syncCollectionService.ts: mapeo colecciones ↔ carpetas
 * - syncGuards.ts: guards de descarga + base URL centralizada
 */

import { esDesktop } from '../../varios-1/desktopService';
import { esSyncEnCurso } from '../varios-1/syncGuards';
import { logSync } from '../varios-1/syncLogger';
import {
    estado,
    guardarConfig,
    type SyncConfig,
} from './syncState';
import {
    detenerSyncBidireccional as detenerBidireccional,
    marcarNoSincronizar as _marcarNoSincronizar,
    marcarNoSincronizarPorId as _marcarNoSincronizarPorId,
    reactivarSync as _reactivarSync,
    obtenerEstadoSync as _obtenerEstadoSync,
    obtenerSamplesNoSincronizados as _obtenerSamplesNoSincronizados,
    moverSampleEnServidorPublico as _moverSampleEnServidorPublico,
} from './syncWatcherSetup';

/* Re-exports de tipos para mantener API pública sin romper importadores */
export { type ProgresoSync, type ProgressCallback, type SyncConfig } from './syncState';

/* Re-exports de módulos extraídos */
export { inicializarSyncService } from '../varios-1/syncInitService';
export {
    sincronizarConServidor,
    sincronizarSampleIndividual,
    forzarResync,
    reforzarSync,
} from '../varios-1/syncOrchestratorService';
export {
    registrarDescarga,
    registrarAccionHistorial,
    registrarSubidaLocal,
    moverArchivoASinColeccion,
    actualizarEstadoSampleHistorial,
} from './syncRegistroService';
export {
    rehidratarImagenesPendientesSync,
    rehidratarImagenesPendientesForzadoSync,
} from './syncRehidratacionService';

/* Re-exports de operaciones del watcher */
export const marcarNoSincronizar = _marcarNoSincronizar;
export const marcarNoSincronizarPorId = _marcarNoSincronizarPorId;
export const reactivarSync = _reactivarSync;
export const obtenerEstadoSync = _obtenerEstadoSync;
export const obtenerSamplesNoSincronizados = _obtenerSamplesNoSincronizados;
export const moverSampleEnServidorPublico = _moverSampleEnServidorPublico;
export const detenerSyncBidireccional = detenerBidireccional;

/* Configuración */

export async function elegirCarpetaSync(): Promise<string | null> {
    if (!esDesktop()) return null;

    try {
        const { open } = await import('@tauri-apps/plugin-dialog');
        const carpeta = await open({
            directory: true,
            multiple: false,
            title: 'Elegir carpeta de sincronización',
        });

        if (carpeta && typeof carpeta === 'string') {
            estado.config.carpetaLocal = carpeta;
            await guardarConfig();
            return carpeta;
        }
    } catch (err) {
        logSync.error('syncService', 'Error eligiendo carpeta', { error: err instanceof Error ? err.message : String(err) });
    }

    return null;
}

export async function toggleSincronizacion(activa: boolean): Promise<void> {
    estado.config.sincronizacionActiva = activa;
    await guardarConfig();
}

export function obtenerConfigSync(): SyncConfig {
    return { ...estado.config };
}

export function haySyncEnCurso(): boolean {
    return esSyncEnCurso();
}

export async function abrirCarpetaSync(): Promise<boolean> {
    if (!esDesktop()) return false;
    if (!estado.config.carpetaLocal) return false;

    try {
        const { invoke } = await import('@tauri-apps/api/core');
        await invoke('abrir_carpeta', { ruta: estado.config.carpetaLocal });
        return true;
    } catch (err) {
        logSync.error('syncService', 'Error abriendo carpeta local', { error: err instanceof Error ? err.message : String(err) });
        return false;
    }
}

/* Consultas */

/*
 * Verifica si un sample ya está descargado localmente.
 * Retorna la ruta local si existe, null si no.
 */
export function obtenerRutaLocal(sampleId: number): string | null {
    const { trackingModule, indiceArchivos } = estado;

    if (trackingModule) {
        const archivo = trackingModule.buscarArchivoPorSampleId(sampleId);
        if (archivo && !archivo.syncDeshabilitado) return archivo.rutaLocal;
    }

    const archivo = indiceArchivos.find(a => a.sampleId === sampleId);
    return archivo?.ruta ?? null;
}

/* Utilidades */

/*
 * Extrae metadata de la ruta del archivo para auto-descripción.
 */
export function extraerMetadataDeRuta(rutaCompleta: string): {
    carpetas: string[];
    nombreArchivo: string;
    extension: string;
} {
    const partes = rutaCompleta.replace(/\\/g, '/').split('/').filter(Boolean);
    const archivoConExt = partes.pop() ?? '';
    const dotIndex = archivoConExt.lastIndexOf('.');

    const nombreArchivo = dotIndex > 0 ? archivoConExt.slice(0, dotIndex) : archivoConExt;
    const extension = dotIndex > 0 ? archivoConExt.slice(dotIndex + 1) : '';
    const carpetas = partes.slice(-3);

    return { carpetas, nombreArchivo, extension };
}

/* Historial y colecciones */

export function obtenerHistorialSync(limite = 50): Array<{
    tipo: string;
    descripcion: string;
    sampleId?: number;
    coleccionId?: number;
    timestamp: number;
}> {
    if (!estado.trackingModule) return [];
    return estado.trackingModule.obtenerHistorial(limite);
}

/**
 * Historial per-sample v2: una entrada por sample con estado evolutivo.
 * Usado por VentanaSincPanel para mostrar historial con imagen y click-to-navigate.
 */
export function obtenerHistorialSamplesSync(limite = 50): Array<{
    sampleId: number;
    nombreArchivo: string;
    estado: 'detectado' | 'subiendo' | 'sincronizado' | 'error' | 'moviendo' | 'descargando' | 'descargado';
    imagenUrl: string | null;
    rutaLocal: string | null;
    coleccionNombre?: string;
    timestampCreado: number;
    timestampActualizado: number;
    error?: string;
}> {
    if (!estado.trackingModule?.obtenerHistorialSamples) return [];
    return estado.trackingModule.obtenerHistorialSamples(limite);
}

export async function limpiarHistorialSync(): Promise<void> {
    if (!estado.trackingModule) return;
    await estado.trackingModule.limpiarHistorial();
    /* Limpiar también historial per-sample v2 */
    if (estado.trackingModule.limpiarHistorialSamples) {
        await estado.trackingModule.limpiarHistorialSamples();
    }
}

/**
 * Re-lee el historial per-sample desde el Tauri Store compartido.
 * Necesario en ventanas MPA (sync panel) para ver actualizaciones de la ventana main
 * (ej: imagen de portada obtenida post-pipeline). Throttle interno de 5s.
 */
export async function recargarHistorialDesdeStore(): Promise<void> {
    if (!estado.trackingModule?.recargarHistorialDesdeStore) return;
    await estado.trackingModule.recargarHistorialDesdeStore();
}

export function obtenerColeccionesSync(): Array<{
    id: number;
    nombre: string;
    carpetaLocal: string;
    archivos: number;
}> {
    const { trackingModule } = estado;
    if (!trackingModule) return [];

    const colecciones = trackingModule.todasLasColecciones();
    const resultado = colecciones.map(col => ({
        id: col.id,
        nombre: col.nombre,
        carpetaLocal: col.carpetaLocal,
        archivos: trackingModule.listarArchivosPorColeccion(col.id).length,
    }));

    const totalSinCol = trackingModule.totalSinColeccion();
    if (totalSinCol > 0) {
        resultado.push({
            id: 0,
            nombre: 'Sin colección',
            carpetaLocal: 'Sin colección',
            archivos: totalSinCol,
        });
    }

    return resultado;
}

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

/**
 * Genera un reporte estructurado de verificación del sistema de sync.
 * Recolecta estado de todos los subsistemas y genera diagnósticos automáticos.
 * Diseñado para ser legible por el agente vía playwright o consola.
 */
export async function generarReporteSync(ventana = 'desconocida'): Promise<SyncReport> {
    const diagnosticos: SyncReport['diagnosticos'] = [];
    const ahora = Date.now();

    /* ── Entorno ── */
    const reporte: SyncReport = {
        generadoEn: ahora,
        entorno: {
            desktop: !!window.__KAMPLES_DESKTOP__,
            version: window.__KAMPLES_VERSION__ ?? 'desconocida',
            ventana,
        },
        auth: {
            logueado: false,
            userId: null,
            tokenValido: null,
        },
        config: {
            carpetaSeleccionada: false,
            carpetaLocal: null,
            sincronizacionActiva: false,
            ultimaSync: 0,
            ultimoCursorDelta: 0,
            intervaloPollingMs: 0,
        },
        backend: {
            conectado: null,
            deltaCursor: 0,
            fullSyncRequired: null,
        },
        tracking: {
            archivos: 0,
            colecciones: 0,
            sinColeccion: 0,
            deshabilitados: 0,
            historialSamples: 0,
            espacioTotalBytes: 0,
        },
        uploadQueue: {
            totalItems: 0,
            pendientes: 0,
            subiendo: 0,
            errores: 0,
            completados: 0,
        },
        journal: {
            activo: false,
            operacionesPendientes: 0,
        },
        circuitBreaker: {
            estado: 'desconocido',
            fallos: 0,
        },
        diagnosticos: [],
    };

    /* ── Auth ── */
    try {
        const ctx = window.GLORY_CONTEXT as Record<string, unknown> | undefined;
        if (ctx?.userId) {
            reporte.auth.userId = Number(ctx.userId);
            reporte.auth.logueado = true;
            reporte.auth.tokenValido = ctx?.tokenValido === true;
        } else {
            /* Intentar leer del store de auth directamente */
            try {
                const { load } = await import('@tauri-apps/plugin-store');
                const store = await load('auth.json');
                const token = await store.get<string>('auth_token');
                const usuario = await store.get<Record<string, unknown>>('auth_usuario');
                if (token && usuario) {
                    reporte.auth.logueado = true;
                    reporte.auth.userId = (usuario.id as number) ?? null;
                    reporte.auth.tokenValido = true;
                }
            } catch {
                /* Store no disponible — probablemente no es Tauri */
            }
        }
    } catch {
        /* GLORY_CONTEXT no definido */
    }

    if (!reporte.auth.logueado) {
        diagnosticos.push({
            nivel: 'warn',
            componente: 'auth',
            mensaje: 'Usuario no autenticado. El sync requiere inicio de sesión.',
        });
    } else {
        diagnosticos.push({
            nivel: 'ok',
            componente: 'auth',
            mensaje: `Usuario autenticado (ID: ${reporte.auth.userId})`,
        });
    }

    /* ── Config ── */
    reporte.config.carpetaLocal = estado.config.carpetaLocal;
    reporte.config.carpetaSeleccionada = !!estado.config.carpetaLocal;
    reporte.config.sincronizacionActiva = estado.config.sincronizacionActiva;
    reporte.config.ultimaSync = estado.config.ultimaSync;
    reporte.config.ultimoCursorDelta = estado.ultimoCursorDelta;
    reporte.config.intervaloPollingMs = estado.intervaloPollingMs;

    if (!reporte.config.carpetaSeleccionada) {
        diagnosticos.push({
            nivel: 'warn',
            componente: 'config',
            mensaje: 'No hay carpeta de sync seleccionada. Usa "Elegir carpeta" para configurar.',
        });
    } else if (!reporte.config.sincronizacionActiva) {
        diagnosticos.push({
            nivel: 'warn',
            componente: 'config',
            mensaje: `Sync desactivado para carpeta: ${reporte.config.carpetaLocal}`,
        });
    } else {
        diagnosticos.push({
            nivel: 'ok',
            componente: 'config',
            mensaje: `Sync activo en: ${reporte.config.carpetaLocal}`,
        });
    }

    if (reporte.config.ultimaSync > 0) {
        const diffMin = Math.floor((ahora - reporte.config.ultimaSync) / 60_000);
        diagnosticos.push({
            nivel: diffMin < 60 ? 'ok' : 'warn',
            componente: 'config',
            mensaje: `Última sync: hace ${diffMin} min`,
        });
    }

    /* ── Circuit Breaker ── */
    try {
        const { circuitoSync } = await import('../varios-1/syncGuards');
        const estadoCircuito = circuitoSync.obtenerEstado();
        const fallos = circuitoSync.obtenerFallosConsecutivos();
        reporte.circuitBreaker.estado = estadoCircuito;
        reporte.circuitBreaker.fallos = fallos;

        if (estadoCircuito === 'abierto') {
            diagnosticos.push({
                nivel: 'error',
                componente: 'circuitBreaker',
                mensaje: `Circuit breaker ABIERTO (${fallos} fallos consecutivos). Sync bloqueada.`,
            });
        } else {
            diagnosticos.push({
                nivel: 'ok',
                componente: 'circuitBreaker',
                mensaje: `Circuit breaker ${estadoCircuito} (${fallos} fallos)`,
            });
        }
    } catch {
        /* Circuito no inicializado */
    }

    /* ── Tracking ── */
    try {
        const { obtenerResumenDebugTracking } = await import('./syncTrackingService');
        const resumen = obtenerResumenDebugTracking();
        reporte.tracking.archivos = resumen.totalArchivos;
        reporte.tracking.colecciones = resumen.totalColecciones;
        reporte.tracking.sinColeccion = resumen.totalSinColeccion;
        reporte.tracking.deshabilitados = resumen.totalDeshabilitados;
        reporte.tracking.historialSamples = resumen.totalHistorialSamples;
        reporte.tracking.espacioTotalBytes = resumen.espacioTotalBytes;

        if (resumen.totalArchivos > 0) {
            diagnosticos.push({
                nivel: 'ok',
                componente: 'tracking',
                mensaje: `${resumen.totalArchivos} archivos en ${resumen.totalColecciones} colecciones (${(resumen.espacioTotalBytes / 1024 / 1024).toFixed(1)} MB)`,
            });
        } else {
            diagnosticos.push({
                nivel: 'info',
                componente: 'tracking',
                mensaje: 'Tracking local vacío — aún no hay archivos sincronizados',
            });
        }

        /* Verificar si hay historial de muestras con error */
        const { obtenerHistorialSamples } = await import('./syncTrackingService');
        const samplesConError = obtenerHistorialSamples(200)
            .filter(s => s.estado === 'error');
        if (samplesConError.length > 0) {
            diagnosticos.push({
                nivel: 'warn',
                componente: 'tracking',
                mensaje: `${samplesConError.length} sample(s) con error en el historial. Primer error: ${samplesConError[0].error ?? 'desconocido'}`,
            });
        }
    } catch {
        /* Tracking no inicializado */
    }

    /* ── Upload Queue ── */
    try {
        const { obtenerResumenDebugUploadQueue } = await import('../../varios-2/uploadQueueService');
        const resumen = obtenerResumenDebugUploadQueue();
        reporte.uploadQueue.totalItems = resumen.totalItems;
        reporte.uploadQueue.pendientes = resumen.totalPendientes;
        reporte.uploadQueue.subiendo = resumen.totalSubiendo;
        reporte.uploadQueue.errores = resumen.totalErrores;
        reporte.uploadQueue.completados = resumen.totalCompletados;

        if (resumen.totalErrores > 0) {
            diagnosticos.push({
                nivel: 'warn',
                componente: 'uploadQueue',
                mensaje: `${resumen.totalErrores} item(s) con error en cola de subida`,
            });
        }
    } catch {
        /* Upload queue no inicializada */
    }

    /* ── Journal ── */
    try {
        const journalModule = await import('../varios-1/syncJournal');
        reporte.journal.activo = journalModule.estaInicializado();
        reporte.journal.operacionesPendientes = journalModule.operacionesPendientesCount();
    } catch {
        /* Journal no inicializado */
    }

    /* ── Backend (delta cursor) ── */
    reporte.backend.deltaCursor = estado.ultimoCursorDelta;
    if (estado.ultimoCursorDelta <= 0) {
        reporte.backend.fullSyncRequired = true;
        diagnosticos.push({
            nivel: 'info',
            componente: 'backend',
            mensaje: 'Cursor delta en 0 — se requiere full sync inicial',
        });
    } else {
        diagnosticos.push({
            nivel: 'ok',
            componente: 'backend',
            mensaje: `Cursor delta: ${estado.ultimoCursorDelta}`,
        });
    }

    /* Intentar verificar conectividad con backend */
    try {
        const { obtenerBaseUrlSync } = await import('../varios-1/syncGuards');
        const baseUrl = obtenerBaseUrlSync();
        if (baseUrl) {
            const controller = new AbortController();
            const timeout = setTimeout(() => controller.abort(), 5000);
            const resp = await fetch(`${baseUrl}/kamples/v1/me/sync/delta?cursor=0`, {
                signal: controller.signal,
                headers: { 'Accept': 'application/json' },
            });
            clearTimeout(timeout);
            reporte.backend.conectado = resp.ok;
            if (resp.ok) {
                diagnosticos.push({
                    nivel: 'ok',
                    componente: 'backend',
                    mensaje: `Backend responde (HTTP ${resp.status})`,
                });
            } else {
                diagnosticos.push({
                    nivel: 'error',
                    componente: 'backend',
                    mensaje: `Backend responde con HTTP ${resp.status}`,
                });
            }
        }
    } catch {
        reporte.backend.conectado = false;
        diagnosticos.push({
            nivel: 'error',
            componente: 'backend',
            mensaje: 'No se puede conectar con el backend',
        });
    }

    /* ── Resumen general ── */
    const errores = diagnosticos.filter(d => d.nivel === 'error').length;
    const warnings = diagnosticos.filter(d => d.nivel === 'warn').length;
    const ok = diagnosticos.filter(d => d.nivel === 'ok').length;

    diagnosticos.unshift({
        nivel: errores > 0 ? 'error' : warnings > 0 ? 'warn' : 'ok',
        componente: 'resumen',
        mensaje: `${ok} ok, ${warnings} advertencias, ${errores} errores`,
    });

    reporte.diagnosticos = diagnosticos;

    /* Persistir a disco para que el agente pueda leerlo luego con read_file */
    try {
        const { writeTextFile, BaseDirectory } = await import('@tauri-apps/plugin-fs');
        const contenido = JSON.stringify(reporte, null, 2);
        const timestamp = new Date(ahora).toISOString().replace(/[:.]/g, '-');
        /* Archivo único con timestamp para trazabilidad histórica */
        await writeTextFile(`sync-report-${timestamp}.json`, contenido, {
            baseDir: BaseDirectory.AppData,
        });
        /* Archivo "latest" siempre sobrescrito — fácil de leer desde el agente */
        await writeTextFile('sync-report-latest.json', contenido, {
            baseDir: BaseDirectory.AppData,
        });
    } catch {
        /* FS no disponible (no Tauri, permisos, etc.) — el reporte se retorna igual */
    }

    return reporte;
}
