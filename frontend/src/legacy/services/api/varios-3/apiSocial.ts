/*
 * Service: apiSocial — Kamples
 * Funciones de red social: follows, likes, publicaciones.
 * Conecta directamente con la API sin fallback a mock.
 */

import { apiGet, apiPost, apiDelete, apiPostFormData, apiPut } from '../varios-1/apiCliente';
import type { RespuestaApi } from '../varios-1/apiCliente';
import type { Publicacion, Comentario, TipoReaccion } from '../../../types';
import { normalizarListaPublicaciones, normalizarPublicacion } from '../../normalizers/postNormalizer';
import { normalizarComentario, normalizarListaComentarios } from '../../normalizers/commentNormalizer';

type RespuestaListadoPublicacionesRaw = {
    items?: unknown[];
    data?: unknown[];
    page?: number;
    per_page?: number;
};

const normalizarRespuestaListadoPublicaciones = (
    res: RespuestaApi<RespuestaListadoPublicacionesRaw | Publicacion[]>
): RespuestaApi<{ data: Publicacion[]; page: number }> => {
    if (!res.ok || !res.data) {
        return res as unknown as RespuestaApi<{ data: Publicacion[]; page: number }>;
    }

    const raw = res.data as RespuestaListadoPublicacionesRaw;
    const items = raw.items ?? raw.data ?? res.data;

    return {
        ...res,
        data: {
            data: normalizarListaPublicaciones(items),
            page: Number(raw.page ?? 1),
        },
    };
};

/* Follows */

export const seguirUsuario = async (usuarioId: number): Promise<RespuestaApi<{ seguido: boolean }>> => {
    return apiPost<{ seguido: boolean }>(`/follow/${usuarioId}`);
};

export const dejarDeSeguir = async (usuarioId: number): Promise<RespuestaApi<{ seguido: boolean }>> => {
    return apiDelete<{ seguido: boolean }>(`/follow/${usuarioId}`);
};

/* Lista de IDs de usuarios seguidos (para filtro "solo seguidos") */
export const obtenerMisSeguidos = async (): Promise<RespuestaApi<{ id: number }[]>> => {
    return apiGet<{ id: number }[]>('/me/seguidos');
};

/* QQ32: Resumen de seguidor para el modal */
export interface SeguidorResumen {
    id: number;
    username: string;
    nombreVisible: string;
    avatarUrl: string | null;
    siguiendo: boolean;
}

/* QQ32: Lista paginada de seguidores de un usuario */
export const obtenerSeguidores = async (
    username: string,
    page = 1,
    perPage = 20
): Promise<RespuestaApi<SeguidorResumen[]>> => {
    return apiGet<SeguidorResumen[]>(
        `/usuarios/${encodeURIComponent(username)}/seguidores`,
        { page, perPage }
    );
};

/* Tipos de contenido que soportan likes y comentarios */
export type TipoLikeable = 'sample' | 'publicacion' | 'cancion' | 'relacion';
export type TipoComentable = 'sample' | 'publicacion' | 'cancion' | 'relacion' | 'articulo';

/* Likes / Reacciones */

export const darLike = async (
    tipo: TipoLikeable,
    targetId: number,
    reaccion: TipoReaccion = 'like'
): Promise<RespuestaApi<{ liked: boolean; reaccion: TipoReaccion }>> => {
    /* Backend espera snake_case: target_id */
    return apiPost<{ liked: boolean; reaccion: TipoReaccion }>(`/like`, { tipo, target_id: targetId, reaccion });
};

export const quitarLike = async (
    tipo: TipoLikeable,
    targetId: number
): Promise<RespuestaApi<{ liked: boolean; reaccion: null }>> => {
    /* DELETE /like espera body con tipo y target_id (snake_case) */
    return apiDelete<{ liked: boolean; reaccion: null }>('/like', { tipo, target_id: targetId });
};

/* Publicaciones */

export const crearPublicacion = async (datos: {
    tipo: 'social' | 'sample';
    contenido: string;
    imagenes?: string[];
    samplesAdjuntos?: number[];
}): Promise<RespuestaApi<Publicacion>> => {
    const res = await apiPost<Record<string, unknown>>('/publicaciones', datos);
    if (!res.ok || !res.data) return res as unknown as RespuestaApi<Publicacion>;
    return { ...res, data: normalizarPublicacion((res.data as Record<string, unknown>).post ?? res.data) };
};

export const listarPublicaciones = async (
    params?: { filtro?: 'todos' | 'siguiendo' | 'populares'; page?: number; perPage?: number }
): Promise<RespuestaApi<{ data: Publicacion[]; page: number }>> => {
    const res = await apiGet<RespuestaListadoPublicacionesRaw>('/publicaciones', {
        filtro: params?.filtro,
        page: params?.page,
        per_page: params?.perPage,
    });
    return normalizarRespuestaListadoPublicaciones(res);
};

