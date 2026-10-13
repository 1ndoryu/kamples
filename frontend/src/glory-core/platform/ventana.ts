/*
 * Frontera de plataforma para el objeto global `window`.
 *
 * Es el UNICO sitio que toca `window.location`, `window.history`, `window.scrollTo`,
 * dimensiones de ventana y `addEventListener` global. El resto del codigo llama a
 * estas funciones: asi el analizador de frontera (regla window-reference-outside-platform)
 * tiene un punto unico de acceso y los cambios de navegador se hacen aqui.
 *
 * Sin estado: cada funcion lee el objeto en el momento de la llamada (SSR-safe en
 * los accesos que lo requieren via obtenerVentana).
 */

/* Ubicacion y navegacion */
export function ubicacionActual(): Location {
    return window.location;
}

export function rutaActual(): string {
    return window.location.pathname;
}

export function busquedaActual(): string {
    return window.location.search;
}

export function origenActual(): string {
    return window.location.origin;
}

export function protocoloActual(): string {
    return window.location.protocol;
}

/* Asignar href navega a la URL indicada */
export function irAUrl(url: string): void {
    window.location.href = url;
}

export function volverAtras(): void {
    window.history.back();
}

export function agregarEntradaHistorial(state: unknown, titulo: string, url?: string | URL | null): void {
    window.history.pushState(state, titulo, url);
}

export function reemplazarEntradaHistorial(state: unknown, titulo: string, url?: string | URL | null): void {
    window.history.replaceState(state, titulo, url);
}

/* Dimensiones y desplazamiento */
export function anchoVentana(): number {
    return window.innerWidth;
}

export function altoVentana(): number {
    return window.innerHeight;
}

export function desplazarVentanaA(opciones: ScrollToOptions): void;
export function desplazarVentanaA(x: number, y: number): void;
export function desplazarVentanaA(x: number | ScrollToOptions, y?: number): void {
    if (typeof x === 'object') {
        window.scrollTo(x);
        return;
    }
    window.scrollTo(x, y as number);
}

/* Listeners globales: devolver el mismo listener a dejarDeEscucharVentana para quitarlo.
 * Las sobrecargas con `string` cubren eventos personalizados (p. ej. 'kamples:...'), que no
 * estan en WindowEventMap. */
export function escucharVentana<K extends keyof WindowEventMap>(
    tipo: K,
    listener: (this: Window, ev: WindowEventMap[K]) => void,
    opciones?: boolean | AddEventListenerOptions,
): void;
export function escucharVentana(
    tipo: string,
    listener: (this: Window, ev: Event) => void,
    opciones?: boolean | AddEventListenerOptions,
): void;
export function escucharVentana(
    tipo: string,
    listener: (this: Window, ev: Event) => void,
    opciones?: boolean | AddEventListenerOptions,
): void {
    window.addEventListener(tipo, listener as EventListener, opciones);
}

export function dejarDeEscucharVentana<K extends keyof WindowEventMap>(
    tipo: K,
    listener: (this: Window, ev: WindowEventMap[K]) => void,
    opciones?: boolean | EventListenerOptions,
): void;
export function dejarDeEscucharVentana(
    tipo: string,
    listener: (this: Window, ev: Event) => void,
    opciones?: boolean | EventListenerOptions,
): void;
export function dejarDeEscucharVentana(
    tipo: string,
    listener: (this: Window, ev: Event) => void,
    opciones?: boolean | EventListenerOptions,
): void {
    window.removeEventListener(tipo, listener as EventListener, opciones);
}
