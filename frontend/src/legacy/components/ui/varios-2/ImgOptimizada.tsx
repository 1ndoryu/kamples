/*
 * ImgOptimizada — [183A-40] actualizado [166A-5]
 * Wrapper de <img> que pasa la URL por nuestro proxy /api/img/ para
 * optimización on-demand (resize, compresión, conversión a WebP).
 * Reemplaza el anterior uso de Jetpack Photon CDN.
 *
 * Uso: <ImgOptimizada src={url} alt="texto" w={300} quality={75} />
 * Gotcha: En localhost, Vite proxy redirige /api/img/ al backend Rust.
 * Gotcha: URLs externas (http://, data:) se pasan sin modificar.
 */

import { optimizedUrl } from '@app/utils/varios-1/imageUtils';

interface ImgOptimizadaProps extends React.ImgHTMLAttributes<HTMLImageElement> {
    src: string;
    alt: string;
    w?: number;
    h?: number;
    quality?: number;
    fit?: 'cover' | 'contain';
}

export const ImgOptimizada = ({
    src,
    alt,
    w,
    h,
    quality = 80,
    fit,
    loading = 'lazy',
    ...rest
}: ImgOptimizadaProps): JSX.Element => {
    /* [166A-5] Usar optimizedUrl en vez de photonUrl para rutas locales.
     * External URLs (data:, http) pasan sin modificar por optimizedUrl. */
    const srcOptimizado = optimizedUrl(src, { width: w, quality, format: 'webp' });

    return (
        <img
            src={srcOptimizado}
            alt={alt}
            loading={loading}
            {...rest}
        />
    );
};
