import { checkpoint as journalCheckpoint } from '../../../varios-1/syncJournal';
import { logSync } from '../../../varios-1/syncLogger';
import { obtenerClaveHistorialSample } from '../claves';
import {
    datos,
    enLote,
    journalActivo,
    setEnLote,
    setVersionLocalConocida,
    STORE_KEY_TRACKING,
    storeCache,
    versionLocalConocida,
} from '../estado';
import { reconstruirIndices } from '../indices';
import type { BaseSyncLocal, EntradaHistorialSample } from '../tipos';

/**
 * Escribe el estado completo al Tauri Store (para acceso cross-window).
 * TC1: Incluye version gate — si otra ventana escribió una versión más nueva,
 * re-leer y fusionar antes de escribir para evitar sobrescribir datos.
 */
export async function escribirEnStore(): Promise<void> {
    if (!storeCache) return;

    const almacenado = await storeCache.get<BaseSyncLocal>(STORE_KEY_TRACKING);
    const versionStore = almacenado?.checkpointVersion ?? 0;

    /* TC1: Si la versión del Store es mayor que la nuestra, otra ventana escribió.
     * Re-leer el estado completo del Store antes de mergear historial. */
    if (versionStore > versionLocalConocida && almacenado) {
        logSync.info('tracking', `TC1: Versión Store (${versionStore}) > local (${versionLocalConocida}), fusionando datos cross-window`);

        /*
         * C287: Guard de userId — si el Store pertenece a otro usuario, NO fusionar
         * colecciones/archivos/sinColeccion. Evita que datos de un usuario anterior
         * se re-importen despues del cleanup de inicializarTracking.
         * Solo fusionar historialSamples (neutro respecto a propiedad).
         */
        const mismoUsuario = !datos.userId || !almacenado.userId || datos.userId === almacenado.userId;

        if (mismoUsuario) {
            /* Fusionar archivos: preferir datos locales (nuestra ventana es la que hizo cambios),
             * pero importar archivos que la otra ventana añadió y nosotros no tenemos */
            for (const [clave, archivoRemoto] of Object.entries(almacenado.archivos)) {
                if (!datos.archivos[clave]) {
                    datos.archivos[clave] = archivoRemoto;
                }
            }

            /* Fusionar colecciones: importar las que no tenemos */
            for (const [idStr, colRemota] of Object.entries(almacenado.colecciones)) {
                const id = Number(idStr);
                if (!datos.colecciones[id]) {
                    datos.colecciones[id] = colRemota;
                }
            }

            /* Fusionar sinColeccion: unión de sets */
            const sinColSet = new Set([...datos.sinColeccion, ...almacenado.sinColeccion]);
            datos.sinColeccion = Array.from(sinColSet);

            reconstruirIndices();
        } else {
            logSync.warn('tracking',
                `TC1: Store pertenece a usuario #${almacenado.userId}, local es #${datos.userId}. Omitiendo merge de colecciones/archivos.`);
        }
    }

    if (almacenado?.historialSamples && datos.historialSamples.length > 0) {
        fusionarHistorialSamplesPersistidos(datos.historialSamples, almacenado.historialSamples);
    }

    /* TC1: Incrementar versión y escribir */
    setVersionLocalConocida(versionStore + 1);
    datos.checkpointVersion = versionLocalConocida;

    await storeCache.set(STORE_KEY_TRACKING, datos);
    await storeCache.save();
}

export async function persistir(): Promise<void> {
    if (enLote) return;
    try {
        if (journalActivo) {
            /* WAL activo: forzar checkpoint (escribe checkpoint file + callback a Store) */
            await journalCheckpoint();
        } else if (storeCache) {
            /* Fallback sin journal (pre-inicialización o entorno sin fs) */
            await escribirEnStore();
        }
    } catch (err) {
        logSync.error('tracking', 'Error persistiendo datos', { error: err instanceof Error ? err.message : String(err) });
    }
}

function fusionarHistorialSamplesPersistidos(
    historialLocal: EntradaHistorialSample[],
    historialPersistido: EntradaHistorialSample[],
): void {
    const mapaPersistido = new Map<string, EntradaHistorialSample>();
    for (const entrada of historialPersistido) {
        mapaPersistido.set(obtenerClaveHistorialSample(entrada), entrada);
    }

    for (const entradaLocal of historialLocal) {
        const entradaPersistida = mapaPersistido.get(obtenerClaveHistorialSample(entradaLocal));
        if (!entradaPersistida) continue;

        if (entradaLocal.sampleId === 0 && entradaPersistida.sampleId > 0) {
            entradaLocal.sampleId = entradaPersistida.sampleId;
        }

        const persistidaMasReciente = entradaPersistida.timestampActualizado > entradaLocal.timestampActualizado;
        if (entradaPersistida.imagenUrl && (!entradaLocal.imagenUrl || persistidaMasReciente)) {
            entradaLocal.imagenUrl = entradaPersistida.imagenUrl;
            entradaLocal.timestampActualizado = Math.max(
                entradaLocal.timestampActualizado,
                entradaPersistida.timestampActualizado,
            );
        }

        if (persistidaMasReciente) {
            if (!entradaLocal.rutaLocal && entradaPersistida.rutaLocal) {
                entradaLocal.rutaLocal = entradaPersistida.rutaLocal;
            }
            if (!entradaLocal.coleccionNombre && entradaPersistida.coleccionNombre) {
                entradaLocal.coleccionNombre = entradaPersistida.coleccionNombre;
            }
            if (!entradaLocal.error && entradaPersistida.error) {
                entradaLocal.error = entradaPersistida.error;
            }
        }
    }
}

/* Lote (batch) */

/** Inicia modo lote: las operaciones no persisten individualmente. */
export function iniciarLote(): void { setEnLote(true); }

/** Finaliza modo lote y persiste todos los cambios acumulados. */
export async function finalizarLote(): Promise<void> {
    setEnLote(false);
    await persistir();
}
