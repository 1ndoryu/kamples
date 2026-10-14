/* Subida de un archivo al servidor: validaciones, deduplicación, POST y registro en syncService. */

import { extraerMetadataDeRuta, registrarSubidaLocal, registrarAccionHistorial, actualizarEstadoSampleHistorial, moverSampleEnServidorPublico, moverArchivoASinColeccion, rehidratarImagenesPendientesForzadoSync } from '../../sync/varios-2/syncService';
import { obtenerBaseUrlSync } from '../../sync/varios-1/syncGuards';
import { estado } from '../../sync/varios-2/syncState';
import { clasificarError, obtenerEstrategia, esErrorDeRed } from '../../varios-1/errorSync';
import { circuitoUpload, CircuitoBiertoError } from '../../varios-1/circuitBreaker';
import { logSync } from '../../sync/varios-1/syncLogger';
import { ItemUploadCola } from './tipos';
import { estadoUploadQueue, claveRutaEnCola, existeArchivoActivoConHash } from './uploadQueueEstado';
import { estaEnCarpetaSync, MAX_REINTENTOS, esperarArchivoDisponible, humanizarNombreArchivo, generarTagsDesdeContexto, obtenerMimeType } from './helpers';
import { guardarHashes } from './persistencia';

/*
 * Sube un archivo individual al servidor.
 *
 * Flujo:
 * 1. Verificar duplicado en tracking v2 (defensa de última línea)
 * 2. Leer archivo del disco (Tauri readFile)
 * 3. Generar título y tags desde nombre y carpetas
 * 4. Construir FormData
 * 5. POST /kamples/v1/samples/upload
 * 6. Registrar descarga en índice local
 * 7. Guardar hash inmediatamente (no esperar fin de cola)
 */
