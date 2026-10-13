/*
 * Service: API Canciones — Kamples
 * Endpoints de Sample Discovery: canciones, artistas, relaciones.
 * Los endpoints son públicos (información cultural abierta).
 */

import { apiGet, apiDelete, apiPost } from './apiCliente';
import type { RespuestaApi } from './apiCliente';
import type {
    Cancion,
    CancionDetalle,
    ArtistaDetalle,
    EstadisticaRelaciones,
    RelacionDetalleCompleta,
    SeccionMusica,
    RelacionSample,
    CancionArtista,
} from '@app/types/varios-1/cancion';
/* [166A-6] Llamada directa a la API Rust (/api/) sin pasar por WordPress REST.
 * Útil para endpoints migrados que ya no existen en el backend PHP. */
async function apiGetRust<T>(
    endpoint: string,
    params?: Record<string, string>,
): Promise<RespuestaApi<T>> {
    const queryStr =
        params && Object.keys(params).length > 0
            ? '?' + new URLSearchParams(params).toString()
            : '';
    try {
        const res = await fetch(`/api${endpoint}${queryStr}`, {
            credentials: 'same-origin',
            cache: 'no-store',
            headers: { Accept: 'application/json' },
        });
        const status = res.status;
        const text = await res.text();
        if (!text.trim()) {
            return { ok: false, data: null, error: 'Respuesta vacía', status };
        }
        let json: unknown;
        try {
            json = JSON.parse(text);
        } catch {
            return {
                ok: false,
                data: null,
                error: `JSON inválido: ${text.slice(0, 200)}`,
                status,
            };
        }
        if (!res.ok) {
            const errObj = json as Record<string, unknown> | undefined;
            return {
                ok: false,
                data: null,
                error: (errObj?.error as string) ?? `Error ${status}`,
                status,
            };
        }
        return { ok: true, data: json as T, error: null, status };
    } catch (err) {
        return {
            ok: false,
            data: null,
            error: err instanceof Error ? err.message : 'Error de red',
            status: 0,
        };
    }
}
/* [274A-4] Normalizador snake_case → camelCase para respuestas de canciones.
 * El backend Rust serializa con serde default (snake_case), pero los componentes
 * legacy esperan camelCase. Tolerante: si ya viene en camelCase, lo respeta.
 * [274A-6b] Extendido para normalizar RelacionSample y CancionDetalle completo. */

type RawObj = Record<string, unknown>;
const esObjeto = (v: unknown): v is RawObj =>
    v !== null && typeof v === 'object' && !Array.isArray(v);

const normalizarRelacion = (valor: unknown): RelacionSample => {
    const r = esObjeto(valor) ? valor : {};
    return {
        id: Number(r.id ?? 0),
        cancionDestinoId: Number(r.cancionDestinoId ?? r.cancion_destino_id ?? 0),
        cancionFuenteId: Number(r.cancionFuenteId ?? r.cancion_fuente_id ?? 0),
        whosampledId: (r.whosampledId ?? r.whosampled_id ?? null) as number | null,
        tipoRelacion: (r.tipoRelacion ?? r.tipo_relacion ?? 'sample') as RelacionSample['tipoRelacion'],
        tipoElemento: (r.tipoElemento ?? r.tipo_elemento ?? null) as RelacionSample['tipoElemento'],
        timingsDestino: (r.timingsDestino ?? r.timings_destino ?? []) as number[],
        timingsFuente: (r.timingsFuente ?? r.timings_fuente ?? []) as number[],
        apareceEnTodo: Boolean(r.apareceEnTodo ?? r.aparece_en_todo ?? false),
        sampleId: (r.sampleId ?? r.sample_id ?? null) as number | null,
        votosTotal: Number(r.votosTotal ?? r.votos_total ?? 0),
        votosPromedio: Number(r.votosPromedio ?? r.votos_promedio ?? 0),
        fuente: (r.fuente ?? 'comunidad') as RelacionSample['fuente'],
        verificada: Boolean(r.verificada ?? false),
        creadoAt: String(r.creadoAt ?? r.created_at ?? ''),
        cancionTitulo: (r.cancionTitulo ?? r.cancion_titulo ?? undefined) as string | undefined,
        cancionSlug: (r.cancionSlug ?? r.cancion_slug ?? undefined) as string | undefined,
        artistaNombre: (r.artistaNombre ?? r.artista_nombre ?? undefined) as string | undefined,
        artistaSlug: (r.artistaSlug ?? r.artista_slug ?? undefined) as string | undefined,
        cancionAnio: (r.cancionAnio ?? r.cancion_anio ?? undefined) as number | null | undefined,
        cancionImagenUrl: (r.cancionImagenUrl ?? r.cancion_imagen_url ?? undefined) as string | null | undefined,
        contribuidorId: (r.contribuidorId ?? r.contribuidor_id ?? undefined) as number | null | undefined,
        contribuidorUsername: (r.contribuidorUsername ?? r.contribuidor_username ?? undefined) as string | null | undefined,
    };
};

