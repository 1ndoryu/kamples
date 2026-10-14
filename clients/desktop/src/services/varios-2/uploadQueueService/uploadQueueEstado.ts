/* Estado de módulo compartido de la cola de subida, con setters y helpers de deduplicación. */

import { Semaforo } from '../semaforo';
import { estado } from '../../sync/varios-2/syncState';
import { ItemUploadCola, OnProgresoUploadFn } from './tipos';

export let cola: ItemUploadCola[] = [];

/*
 * Store agrupado de los Set/Map de la cola (regla Sentinel singleton-mutable-state:
 * no exportar colecciones mutables sueltas a nivel de módulo). Los consumidores usan
 * estadoUploadQueue.<nombre>; los setters de abajo reasignan propiedades.
 */
export const estadoUploadQueue = {
    hashesConocidos: new Set<string>(),
    /*
     * C6: Mapa hash → rutas para poder verificar si un hash corresponde a un archivo
     * activo en tracking (no borrado del servidor). Permite dedup cross-carpeta correcta.
     * 1:N porque un mismo contenido puede existir en múltiples rutas (copias).
     */
    hashARutas: new Map<string, Set<string>>(),
    /* Set O(1) para verificar si una ruta ya está en cola. Evita cola.some() O(n). */
    rutasEnCola: new Set<string>(),
    /*
     * Guard síncrono contra race condition en encolarArchivo().
     * Problema: encolarArchivo es async y el callback del watcher no hace await.
     * Si llegan dos eventos rápidos (create + modify), ambos entran en encolarArchivo,
     * ambos pasan rutasEnCola.has() (que se actualiza DESPUÉS del await calcularHash),
     * y ambos encolan el mismo archivo → duplicado.
     *
     * Solución: este Set se actualiza SÍNCRONAMENTE al inicio de encolarArchivo,
     * antes de cualquier await. El segundo llamado lo ve inmediatamente y retorna false.
     */
    rutasEncolando: new Set<string>(),
    /*
     * Guard de hash en vuelo para uploads paralelos.
     * Con archivosParalelos > 1, dos items con el mismo hash podrían ejecutar
     * subirArchivo() simultáneamente, ambos pasando hashesConocidos.has() antes
     * de que ninguno lo añada. Este Set previene que dos uploads del mismo
     * contenido estén en vuelo al mismo tiempo.
     */
    hashesEnVuelo: new Set<string>(),
    /*
     * Guard de ruta en vuelo: complementa hashesEnVuelo para cubrir el caso
     * donde hash es null (error de FS, archivo bloqueado, OneDrive).
     * Sin esto, dos items del mismo archivo con hash null pueden uploadear
     * en paralelo porque hashesEnVuelo no los cubre.
     */
    rutasEnVuelo: new Set<string>(),
    /*
     * TC2: Guard de hash pendiente para encolarArchivoInterno.
     * Problema: dos llamados concurrentes a encolarArchivoInterno calculan hash en paralelo
     * (await calcularHashParcial). Ambos pueden pasar hashesConocidos.has() y cola.some()
     * porque ninguno ha terminado de encolar aún. Este Set se marca SÍNCRONAMENTE después
     * del await del hash y antes de cola.push, cerrando la ventana de race.
     */
    hashesPendientesEncola: new Set<string>(),
    /* QL69: contador de detecciones por hash (antispam). Ver MAX_DETECCIONES_HASH. */
    contadorHashDetectado: new Map<string, number>(),
};

export let procesando = false;
export let callbackProgreso: OnProgresoUploadFn | null = null;

/*
 * QL69: Antispam — contador de detecciones por hash.
 * Si un mismo contenido (hash) se detecta mas de MAX_DETECCIONES_HASH veces,
 * el sistema deja de sincronizarlo. Previene que un archivo problematico
 * (ej: copia repetida, loop de watcher) sature la cola indefinidamente.
 * Se persiste con la cola para sobrevivir reinicios.
 * [2103A-11] Umbral subido de 6 a 25: con el watcher disparando múltiples eventos
 * por archivo, 6 era demasiado estricto para sample packs con muchos snares/kicks.
 * El contador ya no incrementa una vez alcanzado el límite (evita contadores de +20). */
