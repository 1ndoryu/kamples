/*
 * Servicio: apiMensajes — Kamples (Fase 5.2)
 * Gestión de conversaciones y mensajes del chat.
 * Soporta mensajes multimedia: texto, imagen, audio, sample.
 */

import { apiGet, apiPost, apiPostFormData, type RespuestaApi } from '../varios-1/apiCliente';
import { crearLogger } from '../../varios-1/logger';
import { normalizarConversacion, normalizarListaConversaciones, normalizarMensaje, normalizarListaMensajes } from '../../normalizers/messageNormalizer';
import type { Conversacion, Mensaje } from '../../../types';

const log = crearLogger('apiMensajes');

/* Obtener lista de conversaciones */
export const obtenerConversaciones = async (): Promise<RespuestaApi<Conversacion[]>> => {
    try {
        const resp = await apiGet<Conversacion[]>('/mensajes/conversaciones');
        if (resp.ok && resp.data) {
            resp.data = normalizarListaConversaciones(resp.data);
        }
        return resp;
    } catch (err) {
        log.error('Error obteniendo conversaciones', err);
        return { ok: false, data: [], error: 'Error de red', status: 500 };
    }
};

/* [183A-62] Obtener mensajes de una conversación (cursor-based).
 * Sin antesDeId: retorna los 50 más recientes.
 * Con antesDeId: retorna 50 mensajes anteriores al cursor. */
export const obtenerMensajes = async (
    conversacionId: number,
    antesDeId?: number
): Promise<RespuestaApi<Mensaje[]>> => {
    try {
        const params: Record<string, number> = {};
        if (antesDeId && antesDeId > 0) params.antes_de_id = antesDeId;
        const resp = await apiGet<Mensaje[]>(`/mensajes/${conversacionId}`, params);
        if (resp.ok && resp.data) {
            resp.data = normalizarListaMensajes(resp.data);
        }
        return resp;
    } catch (err) {
        log.error('Error obteniendo mensajes', err);
        return { ok: false, data: [], error: 'Error de red', status: 500 };
    }
};

/* Enviar mensaje de texto */
export const enviarMensajeTexto = async (
    conversacionId: number,
    contenido: string
): Promise<RespuestaApi<Mensaje>> => {
    try {
        const resp = await apiPost<Mensaje>(`/mensajes/${conversacionId}`, {
            contenido,
            tipo: 'texto',
        });
        if (resp.ok && resp.data) {
            resp.data = normalizarMensaje(resp.data);
        }
        return resp;
    } catch (err) {
        log.error('Error enviando mensaje texto', err);
        return { ok: false, data: null, error: 'Error de red', status: 500 };
    }
};

/* Alias de compatibilidad */
export const enviarMensaje = enviarMensajeTexto;

/* Enviar mensaje con archivo multimedia (imagen o audio) */
export const enviarMensajeMultimedia = async (
    conversacionId: number,
    tipo: 'imagen' | 'audio',
    archivo: File,
    contenido?: string
): Promise<RespuestaApi<Mensaje>> => {
    try {
        const formData = new FormData();
        formData.append('tipo', tipo);
        formData.append('media', archivo);
        if (contenido) formData.append('contenido', contenido);

        const resp = await apiPostFormData<Mensaje>(`/mensajes/${conversacionId}`, formData);
        if (resp.ok && resp.data) {
            resp.data = normalizarMensaje(resp.data);
        }
        return resp;
    } catch (err) {
        log.error(`Error enviando mensaje ${tipo}`, err);
        return { ok: false, data: null, error: 'Error de red', status: 500 };
    }
};

/* Compartir un sample en la conversación */
export const enviarMensajeSample = async (
    conversacionId: number,
    sampleId: number,
    contenido?: string
): Promise<RespuestaApi<Mensaje>> => {
    try {
        const resp = await apiPost<Mensaje>(`/mensajes/${conversacionId}`, {
            tipo: 'sample',
            sampleId,
            contenido: contenido ?? '',
        });
        if (resp.ok && resp.data) {
            resp.data = normalizarMensaje(resp.data);
        }
        return resp;
    } catch (err) {
        log.error('Error enviando sample por chat', err);
        return { ok: false, data: null, error: 'Error de red', status: 500 };
    }
};

/* Marcar conversación como leída */
export const marcarConversacionLeida = async (
    conversacionId: number
): Promise<RespuestaApi<void>> => {
    try {
        return await apiPost<void>(`/mensajes/${conversacionId}/leer`);
    } catch (err) {
        log.error('Error marcando conversación leída', err);
        return { ok: false, data: null, error: 'Error de red', status: 500 };
    }
};

/* Marcar TODAS las conversaciones como leídas */
export const marcarTodasConversacionesLeidas = async (): Promise<RespuestaApi<void>> => {
    try {
        return await apiPost<void>('/mensajes/leer-todas');
    } catch (err) {
        log.error('Error marcando todas las conversaciones leídas', err);
        return { ok: false, data: null, error: 'Error de red', status: 500 };
    }
};

/* Iniciar nueva conversación con un usuario */
export const iniciarConversacion = async (
    usuarioId: number
): Promise<RespuestaApi<Conversacion>> => {
    try {
        const resp = await apiPost<Conversacion>('/mensajes/nueva', { usuarioId });
        if (resp.ok && resp.data) {
            resp.data = normalizarConversacion(resp.data);
        }
        return resp;
    } catch (err) {
        log.error('Error iniciando conversación', err);
        return { ok: false, data: null, error: 'Error de red', status: 500 };
    }
};
