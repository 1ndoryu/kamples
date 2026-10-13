/* sentinel-disable-file limite-lineas
 * Justificacion: Cola central de subida con multiples guards de seguridad (race conditions,
 * dedup por hash, antispam, circuit breaker, backoff, semaforo, persistencia). Dividirlo
 * requiere refactoring mayor de estado compartido no relacionado a ninguna tarea actual. */
/*
 * Servicio: uploadQueueService — Cola de subida de samples al servidor.
 *
 * Gestiona la subida automática de archivos detectados por fileWatcherService.
 * Procesa en orden FIFO con reintentos, detección de duplicados por hash parcial,
 * y persistencia en Tauri Store para resistir cierres inesperados.
 *
 * Flujo por archivo:
 *   1. Verificar duplicado (hash parcial: primeros 8KB + últimos 8KB + tamaño)
 *   2. Extraer contexto de ruta (3 carpetas padre + nombre de archivo)
 *   3. Generar título y tags desde el contexto
 *   4. Construir FormData y POST /samples/upload
 *   5. Registrar en índice de syncService
 *   6. Notificar progreso a la UI
 *
 * Detección de duplicados:
 *   Hash parcial = SHA-256 de (primeros 8KB + últimos 8KB + tamaño en bytes)
 *   Esto es rápido (no lee el archivo completo) y suficientemente robusto
 *   para archivos de audio que difieren en contenido.
 */

// Barril: la implementación vive en ./uploadQueueService/. Los importadores usan esta ruta sin cambios.
import '../sync/varios-2/syncService';

export type { EstadoUpload, ItemUploadCola, ProgresoUpload, ResumenDebugUploadQueue } from './uploadQueueService/tipos';
export { inicializarUploadQueue, onProgresoUpload } from './uploadQueueService/init';
export { encolarArchivo } from './uploadQueueService/encolado';
export {
  obtenerEstadoCola,
  obtenerResumenDebugUploadQueue,
  reintentarItem,
  reintentarTodosConError,
  eliminarItemCola,
  limpiarCompletados,
  limpiarHashesConocidos,
  limpiarArchivosSubidosEnDisco,
} from './uploadQueueService/operaciones';
