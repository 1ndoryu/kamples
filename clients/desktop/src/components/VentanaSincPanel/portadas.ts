import { protocoloActual, ubicacionActual } from '@/platform/ventana';
import { obtenerImagenColor } from '@app/services/varios-1/imagenesColor';
import type { EntradaHistorialSample } from '@app/stores/varios-4/syncStore';

const VERSION_PORTADA_INTERVALO_MS = 15_000;

function obtenerOrigenServidorSync(): string | null {
    const config = window.__KAMPLES_CONFIG__ as { serverUrl?: string } | undefined;
    const desdeConfig = config?.serverUrl;
    if (desdeConfig && /^https?:\/\//i.test(desdeConfig)) {
        try {
            return new URL(desdeConfig).origin;
        } catch {
            /* continuar con fallback */
        }
    }

    const ctx = window.GLORY_CONTEXT as { apiUrl?: string; restUrl?: string } | undefined;
    const candidata = ctx?.apiUrl ?? ctx?.restUrl;
    if (candidata && /^https?:\/\//i.test(candidata)) {
        try {
            return new URL(candidata).origin;
        } catch {
            return null;
        }
    }

    return null;
}

function anexarVersionPortada(url: string, version: number): string {
    try {
        const urlNormalizada = new URL(url, ubicacionActual().href);
        urlNormalizada.searchParams.set('_sv', String(version));
        return urlNormalizada.toString();
    } catch {
        const separador = url.includes('?') ? '&' : '?';
        return `${url}${separador}_sv=${version}`;
    }
}

function resolverUrlPortada(
    url: string | null | undefined,
    timestampActualizado?: number,
): string | null {
    if (!url) return null;

    const versionPortada = Math.max(
        timestampActualizado ?? 0,
        Math.floor(Date.now() / VERSION_PORTADA_INTERVALO_MS) * VERSION_PORTADA_INTERVALO_MS,
    );

    if (/^https?:\/\//i.test(url)) return anexarVersionPortada(url, versionPortada);
    if (url.startsWith('//')) return anexarVersionPortada(`${protocoloActual()}${url}`, versionPortada);

    const origen = obtenerOrigenServidorSync();

    if (url.startsWith('/')) {
        if (origen) return anexarVersionPortada(`${origen}${url}`, versionPortada);
        return anexarVersionPortada(url, versionPortada);
    }

    if (origen) return anexarVersionPortada(`${origen}/${url.replace(/^\/+/, '')}`, versionPortada);
    return anexarVersionPortada(url, versionPortada);
}

export function resolverMiniaturaHistorial(entrada: EntradaHistorialSample): string | null {
    const portadaReal = resolverUrlPortada(entrada.imagenUrl, entrada.timestampActualizado);
    if (portadaReal) return portadaReal;
    if (entrada.sampleId > 0) {
        return resolverUrlPortada(obtenerImagenColor(entrada.sampleId), entrada.timestampActualizado);
    }
    return null;
}
