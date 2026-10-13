/* Descarga individual de un sample a su carpeta, con movimiento local para evitar re-descargas (D4.2). */

import { marcarDescargaEnCurso, marcarMovimientoInterno, obtenerBaseUrlSync, obtenerHeadersSyncGet } from '../syncGuards';
import { logSync } from '../syncLogger';
import { verificarTamano } from '../../../varios-1/hashService';
import {
    obtenerArchivo,
    buscarArchivoPorSampleId,
    registrarArchivo,
    registrarAccion,
    eliminarArchivo,
} from '../../varios-2/syncTrackingService';
import type { SampleSync, CallbackProgresoColecciones } from './tipos';

/* Descarga individual */

/**
 * Descarga un sample si no existe en el tracking.
 * Retorna 'nuevo' | 'existente' | 'omitido' | 'error'.
 */
export async function descargarSiNecesario(
    sample: SampleSync,
    coleccionId: number | null,
    carpetaDestino: string,
    onProgreso: CallbackProgresoColecciones | undefined,
    actual: number,
    total: number,
): Promise<'nuevo' | 'existente' | 'omitido' | 'error'> {
    const { exists: existeEnDisco } = await import('@tauri-apps/plugin-fs');

    /* Verificar si ya existe en tracking */
    const existente = obtenerArchivo(sample.id, coleccionId);
    if (existente) {
        if (existente.syncDeshabilitado) {
            onProgreso?.({ fase: 'descarga', actual, total, sampleId: sample.id, nombre: sample.titulo, estado: 'omitido' });
            return 'omitido';
        }

        /*
         * C289c: Verificar existencia real en disco.
         * Si el tracking dice que el archivo existe pero NO está en disco
         * (movido a duplicados, borrado externamente, etc.), limpiar la
         * entrada corrupta y continuar a la fase de descarga.
         */
        const archivoExisteEnDisco = await existeEnDisco(existente.rutaLocal);
        if (archivoExisteEnDisco) {
            onProgreso?.({ fase: 'descarga', actual, total, sampleId: sample.id, nombre: sample.titulo, estado: 'descargado' });
            return 'existente';
        }

        logSync.warn('collectionSync', `Tracking inconsistente: sample ${sample.id} registrado en "${existente.rutaLocal}" pero no existe en disco. Limpiando y re-descargando.`);
        await eliminarArchivo(existente.sampleId, existente.coleccionId);
    }

    /*
     * D4.2: Antes de descargar, verificar si el archivo existe localmente en
     * otra coleccion (sample movido server-side). Si existe en disco, mover
     * en vez de re-descargar para ahorrar ancho de banda.
     */
    const archivoEnOtraCol = buscarArchivoPorSampleId(sample.id);
    if (archivoEnOtraCol && archivoEnOtraCol.coleccionId !== coleccionId) {
        /*
         * Grace period: si el archivo fue registrado recientemente (upload o descarga
         * reciente), confiar en el tracking sobre los datos del servidor que pueden
         * ser stale. Esto previene la race condition donde:
         * 1. Upload → registrarSubidaLocal(sampleId, ruta, colId=85)
         * 2. sincronizarColecciones usa cache stale → sample aparece en sinColeccion
         * 3. D4.2 intenta mover de col 85 → sin-coleccion (INCORRECTO)
         * El tracking del upload es más reciente y confiable que el cache del servidor.
         */
        const GRACIA_UPLOAD_RECIENTE_MS = 60_000;
        const esRegistroReciente = (Date.now() - (archivoEnOtraCol.descargadoEn ?? 0)) < GRACIA_UPLOAD_RECIENTE_MS;
        if (esRegistroReciente) {
            logSync.info('collectionSync', `Sample ${sample.id} registrado hace <60s en col ${archivoEnOtraCol.coleccionId}, omitiendo D4.2 move a col ${coleccionId} (posible cache stale)`);
            onProgreso?.({ fase: 'descarga', actual, total, sampleId: sample.id, nombre: sample.titulo, estado: 'descargado' });
            return 'existente';
        }

        const { exists: existeEnDisco, rename: renombrarArchivo } = await import('@tauri-apps/plugin-fs');
        const { join } = await import('@tauri-apps/api/path');
        const existeLocal = await existeEnDisco(archivoEnOtraCol.rutaLocal);

        if (existeLocal) {
            try {
                const nombreArchivo = archivoEnOtraCol.nombreLocal || archivoEnOtraCol.nombreServidor;
                const rutaNueva = await join(carpetaDestino, nombreArchivo);

                /*
                 * Guard noop: si el archivo ya está en la ruta de destino
                 * (mismo path normalizado), no mover — solo actualizar tracking.
                 * Esto previene moves espurios cuando el tracking tiene un
                 * coleccionId obsoleto pero el archivo físico ya está correcto.
                 */
                const rutaActualNorm = archivoEnOtraCol.rutaLocal.replace(/\\/g, '/');
                const rutaNuevaNorm = rutaNueva.replace(/\\/g, '/');
                if (rutaActualNorm === rutaNuevaNorm) {
                    /* Solo re-registrar con coleccionId correcto */
                    await eliminarArchivo(archivoEnOtraCol.sampleId, archivoEnOtraCol.coleccionId);
                    await registrarArchivo({
                        sampleId: sample.id,
                        coleccionId,
                        rutaLocal: archivoEnOtraCol.rutaLocal,
                        nombreLocal: nombreArchivo,
                        nombreServidor: archivoEnOtraCol.nombreServidor,
                        descargadoEn: archivoEnOtraCol.descargadoEn,
                        tamano: archivoEnOtraCol.tamano,
                        syncDeshabilitado: false,
                    });
                    logSync.info('collectionSync', `Sample ${sample.id} re-registrado en colección ${coleccionId} (archivo ya en ruta correcta)`);
                    onProgreso?.({ fase: 'descarga', actual, total, sampleId: sample.id, nombre: nombreArchivo, estado: 'descargado' });
                    return 'existente';
                }

                marcarDescargaEnCurso(rutaNueva);
                /*
                 * Marcar ruta ORIGEN como movimiento interno para que el watcher
                 * ignore el evento DELETE generado por el rename. Sin esto,
                 * manejarBorradoLocal procesa el DELETE y puede disparar
                 * softDeleteEnServidor → borra el sample del servidor.
                 */
                marcarMovimientoInterno(archivoEnOtraCol.rutaLocal);
                await renombrarArchivo(archivoEnOtraCol.rutaLocal, rutaNueva);

                /* Limpiar tracking viejo y registrar nuevo */
                await eliminarArchivo(archivoEnOtraCol.sampleId, archivoEnOtraCol.coleccionId);
                await registrarArchivo({
                    sampleId: sample.id,
                    coleccionId,
                    rutaLocal: rutaNueva,
                    nombreLocal: nombreArchivo,
                    nombreServidor: archivoEnOtraCol.nombreServidor,
                    descargadoEn: archivoEnOtraCol.descargadoEn,
                    tamano: archivoEnOtraCol.tamano,
                    syncDeshabilitado: false,
                });
                await registrarAccion({
                    tipo: 'movido',
                    descripcion: `Sample "${sample.titulo}" movido localmente entre colecciones`,
                    sampleId: sample.id,
                    coleccionId: coleccionId ?? undefined,
                });

                logSync.info('collectionSync', `Sample ${sample.id} movido localmente (evitada re-descarga)`);
                onProgreso?.({ fase: 'descarga', actual, total, sampleId: sample.id, nombre: nombreArchivo, estado: 'descargado' });
                return 'nuevo';
            } catch (err) {
                logSync.warn('collectionSync', `No se pudo mover sample ${sample.id} localmente, se descargara`, {
                    error: err instanceof Error ? err.message : String(err),
                });
                /* Fallthrough a descarga normal */
            }
        } else {
            /* Archivo no existe en disco, limpiar tracking corrupto */
            await eliminarArchivo(archivoEnOtraCol.sampleId, archivoEnOtraCol.coleccionId);
        }
    }

    /* Descargar desde el servidor */
    try {
        onProgreso?.({ fase: 'descarga', actual, total, sampleId: sample.id, nombre: sample.titulo, estado: 'descargando' });

        const { writeFile } = await import('@tauri-apps/plugin-fs');
        const { join } = await import('@tauri-apps/api/path');
        const baseUrl = obtenerBaseUrlSync();

        /* Obtener URL firmada de descarga */
        const respDescarga = await fetch(`${baseUrl}/kamples/v1/samples/${sample.id}/descargar`, {
            method: 'POST',
            headers: obtenerHeadersSyncGet(),
        });
        if (!respDescarga.ok) {
            throw new Error(`No se pudo obtener URL de descarga: ${respDescarga.status}`);
        }

        interface ResultadoDescarga { url: string; nombre: string; formato: string; tamano: number }
        const { url: audioUrl, nombre, formato, tamano }: ResultadoDescarga = await respDescarga.json();

        /* Descargar el archivo de audio */
        const audioResp = await fetch(audioUrl);
        if (!audioResp.ok) {
            throw new Error(`Error al descargar audio: ${audioResp.status}`);
        }
        const buffer = await audioResp.arrayBuffer();

        const nombreArchivo = nombre.includes('.') ? nombre : `${nombre}.${formato}`;
        const rutaArchivo = await join(carpetaDestino, nombreArchivo);

        /* Marcar ruta antes de escribir para que el watcher la ignore */
        marcarDescargaEnCurso(rutaArchivo);

        await writeFile(rutaArchivo, new Uint8Array(buffer));

        /* F3.3: Verificación post-descarga — confirmar que el archivo no está truncado */
        const tamanoEsperado = tamano || buffer.byteLength;
        if (tamanoEsperado > 0) {
            const tamanoOk = await verificarTamano(rutaArchivo, tamanoEsperado);
            if (!tamanoOk) {
                logSync.warn('syncCollection', `Archivo truncado post-descarga: ${nombreArchivo} (esperado: ${tamanoEsperado})`);
                /* Reintentar la descarga (retry simple: tirar excepción para que el caller reintente) */
                throw new Error(`Archivo truncado post-descarga: esperado ${tamanoEsperado} bytes`);
            }
        }

        /* Registrar en tracking */
        await registrarArchivo({
            sampleId: sample.id,
            coleccionId,
            rutaLocal: rutaArchivo,
            nombreLocal: nombreArchivo,
            nombreServidor: nombreArchivo,
            descargadoEn: Date.now(),
            tamano: tamano || buffer.byteLength,
            syncDeshabilitado: false,
        });

        await registrarAccion({
            tipo: 'descarga',
            descripcion: `Descargado "${sample.titulo}" (${nombreArchivo})`,
            sampleId: sample.id,
            coleccionId: coleccionId ?? undefined,
        });

        onProgreso?.({ fase: 'descarga', actual, total, sampleId: sample.id, nombre: nombreArchivo, estado: 'descargado' });
        return 'nuevo';
    } catch (err) {
        logSync.error('syncCollection', `Error descargando sample ${sample.id}`, { error: err instanceof Error ? err.message : String(err) });
        onProgreso?.({ fase: 'descarga', actual, total, sampleId: sample.id, nombre: sample.titulo, estado: 'error' });
        return 'error';
    }
}
