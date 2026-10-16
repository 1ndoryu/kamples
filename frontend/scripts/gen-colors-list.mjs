/**
 * gen-colors-list.mjs
 * Genera imagenesColorLista.ts a partir de los archivos en public/legacy-assets/colors/.
 * Se ejecuta antes del build para mantener la lista sincronizada con el filesystem.
 * Uso: node scripts/gen-colors-list.mjs
 */

import { readdir, writeFile } from 'node:fs/promises';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const colorsDir = join(__dirname, '..', 'public', 'legacy-assets', 'colors');
const outputFile = join(__dirname, '..', 'src', 'legacy', 'services', 'datos', 'imagenesColorLista.ts');

// `colors/` es un enlace local al tema de WordPress y no está en el repositorio
// (ver .gitignore), así que en cualquier clon —CI, taller, otra máquina— no
// existe. Antes el script abortaba con ENOENT y se llevaba por delante el build
// entero; ahora avisa y deja intacta la lista que sí está versionada, que es la
// que corresponde a esas imágenes ausentes.
let archivos;
try {
    archivos = (await readdir(colorsDir))
        .filter(f => /\.(jpg|jpeg|png|webp)$/i.test(f))
        .sort();
} catch (error) {
    if (error.code !== 'ENOENT') throw error;
    console.warn(
        `[gen-colors-list] WARN: no existe ${colorsDir} (enlace local al tema de WordPress, fuera del repositorio). ` +
        'Se conserva imagenesColorLista.ts tal como está versionado.'
    );
    process.exit(0);
}

if (archivos.length === 0) {
    console.warn('[gen-colors-list] WARN: No se encontraron imágenes en', colorsDir);
}

const contenido = `/*
 * Datos: Lista de archivos de imagen en colors/.
 * GENERADO AUTOMATICAMENTE por scripts/gen-colors-list.mjs
 * No editar manualmente. Cambios se pierden en el siguiente build.
 * Ultima generacion: ${new Date().toISOString()}
 */

export const IMAGENES_COLOR: string[] = [
${archivos.map(f => `    '${f}',`).join('\n')}
];
`;

await writeFile(outputFile, contenido, 'utf-8');
console.log(`[gen-colors-list] Generado: ${archivos.length} imágenes → imagenesColorLista.ts`);
