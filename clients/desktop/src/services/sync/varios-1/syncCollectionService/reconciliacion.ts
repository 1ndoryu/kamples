/* Reconciliación de rutas físicas con colecciones en tracking y renombre atómico de colecciones (F5.1). */

import { logSync } from '../syncLogger';
import {
    buscarColeccionPorCarpeta,
    registrarAccion,
    registrarArchivo,
    eliminarArchivo,
    actualizarNombreColeccion,
    todasLasColecciones,
    todosLosArchivos,
    type ColeccionLocal,
} from '../../varios-2/syncTrackingService';
import { CARPETA_SIN_COLECCION, CARPETA_SIN_COLECCION_LEGACY } from './constantes';
import { sanitizarNombreCarpeta } from './helpers';
import { agregarSampleAColeccion } from './movimiento';

/**
 * Reconciliación local→servidor: detecta samples cuya ruta física en disco
 * pertenece a una carpeta de colección diferente a la registrada en tracking.
 *
 * Causa típica: un move local fue procesado por el watcher pero no se
 * actualizó coleccion_samples en el servidor (bug pre-actualizarColeccionEnMovimiento).
 * El tracking actualizó rutaLocal pero no coleccionId.
 *
 * Para cada discrepancia: actualiza coleccion_samples en servidor + re-registra
 * el tracking entry con el coleccionId correcto.
 */
export async function reconciliarRutasConColecciones(carpetaBase: string): Promise<number> {
    const archivos = todosLosArchivos();
    const coleccionesLocales = todasLasColecciones();
    const baseNorm = carpetaBase.replace(/\\/g, '/').replace(/\/+$/, '');
    let corregidos = 0;

    for (const archivo of archivos) {
        const rutaNorm = archivo.rutaLocal.replace(/\\/g, '/');
        if (!rutaNorm.startsWith(baseNorm + '/')) continue;

        const relativa = rutaNorm.slice(baseNorm.length + 1);
        const partes = relativa.split('/');
        if (partes.length < 2) continue;

        const carpetaPrimaria = partes[0];
        const esSinCol = carpetaPrimaria === CARPETA_SIN_COLECCION
            || carpetaPrimaria === CARPETA_SIN_COLECCION_LEGACY;

        let coleccionIdEsperado: number | null = null;

        if (!esSinCol) {
            const colPadre = buscarColeccionPorCarpeta(carpetaPrimaria);
            if (!colPadre) continue;
            coleccionIdEsperado = colPadre.id;

            /*
             * Subcollections N niveles: caminar cada carpeta intermedia
             * del path (excluyendo la primera y el archivo final) buscando
             * subcollections anidadas. La más profunda que resuelve gana.
             * Ej: k1/k4/k5/sample.wav → k1 → k4 (sub de k1) → k5 (sub de k4)
             */
            let padreActual = colPadre.id;
            for (let nivel = 1; nivel < partes.length - 1; nivel++) {
                const carpetaNivel = partes[nivel];
                const sub = coleccionesLocales.find(
                    c => c.parentId === padreActual
                        && c.carpetaLocal.toLowerCase() === carpetaNivel.toLowerCase(),
                );
                if (!sub) break;
                coleccionIdEsperado = sub.id;
                padreActual = sub.id;
            }
        }

        /* Verificar si hay discrepancia */
        if (archivo.coleccionId === coleccionIdEsperado) continue;

        try {
            /* Actualizar servidor: agregar a colección destino real */
            if (coleccionIdEsperado !== null) {
                await agregarSampleAColeccion(coleccionIdEsperado, archivo.sampleId);
            }

            /* Re-registrar tracking con coleccionId correcto */
            await eliminarArchivo(archivo.sampleId, archivo.coleccionId);
            await registrarArchivo({
                ...archivo,
                coleccionId: coleccionIdEsperado,
            });

            corregidos++;
            logSync.info('collectionSync',
                `Reconciliación: sample ${archivo.sampleId} — tracking col ${archivo.coleccionId} → col ${coleccionIdEsperado} (ruta: ${relativa})`);
        } catch (err) {
            logSync.warn('collectionSync',
                `Error reconciliando sample ${archivo.sampleId}`,
                { error: err instanceof Error ? err.message : String(err) });
        }
    }

    return corregidos;
}

/* Renombre de colección — Operación atómica con rollback (F5.1) */

export async function manejarRenombreColeccion(
    carpetaBase: string,
    colLocal: ColeccionLocal,
    nuevoNombreServer: string,
): Promise<void> {
    const { TransaccionSync } = await import('../../../varios-2/transaccionSync');

    const nuevaCarpeta = sanitizarNombreCarpeta(nuevoNombreServer);
    const nombreAnterior = colLocal.nombre;
    const carpetaAnterior = colLocal.carpetaLocal;

    const tx = new TransaccionSync(`rename-col-${colLocal.id}`);

    /* Paso 1: Renombrar carpeta en disco */
    tx.agregar(
        'renombrar-carpeta-disco',
        async () => {
            const { rename, exists } = await import('@tauri-apps/plugin-fs');
            const { join } = await import('@tauri-apps/api/path');
            const rutaAnterior = await join(carpetaBase, carpetaAnterior);
            const rutaNueva = await join(carpetaBase, nuevaCarpeta);
            const existeAnterior = await exists(rutaAnterior);
            if (existeAnterior) {
                await rename(rutaAnterior, rutaNueva);
            }
        },
        async () => {
            const { rename, exists } = await import('@tauri-apps/plugin-fs');
            const { join } = await import('@tauri-apps/api/path');
            const rutaNueva = await join(carpetaBase, nuevaCarpeta);
            const rutaAnterior = await join(carpetaBase, carpetaAnterior);
            const existeNueva = await exists(rutaNueva);
            if (existeNueva) {
                await rename(rutaNueva, rutaAnterior);
            }
        },
    );

    /* Paso 2: Actualizar tracking local */
    tx.agregar(
        'actualizar-tracking',
        async () => {
            await actualizarNombreColeccion(colLocal.id, nuevoNombreServer, nuevaCarpeta);
        },
        async () => {
            await actualizarNombreColeccion(colLocal.id, nombreAnterior, carpetaAnterior);
        },
    );

    const exito = await tx.ejecutar();

    if (exito) {
        await registrarAccion({
            tipo: 'renombrado',
            descripcion: `Colección renombrada: "${nombreAnterior}" → "${nuevoNombreServer}"`,
            coleccionId: colLocal.id,
        });
        logSync.info('syncCollection', `Colección ${colLocal.id} renombrada: ${carpetaAnterior} → ${nuevaCarpeta}`);
    } else {
        logSync.error('syncCollection', `Error renombrando colección ${colLocal.id} — rollback ejecutado`);
    }
}
