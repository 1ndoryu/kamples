/*
 * Despacho de eventos del watcher (fileWatcherService).
 *
 * procesarEvento y manejarRenameNoPareado forman un ciclo (el rename no pareado
 * se re-despacha como evento pareado con procesarEvento). Por eso viven en este
 * mismo módulo: no separarlos sin rediseñar el re-despacho.
 */

import { logSync } from '../../sync/varios-1/syncLogger';
import { manejarArchivoEliminado, manejarArchivoNuevo } from './fileWatcherArchivos';
import { procesarEventoCarpeta } from './carpetas/fileWatcherCarpetas';
import {
    CARPETAS_EXCLUIDAS_TOTAL,
    CARPETAS_SISTEMA_NO_COLECCION,
    DEBOUNCE_ARCHIVO_MS,
    EXTENSIONES_ARCHIVO_CONOCIDAS,
    EXTENSIONES_AUDIO,
    GRACIA_RENAME_PAR_MS,
    PATRONES_TEMPORALES,
} from './fileWatcherConstantes';
import {
    estadoFileWatcher,
    setRenamePendienteNoPareado,
    renamePendienteNoPareado,
} from './fileWatcherEstado';
import { procesarRenamePareado } from './fileWatcherRenamePareado';
import { procesarEventoSubcarpeta } from './carpetas/fileWatcherSubcarpetas';
import {
    esDirectorioReal,
    esEventoCreacion,
    esEventoEliminacion,
    esEventoRename,
    pareceArchivoConExtension,
} from './fileWatcherUtilidades';

/*
 * Procesa un evento del watcher del filesystem.
 * Filtra por tipos relevantes (create, remove) y valida extensiones.
 * C357: También detecta eventos de carpetas de nivel 1 (colecciones).
 */