const normalizarArtista = (valor: unknown): CancionArtista => {
    const r = esObjeto(valor) ? valor : {};
    return {
        artistaId: Number(r.artistaId ?? r.artista_id ?? 0),
        nombre: String(r.nombre ?? ''),
        slug: String(r.slug ?? ''),
        rol: (r.rol ?? 'principal') as CancionArtista['rol'],
    };
};

const normalizarCancion = (valor: unknown): Cancion => {
    const r = esObjeto(valor) ? valor : {};
    return {
        id: Number(r.id ?? 0),
        titulo: String(r.titulo ?? ''),
        slug: String(r.slug ?? ''),
        artistaId: Number(r.artistaId ?? r.artista_id ?? 0),
        album: (r.album ?? null) as string | null,
        sello: (r.sello ?? null) as string | null,
        anio: (r.anio ?? null) as number | null,
        duracionSegundos: (r.duracionSegundos ?? r.duracion_segundos ?? null) as number | null,
        genero: (r.genero ?? null) as string | null,
        youtubeId: (r.youtubeId ?? r.youtube_id ?? null) as string | null,
        spotifyId: (r.spotifyId ?? r.spotify_id ?? null) as string | null,
        imagenUrl: (r.imagenUrl ?? r.imagen_url ?? null) as string | null,
        whosampledUrl: (r.whosampledUrl ?? r.whosampled_url ?? null) as string | null,
        bpm: (r.bpm ?? null) as number | null,
        tonalidad: (r.tonalidad ?? null) as string | null,
        metadata: (r.metadata ?? {}) as Record<string, unknown>,
        totalSampleada: Number(r.totalSampleada ?? r.total_sampleada ?? 0),
        totalSamplea: Number(r.totalSamplea ?? r.total_samplea ?? 0),
        creadoAt: String(r.creadoAt ?? r.created_at ?? ''),
        actualizadoAt: String(r.actualizadoAt ?? r.updated_at ?? ''),
        artistaNombre: (r.artistaNombre ?? r.artista_nombre ?? undefined) as string | undefined,
        artistaSlug: (r.artistaSlug ?? r.artista_slug ?? undefined) as string | undefined,
        liked: (r.liked ?? undefined) as boolean | undefined,
        totalLikes: (r.totalLikes ?? r.total_likes ?? undefined) as number | undefined,
        reaccion: (r.reaccion ?? null) as string | null,
        sampleAdjunto: (r.sampleAdjunto ?? r.sample_adjunto ?? null) as Cancion['sampleAdjunto'],
    };
};

const normalizarLista = <T>(
    resp: RespuestaApi<T[]>,
    fn: (v: unknown) => T,
): RespuestaApi<T[]> => ({
    ...resp,
    data: resp.data ? resp.data.map(fn) : null,
});

/* Listar canciones recientes */
export const listarCanciones = async (perPage = 20): Promise<RespuestaApi<Cancion[]>> =>
    normalizarLista(await apiGet<Cancion[]>('/canciones', { per_page: perPage }), normalizarCancion);

/* Listar canciones paginadas con total (QL21 — admin table) */
export const listarCancionesPaginado = async (
    pagina = 1,
    porPagina = 50
): Promise<RespuestaApi<Cancion[]>> =>
    normalizarLista(
        await apiGet<Cancion[]>('/canciones', { page: pagina, per_page: porPagina }),
        normalizarCancion,
    );

/* Buscar canciones por texto */
export const buscarCanciones = async (
    query: string,
    perPage = 20
): Promise<RespuestaApi<Cancion[]>> =>
    normalizarLista(
        await apiGet<Cancion[]>('/canciones/buscar', { q: query, per_page: perPage }),
        normalizarCancion,
    );

/* Canciones más sampleadas */
export const cancionesTopSampleadas = async (
    limit = 50
): Promise<RespuestaApi<Cancion[]>> =>
    normalizarLista(
        await apiGet<Cancion[]>('/canciones/top', { limit }),
        normalizarCancion,
    );

