/*
 * Service: imagenesColor — Kamples
 * Asigna imagen de portada determinista desde la carpeta colors/.
 * Se usa en TarjetaSample y TarjetaColeccion cuando no hay imagen propia.
 * Array de imágenes extraído a datos/imagenesColorLista.ts para cumplir SRP.
 *
 * [166A-5] Se eliminó photonUrl de aquí porque imgOptimizada ya aplica
 * optimización a través del proxy /api/img/ del backend (resize + WebP).
 * La URL cruda permite que el componente decida el tamaño según el contexto. */

import { IMAGENES_COLOR } from './datos/imagenesColorLista';
import { resolverRutaAsset } from '@app/utils/resolverRutaAsset';

const RUTA_RELATIVA = '/wp-content/themes/glorytemplate/colors/';
const TOTAL = IMAGENES_COLOR.length;

/*
 * En Tauri (desktop/Android) las rutas relativas no resuelven al servidor.
 * Delegado a resolverRutaAsset (util compartida QL46).
 */
function obtenerRutaBase(): string {
    return resolverRutaAsset(RUTA_RELATIVA);
}

/*
 * Obtiene la URL de una imagen de color determinista basada en un ID numérico.
 * Siempre devuelve la misma imagen para el mismo ID.
 * [166A-5] Ya no pasa por photonUrl; el componente que la usa (ImgOptimizada)
 * aplica optimización vía /api/img/ con el tamaño adecuado al contexto. */
export const obtenerImagenColor = (id: number): string => {
    const base = obtenerRutaBase();
    /* Guard: si id es NaN/undefined o no hay imágenes, devolver placeholder */
    if (!Number.isFinite(id) || TOTAL === 0) {
        return `${base}${IMAGENES_COLOR[0] ?? 'placeholder.jpg'}`;
    }
    const indice = ((id % TOTAL) + TOTAL) % TOTAL;
    return `${base}${IMAGENES_COLOR[indice]}`;
};

/*
 * [224A-x] Resuelve la imagen de portada de un sample:
 * - Si imagenUrl existe, la pasa por resolverRutaAsset para rebasear rutas WP → SPA.
 * - Si es null/vacío, usa obtenerImagenColor(id) como fallback determinista.
 * Centraliza la lógica que antes se repetía en TarjetaSample, SampleDetalle, Reproductor.
 */
export const resolverImagenSample = (imagenUrl: string | null | undefined, id: number): string => {
    if (imagenUrl) {
        return resolverRutaAsset(imagenUrl);
    }
    return obtenerImagenColor(id);
};

/*
 * Obtiene la URL de una imagen de color basada en un string (nombre de colección, etc).
 * Hace un hash simple del string para generar un índice determinista.
 */
export const obtenerImagenColorPorTexto = (texto: string): string => {
    const base = obtenerRutaBase();
    let hash = 0;
    for (let i = 0; i < texto.length; i++) {
        hash = ((hash << 5) - hash) + texto.charCodeAt(i);
        hash |= 0;
    }
    const indice = ((hash % TOTAL) + TOTAL) % TOTAL;
    return `${base}${IMAGENES_COLOR[indice]}`;
};
