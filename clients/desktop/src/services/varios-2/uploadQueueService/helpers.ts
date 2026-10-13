/* Helpers de la cola de subida: constantes, disponibilidad de archivo, hash parcial, texto/MIME y progreso. */

import { obtenerConfigSync } from '../../sync/varios-2/syncService';
import { logSync } from '../../sync/varios-1/syncLogger';
import { ItemUploadCola } from './tipos';
import { cola, callbackProgreso } from './uploadQueueEstado';

export const STORE_FILE = 'upload-queue.json';

/*
 * SEC-A3: Extensiones de audio validas para subida.
 * Solo estos formatos se aceptan — cualquier otro se rechaza antes de encolar.
 * Server-side tambien valida (defensa en profundidad).
 */
const EXTENSIONES_AUDIO_VALIDAS = new Set(['.wav', '.mp3', '.flac', '.aiff', '.aif', '.ogg']);

export function esExtensionAudioValida(nombreArchivo: string): boolean {
    const ultimoPunto = nombreArchivo.lastIndexOf('.');
    if (ultimoPunto < 0) return false;
    return EXTENSIONES_AUDIO_VALIDAS.has(nombreArchivo.substring(ultimoPunto).toLowerCase());
}

/*
 * Verifica si una ruta de archivo está dentro de la carpeta de sync configurada.
 * Normaliza separadores y casing para comparación segura en Windows.
 * Retorna false si no hay carpeta configurada (failsafe: bloquear).
 */
export function estaEnCarpetaSync(rutaArchivo: string): boolean {
    const config = obtenerConfigSync();
    if (!config?.carpetaLocal) return false;
    const carpetaNorm = config.carpetaLocal.replace(/\\/g, '/').toLowerCase();
    const rutaNorm = rutaArchivo.replace(/\\/g, '/').toLowerCase();
    return rutaNorm.startsWith(carpetaNorm);
}
export const STORE_KEY_COLA = 'upload_cola';
export const STORE_KEY_HASHES = 'upload_hashes_procesados';

export const MAX_REINTENTOS = 3;
export const BACKOFF_BASE_MS = 2000;

/*
 * Espera a que un archivo sea legible (no bloqueado por otro proceso).
 *
 * En Windows, los file watchers emiten CREATE antes de que la escritura finalice.
 * OneDrive y operaciones de copia mantienen un lock exclusivo hasta completar.
 * Intentar readFile inmediatamente causa "os error 32: being used by another process".
 *
 * Usa backoff exponencial corto: 300ms, 600ms, 1.2s, 2.4s, 4.8s (~9.3s total).
 * Si tras 5 intentos sigue bloqueado, retorna false para que el flujo
 * normal de reintentos de la cola lo maneje.
 */
const READINESS_MAX_INTENTOS = 5;
const READINESS_DELAY_MS = 300;

export async function esperarArchivoDisponible(rutaArchivo: string): Promise<boolean> {
    const { stat } = await import('@tauri-apps/plugin-fs');

    for (let intento = 0; intento < READINESS_MAX_INTENTOS; intento++) {
        try {
            const info = await stat(rutaArchivo);
            /*
             * stat() puede funcionar en archivos locked (solo lee metadata del FS).
             * Pero si el tamanio es 0, el archivo aun no tiene contenido visible.
             * Un tamanio > 0 es condicion necesaria (no suficiente) para readability.
             */
            if (info.size && info.size > 0) {
                /*
                 * Intentar lectura minima (1 byte) para confirmar que el lock de escritura
                 * se ha liberado. Esto es lo que realmente falla con os error 32.
                 */
                try {
                    const { readFile } = await import('@tauri-apps/plugin-fs');
                    await readFile(rutaArchivo, { offset: 0, length: 1 } as Parameters<typeof readFile>[1]);
                    return true;
                } catch {
                    /* Lock sigue activo, continuar esperando */
                }
            }
        } catch {
            /* Archivo no existe o inaccesible */
        }

        if (intento < READINESS_MAX_INTENTOS - 1) {
            const delay = READINESS_DELAY_MS * Math.pow(2, intento);
            await new Promise(resolve => setTimeout(resolve, delay));
        }
    }

    return false;
}

/*
 * Calcula un hash parcial del archivo para detección rápida de duplicados.
 * Lee solo: primeros 8KB + últimos 8KB + tamaño total.
 * Esto evita leer archivos completos de 50MB solo para verificar duplicados.
 */