export async function procesarEvento(
    evento: { type: unknown; paths: string[] },
    carpetaBase: string,
): Promise<void> {
    const tipo = evento.type;

    const baseNormalizada = carpetaBase.replace(/\\/g, '/').replace(/\/$/, '');

    /*
     * Manejo explícito de rename/move nativo del FS.
     * Algunos proveedores emiten modify.kind = 'name' con 2 paths (origen, destino)
     * en vez de remove+create. Si no lo manejamos aquí, el move se pierde.
     * Si el rename no queda resuelto, el flujo sigue al bucle genérico (igual que antes).
     */
    if (esEventoRename(tipo) && evento.paths.length >= 2) {
        if (await procesarRenamePareado(evento.paths, baseNormalizada)) return;
    }

    /*
     * C287: Fallback para rename events con un solo path (From/To separados).
     * En Windows, notify puede emitir RenameMode::From y RenameMode::To como
     * eventos independientes con modify.kind='name' y paths.length=1.
     * Sin este fallback, ambos se ignoran silenciosamente porque
     * esEventoCreacion/esEventoEliminacion no detectan modify.kind='name'.
     *
     * Estrategia: buffear el primer path (From) y esperar el segundo (To).
     * Si llega par, re-despachar como rename pareado con 2 paths.
     * Si no llega par (timeout), despachar como DELETE sintético para que
     * el patrón delete+create existente detecte el rename.
     */
    if (esEventoRename(tipo) && evento.paths.length === 1) {
        manejarRenameNoPareado(evento.paths[0], baseNormalizada, carpetaBase);
        return;
    }

    for (const ruta of evento.paths) {
        const normalizada = ruta.replace(/\\/g, '/');
        const nombreArchivo = normalizada.split('/').pop() ?? '';

        /* Ignorar archivos temporales */
        if (PATRONES_TEMPORALES.some(p => p.test(nombreArchivo))) continue;

        /*
         * P1+P7: Filtro temprano de carpetas excluidas.
         * Se evalúa ANTES de clasificar tipo de evento para cortar lo más pronto posible.
         */
        const relativa = normalizada.startsWith(baseNormalizada + '/')
            ? normalizada.slice(baseNormalizada.length + 1)
            : '';

        /*
         * Guard crítico: omitir eventos de rutas que NO están dentro de la carpeta base.
         * En Windows con OneDrive/SMB el driver del sistema de archivos puede emitir
         * eventos para directorios hermanos o padres de la carpeta vigilada.
         * Sin este filtro, archivos fuera del scope configurado se encolarían para subida.
         */
        if (!relativa) continue;

        const segmentosRuta = relativa.split('/');

        /* Exclusión total: ignorar cualquier evento dentro de .papelera/ */
        if (segmentosRuta.some(s => CARPETAS_EXCLUIDAS_TOTAL.has(s))) continue;

        /*
         * QL70: Carpetas sistema (sin colección, duplicados) permiten CREATEs de archivos
         * pero NO deben generar eventos de carpeta (procesarEventoCarpeta).
         * El filtro de carpeta se aplica más abajo en la detección de nivel 1.
         */

        /*
         * C357: Detectar eventos de carpetas de nivel 1 (hijas directas de carpetaBase).
         * Una carpeta de nivel 1 = carpeta de colección. Si se crea o renombra, sincronizar.
         * Heurística: si el path NO tiene extensión de audio y es hijo directo de base,
         * tratarlo como posible evento de carpeta.
         * QL62: Si tiene extensión de archivo conocida (txt, gif, png, etc.) NO es carpeta.
         * Evita que archivos sueltos como "READ ME!.txt" creen colecciones fantasma.
         */
        const extension = nombreArchivo.split('.').pop()?.toLowerCase() ?? '';

        if (relativa && !relativa.includes('/') && !EXTENSIONES_AUDIO.has(extension)) {
            /* QL62+QL68: Archivos con extensión conocida o desconocida no son carpetas */
            if (extension && (EXTENSIONES_ARCHIVO_CONOCIDAS.has(extension) || pareceArchivoConExtension(nombreArchivo))) {
                logSync.warn('watcher', `Archivo no-audio ignorado en raiz sync: ${nombreArchivo}`);
                continue;
            }
            if (esEventoCreacion(tipo) && !await esDirectorioReal(normalizada)) {
                logSync.warn('watcher', `Archivo sin extension ignorado en raiz sync (no es carpeta real): ${nombreArchivo}`);
                continue;
            }
            /* QL70: No crear colección para carpetas de sistema (sin colección, duplicados).
             * Archivos de audio dentro de ellas sí se suben (procesados abajo). */
            if (CARPETAS_SISTEMA_NO_COLECCION.has(relativa.toLowerCase())) {
                continue;
            }
            /* Es un path directo bajo carpetaBase sin extensión de audio → posible carpeta */
            procesarEventoCarpeta(tipo, normalizada, relativa, baseNormalizada);
            continue;
        }

        /*
         * C387: Detectar eventos de subcarpetas de nivel 2 (subcolecciones).
         * Exactamente 2 segmentos en la ruta relativa: carpetaPadre/nombreSub.
         * Ej: "MiColeccion/Kicks" → carpetaPadre="MiColeccion", nombreSub="Kicks".
         * QL62: Misma proteccion que nivel 1 — ignorar si tiene extension conocida.
         */
        if (segmentosRuta.length === 2 && !EXTENSIONES_AUDIO.has(extension)) {
            if (extension && (EXTENSIONES_ARCHIVO_CONOCIDAS.has(extension) || pareceArchivoConExtension(segmentosRuta[1]))) {
                logSync.warn('watcher', `Archivo no-audio ignorado en subcarpeta: ${relativa}`);
                continue;
            }
            if (esEventoCreacion(tipo) && !await esDirectorioReal(normalizada)) {
                logSync.warn('watcher', `Archivo sin extension ignorado en subcarpeta (no es carpeta real): ${relativa}`);
                continue;
            }
            const [carpetaPadre, nombreSubcarpeta] = segmentosRuta;
            procesarEventoSubcarpeta(tipo, normalizada, nombreSubcarpeta, carpetaPadre, baseNormalizada);
            continue;
        }

        /* Verificar extensión de audio para archivos normales */
        if (!EXTENSIONES_AUDIO.has(extension)) continue;

        /* Debounce por archivo: ignorar si fue procesado recientemente */
        const ahora = Date.now();
        const ultimoProcesado = estadoFileWatcher.archivosRecientes.get(normalizada);
        if (ultimoProcesado && (ahora - ultimoProcesado) < DEBOUNCE_ARCHIVO_MS) continue;
        estadoFileWatcher.archivosRecientes.set(normalizada, ahora);

        if (esEventoCreacion(tipo)) {
            manejarArchivoNuevo(ruta, normalizada, carpetaBase);
        } else if (esEventoEliminacion(tipo)) {
            /*
             * Para eliminaciones, limpiar del debounce cache: si luego llega un
             * create (move), no debe ser bloqueado por el debounce del delete previo.
             */
            estadoFileWatcher.archivosRecientes.delete(normalizada);
            manejarArchivoEliminado(ruta);
        }
    }
}