export const MAX_DETECCIONES_HASH = 25;

export function registrarDeteccionHash(hash: string): number {
    const actual = (estadoUploadQueue.contadorHashDetectado.get(hash) ?? 0) + 1;
    /* [2103A-11] Capear en MAX para que el contador no crezca indefinidamente
     * cuando el orphan analysis re-descubre el mismo archivo continuamente. */
    const capeado = Math.min(actual, MAX_DETECCIONES_HASH);
    estadoUploadQueue.contadorHashDetectado.set(hash, capeado);
    return actual; // retornar actual (no capeado) para el log de bloqueo
}

export function esHashBloqueadoPorAntispam(hash: string): boolean {
    return (estadoUploadQueue.contadorHashDetectado.get(hash) ?? 0) >= MAX_DETECCIONES_HASH;
}

export function normalizarRutaCola(ruta: string): string {
    return ruta.replace(/\\/g, '/');
}

export function claveRutaEnCola(ruta: string): string {
    return normalizarRutaCola(ruta).toLowerCase();
}

function hashSimpleTexto(texto: string): string {
    let hash = 2166136261;
    for (let i = 0; i < texto.length; i++) {
        hash ^= texto.charCodeAt(i);
        hash += (hash << 1) + (hash << 4) + (hash << 7) + (hash << 8) + (hash << 24);
    }
    return (hash >>> 0).toString(16).padStart(8, '0');
}

/*
 * C6: Verifica si existe al menos un archivo activo en tracking que fue
 * subido con el hash dado. Usa hashARutas (1:N) para obtener todas las rutas
 * asociadas y verifica en tracking que alguna sigue activa (no borrada).
 * Si ninguna ruta está activa, recorre la cola completada como fallback.
 */
export function existeArchivoActivoConHash(hash: string): boolean {
    const rutas = estadoUploadQueue.hashARutas.get(hash);
    if (rutas && rutas.size > 0 && estado.trackingModule) {
        for (const ruta of rutas) {
            const enTracking = estado.trackingModule.buscarArchivoPorRuta(ruta);
            if (enTracking && !enTracking.syncDeshabilitado) return true;
        }
    }
    /* Fallback: verificar si algún item realmente subido en cola tiene este hash.
     * Requiere sampleIdServidor para descartar items marcados 'completado' por
     * falso positivo de dedup (no tienen server ID, no son uploads reales). */
    return cola.some(i => i.hashParcial === hash && i.estado === 'completado' && !!i.sampleIdServidor);
}

export function crearIdempotencyKeyDeterministica(rutaArchivo: string, nombreArchivo: string, hashParcial?: string): string {
    if (hashParcial && hashParcial.length > 0) {
        return `up-hash-${hashParcial.slice(0, 48)}`;
    }
    const rutaNorm = normalizarRutaCola(rutaArchivo);
    return `up-ruta-${hashSimpleTexto(`${rutaNorm}|${nombreArchivo.toLowerCase()}`)}`;
}

/* Semáforo de concurrencia: controla cuántos archivos se suben en paralelo.
 * El límite se lee de configAvanzada.archivosParalelos al iniciar procesarCola. */
export let semaforoUpload: Semaforo | null = null;
export function tienePendientes(): boolean {
    return cola.some(i => i.estado === 'pendiente');
}

/*
 * Setters del estado compartido. Los módulos importan los `let` como bindings vivos: los leen
 * pero no pueden reasignarlos, así que toda reasignación pasa por aquí.
 */
export function setCola(valor: ItemUploadCola[]): void { cola = valor; }
export function setRutasEnCola(valor: Set<string>): void { estadoUploadQueue.rutasEnCola = valor; }
export function setHashesConocidos(valor: Set<string>): void { estadoUploadQueue.hashesConocidos = valor; }
export function setHashARutas(valor: Map<string, Set<string>>): void { estadoUploadQueue.hashARutas = valor; }
export function setProcesando(valor: boolean): void { procesando = valor; }
export function setCallbackProgreso(valor: OnProgresoUploadFn | null): void { callbackProgreso = valor; }
export function setSemaforoUpload(valor: Semaforo | null): void { semaforoUpload = valor; }
