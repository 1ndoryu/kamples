/*
 * [15A-9] Cursor de arrastre durante un resize o un arrastre con raton.
 *
 * Cada hook ponia `style.cursor` y `style.userSelect` inline sobre el elemento y
 * los borraba al soltar. Ahora el elemento pide el cursor con un atributo y las
 * cuatro reglas de `legacy/styles/layout.css` lo traducen: una sola fuente, y el
 * navegador aplica y deshace el estilo sin escribir en el DOM desde JS.
 */

export type CursorArrastre = 'agarrando' | 'columna' | 'ancho' | 'alto';

/** Pinta el cursor de arrastre en el elemento. */
export function activarCursorArrastre(el: HTMLElement, cursor: CursorArrastre) {
    el.dataset.cursorArrastre = cursor;
}

/** Quita el cursor de arrastre del elemento. */
export function desactivarCursorArrastre(el: HTMLElement) {
    delete el.dataset.cursorArrastre;
}