/* Detalle de canción con relaciones */
export const obtenerCancionDetalle = async (
    slug: string
): Promise<RespuestaApi<CancionDetalle>> => {
    const resp = await apiGet<CancionDetalle>(`/canciones/${encodeURIComponent(slug)}`);
    if (!resp.ok || !resp.data) return resp;
    const raw = resp.data as unknown as RawObj;
    const samplesDeRaw = (raw.samplesDe ?? raw.samples_de ?? []) as unknown[];
    const sampleadaEnRaw = (raw.sampleadaEn ?? raw.sampleada_en ?? []) as unknown[];
    const artistasRaw = (raw.artistas ?? []) as unknown[];
    return {
        ...resp,
        data: {
            cancion: normalizarCancion(raw.cancion ?? raw),
            artistas: artistasRaw.map(normalizarArtista),
            samplesDe: samplesDeRaw.map(normalizarRelacion),
            sampleadaEn: sampleadaEnRaw.map(normalizarRelacion),
        },
    };
};

/* [223A-4][223A-3-E][166A-6] Canción aleatoria con detalle completo para modal descubrimiento.
 * Migrado a Rust API. Acepta filtros opcionales de género y década (comma-separated).
 * Normaliza snake_case → camelCase para samplesDe/sampleadaEn. */
export const obtenerCancionAleatoria = async (
    generos?: string[],
    decadas?: number[]
): Promise<RespuestaApi<CancionDetalle>> => {
    const params: Record<string, string> = {};
    if (generos && generos.length > 0) params.generos = generos.join(',');
    if (decadas && decadas.length > 0) params.decadas = decadas.join(',');
    const resp = await apiGetRust<CancionDetalle>(
        '/canciones/aleatorio',
        Object.keys(params).length > 0 ? params : undefined,
    );
    if (!resp.ok || !resp.data) return resp;
    const raw = resp.data as unknown as RawObj;
    const samplesDeRaw = (raw.samplesDe ?? raw.samples_de ?? []) as unknown[];
    const sampleadaEnRaw = (raw.sampleadaEn ?? raw.sampleada_en ?? []) as unknown[];
    const artistasRaw = (raw.artistas ?? []) as unknown[];
    return {
        ...resp,
        data: {
            cancion: normalizarCancion(raw.cancion ?? raw),
            artistas: artistasRaw.map(normalizarArtista),
            samplesDe: samplesDeRaw.map(normalizarRelacion),
            sampleadaEn: sampleadaEnRaw.map(normalizarRelacion),
        },
    };
};

/* [223A-3-E][166A-6] Géneros distintos disponibles en el catálogo de canciones.
 * Migrado a Rust API. */
export const obtenerGenerosCanciones = (): Promise<RespuestaApi<string[]>> =>
    apiGetRust<string[]>('/canciones/generos');

/* Top artistas por canciones */
export const artistasTop = (
    limit = 50
): Promise<RespuestaApi<Array<{ id: number; nombre: string; slug: string; totalCanciones: number }>>> =>
    apiGet('/artistas/top', { limit });

/* Detalle de artista con canciones */
export const obtenerArtistaDetalle = (
    slug: string
): Promise<RespuestaApi<ArtistaDetalle>> =>
    apiGet<ArtistaDetalle>(`/artistas/${encodeURIComponent(slug)}`);

/* Estadísticas generales */
export const obtenerEstadisticasRelaciones = (): Promise<RespuestaApi<EstadisticaRelaciones>> =>
    apiGet<EstadisticaRelaciones>('/sample-discovery/estadisticas');

/*
 * Relación vinculada a un sample de Kamples (S4.4).
 * Retorna null si el sample no tiene relación con canciones.
 */
export const obtenerRelacionPorSampleId = (
    sampleId: number
): Promise<RespuestaApi<RelacionDetalleCompleta | null>> =>
    apiGet<RelacionDetalleCompleta | null>(`/sample-discovery/relacion/${sampleId}`);

/* Detalle completo de una relación de sampleo (ambas canciones + metadata) */
export const obtenerRelacionDetalle = (
    id: number
): Promise<RespuestaApi<RelacionDetalleCompleta>> =>
    apiGet<RelacionDetalleCompleta>(`/relaciones/${id}`);

/* Cadena de samples: A sampleó B sampleó C... (S4.5) */
export interface CadenaSamplesResp {
    cancion_raiz: Cancion;
    cadena: NodoCadena[];
}

