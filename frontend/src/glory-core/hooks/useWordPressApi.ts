/*
 * Hook para consumir la REST API de WordPress/Glory con tipado fuerte.
 * Maneja autenticacion (nonce), cache basico, y estados de carga/error.
 *
 * IMPORTANTE: Si se pasa `options`, el consumidor DEBE memoizarlo con useMemo
 * o definirlo fuera del componente. De lo contrario se creara un objeto nuevo
 * en cada render y el hook re-fetcheara en bucle infinito.
 *
 * Uso:
 * const { data, isLoading, error, refetch } = useWordPressApi<ImageListResponse>('/glory/v1/images');
 */
/* Re-exports: el hook vive en useWordPressApiCore.ts */
export { useWordPressApi } from './useWordPressApiCore';
export { clearApiCache, invalidateApiCache } from '../utils/apiCache';
export { resetApiCredentials } from '../utils/wpCredentials';
