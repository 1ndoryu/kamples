/*
 * Service: apiBusqueda
 * Llamada al endpoint de búsqueda rápida para el dropdown de resultados.
 */

import { apiPeticion, type RespuestaApi } from './apiCliente';
import { normalizarUsuarioResumen } from '../../normalizers/sampleNormalizer';

/* Tipos para resultados de búsqueda rápida */

export interface ResultadoCancion {
    id: number;
    titulo: string;
    slug: string;
    artistaNombre: string | null;
    imagenUrl: string | null;
    totalSampleada: number;
}

export interface ResultadoSample {
    id: number;
    titulo: string;
    slug: string;
    imagenUrl: string | null;
    creador: {
        username: string;
        nombreVisible: string;
        avatarUrl: string | null;
    };
}

export interface ResultadoSampleo {
    id: number;
    fuente: {
        titulo: string;
        slug: string;
        imagenUrl: string | null;
        artista: string;
    };
    destino: {
        titulo: string;
        slug: string;
        imagenUrl: string | null;
        artista: string;
    };
}

export interface ResultadoUsuario {
    id: number;
    username: string;
    nombreVisible: string;
    avatarUrl: string | null;
    verificado: boolean;
    totalSeguidores: number;
}

export interface ResultadoColeccion {
    id: number;
    nombre: string;
    slug: string;
    portadaUrl: string | null;
    totalSamples: number;
    creador: string;
}

export type TipoResultado = 'cancion' | 'sample' | 'sampleo' | 'usuario' | 'coleccion';

export interface ResultadoUnificado {
    tipo: TipoResultado;
    score: number;
    datos: ResultadoCancion | ResultadoSample | ResultadoSampleo | ResultadoUsuario | ResultadoColeccion;
}

export interface ResultadosBusquedaRapida {
    canciones: ResultadoCancion[];
    samples: ResultadoSample[];
    sampleos: ResultadoSampleo[];
    usuarios: ResultadoUsuario[];
    colecciones: ResultadoColeccion[];
    todos: ResultadoUnificado[];
}

type RawRecord = Record<string, unknown>;

const esObjeto = (valor: unknown): valor is RawRecord =>
    valor !== null && typeof valor === 'object' && !Array.isArray(valor);

const normalizarCancion = (valor: unknown): ResultadoCancion => {
    const raw = esObjeto(valor) ? valor : {};
    return {
        id: Number(raw.id ?? 0),
        titulo: String(raw.titulo ?? ''),
        slug: String(raw.slug ?? ''),
        artistaNombre: (raw.artistaNombre ?? raw.artista_nombre ?? null) as string | null,
        imagenUrl: (raw.imagenUrl ?? raw.imagen_url ?? null) as string | null,
        totalSampleada: Number(raw.totalSampleada ?? raw.total_sampleada ?? 0),
    };
};

const normalizarSample = (valor: unknown): ResultadoSample => {
    const raw = esObjeto(valor) ? valor : {};
    const creadorRaw = esObjeto(raw.creador) ? raw.creador : undefined;
    const creador = normalizarUsuarioResumen(creadorRaw, 'autor');
    return {
        id: Number(raw.id ?? 0),
        titulo: String(raw.titulo ?? ''),
        slug: String(raw.slug ?? ''),
        imagenUrl: (raw.imagenUrl ?? raw.imagen_url ?? null) as string | null,
        creador: {
            username: creador.username,
            nombreVisible: creador.nombreVisible,
            avatarUrl: creador.avatarUrl,
        },
    };
};

const normalizarSampleoSide = (valor: unknown): ResultadoSampleo['fuente'] => {
    const raw = esObjeto(valor) ? valor : {};
    return {
        titulo: String(raw.titulo ?? ''),
        slug: String(raw.slug ?? ''),
        imagenUrl: (raw.imagenUrl ?? raw.imagen_url ?? null) as string | null,
        artista: String(raw.artista ?? ''),
    };
};

const normalizarSampleo = (valor: unknown): ResultadoSampleo => {
    const raw = esObjeto(valor) ? valor : {};
    return {
        id: Number(raw.id ?? 0),
        fuente: normalizarSampleoSide(raw.fuente),
        destino: normalizarSampleoSide(raw.destino),
    };
};

const normalizarUsuario = (valor: unknown): ResultadoUsuario => {
    const raw = esObjeto(valor) ? valor : {};
    const usuario = normalizarUsuarioResumen(raw, 'usuario');
    return {
        id: usuario.id,
        username: usuario.username,
        nombreVisible: usuario.nombreVisible,
        avatarUrl: usuario.avatarUrl,
        verificado: usuario.verificado,
        totalSeguidores: Number(raw.totalSeguidores ?? raw.total_seguidores ?? 0),
    };
};

const normalizarColeccion = (valor: unknown): ResultadoColeccion => {
    const raw = esObjeto(valor) ? valor : {};
    return {
        id: Number(raw.id ?? 0),
        nombre: String(raw.nombre ?? ''),
        slug: String(raw.slug ?? ''),
        portadaUrl: (raw.portadaUrl ?? raw.portada_url ?? null) as string | null,
        totalSamples: Number(raw.totalSamples ?? raw.total_samples ?? 0),
        creador: String(raw.creador ?? 'Autor'),
    };
};

const normalizarTodo = (valor: unknown): ResultadoUnificado | null => {
    const raw = esObjeto(valor) ? valor : {};
    const tipo = String(raw.tipo ?? '');
    const score = Number(raw.score ?? 0);
    const datos = raw.datos;

    switch (tipo) {
        case 'cancion':
            return { tipo, score, datos: normalizarCancion(datos) };
        case 'sample':
            return { tipo, score, datos: normalizarSample(datos) };
        case 'sampleo':
            return { tipo, score, datos: normalizarSampleo(datos) };
        case 'usuario':
            return { tipo, score, datos: normalizarUsuario(datos) };
        case 'coleccion':
            return { tipo, score, datos: normalizarColeccion(datos) };
        default:
            return null;
    }
};

const normalizarResultadosBusquedaRapida = (valor: unknown): ResultadosBusquedaRapida => {
    const raw = esObjeto(valor) ? valor : {};
    return {
        canciones: Array.isArray(raw.canciones) ? raw.canciones.map(normalizarCancion) : [],
        samples: Array.isArray(raw.samples) ? raw.samples.map(normalizarSample) : [],
        sampleos: Array.isArray(raw.sampleos) ? raw.sampleos.map(normalizarSampleo) : [],
        usuarios: Array.isArray(raw.usuarios) ? raw.usuarios.map(normalizarUsuario) : [],
        colecciones: Array.isArray(raw.colecciones) ? raw.colecciones.map(normalizarColeccion) : [],
        todos: Array.isArray(raw.todos)
            ? raw.todos.map(normalizarTodo).filter((item): item is ResultadoUnificado => item !== null)
            : [],
    };
};

/**
 * Busca canciones, samples, sampleos y usuarios en un solo request.
 * Mínimo 2 caracteres para disparar la búsqueda.
 */
export const busquedaRapida = (
    q: string,
    signal?: AbortSignal
): Promise<RespuestaApi<ResultadosBusquedaRapida>> =>
    apiPeticion<unknown>('/busqueda/rapida', {
        params: { q },
        signal,
    }).then((resp) => {
        if (!resp.ok || !resp.data) {
            return {
                ok: resp.ok,
                data: null,
                error: resp.error,
                status: resp.status,
            };
        }

        return {
            ok: true,
            data: normalizarResultadosBusquedaRapida(resp.data),
            error: null,
            status: resp.status,
        };
    });
