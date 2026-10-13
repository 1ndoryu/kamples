/*
 * Util: resolverRutaAsset — Kamples (QL46)
 * En Tauri (desktop/Android APK), las rutas relativas al origin no resuelven al servidor
 * porque el WebView carga desde tauri.localhost. Esta función prefija la URL de producción
 * cuando se detecta el entorno Tauri.
 *
 * [204A-1] En el SPA Vite, los assets del tema WordPress fueron copiados a
 * /legacy-assets/. Reescribimos /wp-content/themes/glorytemplate/App/Assets/* →
 * /legacy-assets/* para que las imagenes y fuentes resuelvan en el navegador.
 *
 * [224A-4] Bug corregido: antes se usaba __KAMPLES_DESKTOP__ como proxy de Tauri, pero
 * gloryContextShim.ts lo pone en true en TODOS los entornos (incluyendo browser/web) para
 * activar auth JWT. Eso hacía que resolverRutaAsset devolviera la URL absoluta de producción
 * en el browser, causando 404 porque el backend Rust no sirve /wp-content/.
 * Fix: usar __TAURI_INTERNALS__ que Tauri v2 inyecta ÚNICAMENTE en su WebView. */

const SERVIDOR_PROD = 'https://kamples.com';
const PREFIJO_LEGACY_WP = '/wp-content/themes/glorytemplate/App/Assets';
const PREFIJO_SPA = '/legacy-assets';
/* [224A-x] Las imágenes de color viven fuera de App/Assets; se rebasean por separado.
 * En producción WordPress el path original funciona. En el SPA standalone (Vite),
 * /wp-content/themes/glorytemplate/colors/ → /legacy-assets/colors/ (junction local). */
const PREFIJO_COLORS_WP = '/wp-content/themes/glorytemplate/colors';
const PREFIJO_COLORS_SPA = '/legacy-assets/colors';

/* Detecta el WebView de Tauri v2 via __TAURI_INTERNALS__ (inyectado por Tauri).
 * NO usar __KAMPLES_DESKTOP__ — ese flag se activa en todos los entornos para JWT auth. */
const esEntornoTauri = (): boolean =>
    typeof window !== 'undefined' && !!(window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;

function rebaseAsset(ruta: string): string {
    if (ruta.startsWith(PREFIJO_LEGACY_WP)) {
        return PREFIJO_SPA + ruta.slice(PREFIJO_LEGACY_WP.length);
    }
    if (ruta.startsWith(PREFIJO_COLORS_WP)) {
        return PREFIJO_COLORS_SPA + ruta.slice(PREFIJO_COLORS_WP.length);
    }
    return ruta;
}

/**
 * Resuelve una ruta relativa al servidor correcto.
 * En web, retorna la ruta reescrita al SPA (legacy-assets).
 * En Tauri/APK, prefija con el servidor de producción.
 *
 * [224A-3] También acepta URLs absolutas con dominio de producción:
 * en entorno no-Tauri (SPA/Vite), stripea el dominio y rebasea la ruta.
 * Esto corrige imagenUrl almacenados como "https://kamples.com/wp-content/..."
 * que en dev no resuelven.
 */
export const resolverRutaAsset = (rutaRelativa: string): string => {
    if (esEntornoTauri()) {
        /* [256A-1c] En Tauri dev mode, usar el Vite dev server local que ahora
         * sirve /legacy-assets/ gracias a publicDir extendido en vite.config.ts.
         * En producción Tauri (APK/paquete), prefijar con servidor de producción. */
        if (import.meta.env.DEV) {
            const rutaNormalizada = rutaRelativa.startsWith(SERVIDOR_PROD)
                ? rutaRelativa.slice(SERVIDOR_PROD.length)
                : rutaRelativa;
            return rebaseAsset(rutaNormalizada);
        }
        /* En producción Tauri, si ya es absoluta con el dominio correcto, devolverla tal cual */
        if (rutaRelativa.startsWith(SERVIDOR_PROD)) return rutaRelativa;
        return `${SERVIDOR_PROD}${rutaRelativa}`;
    }
    /* Stripear dominio de producción en entorno SPA/dev para rebasear correctamente */
    const rutaNormalizada = rutaRelativa.startsWith(SERVIDOR_PROD)
        ? rutaRelativa.slice(SERVIDOR_PROD.length)
        : rutaRelativa;
    return rebaseAsset(rutaNormalizada);
};
