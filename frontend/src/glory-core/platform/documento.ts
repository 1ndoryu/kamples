/*
 * Frontera de plataforma para el DOM global (`document`).
 *
 * Unico sitio que accede a `document.body`, `document.head`, `createElement` y
 * consultas por selector. Ver ventana.ts para el motivo (regla dom-access-outside-platform).
 */

export function cuerpoDocumento(): HTMLElement {
    return document.body;
}

export function cabezaDocumento(): HTMLHeadElement {
    return document.head;
}

export function crearElemento<K extends keyof HTMLElementTagNameMap>(etiqueta: K): HTMLElementTagNameMap[K] {
    return document.createElement(etiqueta);
}

export function consultarElemento<E extends Element = Element>(selector: string): E | null {
    return document.querySelector<E>(selector);
}

export function consultarElementos<E extends Element = Element>(selector: string): NodeListOf<E> {
    return document.querySelectorAll<E>(selector);
}

export function elementoPorId(id: string): HTMLElement | null {
    return document.getElementById(id);
}
