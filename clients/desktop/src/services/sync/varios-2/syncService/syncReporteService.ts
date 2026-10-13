import type { SyncReport } from './syncReporteTipos';
import { estado } from '../syncState';

export type { SyncReport } from './syncReporteTipos';

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
        const { circuitoSync } = await import('../../varios-1/syncGuards');
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
        const { obtenerResumenDebugTracking } = await import('../syncTrackingService');
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
        const { obtenerHistorialSamples } = await import('../syncTrackingService');
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
        const { obtenerResumenDebugUploadQueue } = await import('../../../varios-2/uploadQueueService');
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
        const journalModule = await import('../../varios-1/syncJournal');
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
        const { obtenerBaseUrlSync } = await import('../../varios-1/syncGuards');
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