/* Listar publicaciones de un usuario específico (para tab perfil) */
export const listarPublicacionesUsuario = async (
    username: string,
    page = 1
): Promise<RespuestaApi<{ data: Publicacion[]; page: number }>> => {
    const res = await apiGet<RespuestaListadoPublicacionesRaw>('/publicaciones', { autor: username, page });
    return normalizarRespuestaListadoPublicaciones(res);
};

export const obtenerFeedInicio = async (page = 1): Promise<RespuestaApi<Publicacion[]>> => {
    return apiGet<Publicacion[]>('/feed/inicio', { page });
};

/* Comentarios */

/* C-comment-fix: backend devuelve { items, page, per_page } — extraer items y normalizar */
export const obtenerComentarios = async (
    tipo: TipoComentable,
    targetId: number,
    page = 1
): Promise<RespuestaApi<Comentario[]>> => {
    const res = await apiGet<Record<string, unknown>>(`/comentarios/${tipo}/${targetId}`, { page });
    if (!res.ok || !res.data) return res as unknown as RespuestaApi<Comentario[]>;
    const raw = res.data as Record<string, unknown>;
    const items = Array.isArray(raw.items) ? raw.items : Array.isArray(raw) ? raw : [];
    return { ...res, data: normalizarListaComentarios(items) };
};

/* C-comment-fix: backend devuelve { ok, comment } — extraer comment y normalizar.
 * BUG: backend espera parent_id (snake_case) en JSON, no parentId (camelCase).
 * multipart ya acepta ambos nombres en payload.rs. */
export const crearComentario = async (
    tipo: TipoComentable,
    targetId: number,
    contenido: string,
    parentId?: number
): Promise<RespuestaApi<Comentario>> => {
    const body: Record<string, unknown> = { contenido };
    if (parentId) body.parent_id = parentId;
    const res = await apiPost<Record<string, unknown>>(`/comentarios/${tipo}/${targetId}`, body);
    if (!res.ok || !res.data) return res as unknown as RespuestaApi<Comentario>;
    const raw = res.data as Record<string, unknown>;
    return { ...res, data: normalizarComentario(raw.comment ?? raw) };
};

/* C130: Crear comentario multimedia (imagen o audio).
 * Envía FormData con el archivo + contenido opcional (caption).
 * C-comment-fix: extraer .comment del wrapper { ok, comment } y normalizar.
 */
export const crearComentarioMultimedia = async (
    tipo: TipoComentable,
    targetId: number,
    tipoContenido: 'imagen' | 'audio',
    archivo: File,
    contenido?: string,
    parentId?: number
): Promise<RespuestaApi<Comentario>> => {
    const formData = new FormData();
    formData.append('tipoContenido', tipoContenido);
    formData.append('media', archivo);
    if (contenido) formData.append('contenido', contenido);
    if (parentId) formData.append('parentId', String(parentId));
    const res = await apiPostFormData<Record<string, unknown>>(`/comentarios/${tipo}/${targetId}`, formData);
    if (!res.ok || !res.data) return res as unknown as RespuestaApi<Comentario>;
    const raw = res.data as Record<string, unknown>;
    return { ...res, data: normalizarComentario(raw.comment ?? raw) };
};

/* C264: Editar comentario (solo autor).
 * C-comment-fix: backend devuelve { ok, comment } — extraer comment y normalizar.
 * updated_at (snake_case) → editadoAt (camelCase). */
export const editarComentario = async (
    id: number,
    contenido: string
): Promise<RespuestaApi<{ contenido: string; editadoAt: string }>> => {
    const res = await apiPut<Record<string, unknown>>(`/comentarios/${id}`, { contenido });
    if (!res.ok || !res.data) return res as unknown as RespuestaApi<{ contenido: string; editadoAt: string }>;
    const raw = res.data as Record<string, unknown>;
    const comment = (raw.comment ?? raw) as Record<string, unknown>;
    return {
        ...res,
        data: {
            contenido: String(comment.contenido ?? contenido),
            editadoAt: String(comment.updated_at ?? comment.editadoAt ?? new Date().toISOString()),
        },
    };
};

/* C264: Eliminar comentario (autor o admin) */
export const eliminarComentario = async (id: number): Promise<RespuestaApi<{ ok: boolean }>> => {
    return apiDelete<{ ok: boolean }>(`/comentarios/${id}`);
};

/* C264: Reportar comentario */
export const reportarComentario = async (
    id: number,
    razon: string
): Promise<RespuestaApi<{ ok: boolean; message: string }>> => {
    return apiPost<{ ok: boolean; message: string }>(`/comentarios/${id}/reportar`, { razon });
};

/* C265: Like/unlike comentario.
 * C-comment-fix: backend devuelve { ok, liked, reaccion } — totalLikes viene del recount
 * pero no se incluye en LikeResponse. Usamos liked del backend directamente.
 * BUG: like_comment handler espera Json<CommentLikeRequest> — sin body, Axum devuelve 422.
 * Fix: enviar {} como body mínimo. */
