/*
 * Estado mutable del watcher (fileWatcherService).
 *
 * Los mapas se exportan como const (se mutan con set/delete/clear).
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

/* Cache de archivos recientemente procesados para ignorar eventos duplicados */
export const archivosRecientes = new Map<string, number>();

/* Cache de carpetas recientemente procesadas para ignorar eventos duplicados */
export const carpetasRecientes = new Map<string, number>();

/* Carpetas pendientes de creacion (debounce de rename de nivel 1) */
export const carpetasPendientesCreacion = new Map<string, PendienteCreacionCarpeta>();

/* Mapa: nombreArchivo normalizado → EliminacionPendiente (buffer de MOVE) */
export const eliminacionesPendientes = new Map<string, EliminacionPendiente>();

/* Buffer de acumulación de archivos nuevos (QL76). Se vacía con splice. */
export const bufferArchivosNuevos: ArchivoBufferado[] = [];

/* Buffer de eliminaciones de carpetas de nivel 1 para detectar renames */
export const carpetasEliminadasPendientes = new Map<string, CarpetaEliminadaPendiente>();

/* C387: State para subcarpetas de nivel 2. Claves compuestas carpetaPadre/nombreSub */
export const subcarpetasRecientes = new Map<string, number>();
export const subcarpetasPendientesCreacion = new Map<string, PendienteCreacionSubcarpeta>();
export const subcarpetasEliminadasPendientes = new Map<string, SubcarpetaEliminadaPendiente>();

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
    archivosRecientes.clear();
    carpetasRecientes.clear();

    /* Limpiar eliminaciones pendientes para evitar callbacks sueltos */
    for (const [, pendiente] of eliminacionesPendientes) {
        clearTimeout(pendiente.timeout);
    }
    eliminacionesPendientes.clear();

    /* C357: Limpiar carpetas pendientes de rename */
    for (const [, pendiente] of carpetasEliminadasPendientes) {
        clearTimeout(pendiente.timeout);
    }
    carpetasEliminadasPendientes.clear();

    /* Limpiar creaciones de carpeta pendientes (debounce rename) */
    for (const [, pendiente] of carpetasPendientesCreacion) {
        clearTimeout(pendiente.timeout);
    }
    carpetasPendientesCreacion.clear();

    /* C387: Limpiar state de subcarpetas */
    for (const [, pendiente] of subcarpetasPendientesCreacion) {
        clearTimeout(pendiente.timeout);
    }
    subcarpetasPendientesCreacion.clear();
    for (const [, pendiente] of subcarpetasEliminadasPendientes) {
        clearTimeout(pendiente.timeout);
    }
    subcarpetasEliminadasPendientes.clear();
    subcarpetasRecientes.clear();

    /* C287: Limpiar rename no pareado pendiente */
    if (renamePendienteNoPareado) {
        clearTimeout(renamePendienteNoPareado.timeout);
        renamePendienteNoPareado = null;
    }
}
