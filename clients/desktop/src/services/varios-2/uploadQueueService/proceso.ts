/* Procesamiento secuencial de la cola: procesarCola y procesarItemUpload. */

import { estaOnline } from '../../varios-1/desktopService';
import { registrarAccionHistorial, actualizarEstadoSampleHistorial } from '../../sync/varios-2/syncService';
import { Semaforo } from '../semaforo';
import { flushPersistencia } from '../persistenciaDebounce';
import { estado } from '../../sync/varios-2/syncState';
import { circuitoUpload } from '../../varios-1/circuitBreaker';
import { logSync } from '../../sync/varios-1/syncLogger';
import { ItemUploadCola } from './tipos';
import { cola, estadoUploadQueue, procesando, normalizarRutaCola, claveRutaEnCola, semaforoUpload, setProcesando, setSemaforoUpload } from './uploadQueueEstado';
import { MAX_REINTENTOS, BACKOFF_BASE_MS, emitirProgreso } from './helpers';
import { guardarColaDebounced, guardarHashes } from './persistencia';
import { subirArchivo } from './subirArchivo';

/*
 * Procesa la cola con paralelismo controlado.
 * El número de uploads simultáneos se lee de configAvanzada.archivosParalelos.
 * Incluye throttle inter-archivo si hay límite de velocidad configurado.
 */
export async function procesarCola(): Promise<void> {
    if (procesando || !estaOnline()) return;

    /* Circuit breaker: si el servidor está caído, no intentar uploads */
    if (!circuitoUpload.puedeEjecutar()) {
        logSync.debug('uploadQueue', 'Circuit breaker abierto, posponiendo procesamiento');
        return;
    }

    setProcesando(true);

    const configAvanzada = estado.configAvanzada;
    const maxParalelos = Math.max(1, Math.min(5, configAvanzada.archivosParalelos));

    /* Crear o actualizar semáforo */
    if (!semaforoUpload || semaforoUpload.limite !== maxParalelos) {
        setSemaforoUpload(new Semaforo(maxParalelos));
    }

    const promesasActivas: Promise<void>[] = [];

    try {
        while (true) {
            const siguiente = cola.find(i => i.estado === 'pendiente');

            if (!siguiente) {
                /* No hay más pendientes: esperar a que terminen las activas */
                if (promesasActivas.length > 0) {
                    await Promise.race(promesasActivas);
                    continue;
                }
                break;
            }

            if (!estaOnline()) break;

            siguiente.estado = 'subiendo';
            siguiente.timestampActualizado = Date.now();
            emitirProgreso(siguiente);
            guardarColaDebounced();

            /* Historial per-sample: actualizar a "subiendo" solo en primer intento */
            if (siguiente.intentos === 0) {
                actualizarEstadoSampleHistorial({
                    sampleId: 0,
                    nombreArchivo: siguiente.nombreArchivo,
                    estado: 'subiendo',
                    rutaLocal: siguiente.rutaArchivo,
                }).catch(() => {});

                registrarAccionHistorial({
                    tipo: 'subiendo',
                    descripcion: `Subiendo: "${siguiente.nombreArchivo}"`,
                }).catch(() => {});
            }

            /* Adquirir slot del semáforo (espera si pool lleno) */
            await semaforoUpload!.adquirir();

            const itemRef = siguiente;
            const promesa = procesarItemUpload(itemRef, configAvanzada)
                .finally(() => {
                    semaforoUpload!.liberar();
                    const idx = promesasActivas.indexOf(promesa);
                    if (idx !== -1) promesasActivas.splice(idx, 1);
                });

            promesasActivas.push(promesa);
        }

        /* Esperar que TODAS las activas terminen */
        await Promise.all(promesasActivas);
    } finally {
        setProcesando(false);
        await guardarHashes();
        await flushPersistencia('upload_cola');
    }
}

/*
 * Procesa un item individual de la cola.
 * Gestiona rethintos, backoff, y throttle inter-archivo.
 */
