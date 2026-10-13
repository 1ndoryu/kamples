/*
 * Service: apiSamples — Kamples
 * Funciones de acceso a datos de samples.
 * Conecta directamente con la API real, sin fallback a mock.
 */

import { apiGet, apiPost, apiPostFormData, apiDelete, apiPatch } from '../varios-1/apiCliente';
import type { RespuestaApi } from '../varios-1/apiCliente';
import type { SampleResumen, Sample } from '../../../types';
import type { CategoriaTag } from '../../varios-2/tagUtils';
import { normalizarTag } from '../../varios-2/tagUtils';
import {
    normalizarListaSamples,
    normalizarSampleDetalle,
    normalizarSampleResumen,
} from '../../normalizers/sampleNormalizer';

export interface PaginacionSamples {
    page: number;
    per_page: number;
    total: number;
    pages: number;
}

export interface RespuestaListaSamples {
    data: SampleResumen[];
    pagination: PaginacionSamples;
}

interface RespuestaListaSamplesRaw {
    data?: unknown[];
    pagination?: PaginacionSamples;
}

interface FeedResponseRaw {
    items?: unknown[];
    limit?: number;
    offset?: number;
    hay_mas?: boolean;
    hayMas?: boolean;
}

const FEED_PAGE_SIZE = 20;

const copiarMetaRespuesta = <T>(resp: RespuestaApi<unknown>, data: T): RespuestaApi<T> => ({
    ok: resp.ok,
    data,
    error: resp.error,
    status: resp.status,
    total: resp.total,
    hayMas: resp.hayMas,
});

const copiarErrorRespuesta = <T>(resp: RespuestaApi<unknown>): RespuestaApi<T> => ({
    ok: resp.ok,
    data: null,
    error: resp.error,
    status: resp.status,
    total: resp.total,
    hayMas: resp.hayMas,
});

const normalizarRespuestaListaSamples = (
    resp: RespuestaApi<RespuestaListaSamplesRaw>,
    fallbackPerPage: number,
    fallbackPage: number
): RespuestaApi<RespuestaListaSamples> => {
    if (!resp.ok || !resp.data) {
        return copiarErrorRespuesta(resp);
    }

    const pagination = resp.data.pagination ?? {
        page: fallbackPage,
        per_page: fallbackPerPage,
        total: normalizarListaSamples(resp.data.data).length,
        pages: 1,
    };

    return {
        ...copiarMetaRespuesta(resp, {
            data: normalizarListaSamples(resp.data.data),
            pagination,
        }),
        total: resp.total ?? pagination.total,
        hayMas: resp.hayMas ?? pagination.page < pagination.pages,
    };
};

const normalizarRespuestaListaSimple = (
    resp: RespuestaApi<unknown>
): RespuestaApi<SampleResumen[]> => {
    if (!resp.ok || !resp.data) {
        return copiarMetaRespuesta(resp, []);
    }

    const raw = resp.data as { data?: unknown[] } | unknown[];
    const items = Array.isArray(raw) ? raw : raw.data;
    return copiarMetaRespuesta(resp, normalizarListaSamples(items));
};

export interface FiltrosSamples {
    busqueda?: string;
    genero?: string;
    bpmMin?: number;
    bpmMax?: number;
    key?: string;
    tipo?: string;
    creador?: string;
    page?: number;
    perPage?: number;
}

/*
 * Lista samples con filtros y paginación.
 */
export const listarSamples = async (filtros: FiltrosSamples = {}): Promise<RespuestaApi<RespuestaListaSamples>> => {
    /* [193A-34] Enviar forma normalizada para que el backend busque sinónimos */
    const busquedaNorm = filtros.busqueda ? normalizarTag(filtros.busqueda) : undefined;
    const resp = await apiGet<RespuestaListaSamplesRaw>('/samples', {
        page: filtros.page ?? 1,
        per_page: filtros.perPage ?? 12,
        busqueda: filtros.busqueda,
        busqueda_norm: busquedaNorm && busquedaNorm !== filtros.busqueda?.toLowerCase().trim() ? busquedaNorm : undefined,
        genero: filtros.genero,
        bpm_min: filtros.bpmMin,
        bpm_max: filtros.bpmMax,
        key: filtros.key,
        tipo: filtros.tipo,
        creador: filtros.creador,
    });

    return normalizarRespuestaListaSamples(resp, filtros.perPage ?? 12, filtros.page ?? 1);
};

/*
 * Obtiene un sample individual por slug.
 */
export const obtenerSample = async (slug: string): Promise<RespuestaApi<Sample>> => {
    const resp = await apiGet<unknown>(`/samples/${slug}`);
    if (!resp.ok || !resp.data) {
        return copiarErrorRespuesta(resp);
    }
    return copiarMetaRespuesta(resp, normalizarSampleDetalle(resp.data));
};