/*
 * C287: Maneja rename events con un solo path (From/To separados).
 *
 * En Windows, notify-rs puede emitir RenameMode::From y RenameMode::To
 * como eventos independientes con modify.kind='name'. Tauri descarta
 * el RenameMode, así que ambos llegan como { modify: { kind: 'name' } }
 * con paths.length=1. Sin este handler, caen al for loop donde
 * esEventoCreacion y esEventoEliminacion no los reconocen → se ignoran.
 *
 * Flujo:
 *   1. Primer path (From, viejo) → buffear con timeout.
 *   2. Segundo path (To, nuevo) → parear con buffer → re-despachar
 *      como rename con 2 paths (procesarEvento recursivo).
 *   3. Si solo llega 1 path (timeout sin par) → despachar como DELETE
 *      sintético para que el patrón delete+create existente lo detecte
 *      cuando llegue un CREATE posterior.
 */
function manejarRenameNoPareado(
    ruta: string,
    baseNormalizada: string,
    carpetaBase: string,
): void {
    const normalizada = ruta.replace(/\\/g, '/');
    const relativa = normalizada.startsWith(baseNormalizada + '/')
        ? normalizada.slice(baseNormalizada.length + 1)
        : '';
    if (!relativa) return;

    const segmentos = relativa.split('/').filter(Boolean);
    if (segmentos.some(s => CARPETAS_EXCLUIDAS_TOTAL.has(s))) return;

    if (renamePendienteNoPareado) {
        /* Segundo path — parear con el primero y re-despachar como rename pareado */
        clearTimeout(renamePendienteNoPareado.timeout);
        const origenRuta = renamePendienteNoPareado.ruta;
        setRenamePendienteNoPareado(null);

        logSync.info('watcher', `Rename pareado desde eventos separados: ${origenRuta} → ${ruta}`);

        /* Re-despachar: el handler de rename pareado (2 paths) resuelve nivel 1, 2 y archivos */
        void procesarEvento(
            { type: { modify: { kind: 'name' } }, paths: [origenRuta, ruta] },
            carpetaBase,
        );
    } else {
        /* Primer path — buffear esperando el segundo */
        const timeout = setTimeout(() => {
            const pendiente = renamePendienteNoPareado;
            setRenamePendienteNoPareado(null);
            if (!pendiente) return;

            /*
             * Sin par: despachar como DELETE sintético.
             * Si luego llega un CREATE (nuevo nombre), el patrón delete+create
             * en procesarEventoCarpeta/Subcarpeta detectará el rename.
             */
            const norm = pendiente.ruta.replace(/\\/g, '/');
            const rel = norm.startsWith(pendiente.baseNormalizada + '/')
                ? norm.slice(pendiente.baseNormalizada.length + 1)
                : '';
            if (!rel) return;

            const segs = rel.split('/').filter(Boolean);
            const tipoRemove = { remove: { kind: 'any' } };

            if (segs.length === 1 && !EXTENSIONES_AUDIO.has(segs[0].split('.').pop()?.toLowerCase() ?? '')) {
                logSync.info('watcher', `Rename sin par → dispatch como eliminación carpeta: ${rel}`);
                procesarEventoCarpeta(tipoRemove, norm, rel, pendiente.baseNormalizada);
            } else if (segs.length === 2 && !EXTENSIONES_AUDIO.has(segs[1].split('.').pop()?.toLowerCase() ?? '')) {
                logSync.info('watcher', `Rename sin par → dispatch como eliminación subcarpeta: ${rel}`);
                procesarEventoSubcarpeta(tipoRemove, norm, segs[1], segs[0], pendiente.baseNormalizada);
            }
        }, GRACIA_RENAME_PAR_MS);

        setRenamePendienteNoPareado({
            ruta,
            timestamp: Date.now(),
            timeout,
            baseNormalizada,
            carpetaBase,
        });
    }
}