async function procesarItemUpload(
    item: ItemUploadCola,
    configAvanzada: { velocidadMaximaSubidaMbps: number },
): Promise<void> {
    const exito = await subirArchivo(item);

    if (exito) {
        item.estado = 'completado';
        if (item.hashParcial) {
            estadoUploadQueue.hashesConocidos.add(item.hashParcial);
            const rutaNorm = normalizarRutaCola(item.rutaArchivo);
            const existentes = estadoUploadQueue.hashARutas.get(item.hashParcial) ?? new Set<string>();
            existentes.add(rutaNorm);
            estadoUploadQueue.hashARutas.set(item.hashParcial, existentes);
            /* Persistir hash inmediatamente para que uploads paralelos lo vean.
             * No esperar al fin de procesarCola — previene duplicados por race condition. */
            guardarHashes().catch(() => {});
        }
        estadoUploadQueue.rutasEnCola.delete(claveRutaEnCola(item.rutaArchivo));

        /* QK97: Borrar archivo local tras upload exitoso si la opción está activa.
         * Se ejecuta después de registrar hash y tracking para no perder referencia.
         * Fallo de borrado NO afecta al estado del upload (ya está completado). */
        if (estado.configAvanzada.borrarAlSubirExitoso) {
            if (!tieneSubidaPersistidaConfirmada(item)) {
                logSync.warn('uploadQueue', `Borrado local omitido por falta de confirmacion persistida: ${item.nombreArchivo}`);
            } else {
                try {
                    const { remove } = await import('@tauri-apps/plugin-fs');
                    await remove(item.rutaArchivo);
                    logSync.info('uploadQueue', `Archivo local borrado tras subida: ${item.nombreArchivo}`);
                } catch (errBorrado) {
                    logSync.warn('uploadQueue', `No se pudo borrar archivo local tras subida: ${item.nombreArchivo}`, {
                        error: errBorrado instanceof Error ? errBorrado.message : String(errBorrado),
                    });
                }
            }
        }

        /* Throttle inter-archivo: si hay límite de velocidad, esperar proporcionalmente */
        if (configAvanzada.velocidadMaximaSubidaMbps > 0 && item.hashParcial) {
            const tamanoEstimadoBytes = 5 * 1024 * 1024; /* ~5MB promedio */
            const bytesPerSegundo = configAvanzada.velocidadMaximaSubidaMbps * 125_000;
            const delayMs = Math.max(0, (tamanoEstimadoBytes / bytesPerSegundo) * 1000 - 500);
            if (delayMs > 100) {
                await new Promise(r => setTimeout(r, delayMs));
            }
        }
    } else {
        item.intentos++;
        if (item.intentos >= MAX_REINTENTOS) {
            item.estado = 'error';
            estadoUploadQueue.rutasEnCola.delete(claveRutaEnCola(item.rutaArchivo));

            actualizarEstadoSampleHistorial({
                sampleId: 0,
                nombreArchivo: item.nombreArchivo,
                estado: 'error',
                rutaLocal: item.rutaArchivo,
                error: item.ultimoError ?? 'Máximo de reintentos alcanzado',
            }).catch(() => {});

            registrarAccionHistorial({
                tipo: 'error_subida',
                descripcion: `Error al subir: "${item.nombreArchivo}" — ${item.ultimoError ?? 'desconocido'}`,
            }).catch(() => {});

            logSync.error('uploadQueueService', '[UploadQueue] Máximo de reintentos alcanzado:', item.nombreArchivo);
        } else {
            item.estado = 'pendiente';
            /* Backoff exponencial antes de reintentar */
            const espera = BACKOFF_BASE_MS * Math.pow(2, item.intentos - 1);
            await new Promise(r => setTimeout(r, espera));
        }
    }

    item.timestampActualizado = Date.now();
    emitirProgreso(item);
    guardarColaDebounced();
}

/*
 * B6: Mueve un archivo duplicado a la carpeta duplicados/ manteniendo
 * la estructura de carpetas relativa desde la raíz de sync.
 *
 * Ejemplo: sync/Hip Hop/kick.wav → sync/duplicados/Hip Hop/kick.wav
 *
 * Si el archivo destino ya existe, agrega timestamp al nombre para
 * evitar colisiones (kick_1749839234.wav).
 */
/*
 * @deprecated QL66-EXTRA: Ya no se usa. Los duplicados se suben al servidor
 * para que el admin los revise. Se conserva temporalmente por si se necesita
 * revertir el cambio.
 */
async function moverADuplicados(_rutaArchivo: string, _carpetas: string[]): Promise<boolean> {
    return false;
}

export function tieneSubidaPersistidaConfirmada(
    item: Pick<ItemUploadCola, 'rutaArchivo' | 'sampleIdServidor'>,
): boolean {
    const rutaNorm = normalizarRutaCola(item.rutaArchivo);
    const enTracking = estado.trackingModule?.buscarArchivoPorRuta(rutaNorm);
    if (enTracking && !enTracking.syncDeshabilitado) return true;
    return typeof item.sampleIdServidor === 'number' && item.sampleIdServidor > 0;
}