export async function calcularHashParcial(rutaArchivo: string): Promise<string | null> {
    try {
        const { stat, readFile } = await import('@tauri-apps/plugin-fs');

        const info = await stat(rutaArchivo);
        const tamano = info.size;

        if (!tamano || tamano === 0) return null;

        const CHUNK_SIZE = 8192;

        /* Leer primeros 8KB */
        const inicio = await readFile(rutaArchivo, {
            offset: 0,
            length: Math.min(CHUNK_SIZE, tamano),
        } as Parameters<typeof readFile>[1]);

        /* Leer últimos 8KB */
        let fin: Uint8Array;
        if (tamano > CHUNK_SIZE) {
            fin = await readFile(rutaArchivo, {
                offset: tamano - CHUNK_SIZE,
                length: CHUNK_SIZE,
            } as Parameters<typeof readFile>[1]);
        } else {
            fin = new Uint8Array(0);
        }

        /* Concatenar: inicio + fin + tamaño como string */
        const tamanoBytes = new TextEncoder().encode(tamano.toString());
        const combinado = new Uint8Array(inicio.length + fin.length + tamanoBytes.length);
        combinado.set(inicio, 0);
        combinado.set(fin, inicio.length);
        combinado.set(tamanoBytes, inicio.length + fin.length);

        /* Hash con Web Crypto API (disponible en Tauri webview) */
        const hashBuffer = await crypto.subtle.digest('SHA-256', combinado);
        const hashArray = Array.from(new Uint8Array(hashBuffer));
        return hashArray.map(b => b.toString(16).padStart(2, '0')).join('');
    } catch (err) {
        logSync.warn('uploadQueueService', '[UploadQueue] No se pudo calcular hash parcial:', err);
        return null;
    }
}

/*
 * Convierte nombre de archivo a título legible.
 * "deep_sub_bass_hit" → "Deep Sub Bass Hit"
 * "808-kick-hard" → "808 Kick Hard"
 */
export function humanizarNombreArchivo(nombre: string): string {
    return nombre
        .replace(/[_-]+/g, ' ')
        .replace(/\s+/g, ' ')
        .trim()
        .split(' ')
        .map(palabra => {
            if (palabra.length === 0) return '';
            /* Mantener números y siglas en mayúsculas como están */
            if (/^\d+$/.test(palabra) || /^[A-Z]{2,}$/.test(palabra)) return palabra;
            return palabra.charAt(0).toUpperCase() + palabra.slice(1).toLowerCase();
        })
        .join(' ');
}

/*
 * Genera tags desde el contexto de la ruta.
 * Combina carpetas padre + palabras del nombre del archivo.
 * Garantiza mínimo 2 tags (requisito del endpoint de upload).
 *
 * Ejemplo: carpetas=['Samples', '808', 'Bass'], nombre='deep_sub_hit'
 * → ['808', 'bass', 'deep', 'sub', 'hit']
 */
export function generarTagsDesdeContexto(carpetas: string[], nombreArchivo: string): string[] {
    const tagsSet = new Set<string>();

    /* Tags de carpetas (ignorar carpetas genéricas) */
    const carpetasGenericas = new Set([
        'samples', 'audio', 'music', 'sounds', 'downloads',
        'descargas', 'general', 'sync', 'kamples',
        /* Carpetas del sistema operativo y rutas comunes */
        'users', 'user', 'owner', 'documents', 'documentos',
        'desktop', 'escritorio', 'onedrive', 'dropbox',
        'google drive', 'icloud', 'home', 'library',
        'appdata', 'local', 'roaming', 'program files',
        'applications', 'volumes', 'media', 'mnt', 'tmp',
    ]);

    for (const carpeta of carpetas) {
        const normalizada = carpeta.toLowerCase().trim();
        /* Filtrar: menores a 2 chars, genéricas, y letras de unidad (C:, D:) */
        const esDriveLabel = /^[a-z]:?$/.test(normalizada);
        if (normalizada.length >= 2 && !carpetasGenericas.has(normalizada) && !esDriveLabel) {
            tagsSet.add(normalizada);
        }
    }

    /*
     * Tags del nombre del archivo.
     * B4 fix: No agregar palabras sueltas que sean fragmentos de tags ya existentes
     * (ej: si "hip hop" ya es tag de carpeta, no agregar "hip" ni "hop" por separado).
     */
    const palabras = nombreArchivo
        .replace(/[_\-\.]+/g, ' ')
        .split(/\s+/)
        .map(p => p.toLowerCase().trim())
        .filter(p => p.length >= 2 && !/^\d+$/.test(p));

    const tagsCarpetaExistentes = Array.from(tagsSet);
    for (const palabra of palabras) {
        if (tagsSet.size >= 8) break;
        const esFragmento = tagsCarpetaExistentes.some(
            tag => tag.includes(' ') && tag.includes(palabra) && tag !== palabra
        );
        if (!esFragmento) {
            tagsSet.add(palabra);
        }
    }

    /* Garantizar mínimo 2 tags */
    const resultado = Array.from(tagsSet);
    if (resultado.length < 2) {
        if (resultado.length === 0) {
            resultado.push('sample', 'audio');
        } else {
            resultado.push('sample');
        }
    }

    return resultado;
}

/*
 * Determina el MIME type por extensión de archivo.
 */
export function obtenerMimeType(extension: string): string {
    const mapa: Record<string, string> = {
        wav: 'audio/wav',
        mp3: 'audio/mpeg',
        flac: 'audio/flac',
        aiff: 'audio/aiff',
        aif: 'audio/aiff',
        ogg: 'audio/ogg',
    };
    return mapa[extension.toLowerCase()] ?? 'audio/wav';
}

/*
 * Emite progreso al callback registrado.
 */
export function emitirProgreso(item: ItemUploadCola): void {
    if (!callbackProgreso) return;

    const pendientes = cola.filter(i => i.estado === 'pendiente' || i.estado === 'subiendo');
    const posicion = pendientes.indexOf(item);

    callbackProgreso({
        item,
        totalEnCola: pendientes.length,
        posicionEnCola: posicion >= 0 ? posicion + 1 : 0,
    });
}
