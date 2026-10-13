/* [104A-5] Utilidades de optimización de imágenes.
 * Genera URLs al proxy de imágenes del backend (/api/img/) con parámetros de optimización.
 * Similar a Jetpack Photon CDN: las imágenes se procesan on-demand con cache del lado del servidor. */

/* Anchos predefinidos que coinciden con los permitidos en el backend */
const ALLOWED_WIDTHS = [150, 300, 480, 640, 800, 1024, 1200, 1600, 2400] as const;

const OPTIMIZABLE_PREFIXES = ['/uploads/', '/assets/', '/legacy-assets/'] as const;

type AllowedWidth = typeof ALLOWED_WIDTHS[number];

interface OptimizeOptions {
    width?: AllowedWidth | number;
    quality?: number;
    format?: 'webp' | 'jpeg' | 'png';
}

const DEFAULT_WIDTHS = [...ALLOWED_WIDTHS] as AllowedWidth[];

function normalizeDevicePixelRatio(devicePixelRatio = 1): number {
    if (!Number.isFinite(devicePixelRatio) || devicePixelRatio <= 0) {
        return 1;
    }

    return Math.min(Math.max(devicePixelRatio, 1), 3);
}

function findWidthAbove(targetWidth: number): AllowedWidth {
    return ALLOWED_WIDTHS.find(width => width >= targetWidth) ?? ALLOWED_WIDTHS[ALLOWED_WIDTHS.length - 1];
}

/* [104A-13] Ajusta los buckets del srcSet al ancho real renderizado.
 * Evita servir 300w/640w por defecto a logos, avatars o cards pequeñas.
 * Mantiene un bucket por debajo y otro por encima para cambios de layout sin descargar originales. */
export function resolveResponsiveWidths(
    renderedWidth?: number,
    devicePixelRatio = 1,
): AllowedWidth[] {
    if (!renderedWidth || !Number.isFinite(renderedWidth) || renderedWidth <= 0) {
        return DEFAULT_WIDTHS;
    }

    const normalizedWidth = Math.round(renderedWidth);
    const normalizedDpr = normalizeDevicePixelRatio(devicePixelRatio);
    const lowerBound = normalizedWidth * 0.75;
    const upperBound = normalizedWidth * Math.max(normalizedDpr * 1.5, 1.5);
    const widthsInRange = ALLOWED_WIDTHS.filter(width => width >= lowerBound && width <= upperBound);
    const previousWidth = [...ALLOWED_WIDTHS].reverse().find(width => width < lowerBound);
    const nextWidth = ALLOWED_WIDTHS.find(width => width > upperBound);
    const selectedWidths = new Set<AllowedWidth>(widthsInRange);

    if (previousWidth) {
        selectedWidths.add(previousWidth);
    }

    if (nextWidth) {
        selectedWidths.add(nextWidth);
    }

    if (selectedWidths.size === 0) {
        selectedWidths.add(findWidthAbove(normalizedWidth * normalizedDpr));
    }

    return Array.from(selectedWidths).sort((left, right) => left - right);
}

export function resolveBestWidth(
    renderedWidth?: number,
    devicePixelRatio = 1,
): AllowedWidth | undefined {
    if (!renderedWidth || !Number.isFinite(renderedWidth) || renderedWidth <= 0) {
        return undefined;
    }

    return findWidthAbove(Math.round(renderedWidth * normalizeDevicePixelRatio(devicePixelRatio)));
}

function isOptimizableSrc(src: string): boolean {
    return OPTIMIZABLE_PREFIXES.some(prefix => src.startsWith(prefix));
}

function toProxyRelativePath(src: string): string {
    if (src.startsWith('/uploads/')) {
        return encodeProxyRelativePath(src.replace(/^\/uploads\//, ''));
    }

    return encodeProxyRelativePath(src.replace(/^\//, ''));
}

function encodeProxyRelativePath(relativePath: string): string {
    /* [105A-5] El srcset separa por espacios; codificamos segmentos para que assets con espacios no pierdan variantes. */
    return relativePath
        .split('/')
        .map(segment => {
            try {
                return encodeURIComponent(decodeURIComponent(segment));
            } catch {
                return encodeURIComponent(segment);
            }
        })
        .join('/');
}

/* Genera la URL optimizada para una imagen.
 * Si la src ya es una URL externa o un data URI, la retorna sin modificar.
 * Si no tiene parámetros de optimización, retorna la URL original. */
export function optimizedUrl(src: string, options: OptimizeOptions = {}): string {
    if (!src) return src;

    /* No procesar data URIs, URLs externas o SVGs */
    if (src.startsWith('data:') || src.startsWith('http') || src.endsWith('.svg')) {
        return src;
    }

    /* Solo procesar imágenes locales conocidas */
    if (!isOptimizableSrc(src)) {
        return src;
    }

    /* Si no hay opciones de optimización, retornar original */
    const { width, quality, format } = options;
    if (!width && !quality && !format) {
        return src;
    }

    /* Construir ruta relativa para el proxy */
    const relativePath = toProxyRelativePath(src);

    /* Construir query params */
    const params = new URLSearchParams();
    if (width) params.set('w', String(width));
    if (quality) params.set('q', String(quality));
    if (format) params.set('fmt', format);

    return `/api/img/${relativePath}?${params.toString()}`;
}

/* Genera un srcSet para imágenes responsive.
 * Retorna pares ancho→URL para los breakpoints especificados. */
export function generateSrcSet(
    src: string,
    widths: AllowedWidth[] = DEFAULT_WIDTHS,
    options: Omit<OptimizeOptions, 'width'> = {},
): string {
    if (!src || src.startsWith('data:') || src.startsWith('http') || src.endsWith('.svg')) {
        return '';
    }

    if (!isOptimizableSrc(src)) {
        return '';
    }

    return widths
        .map(w => `${optimizedUrl(src, { ...options, width: w })} ${w}w`)
        .join(', ');
}

/* Genera srcSet en formato WebP para usar con <picture> <source> */
export function generateWebPSrcSet(
    src: string,
    widths: AllowedWidth[] = DEFAULT_WIDTHS,
    quality = 80,
): string {
    return generateSrcSet(src, widths, { format: 'webp', quality });
}
