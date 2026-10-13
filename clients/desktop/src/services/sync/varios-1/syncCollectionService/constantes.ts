/* Constantes del servicio de colecciones. Sin estado mutable: el estado de módulo vive en estado.ts. */

export const CARPETA_SIN_COLECCION = 'Sin colecci\u00f3n';
/* Nombre legacy que se escribió a disco cuando el source tenía encoding corrupto.
 * ó (U+00F3) en UTF-8 = bytes C3 B3 → interpretados como Windows-1252 = Ã (U+00C3) + ³ (U+00B3).
 * Usamos escapes Unicode explícitos para independizarnos del encoding del archivo. */
export const CARPETA_SIN_COLECCION_LEGACY = 'Sin colecci\u00c3\u00b3n';

/* Caracteres no válidos en nombres de carpeta Windows/macOS/Linux */
export const REGEX_CARACTERES_INVALIDOS = /[/\\:*?"<>|]/g;

/*
 * Gracia de presencia en servidor para samples recién registrados localmente.
 * Evita falsos borrados cuando el endpoint /me/sync/colecciones todavía no refleja
 * un sample nuevo (pipeline async, latencia de replicación o caché).
 */
export const GRACIA_PRESENCIA_SERVIDOR_MS = 15 * 60 * 1000;
export const MAX_REINTENTOS_CREAR_COLECCION = 4;
export const BACKOFF_BASE_CREAR_COLECCION_MS = 1200;

/* Margen minimo de seguridad: 500 MB libres despues de la descarga */
export const MARGEN_DISCO_BYTES = 500 * 1024 * 1024;