export const darLikeComentario = async (
    id: number
): Promise<RespuestaApi<{ liked: boolean; totalLikes?: number }>> => {
    const res = await apiPost<Record<string, unknown>>(`/comentarios/${id}/like`, {});
    if (!res.ok || !res.data) return res as unknown as RespuestaApi<{ liked: boolean }>;
    const raw = res.data as Record<string, unknown>;
    return { ...res, data: { liked: Boolean(raw.liked), totalLikes: typeof raw.total_likes === 'number' ? raw.total_likes : undefined } };
};

export const quitarLikeComentario = async (
    id: number
): Promise<RespuestaApi<{ liked: boolean; totalLikes?: number }>> => {
    const res = await apiDelete<Record<string, unknown>>(`/comentarios/${id}/like`);
    if (!res.ok || !res.data) return res as unknown as RespuestaApi<{ liked: boolean }>;
    const raw = res.data as Record<string, unknown>;
    return { ...res, data: { liked: Boolean(raw.liked), totalLikes: typeof raw.total_likes === 'number' ? raw.total_likes : undefined } };
};

/* C265: Obtener respuestas de un comentario */
/* C-comment-fix: backend devuelve { items, limit } — extraer items y normalizar */
export const obtenerRespuestas = async (
    comentarioId: number
): Promise<RespuestaApi<Comentario[]>> => {
    const res = await apiGet<Record<string, unknown>>(`/comentarios/${comentarioId}/respuestas`);
    if (!res.ok || !res.data) return res as unknown as RespuestaApi<Comentario[]>;
    const raw = res.data as Record<string, unknown>;
    const items = Array.isArray(raw.items) ? raw.items : Array.isArray(raw) ? raw : [];
    return { ...res, data: normalizarListaComentarios(items) };
};

/* Reposts */

export const repostear = async (publicacionId: number): Promise<RespuestaApi<{ reposteado: boolean }>> => {
    return apiPost<{ reposteado: boolean }>(`/publicaciones/${publicacionId}/repost`);
};

export const quitarRepost = async (publicacionId: number): Promise<RespuestaApi<{ reposteado: boolean }>> => {
    return apiDelete<{ reposteado: boolean }>(`/publicaciones/${publicacionId}/repost`);
};

/* Obtener una publicación individual (para actualizar en tiempo real tras edición) */
export const obtenerPublicacion = async (id: number): Promise<RespuestaApi<Publicacion>> => {
    const res = await apiGet<Record<string, unknown>>(`/publicaciones/${id}`);
    if (!res.ok || !res.data) return res as unknown as RespuestaApi<Publicacion>;
    return { ...res, data: normalizarPublicacion((res.data as Record<string, unknown>).post ?? res.data) };
};

/* Subir imagen para publicación al servidor (evita blob:// URLs) */
export const subirImagenPublicacion = async (archivo: File): Promise<RespuestaApi<{ url: string }>> => {
    const formData = new FormData();
    formData.append('imagen', archivo);
    return apiPostFormData<{ url: string }>('/publicaciones/imagenes', formData);
};

/* C322: Reportar publicación */
export const reportarPublicacion = async (
    publicacionId: number,
    razon: string = 'contenido inapropiado'
): Promise<RespuestaApi<{ ok: boolean; message: string }>> => {
    return apiPost<{ ok: boolean; message: string }>(`/publicaciones/${publicacionId}/reportar`, { razon });
};

/* QQ23: Reportar usuario */
export const reportarUsuario = async (
    usuarioId: number,
    razon: string,
    detalles?: string,
): Promise<RespuestaApi<{ ok: boolean; message: string }>> => {
    return apiPost<{ ok: boolean; message: string }>(`/reportar-usuario/${usuarioId}`, { razon, detalles });
};

/* C126: Datos editables de una publicación */
export interface DatosActualizarPublicacion {
    contenido?: string;
    imagenes?: string[];
    samplesAdjuntos?: number[];
    moderacionEstado?: string; /* solo admin */
}

/*
 * C126: Actualizar publicación.
 * Solo el autor o admin pueden editar.
 */
export const actualizarPublicacion = async (
    publicacionId: number,
    datos: DatosActualizarPublicacion
): Promise<RespuestaApi<{ ok: boolean }>> => {
    return apiPut<{ ok: boolean }>(`/publicaciones/${publicacionId}`, datos);
};

/*
 * C126: Eliminar publicación.
 * Solo el autor o admin pueden eliminar.
 */
export const eliminarPublicacion = async (
    publicacionId: number
): Promise<RespuestaApi<{ eliminado: boolean }>> => {
    return apiDelete<{ eliminado: boolean }>(`/publicaciones/${publicacionId}`);
};
