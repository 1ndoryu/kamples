/* Encolado de archivos detectados por el watcher, con dedup y antispam. */

import { estaOnline } from '../../varios-1/desktopService';
import { registrarAccionHistorial, actualizarEstadoSampleHistorial } from '../../sync/varios-2/syncService';
import { estado } from '../../sync/varios-2/syncState';
import { esRutaPapelera, normalizarNombreParaDedup } from '../../varios-1/normalizarNombreArchivo';
import { logSync } from '../../sync/varios-1/syncLogger';
import { ItemUploadCola } from './tipos';
import { cola, estadoUploadQueue, MAX_DETECCIONES_HASH, registrarDeteccionHash, esHashBloqueadoPorAntispam, normalizarRutaCola, claveRutaEnCola, existeArchivoActivoConHash, crearIdempotencyKeyDeterministica } from './uploadQueueEstado';
import { esExtensionAudioValida, estaEnCarpetaSync, calcularHashParcial } from './helpers';
import { guardarColaDebounced } from './persistencia';
import { procesarCola } from './proceso';

/*
 * Encola un archivo nuevo para subir al servidor.
 * Llamado por fileWatcherService cuando detecta un archivo nuevo.
 *
 * Retorna false si el archivo es duplicado (ya fue subido).
 */
export async function encolarArchivo(
    rutaArchivo: string,
    nombreArchivo: string,
    carpetas: string[],
): Promise<boolean> {
    const rutaNormalizada = normalizarRutaCola(rutaArchivo);
    const rutaClave = claveRutaEnCola(rutaNormalizada);

    /* P3: Rechazar archivos dentro de .papelera — defensa en profundidad */
    if (esRutaPapelera(rutaNormalizada)) {
        logSync.info('uploadQueueService', '[UploadQueue] Archivo en .papelera, rechazando:', nombreArchivo);
        return false;
    }

    /* SEC-A3: Rechazar archivos que no sean audio — solo extensiones validas */
    if (!esExtensionAudioValida(nombreArchivo)) {
        logSync.warn('uploadQueue', `Archivo ignorado (no audio): ${nombreArchivo}`);
        return false;
    }

    /* Guard: rechazar archivos fuera de la carpeta de sync configurada.
     * Defensa en profundidad — complementa el guard del watcher (C378).
     * Cubre edge cases de OneDrive/Windows donde el driver FS emite rutas externas. */
    if (!estaEnCarpetaSync(rutaNormalizada)) {
        logSync.warn('uploadQueueService', '[UploadQueue] Archivo fuera de carpeta sync, rechazando en encolamiento:', [nombreArchivo, rutaNormalizada]);
        return false;
    }

    /*
     * Guard síncrono: prevenir race condition entre eventos create y modify.
     * DEBE ejecutarse ANTES de cualquier await para ser efectivo.
     * Si otro llamado a encolarArchivo() ya está procesando esta ruta
     * (calculando hash, etc.), rechazar inmediatamente.
     */
    if (estadoUploadQueue.rutasEncolando.has(rutaClave)) {
        logSync.info('uploadQueueService', '[UploadQueue] Archivo ya en proceso de encolamiento, ignorando:', nombreArchivo);
        return false;
    }
    estadoUploadQueue.rutasEncolando.add(rutaClave);

    try {
        return await encolarArchivoInterno(rutaNormalizada, nombreArchivo, carpetas);
    } finally {
        estadoUploadQueue.rutasEncolando.delete(rutaClave);
    }
}

/*
 * Lógica interna de encolamiento, separada para que el guard síncrono
 * de encolarArchivo() pueda usar try/finally limpiamente.
 */