export interface NodoCadena {
    id: number;
    cancion_fuente_id: number;
    cancion_destino_id: number;
    tipo_relacion: string;
    nivel: number;
    fuente_titulo: string;
    fuente_slug: string;
    fuente_artista: string;
    destino_titulo: string;
    destino_slug: string;
    destino_artista: string;
}

export const obtenerCadenaSamples = (
    slug: string,
    profundidad = 5
): Promise<RespuestaApi<CadenaSamplesResp>> =>
    apiGet<CadenaSamplesResp>(`/canciones/${encodeURIComponent(slug)}/cadena`, { profundidad });

/* C812: Feed paginado de canciones con ordenamiento */
export type OrdenFeedCanciones = 'inteligente' | 'top_sampleados' | 'hot';

export interface RespuestaFeedCanciones {
    ok: boolean;
    data: Cancion[];
    total: number;
    page: number;
}

export const feedCanciones = async (
    orden: OrdenFeedCanciones = 'inteligente',
    pagina = 1,
    porPagina = 20
): Promise<RespuestaApi<Cancion[]>> =>
    normalizarLista(
        await apiGet<Cancion[]>('/canciones/feed', { orden, page: pagina, per_page: porPagina }),
        normalizarCancion,
    );

/* QK18/QK22: Secciones estilo Spotify — multiples secciones con dedup en un request */
export const seccionesCanciones = async (
    porSeccion = 15
): Promise<RespuestaApi<SeccionMusica[]>> => {
    const resp = await apiGet<SeccionMusica[]>('/canciones/secciones', { por_seccion: porSeccion });
    if (!resp.ok || !resp.data) return resp;
    return {
        ...resp,
        data: resp.data.map((sec) => ({
            ...sec,
            canciones: sec.canciones ? sec.canciones.map(normalizarCancion) : sec.canciones,
        })),
    };
};

/* ── Endpoints de desarrollo (solo disponibles con WP_DEBUG = true) ── */

interface RespuestaPurga {
    ok: boolean;
    mensaje: string;
    tablas: string[];
}

interface RespuestaScraper {
    ok: boolean;
    mensaje: string;
    pid?: number;
    log?: string;
}

/* Trunca todas las tablas del módulo Sample Discovery */
export const devPurgarCanciones = (): Promise<RespuestaApi<RespuestaPurga>> =>
    apiDelete<RespuestaPurga>('/dev/canciones');

/* Lanza el spider indicado en segundo plano.
 * Si se pasa url (URL de WhoSampled), se usa el spider 'track' automáticamente. */
export const devEjecutarScraper = (
    spider: 'hot_samples' | 'browse_year' | 'track' = 'hot_samples',
    limit = 0,
    url = ''
): Promise<RespuestaApi<RespuestaScraper>> =>
    apiPost<RespuestaScraper>('/dev/scraper/run', { spider, limit, ...(url ? { url } : {}) });

interface RespuestaCola {
    ok: boolean;
    cola_vacia: boolean;
    mensaje: string;
    url?: string;
    tipo?: string;
    spider?: string;
    pid?: number;
}

/* Toma la URL pendiente más antigua de la cola y lanza el spider correspondiente. */
export const devProcesarDeCola = (): Promise<RespuestaApi<RespuestaCola>> =>
    apiPost<RespuestaCola>('/dev/scraper/cola', {});

/* Encolar extracción bilateral + lanzar pipeline + publicar para una relación */
interface RespuestaRecorte {
    ok: boolean;
    mensaje: string;
    encolados: number;
    publicados?: number;
    errores?: number;
    log?: string;
}

export const devGenerarRecorte = (
    relacionId: number
): Promise<RespuestaApi<RespuestaRecorte>> =>
    apiPost<RespuestaRecorte>('/dev/recorte/generar', { relacion_id: relacionId });

/* Publicar samples extraidos a traves del flujo estandar (PipelineAudio) */
interface RespuestaPublicacion {
    ok: boolean;
    publicados: number;
    errores: number;
    mensaje?: string;
    resultados?: Array<{
        cola_id: number;
        ok: boolean;
        sample_id?: number;
        id_corto?: string;
        error?: string;
    }>;
}

export const devPublicarExtracciones = (
    limit = 10
): Promise<RespuestaApi<RespuestaPublicacion>> =>
    apiPost<RespuestaPublicacion>('/dev/extraccion/publicar', { limit });
