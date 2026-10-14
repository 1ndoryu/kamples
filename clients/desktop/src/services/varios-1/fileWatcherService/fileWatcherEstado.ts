/*
 * Estado mutable del watcher (fileWatcherService).
 *
 * Los mapas van agrupados en `estadoFileWatcher` (const; se mutan con set/delete/clear).
 * Los escalares y callbacks que se reasignan se exportan como `let`: los
 * importadores ven el valor vigente (binding vivo de ESM) pero no pueden
 * reasignarlos; las reasignaciones van por los setters de este módulo.
 */

import type {
    ArchivoBufferado,
    CarpetaEliminadaPendiente,
    EliminacionPendiente,
    OnArchivoEliminadoFn,
    OnArchivoMovidoFn,
    OnArchivoNuevoFn,
    OnCarpetaNuevaFn,
    OnCarpetaRenombradaFn,
    OnSubcarpetaNuevaFn,
    OnSubcarpetaRenombradaFn,
    PendienteCreacionCarpeta,
    PendienteCreacionSubcarpeta,
    RenameNoPareado,
    SubcarpetaEliminadaPendiente,
    UnwatchFn,
} from './fileWatcherTipos';

/* Mapas de estado del watcher agrupados en un único objeto (regla Sentinel singleton-mutable-state). */
export const estadoFileWatcher = {
    /* Cache de archivos recientemente procesados para ignorar eventos duplicados */
    archivosRecientes: new Map<string, number>(),

    /* Cache de carpetas recientemente procesadas para ignorar eventos duplicados */
    carpetasRecientes: new Map<string, number>(),

    /* Carpetas pendientes de creacion (debounce de rename de nivel 1) */
    carpetasPendientesCreacion: new Map<string, PendienteCreacionCarpeta>(),

    /* Mapa: nombreArchivo normalizado → EliminacionPendiente (buffer de MOVE) */
    eliminacionesPendientes: new Map<string, EliminacionPendiente>(),

    /* Buffer de eliminaciones de carpetas de nivel 1 para detectar renames */
    carpetasEliminadasPendientes: new Map<string, CarpetaEliminadaPendiente>(),

    /* C387: State para subcarpetas de nivel 2. Claves compuestas carpetaPadre/nombreSub */
    subcarpetasRecientes: new Map<string, number>(),
    subcarpetasPendientesCreacion: new Map<string, PendienteCreacionSubcarpeta>(),
    subcarpetasEliminadasPendientes: new Map<string, SubcarpetaEliminadaPendiente>(),
};

/* Buffer de acumulación de archivos nuevos (QL76). Se vacía con splice. */
export const bufferArchivosNuevos: ArchivoBufferado[] = [];

/* Timers y escalares reasignables: usar los setters de abajo */
export let timeoutFlushBatch: ReturnType<typeof setTimeout> | null = null;
export let timestampInicioBatch: number | null = null;
export let purgaInterval: ReturnType<typeof setInterval> | null = null;
export let renamePendienteNoPareado: RenameNoPareado | null = null;
export let unwatchFn: UnwatchFn | null = null;
export let observando = false;

/* Callbacks externos (registrados desde fileWatcherRegistro) */
export let onArchivoNuevo: OnArchivoNuevoFn | null = null;
export let onArchivoEliminado: OnArchivoEliminadoFn | null = null;
export let onArchivoMovido: OnArchivoMovidoFn | null = null;
export let onCarpetaNueva: OnCarpetaNuevaFn | null = null;
export let onCarpetaRenombrada: OnCarpetaRenombradaFn | null = null;
export let onSubcarpetaNueva: OnSubcarpetaNuevaFn | null = null;
export let onSubcarpetaRenombrada: OnSubcarpetaRenombradaFn | null = null;

export function setTimeoutFlushBatch(valor: ReturnType<typeof setTimeout> | null): void {
    timeoutFlushBatch = valor;
}

export function setTimestampInicioBatch(valor: number | null): void {
    timestampInicioBatch = valor;
}

export function setPurgaInterval(valor: ReturnType<typeof setInterval> | null): void {
    purgaInterval = valor;
}

export function setRenamePendienteNoPareado(valor: RenameNoPareado | null): void {
    renamePendienteNoPareado = valor;
}

export function setUnwatchFn(valor: UnwatchFn | null): void {
    unwatchFn = valor;
}

export function setObservando(valor: boolean): void {
    observando = valor;
}

export function setCallbacksArchivos(
    cbNuevo: OnArchivoNuevoFn,
    cbEliminado: OnArchivoEliminadoFn,
    cbMovido?: OnArchivoMovidoFn,
): void {
    onArchivoNuevo = cbNuevo;
    onArchivoEliminado = cbEliminado;
    onArchivoMovido = cbMovido ?? null;
}

export function setCallbacksCarpetas(cbNueva: OnCarpetaNuevaFn, cbRenombrada: OnCarpetaRenombradaFn): void {
    onCarpetaNueva = cbNueva;
    onCarpetaRenombrada = cbRenombrada;
}

export function setCallbacksSubcarpetas(cbNueva: OnSubcarpetaNuevaFn, cbRenombrada: OnSubcarpetaRenombradaFn): void {
    onSubcarpetaNueva = cbNueva;
    onSubcarpetaRenombrada = cbRenombrada;
}

/*
 * Limpia todos los timers y cachés pendientes del watcher.
 * La usa detenerObservacion; no incluye el flush del buffer de archivos
 * (ese lo ejecuta detenerObservacion después, para que los archivos
 * acumulados se despachen igualmente).
 */
export function limpiarEstadoObservacion(): void {
    estadoFileWatcher.archivosRecientes.clear();
    estadoFileWatcher.carpetasRecientes.clear();

    /* Limpiar eliminaciones pendientes para evitar callbacks sueltos */
    for (const [, pendiente] of estadoFileWatcher.eliminacionesPendientes) {
        clearTimeout(pendiente.timeout);
    }
    estadoFileWatcher.eliminacionesPendientes.clear();

    /* C357: Limpiar carpetas pendientes de rename */
    for (const [, pendiente] of estadoFileWatcher.carpetasEliminadasPendientes) {
        clearTimeout(pendiente.timeout);
    }
    estadoFileWatcher.carpetasEliminadasPendientes.clear();

    /* Limpiar creaciones de carpeta pendientes (debounce rename) */
    for (const [, pendiente] of estadoFileWatcher.carpetasPendientesCreacion) {
        clearTimeout(pendiente.timeout);
    }
    estadoFileWatcher.carpetasPendientesCreacion.clear();

    /* C387: Limpiar state de subcarpetas */
    for (const [, pendiente] of estadoFileWatcher.subcarpetasPendientesCreacion) {
        clearTimeout(pendiente.timeout);
    }
    estadoFileWatcher.subcarpetasPendientesCreacion.clear();
    for (const [, pendiente] of estadoFileWatcher.subcarpetasEliminadasPendientes) {
        clearTimeout(pendiente.timeout);
    }
    estadoFileWatcher.subcarpetasEliminadasPendientes.clear();
    estadoFileWatcher.subcarpetasRecientes.clear();

    /* C287: Limpiar rename no pareado pendiente */
    if (renamePendienteNoPareado) {
        clearTimeout(renamePendienteNoPareado.timeout);
        renamePendienteNoPareado = null;
    }
}