async function encolarArchivoInterno(
    rutaArchivo: string,
    nombreArchivo: string,
    carpetas: string[],
): Promise<boolean> {
    /*
     * P4: Normalizar nombre (elimina prefijo timestamp de papelera) para
     * que el dedup por nombre funcione aunque el archivo haya pasado por
     * papelera y luego reaparecido con prefijo timestamp.
     */
    const nombreNormalizado = normalizarNombreParaDedup(nombreArchivo);

    /* Verificar si ya está en la cola (misma ruta) — O(1) con Set */
    const rutaClave = claveRutaEnCola(rutaArchivo);
    if (estadoUploadQueue.rutasEnCola.has(rutaClave) && cola.some(i => claveRutaEnCola(i.rutaArchivo) === rutaClave && i.estado !== 'error')) {
        logSync.info('uploadQueueService', '[UploadQueue] Archivo ya en cola, ignorando:', nombreNormalizado);
        return false;
    }

    /* Calcular hash parcial para detectar duplicados por contenido */
    const hash = await calcularHashParcial(rutaArchivo);

    /*
     * QL69: Antispam — verificar si este hash ha sido detectado demasiadas veces.
     * Si un archivo se detecta 6+ veces, el sistema lo bloquea para evitar
     * saturar la cola con el mismo contenido repetidamente.
     */
    if (hash) {
        /* [2103A-11] Verificar ANTES de incrementar: si ya está bloqueado, retornar
         * inmediatamente sin seguir incrementando el contador (evita log de +20 detecciones). */
        if (esHashBloqueadoPorAntispam(hash)) {
            logSync.warn('uploadQueue', `Antispam: hash bloqueado (ya superado límite): ${nombreNormalizado}`);
            return false;
        }
        const detecciones = registrarDeteccionHash(hash);
        if (detecciones >= MAX_DETECCIONES_HASH) {
            logSync.warn('uploadQueue', `Antispam: hash bloqueado tras ${detecciones} detecciones: ${nombreNormalizado}`);
            return false;
        }
    }

    /*
     * QL66-EXTRA: Pre-check contra servidor ELIMINADO.
     * Antes, el desktop consultaba /check-duplicate y bloqueaba la subida si era
     * duplicado del mismo usuario. Ahora TODOS los archivos se suben al servidor
     * y PipelineAudio decide si es duplicado (paso 2.5, antes de IA).
     * El admin revisa en duplicados_pendientes y aprueba/rechaza.
     */

    /* TC2: Verificar si otro encolarArchivoInterno concurrente już reservó este hash.
     * Este check es síncrono e inmediatamente después del await, cerrando la ventana
     * de race entre dos llamados que calculan hash en paralelo. */
    if (hash && estadoUploadQueue.hashesPendientesEncola.has(hash)) {
        /* QL66-EXTRA: skip sin mover a duplicados/ — el archivo ya está siendo encolado por otro hilo concurrente */
        logSync.debug('uploadQueue', `Hash ya reservado por encola concurrente, ignorando: ${nombreNormalizado}`);
        return false;
    }
    /* Reservar hash síncronamente para que el siguiente llamado concurrente lo vea */
    if (hash) estadoUploadQueue.hashesPendientesEncola.add(hash);

    try {

    /* Verificar también si otro item en cola ya tiene este hash (race entre encolas). */
    if (hash && cola.some(i => i.hashParcial === hash && i.estado !== 'error' && i.estado !== 'completado')) {
        /* QL66-EXTRA: skip sin mover — evitar upload duplicado del mismo batch */
        logSync.debug('uploadQueue', `Hash ya encolado en batch actual, ignorando: ${nombreNormalizado}`);
        return false;
    }

    if (hash && estadoUploadQueue.hashesConocidos.has(hash)) {
        /*
         * C6 fix: Hash conocido → verificar si ALGÚN archivo activo en tracking
         * tiene ese contenido. Si existe, es duplicado confirmado (mismo contenido,
         * posiblemente nombre diferente). El servidor ya tiene este sample.
         *
         * QL106: Duplicado confirmado → borrar archivo local + log claro.
         * Hash parcial (size + primeros/últimos 8KB SHA-256) es suficientemente
         * fiable para audio. El servidor tiene dedup adicional en PipelineAudio.
         */
        if (estado.trackingModule) {
            const hayActivoConHash = existeArchivoActivoConHash(hash);
            if (!hayActivoConHash) {
                logSync.info('uploadQueueService', '[UploadQueue] Hash conocido pero sin archivo activo en tracking, evictando hash stale:', nombreNormalizado);
                estadoUploadQueue.hashesConocidos.delete(hash);
                /* Continuar con encolamiento normal */
            } else {
                logSync.info('uploadQueueService', `[UploadQueue] Duplicado confirmado — "${nombreNormalizado}" tiene el mismo contenido que un sample ya subido. Eliminando archivo local.`);
                try {
                    const { remove } = await import('@tauri-apps/plugin-fs');
                    await remove(rutaArchivo);
                    logSync.info('uploadQueue', `Archivo duplicado eliminado de disco: ${nombreNormalizado}`);
                } catch (errBorrar) {
                    logSync.warn('uploadQueue', `No se pudo eliminar duplicado "${nombreNormalizado}": ${errBorrar instanceof Error ? errBorrar.message : String(errBorrar)}`);
                }
                return false;
            }
        } else {
            logSync.debug('uploadQueue', `Hash conocido, ignorando sin mover: ${nombreNormalizado}`);
            return false;
        }
    }

    const item: ItemUploadCola = {
        id: `upload-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
        rutaArchivo,
        nombreArchivo: nombreNormalizado,
        carpetas,
        estado: 'pendiente',
        intentos: 0,
        timestampCreado: Date.now(),
        timestampActualizado: Date.now(),
        hashParcial: hash ?? undefined,
        idempotencyKey: crearIdempotencyKeyDeterministica(rutaArchivo, nombreNormalizado, hash ?? undefined),
    };

    cola.push(item);
    estadoUploadQueue.rutasEnCola.add(rutaClave);
    guardarColaDebounced();

    /* Historial per-sample: entrada inicial "detectado" (se actualiza a subiendo/sincronizado) */
    actualizarEstadoSampleHistorial({
        sampleId: 0, /* Temporal: se actualiza con sampleId real tras upload exitoso */
        nombreArchivo,
        estado: 'detectado',
        rutaLocal: rutaArchivo,
    }).catch(() => { /* No bloquear encolamiento por fallo en historial */ });

    /* Historial legacy para compatibilidad */
    registrarAccionHistorial({
        tipo: 'subida_pendiente',
        descripcion: `Archivo detectado: "${nombreArchivo}"`,
    }).catch(() => { /* No bloquear encolamiento por fallo en historial */ });

    logSync.debug('uploadQueue', `Archivo encolado: ${nombreArchivo}`, {
        carpetas,
        longitudCola: cola.length,
    });

    /* Si estamos online, procesar inmediatamente */
    if (estaOnline()) {
        procesarCola();
    }

    return true;

    } finally {
        /* TC2: Liberar hash reservado ahora que el item ya está en cola
         * (rutasEnCola lo cubre a partir de aquí) o se rechazó/falló. */
        if (hash) estadoUploadQueue.hashesPendientesEncola.delete(hash);
    }
}