/* [2103A-12] Obtiene un sample aleatorio del top 1000 del catálogo activo.  */
/* [223A-7] coleccionId opcional para aleatorio dentro de una colección (incluye subcolecciones) */
export const obtenerSampleAleatorio = async (coleccionId?: number): Promise<RespuestaApi<SampleResumen>> => {
    const resp = await apiGet<unknown>('/samples/aleatorio', coleccionId ? { coleccion_id: coleccionId } : undefined);
    if (!resp.ok || !resp.data) {
        return copiarErrorRespuesta(resp);
    }
    return copiarMetaRespuesta(resp, normalizarSampleResumen(resp.data));
};

/* [193A-82] Filtros backend para /feed — evitan filtrado client-side que causaba
 * conteos incorrectos y paginación rota. ocultarReproducidos se queda client-side
 * porque el historial vive en localStorage, no en la BD. */
export interface FiltrosFeedBackend {
    soloEncanta?: boolean;
    soloLike?: boolean;
    soloWav?: boolean;
    ocultarDescargados?: boolean;
    ocultarColeccionados?: boolean;
    ocultarLikeados?: boolean;
    soloDeSeguidos?: boolean;
}

/*
 * Obtiene el feed de descubrimiento con paginación.
 */
export const obtenerFeed = async (
    tipo: 'descubrir' | 'trending' | 'recientes' = 'descubrir',
    page = 1,
    busqueda = '',
    filtrosBackend: FiltrosFeedBackend = {}
): Promise<RespuestaApi<SampleResumen[]>> => {
    const params: Record<string, string | number> = {
        tipo,
        page,
        limit: FEED_PAGE_SIZE,
        offset: Math.max(0, (page - 1) * FEED_PAGE_SIZE),
    };
    if (busqueda.trim()) {
        params.busqueda = busqueda.trim();
        /* [193A-34] Enviar forma normalizada para sinónimos (guitarra→guitar, vocals→vocal) */
        const norm = normalizarTag(busqueda.trim());
        if (norm && norm !== busqueda.trim().toLowerCase()) {
            params.busqueda_norm = norm;
        }
    }
    /* [193A-82] Enviar filtros backend activos */
    if (filtrosBackend.soloEncanta) params.solo_encanta = 1;
    if (filtrosBackend.soloLike) params.solo_like = 1;
    if (filtrosBackend.soloWav) params.solo_wav = 1;
    if (filtrosBackend.ocultarDescargados) params.ocultar_descargados = 1;
    if (filtrosBackend.ocultarColeccionados) params.ocultar_coleccionados = 1;
    if (filtrosBackend.ocultarLikeados) params.ocultar_likeados = 1;
    if (filtrosBackend.soloDeSeguidos) params.solo_de_seguidos = 1;
    /* [193A-31] Debug score para admin: solo envía param si está activo en localStorage */
    if (typeof window !== 'undefined' && localStorage.getItem('kamples_debug_score') === '1') {
        params.debug = 1;
    }
    const resp = await apiGet<FeedResponseRaw>('/feed', params);
    if (!resp.ok || !resp.data) {
        return copiarMetaRespuesta(resp, []);
    }

    const raw = resp.data;
    const items = normalizarListaSamples(Array.isArray(raw) ? raw : raw.items);
    const hayMas = typeof raw.hay_mas === 'boolean'
        ? raw.hay_mas
        : typeof raw.hayMas === 'boolean'
            ? raw.hayMas
            : items.length >= FEED_PAGE_SIZE;

    return {
        ...copiarMetaRespuesta(resp, items),
        hayMas,
    };
};

/*
 * C4: Tags agregados con conteo, calculados en el servidor.
 * Soporta mismos filtros que listarSamples para faceted search.
 */
export type TagConConteo = { tag: string; conteo: number };
export type TagsAgregadosResp = Record<CategoriaTag, TagConConteo[]>;

export const obtenerTagsAgregados = async (
    filtros: Pick<FiltrosSamples, 'genero' | 'bpmMin' | 'bpmMax' | 'key' | 'tipo'> = {}
): Promise<RespuestaApi<TagsAgregadosResp>> => {
    return apiGet<TagsAgregadosResp>('/tags/aggregates', {
        genero: filtros.genero,
        bpm_min: filtros.bpmMin,
        bpm_max: filtros.bpmMax,
        key: filtros.key,
        tipo: filtros.tipo,
    });
};

/* Respuesta del endpoint de subida */
export interface RespuestaSubida {
    ok: boolean;
    sample_id: number | null;
    id_corto: string;
    slug: string;
    url: string;
    estado: string;
}

