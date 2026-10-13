import axios from 'axios';
import type { AxiosRequestConfig } from 'axios';

const instance = axios.create({
  baseURL: import.meta.env.VITE_API_URL || 'http://localhost:3001',
});

/* Interceptor: agrega el token JWT a cada request si existe */
instance.interceptors.request.use((config) => {
  const token = localStorage.getItem('token');
  if (token) {
    config.headers.Authorization = `Bearer ${token}`;
  }
  return config;
});

/**
 * Mutator para Orval — envuelve axios para que los hooks generados
 * funcionen con React Query y cancellation.
 */
type OrvalRequestOptions = RequestInit & {
  params?: AxiosRequestConfig['params'];
  responseType?: AxiosRequestConfig['responseType'];
  timeout?: AxiosRequestConfig['timeout'];
};

function normalizeHeaders(headers?: HeadersInit): AxiosRequestConfig['headers'] {
  if (!headers) {
    return undefined;
  }
  if (headers instanceof Headers) {
    return Object.fromEntries(headers.entries());
  }
  if (Array.isArray(headers)) {
    return Object.fromEntries(headers);
  }
  return headers;
}

export const customInstance = <T>(
  urlOrConfig: string | AxiosRequestConfig,
  options?: OrvalRequestOptions,
): Promise<T> => {
  const requestConfig: AxiosRequestConfig = typeof urlOrConfig === 'string'
    ? (() => {
        const { body, headers, method, signal, ...restOptions } = options ?? {};
        return {
          ...(restOptions as AxiosRequestConfig),
          url: urlOrConfig,
          method: method as AxiosRequestConfig['method'],
          data: body,
          signal: signal ?? undefined,
          headers: normalizeHeaders(headers),
        };
      })()
    : urlOrConfig;

  const source = axios.CancelToken.source();
  const promise = instance({
    ...requestConfig,
    cancelToken: source.token,
  }).then(({ data, headers, status }) => ({
    data,
    headers: toFetchHeaders(headers),
    status,
  }) as T);

  // @ts-expect-error -- propiedad cancel para React Query
  promise.cancel = () => source.cancel('Query cancelado');

  return promise;
};

export default instance;

function toFetchHeaders(headers: AxiosRequestConfig['headers']): Headers {
  const normalized = new Headers();
  if (!headers) {
    return normalized;
  }

  const asObject = (typeof (headers as { toJSON?: () => unknown }).toJSON === 'function'
    ? (headers as { toJSON: () => unknown }).toJSON()
    : headers) as Record<string, unknown>;

  for (const [key, value] of Object.entries(asObject)) {
    if (value == null) {
      continue;
    }
    normalized.set(key, String(value));
  }

  return normalized;
}
