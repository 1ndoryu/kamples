/*
 * Constantes del watcher de carpeta de sincronización (fileWatcherService).
 * Sin estado mutable: solo valores fijos y umbrales de debounce/gracia.
 */

export const EXTENSIONES_AUDIO = new Set([
    'wav', 'mp3', 'flac', 'aiff', 'aif', 'ogg',
]);

/*
 * QL62: Extensiones conocidas de archivos no-audio.
 * Si un path de nivel 1 tiene alguna de estas extensiones, NO es una carpeta,
 * es un archivo suelto que el usuario colocó en la raíz de la carpeta sync.
 * Esto previene que "READ ME!.txt" o "PandaFX.gif" se traten como colecciones.
 */
export const EXTENSIONES_ARCHIVO_CONOCIDAS = new Set([
    'txt', 'md', 'pdf', 'doc', 'docx', 'xls', 'xlsx', 'ppt', 'pptx', 'csv',
    'gif', 'png', 'jpg', 'jpeg', 'webp', 'bmp', 'svg', 'ico', 'tiff', 'psd', 'ai',
    'exe', 'msi', 'dmg', 'app', 'bat', 'sh', 'ps1', 'cmd',
    'zip', 'rar', 'tar', 'gz', '7z', 'bz2', 'xz', 'iso',
    'json', 'xml', 'html', 'htm', 'css', 'js', 'ts', 'jsx', 'tsx', 'py', 'rb', 'php',
    'avi', 'mov', 'mp4', 'mkv', 'wmv', 'flv', 'webm', 'm4v',
    'mid', 'midi', 'sf2', 'sfz', 'vst', 'dll', 'so', 'dylib',
    'als', 'flp', 'logic', 'cpr', 'ptx', 'rpp',
    'torrent', 'lnk', 'url', 'desktop',
    'log', 'bak', 'old', 'orig', 'swp', 'swo',
]);

/* Archivos temporales que los editores/DAWs crean durante grabación */
export const PATRONES_TEMPORALES = [
    /^\./, /~$/, /\.tmp$/i, /\.part$/i, /\.crdownload$/i,
    /\.download$/i, /Thumbs\.db$/i, /desktop\.ini$/i,
];

/*
 * P1+P7: Carpetas internas excluidas del watcher.
 * CARPETAS_EXCLUIDAS_TOTAL: se ignora cualquier evento (create, delete, modify).
 * CARPETAS_SISTEMA_NO_COLECCION: carpetas que permiten subida de archivos de audio
 *   pero NO deben generar colecciones en el servidor. El watcher procesa CREATEs
 *   de archivos pero bloquea eventos de creacion de carpeta para estos nombres.
 *   QL70: "Sin colección" y "duplicados" ahora suben archivos al servidor.
 * El filtro se aplica por segmento de ruta (split('/')) para evitar falsos
 * positivos con nombres como "carpeta.papelera-mix/".
 */
export const CARPETAS_EXCLUIDAS_TOTAL = new Set([
    '.papelera',
]);

/*
 * QL70: Carpetas del sistema sync que permiten subida de archivos pero NO deben
 * generar colecciones. En manejarArchivoNuevo se descartan estos nombres de
 * la lista de carpetas para que el server los trate como "sin colección".
 */
export const CARPETAS_SISTEMA_NO_COLECCION = new Set([
    'sin colección',
    'sin coleccion',
    'duplicados',
]);

/* Cache de archivos recientemente procesados para ignorar eventos duplicados */
export const DEBOUNCE_ARCHIVO_MS = 3000;

/* Cache de carpetas recientemente procesadas para ignorar eventos duplicados.
 * Evita que eventos create + modify sobre la misma carpeta disparen
 * múltiples llamadas a onCarpetaNueva (causa raíz de duplicación de colecciones). */
export const DEBOUNCE_CARPETA_MS = 5000;

/*
 * Carpetas pendientes de creacion: cuando Windows crea una carpeta nueva,
 * la nombra "Nueva carpeta" y luego el usuario la renombra.
 * Si disparamos onCarpetaNueva inmediatamente, creamos una coleccion con
 * nombre incorrecto en el servidor. Delay de 3s da tiempo al rename.
 * Si el rename llega antes del timeout, cancelamos y usamos el nombre final.
 */
export const DELAY_CREACION_CARPETA_MS = 3000;

/*
 * C1: Nombres de carpetas temporales que los sistemas operativos crean
 * al hacer "Nueva carpeta". NUNCA deben generar colecciones — el timeout
 * se extiende a 60s (tiempo suficiente para que el usuario renombre).
 * Si no se renombra en 60s, se ignora silenciosamente.
 */
export const NOMBRES_CARPETA_TEMPORAL = new Set([
    'nueva carpeta',
    'new folder',
    'nuevo directorio',
    'untitled folder',
    'sans titre',
    'neuer ordner',
]);
export const DELAY_CARPETA_TEMPORAL_MS = 60_000;

/*
 * QL76: Buffer de acumulacion para copias masivas.
 * Cuando el usuario copia/mueve muchos archivos a la carpeta de sync,
 * el sistema acumulaba cada deteccion y procesaba inmediatamente (hash I/O + upload),
 * compitiendo por I/O con la copia en curso y ralentizando el sistema.
 *
 * Solucion: acumular archivos nuevos en buffer y procesarlos despues de un
 * periodo de quietud (sin nuevos archivos detectados). Si siguen llegando
 * archivos, el timer se reinicia. Hay un tope maximo para no bloquear
 * indefinidamente en copias muy grandes.
 */
export const QUIETUD_BATCH_MS = 5_000;
export const MAX_ESPERA_BATCH_MS = 30_000;

/* Purga periódica del cache para evitar crecimiento ilimitado en batches grandes */
export const PURGA_INTERVALO_MS = 10_000;
export const PURGA_TTL_MS = 30_000;

/*
 * Gracia para detección de MOVEs.
 * Un MOVE genera DELETE + CREATE. Bufferamos el DELETE por GRACIA_MOVE_MS
 * y si aparece un CREATE con el mismo nombre, se trata como move.
 */
export const GRACIA_MOVE_MS = 5000;

/*
 * C287: Gracia para parear los dos rename events (From/To) de Windows.
 * Si no llega el par en este tiempo, se despacha como DELETE sintético.
 */
export const GRACIA_RENAME_PAR_MS = 2000;

export const GRACIA_RENAME_CARPETA_MS = 3000;
