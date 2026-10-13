/*
 * Constantes del escaneo y de las carpetas de sistema, compartidas por varios módulos.
 * Módulo hoja: no importa de los demás módulos de syncWatcherSetup.
 */

/* Carpetas locales del sistema que NO deben crear colecciones en el servidor.
 * QL70: Archivos dentro de estas carpetas sí se escanean y encolan para subida,
 * pero con carpetas vacías para que el server los trate como "sin colección". */
export const CARPETAS_SISTEMA_SYNC = new Set([
    'sin colecci\u00f3n',
    'sin coleccion',
    'sin colecci\u00c3\u00b3n',
    'duplicados',
]);

/* Extensiones de audio reconocidas — debe mantenerse en sync con fileWatcherService.ts */
export const EXTENSIONES_AUDIO_SCAN = new Set(['wav', 'mp3', 'flac', 'aiff', 'aif', 'ogg']);

/*
 * Carpetas totalmente excluidas del escaneo local.
 * Espeja CARPETAS_EXCLUIDAS_TOTAL de fileWatcherService.ts.
 */
export const CARPETAS_EXCLUIDAS_SCAN = new Set(['.papelera']);