/* Datos para construir el FormData de subida */
export interface DatosSubida {
    audio: File;
    titulo?: string;
    contenido?: string;
    tags?: string[];
    permitirDescarga?: boolean;
    licenciaLibre?: boolean;
    esPremium?: boolean;
    precio?: number;
    /* C220: Toggle visibilidad en comunidad */
    mostrarEnComunidad?: boolean;
    /* C802c: Contexto de adjuncion a cancion/relacion de sampleo */
    cancionOrigenId?: number;
    relacionId?: number;
    ladoRelacion?: 'fuente' | 'destino';
    /* L7.2: Timing de inicio del sample en la cancion (segundos) */
    inicioSegundos?: number;
    /* Tipo de elemento sampleado (hook_riff, vocals, etc.) */
    tipoElemento?: string;
    /* QQ90: Imagen de portada del sample */
    portada?: File;
}

/*
 * Sube un sample al backend via multipart/form-data.
 * Endpoint: POST /kamples/v1/samples/upload
 */
export const subirSample = async (datos: DatosSubida): Promise<RespuestaApi<RespuestaSubida>> => {
    const formData = new FormData();

    formData.append('audio', datos.audio, datos.audio.name);

    if (datos.titulo) formData.append('titulo', datos.titulo);
    if (datos.contenido) formData.append('contenido', datos.contenido);
    if (datos.tags && datos.tags.length > 0) {
        formData.append('tags', JSON.stringify(datos.tags));
    }
    formData.append('permitir_descarga', String(datos.permitirDescarga ?? true));
    formData.append('licencia_libre', String(datos.licenciaLibre ?? false));
    formData.append('es_premium', String(datos.esPremium ?? false));
    formData.append('mostrar_en_comunidad', String(datos.mostrarEnComunidad ?? true));
    if (datos.precio != null && datos.precio > 0) {
        formData.append('precio', String(datos.precio));
    }
    if (datos.cancionOrigenId) {
        formData.append('cancion_origen_id', String(datos.cancionOrigenId));
    }
    if (datos.relacionId && datos.ladoRelacion) {
        formData.append('relacion_id', String(datos.relacionId));
        formData.append('lado_relacion', datos.ladoRelacion);
    }
    if (datos.inicioSegundos != null && datos.inicioSegundos >= 0) {
        formData.append('inicio_segundos', String(datos.inicioSegundos));
    }
    if (datos.tipoElemento) {
        formData.append('tipo_elemento', datos.tipoElemento);
    }
    /* QQ90: Portada del sample */
    if (datos.portada) {
        formData.append('portada', datos.portada, datos.portada.name);
    }

    return apiPostFormData<RespuestaSubida>('/samples/upload', formData);
};

/*
 * Eliminar un sample.
 * Solo el propietario o un admin pueden borrar.
 * Endpoint: DELETE /samples/{slug} — acepta slug o id_corto, NO id numérico.
 */
export const eliminarSample = async (slug: string): Promise<RespuestaApi<{ eliminado: boolean }>> => {
    return apiDelete<{ eliminado: boolean }>(`/samples/${slug}`);
};

/* D8: Subir/reemplazar imagen de portada de un sample */
export const subirImagenSample = async (
    sampleId: number,
    archivo: File
): Promise<RespuestaApi<{ imagenUrl: string }>> => {
    const fd = new FormData();
    fd.append('imagen', archivo);
    return apiPostFormData<{ imagenUrl: string }>(`/samples/${sampleId}/imagen`, fd);
};

/* C126: Datos editables de un sample */
export interface DatosActualizarSample {
    titulo?: string;
    descripcion?: string;
    tags?: string[];
    tipo?: string;
    esPremium?: boolean;
    precio?: number | null;
    permitirDescarga?: boolean;
    licenciaLibre?: boolean;
    imagenUrl?: string;
    estado?: string; /* solo admin */
    verificado?: boolean; /* solo admin — C178 */
    /* C220: Toggle visibilidad en comunidad */
    mostrarEnComunidad?: boolean;
}

/*
 * C126: Actualizar metadatos de un sample.
 * Solo el propietario o admin pueden editar.
 * Endpoint: PATCH /samples/{slug} — el backend Rust acepta slug o id_corto, NO id numerico.
 * [254A-8a] Migrado de PUT /samples/{id} a PATCH /samples/{slug} para alinear
 *   con el contrato del backend Rust (`PATCH /api/samples/:slug`).
 */
export const actualizarSample = async (
    slug: string,
    datos: DatosActualizarSample
): Promise<RespuestaApi<Sample>> => {
    const resp = await apiPatch<unknown>(`/samples/${slug}`, datos);
    if (!resp.ok || !resp.data) {
        return copiarErrorRespuesta(resp);
    }
    return copiarMetaRespuesta(resp, normalizarSampleDetalle(resp.data));
};

