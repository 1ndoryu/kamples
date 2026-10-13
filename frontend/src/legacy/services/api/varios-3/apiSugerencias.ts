/*
 * Servicio: apiSugerencias — Kamples (C140)
 * Endpoints de sugerencias "Más Ideas" para descargas y favoritos.
 * Patron idéntico a apiColecciones.obtenerSugerencias().
 */

import { apiGet } from '../varios-1/apiCliente';
import type { RespuestaApi } from '../varios-1/apiCliente';
import type { SampleResumen } from '@app/types';
import { normalizarListaSamples } from '../../normalizers/sampleNormalizer';

/*
 * Sugerencias basadas en los samples descargados del usuario.
 * Backend analiza tags/BPM/key de descargas y sugiere similares.
 */
export const obtenerSugerenciasDescargas = async (
    pagina = 1, limite = 20
): Promise<RespuestaApi<SampleResumen[]>> => {
    const resp = await apiGet<unknown>('/me/descargas/sugerencias', { pagina, limite });
    if (!resp.ok || !resp.data) {
        return { ok: resp.ok, data: [], error: resp.error, status: resp.status, total: resp.total, hayMas: resp.hayMas };
    }
    return {
        ok: true,
        data: normalizarListaSamples(resp.data),
        error: null,
        status: resp.status,
        total: resp.total,
        hayMas: resp.hayMas,
    };
};

/*
 * Sugerencias basadas en los samples favoritos del usuario.
 * Backend analiza tags/BPM/key de favoritos y sugiere similares.
 */
export const obtenerSugerenciasFavoritos = async (
    pagina = 1, limite = 20
): Promise<RespuestaApi<SampleResumen[]>> => {
    const resp = await apiGet<unknown>('/me/favoritos/sugerencias', { pagina, limite });
    if (!resp.ok || !resp.data) {
        return { ok: resp.ok, data: [], error: resp.error, status: resp.status, total: resp.total, hayMas: resp.hayMas };
    }
    return {
        ok: true,
        data: normalizarListaSamples(resp.data),
        error: null,
        status: resp.status,
        total: resp.total,
        hayMas: resp.hayMas,
    };
};