export async function subirArchivo(item: ItemUploadCola): Promise<boolean> {
    const rutaClave = claveRutaEnCola(item.rutaArchivo);

    try {
        /* Guard: descartar items cuya ruta está fuera de la carpeta de sync.
         * Usa función centralizada estaEnCarpetaSync (normaliza separadores + casing). */
        if (!estaEnCarpetaSync(item.rutaArchivo)) {
            logSync.warn('uploadQueueService', '[UploadQueue] Item fuera de carpeta sync, descartando:', item.rutaArchivo);
            return true; /* true = no reintentar, simplemente descartar */
        }

        /*
         * Guard de ruta en vuelo: previene que dos uploads del mismo archivo
         * estén en progreso simultáneamente. Complementa hashesEnVuelo para
         * cubrir el caso donde hash es null (error de FS, OneDrive lock).
         */
        if (estadoUploadQueue.rutasEnVuelo.has(rutaClave)) {
            logSync.info('uploadQueueService', '[UploadQueue] Ruta en vuelo (upload paralelo), omitiendo:', item.nombreArchivo);
            return true;
        }
        estadoUploadQueue.rutasEnVuelo.add(rutaClave);

        /*
         * Verificación de última línea contra tracking v2: entre el momento de encolar
         * y el momento de subir puede haber pasado tiempo (backoff, semáforo, etc.).
         * Otro upload del mismo archivo podría haber terminado en ese intervalo.
         *
         * SOLO lookup por ruta (no por nombre): buscarArchivoPorNombre es demasiado
         * amplio y causa falsos positivos cuando archivos con el mismo nombre existen
         * en colecciones diferentes (ej: kick.wav en "808" y en "Drums").
         */
        if (estado.trackingModule) {
            const enTracking = estado.trackingModule.buscarArchivoPorRuta(item.rutaArchivo);
            if (enTracking && !enTracking.syncDeshabilitado) {
                logSync.info('uploadQueueService', '[UploadQueue] Duplicado detectado en tracking pre-upload, omitiendo:', item.nombreArchivo);
                item.sampleIdServidor = enTracking.sampleId;
                /* Marcar como completado sin subir */
                return true;
            }
        }

        /* Re-verificar hash por si otro upload paralelo lo añadió.
         * Incluye evicción de hashes stale: si el sample fue borrado del servidor
         * y localmente, hashesConocidos aún retiene el hash fantasma. Verificar
         * que exista un archivo activo real antes de declarar duplicado. */
        if (item.hashParcial && estadoUploadQueue.hashesConocidos.has(item.hashParcial)) {
            if (estado.trackingModule) {
                const hayActivo = existeArchivoActivoConHash(item.hashParcial);
                if (!hayActivo) {
                    logSync.info('uploadQueueService', '[UploadQueue] Hash stale en pre-upload (sin archivo activo), evictando:', item.nombreArchivo);
                    estadoUploadQueue.hashesConocidos.delete(item.hashParcial);
                    guardarHashes().catch(() => {});
                    /* Continuar con upload normal */
                } else {
                    logSync.info('uploadQueueService', '[UploadQueue] Duplicado por hash detectado pre-upload:', item.nombreArchivo);
                    return true;
                }
            } else {
                logSync.info('uploadQueueService', '[UploadQueue] Duplicado por hash detectado pre-upload (sin tracking):', item.nombreArchivo);
                return true;
            }
        }

        /*
         * Guard de hash en vuelo: si otro upload paralelo del mismo contenido
         * está en progreso, esperar a que termine en vez de subir dos veces.
         * Esto cubre la ventana entre que subirArchivo() empieza y hashesConocidos
         * se actualiza tras el POST exitoso.
         */
        if (item.hashParcial) {
            if (estadoUploadQueue.hashesEnVuelo.has(item.hashParcial)) {
                logSync.info('uploadQueueService', '[UploadQueue] Hash en vuelo (upload paralelo), omitiendo:', item.nombreArchivo);
                return true;
            }
            estadoUploadQueue.hashesEnVuelo.add(item.hashParcial);
        }

        const { readFile, exists } = await import('@tauri-apps/plugin-fs');
        const baseUrl = obtenerBaseUrlSync();

        /*
         * P3: Verificar existencia antes de leer.
         * En entornos OneDrive el archivo puede haberse movido a papelera
         * o estar en proceso de sincronización cloud → "failed to open file".
         * Detectarlo aquí evita reintentos inútiles contra un archivo que ya no existe.
         */
        const archivoExiste = await exists(item.rutaArchivo);
        if (!archivoExiste) {
            const esOneDrive = item.rutaArchivo.includes('OneDrive');
            item.ultimoError = esOneDrive
                ? 'Archivo no encontrado (posible conflicto OneDrive/papelera)'
                : 'Archivo no encontrado en disco';
            /*
             * C384b: Retornar true (descartado) para que procesarItemUpload marque el item
             * como 'completado' y lo saque de rutasEnCola permanentemente.
             * Retornar false causaba que el item quedara como 'error' en el Store, y al
             * hacer 'Sincronizar ahora' el listener reintentar-errores-upload lo reseteaba
             * a 'pendiente' creando un ciclo infinito de reintentos fallidos.
             */
            actualizarEstadoSampleHistorial({
                sampleId: 0,
                nombreArchivo: item.nombreArchivo,
                estado: 'error',
                rutaLocal: item.rutaArchivo,
                error: item.ultimoError,
            }).catch(() => {});
            registrarAccionHistorial({
                tipo: 'error_subida',
                descripcion: `Archivo no encontrado, descartado: "${item.nombreArchivo}"`,
            }).catch(() => {});
            logSync.warn('uploadQueueService', '[UploadQueue] Archivo no existe, descartando permanentemente:', item.rutaArchivo);
            return true; /* true = completado/descartado, no reintentar */
        }

        /*
         * Esperar a que el archivo esté disponible para lectura.
         * En Windows, el watcher emite CREATE antes de que la copia finalice.
         * Sin esta espera, readFile falla con "os error 32" (file locked).
         * El backoff corto (~9s max) evita consumir un reintento de cola por
         * una condicion temporal que se resuelve en milisegundos.
         */
        const disponible = await esperarArchivoDisponible(item.rutaArchivo);
        if (!disponible) {
            item.ultimoError = 'Archivo bloqueado por otro proceso (copia en curso o OneDrive sync)';
            logSync.warn('uploadQueue', `Archivo no disponible tras espera, reintentando via cola: ${item.nombreArchivo}`);
            return false; /* false = reintentar via backoff normal de la cola */
        }

        /* Leer el archivo de audio del disco */
        const contenidoArchivo = await readFile(item.rutaArchivo);

        /* Generar metadata desde la ruta */
        const metaRuta = extraerMetadataDeRuta(item.rutaArchivo);

        /* Generar título: nombre del archivo sin extensión, humanizado */
        const titulo = humanizarNombreArchivo(metaRuta.nombreArchivo);

        /* Generar tags desde carpetas padre y nombre */
        const tags = generarTagsDesdeContexto(metaRuta.carpetas, metaRuta.nombreArchivo);

        /* Detectar MIME type por extensión */
        const mimeType = obtenerMimeType(metaRuta.extension);

        /* Construir FormData */
        /* QL107: TS 5.9 no acepta Uint8Array<ArrayBufferLike> como BlobPart — crear vista con buffer concreto */
        const blob = new Blob([new Uint8Array(contenidoArchivo)], { type: mimeType });
        const formData = new FormData();
        formData.append('audio', blob, item.nombreArchivo);
        formData.append('titulo', titulo);
        formData.append('contenido', '');
        formData.append('origen_subida', metaRuta.carpetas.join(' / '));
        formData.append('tags', JSON.stringify(tags));
        formData.append('permitir_descarga', 'true');
        formData.append('licencia_libre', 'false');
        formData.append('es_premium', 'false');
        formData.append('mostrar_en_comunidad', 'false');
        /* [193A-4] Marcar como sync para que el servidor use la cola IA con gap (216s)
         * en vez de procesar con PipelineAudio inmediatamente. Evita consumir créditos
         * de Groq a toda velocidad cuando se sincronizan librerías enteras. */
        formData.append('sync_upload', 'true');

        /* POST al servidor — P5: incluir clave de idempotencia para evitar duplicados por timeout */
        const respuesta = await fetch(`${baseUrl}/kamples/v1/samples/upload`, {
            method: 'POST',
            body: formData,
            headers: {
                'X-Idempotency-Key': item.idempotencyKey,
            },
            /* Content-Type se genera automáticamente con el boundary correcto */
        });

        if (!respuesta.ok) {
            const errorBody = await respuesta.json().catch(() => ({}));
            const errorMsg = (errorBody as { error?: string }).error ?? `HTTP ${respuesta.status}`;
            item.ultimoError = errorMsg;

            /* Clasificar error para estrategia de retry inteligente */
            const categoria = clasificarError(respuesta.status);
            const estrategia = obtenerEstrategia(categoria);

            if (categoria === 'permanente') {
                /* Errores permanentes (400, 404, 422): no reintentar */
                logSync.warn('uploadQueue', `Error permanente (${respuesta.status}), descartando: ${item.nombreArchivo}`, { error: errorMsg });
                item.intentos = MAX_REINTENTOS;
                return false;
            }

            if (categoria === 'autenticacion') {
                logSync.warn('uploadQueue', `Error de autenticación (${respuesta.status}): ${item.nombreArchivo}`);
            }

            /* Registrar fallo en circuit breaker para errores de servidor */
            if (categoria === 'transitorio' || categoria === 'rate_limit') {
                circuitoUpload.registrarFallo();
            }

            logSync.info('uploadQueue', `Error ${respuesta.status} (${categoria}): ${item.nombreArchivo}`, { intentos: item.intentos });
            return false;
        }

        /* Upload exitoso — registrar en circuit breaker */
        circuitoUpload.registrarExito();

        const resultado = await respuesta.json() as {
            ok: boolean;
            sample_id?: number;
            id_corto?: string;
        };

        if (!resultado.ok || !resultado.sample_id) {
            item.ultimoError = 'Respuesta inesperada del servidor';
            return false;
        }

        item.sampleIdServidor = resultado.sample_id;

        /*
         * Resolver colección real a partir del nombre de carpeta local.
         * Permite pasar coleccionId a registrarSubidaLocal para tracking correcto
         * y agregar el sample a la tabla coleccion_samples en el servidor.
         *
         * B2: Si el sample está en subcarpeta (carpetas[1]), resolver subcollección
         * y asignar a AMBAS: la colección padre Y la subcolección.
         * Así el sample aparece en la colección principal al expandirla
         * y también se puede filtrar por subcolección.
         */
        let coleccionIdResuelta: number | null = null;
        let subcoleccionIdResuelta: number | null = null;
        if (item.carpetas.length > 0) {
            const nombreCarpeta = item.carpetas[0] || '';
            if (nombreCarpeta && estado.trackingModule) {
                const coleccion = estado.trackingModule.buscarColeccionPorCarpeta(nombreCarpeta);
                if (coleccion) {
                    coleccionIdResuelta = coleccion.id;

                    /* B2: Resolver subcolección si hay segundo nivel de carpeta */
                    if (item.carpetas.length >= 2 && item.carpetas[1]) {
                        const sub = estado.trackingModule.buscarSubcoleccion(nombreCarpeta, item.carpetas[1]);
                        if (sub) {
                            subcoleccionIdResuelta = sub.id;
                        }
                    }
                }
            }
        }

        /* Registrar en tracking + historial para feedback persistente en panel.
         * B2: Usar la colección más específica (subcolección > padre) para tracking. */
        await registrarSubidaLocal(
            resultado.sample_id,
            item.rutaArchivo,
            item.nombreArchivo,
            subcoleccionIdResuelta ?? coleccionIdResuelta,
        );

        /* Rehidratación centralizada: usar snapshot /me/sync/colecciones como fuente
         * única para imagenes. Evita depender de slug/endpoint individual y mantiene
         * consistencia entre ventana main y ventana sync-panel. */
        rehidratarImagenesPendientesForzadoSync().catch(() => {
            /* No bloquear flujo de upload por fallo de rehidratación */
        });

        /*
         * Asignar sample a la colección correspondiente en el servidor.
         *
         * Con UNIQUE(usuario_id, sample_id), un sample solo puede estar en UNA
         * colección por usuario. Se asigna a la más específica: subcolección si
         * existe, padre si no. agregarSampleAColeccion usa ON CONFLICT DO UPDATE
         * que MUEVE el sample si ya estaba en otra colección.
         *
         * PUT /me/coleccionados/{id}/carpeta actualiza metadata (no coleccion_samples).
         *
         * C384: Si hay carpeta pero NO colección en tracking, crearla ahora.
         * crearColeccionDesdeLocal es idempotente.
         */
        if (item.carpetas.length > 0) {
            if (!coleccionIdResuelta && item.carpetas[0]) {
                try {
                    const { crearColeccionDesdeLocal } = await import('../../sync/varios-1/syncCollectionService');
                    const idCreada = await crearColeccionDesdeLocal(item.carpetas[0]);
                    if (idCreada) {
                        coleccionIdResuelta = idCreada;
                        logSync.info('uploadQueueService', '[UploadQueue] Colección creada/resuelta para carpeta:', [item.carpetas[0], '→ id:', idCreada]);
                    }
                } catch (err) {
                    logSync.error('uploadQueueService', '[UploadQueue] Error creando colección para carpeta:', [item.carpetas[0], err]);
                }
            }

            /* Resolver/crear subcolección si hay segundo nivel */
            if (!subcoleccionIdResuelta && item.carpetas[1] && coleccionIdResuelta) {
                try {
                    const { crearColeccionDesdeLocal } = await import('../../sync/varios-1/syncCollectionService');
                    const idSub = await crearColeccionDesdeLocal(item.carpetas[1], coleccionIdResuelta);
                    if (idSub) {
                        subcoleccionIdResuelta = idSub;
                        logSync.info('uploadQueueService', '[UploadQueue] Subcolección creada/resuelta:', [item.carpetas[1], '→ id:', idSub]);
                    }
                } catch (err) {
                    logSync.error('uploadQueueService', '[UploadQueue] Error creando subcolección:', [item.carpetas[1], err]);
                }
            }

            /*
             * Asignar a la colección más específica (sub > padre).
             * UNIQUE(usuario_id, sample_id) impide estar en ambas; asignar a
             * ambas causaba un MOVE implícito server-side y changelogs espurios
             * que disparaban D4.2 local moves incorrectos.
             */
            const coleccionFinal = subcoleccionIdResuelta ?? coleccionIdResuelta;
            if (coleccionFinal) {
                try {
                    const { agregarSampleAColeccion } = await import('../../sync/varios-1/syncCollectionService');
                    await agregarSampleAColeccion(coleccionFinal, resultado.sample_id);
                } catch (err) {
                    logSync.error('uploadQueueService', '[UploadQueue] Error agregando sample a colección:', err);
                }
            }

            const primaria = item.carpetas[0] || 'General';
            const secundaria = item.carpetas[1] || '';
            await moverSampleEnServidorPublico(resultado.sample_id, primaria, secundaria);
        } else {
            /*
             * Archivo estaba en la raíz de sync (sin subcarpeta).
             * Después de subirlo, moverlo a "Sin colección" para organización.
             */
            const nuevaRuta = await moverArchivoASinColeccion(
                item.rutaArchivo,
                item.nombreArchivo,
                resultado.sample_id,
            );
            if (nuevaRuta) {
                item.rutaArchivo = nuevaRuta;
            }
        }

        logSync.info(
            'uploadQueueService', '[UploadQueue] Subido exitosamente:',
            [item.nombreArchivo,
            '→ sample_id:',
            resultado.sample_id],
        );
        return true;
    } catch (err) {
        item.ultimoError = err instanceof Error ? err.message : String(err);

        if (err instanceof CircuitoBiertoError) {
            logSync.info('uploadQueue', `Circuit breaker abierto, posponiendo: ${item.nombreArchivo}`);
            return false;
        }

        if (esErrorDeRed(err)) {
            circuitoUpload.registrarFallo();
            logSync.warn('uploadQueue', `Error de red subiendo: ${item.nombreArchivo}`);
        } else {
            logSync.error('uploadQueue', `Error subiendo: ${item.nombreArchivo}`, {
                error: item.ultimoError,
            });
        }
        return false;
    } finally {
        /* Liberar guards en vuelo siempre, éxito o fallo */
        if (item.hashParcial) {
            estadoUploadQueue.hashesEnVuelo.delete(item.hashParcial);
        }
        estadoUploadQueue.rutasEnVuelo.delete(rutaClave);
    }
}