/*
 * Samples publicados vinculados a una relación de sampleo (sample_fuente_id / sample_destino_id).
 * Usados en RelacionDetalleIsland para mostrar los samples generados.
 */
export const obtenerSamplesDeRelacion = async (
    relacionId: number
): Promise<RespuestaApi<SampleResumen[]>> => {
    const resp = await apiGet<unknown>(`/relaciones/${relacionId}/samples`);
    return normalizarRespuestaListaSimple(resp);
};

/*
 * Samples publicados extraídos de una canción concreta (cancion_origen_id = cancion.id).
 * Usados en CancionDetalleIsland para mostrar los samples generados del pipeline.
 */
export const obtenerSamplesDeCancion = async (
    slug: string
): Promise<RespuestaApi<SampleResumen[]>> => {
    const resp = await apiGet<unknown>(`/canciones/${encodeURIComponent(slug)}/samples`);
    return normalizarRespuestaListaSimple(resp);
};

/*
 * C87: Obtiene los samples favoritos (liked) del usuario autenticado.
 */
/* [223A-5] soloLike: filtra solo likes (sin encanta) */
export const obtenerMisFavoritos = async (page = 1, perPage = 20, orden = 'recientes', busqueda = '', soloEncanta = false, soloLike = false): Promise<RespuestaApi<RespuestaListaSamples>> => {
    const params: Record<string, string | number> = { page, per_page: perPage, orden };
    if (busqueda) params.busqueda = busqueda;
    if (soloEncanta) params.solo_encanta = 1;
    if (soloLike) params.solo_like = 1;
    const resp = await apiGet<RespuestaListaSamplesRaw>('/me/favoritos', params);
    return normalizarRespuestaListaSamples(resp, perPage, page);
};

/*
 * C87: Obtiene los samples descargados por el usuario autenticado.
 */
export const obtenerMisDescargas = async (page = 1, perPage = 20, orden = 'recientes'): Promise<RespuestaApi<RespuestaListaSamples>> => {
    const resp = await apiGet<RespuestaListaSamplesRaw>('/me/descargas', { page, per_page: perPage, orden });
    return normalizarRespuestaListaSamples(resp, perPage, page);
};

/*
 * C800: Corregir metadata IA de un sample extraído del pipeline.
 * El usuario admin envía instrucciones de corrección y el backend re-procesa.
 */
export interface RespuestaCorreccionIA {
    ok: boolean;
    mensaje: string;
    cambios?: Record<string, unknown>;
}

export const corregirMetadataIA = (
    sampleId: number,
    instrucciones: string
): Promise<RespuestaApi<RespuestaCorreccionIA>> =>
    apiPost<RespuestaCorreccionIA>(`/samples/${sampleId}/corregir-ia`, { instrucciones });

/*
 * QQ130: Extender recorte de un sample de extraccion.
 * Solo admin. Re-descarga de YouTube y re-corta con timing extendido.
 */
export interface RespuestaExtenderRecorte {
    ok: boolean;
    mensaje: string;
    duracion?: number;
    audioHash?: string | null;
}

export const extenderRecorte = (
    sampleId: number,
    segundosAntes: number,
    segundosDespues: number
): Promise<RespuestaApi<RespuestaExtenderRecorte>> =>
    apiPost<RespuestaExtenderRecorte>(`/samples/${sampleId}/extender-recorte`, {
        segundosAntes,
        segundosDespues,
    });

/*
 * QQ130-B: Generar sample del segmento siguiente al actual.
 * Solo admin. Crea un nuevo sample que empieza donde termina el actual.
 */
export interface RespuestaGenerarSiguiente {
    ok: boolean;
    mensaje: string;
    nuevoSampleId?: number;
}

export const generarSiguienteSample = (
    sampleId: number,
    duracion: number
): Promise<RespuestaApi<RespuestaGenerarSiguiente>> =>
    apiPost<RespuestaGenerarSiguiente>(`/samples/${sampleId}/generar-siguiente`, {
        duracion,
    });

/*
 * QK59: Restaurar recorte al timing original (antes de extensiones).
 * Solo admin.
 */
export interface RespuestaRestaurarRecorte {
    ok: boolean;
    mensaje: string;
    duracion?: number;
    audioHash?: string | null;
}

export const restaurarRecorte = (
    sampleId: number,
): Promise<RespuestaApi<RespuestaRestaurarRecorte>> =>
    apiPost<RespuestaRestaurarRecorte>(`/samples/${sampleId}/restaurar-recorte`, {});

/*
 * [2103A-16] Invalida el cache del algoritmo del usuario actual en el servidor.
 * El usuario verá resultados frescos en el próximo GET /feed.
 */
export const recargarCacheFeed = (): Promise<RespuestaApi<{ ok: boolean }>> =>
    apiPost<{ ok: boolean }>('/feed/recargar', {});
