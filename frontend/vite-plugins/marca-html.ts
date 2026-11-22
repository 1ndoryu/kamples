/* Sustituye `%MARCA%` en los HTML por el nombre de `marca.json`.
 *
 * [por que] El `<title>` de un HTML estático no puede importar TypeScript. Se
 * escribe `%MARCA%` y este plugin lo cambia en dev y en build; así el nombre
 * sigue saliendo de una sola fuente.
 */

import type { Plugin } from 'vite';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const RAIZ = join(dirname(fileURLToPath(import.meta.url)), '..', '..');

export function marcaHtml(): Plugin {
    const { nombre } = JSON.parse(readFileSync(join(RAIZ, 'marca.json'), 'utf8'));
    return {
        name: 'kamples-marca-html',
        transformIndexHtml: (html) => html.replaceAll('%MARCA%', nombre),
    };
}
