#!/usr/bin/env node

/* Compila y prueba en la VPS 89 (taller remoto). Este wrapper ya no compila en
 * el portátil: no hay toolchain de Rust desde el 2026-10-16 (AGENTS §1).
 * El sync y el target remoto los lleva rcargo.mjs bajo cm-heavy. */

import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

/* KAMPLES es hermano de workspace-manager en la raíz del área: desde
 * `KAMPLES/scripts/` son dos niveles hasta el área. */
const RCARGO_DIR = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..',
  '..',
  'workspace-manager',
  'scripts',
  'remoto',
);
const taller = await import(pathToFileURL(path.join(RCARGO_DIR, 'rcargo.mjs')).href);
const config = taller.cargarConfig();
const proyecto = taller.detectarProyecto(process.cwd());

/* Entorno que el build cruzado necesita y que el taller no tiene por defecto. Antes se exportaba
 * a mano en la 89 (`CFLAGS`, `SWAGGER_UI_DOWNLOAD_URL`) y al perderse el export el build moría:
 * primero en `libwebp-sys` (ya curado en `Cargo.toml` con la feature `sse41`), luego en
 * `utoipa-swagger-ui`, que intenta descargarse el zip de GitHub desde `build.rs`. Se fijan aquí,
 * el único punto por el que pasa cargo de este proyecto; lo que se pase a mano manda. */
const POR_DEFECTO = [
  `--prepath=/usr/lib/llvm-18/bin`,
  // `cm-sweep` no toca `~/.local/share`: el zip sobrevive al barrido (a diferencia de `/var/tmp`).
  `--env=SWAGGER_UI_DOWNLOAD_URL=file:///home/dev/.local/share/kamples/swagger-ui-v5.17.12.zip`,
];
const argvUsuario = process.argv.slice(2);
const argvCargo = [
  ...argvUsuario,
  ...POR_DEFECTO.filter((d) => {
    const clave = d.slice(0, d.indexOf('='));
    return !argvUsuario.some((a) => a === clave || a.startsWith(`${clave}=`));
  }),
];
if (taller.modoEfectivo(argvUsuario, config, proyecto) !== 'remoto') {
  /* El modo local ya no existe: se sale en claro en vez de fallar con un ENOENT. */
  console.error('[run-cargo] El portátil no compila Rust: quita --local y compila en el taller.');
  process.exit(1);
}

const r = spawnSync(process.execPath, [path.join(RCARGO_DIR, 'rcargo.mjs'), ...argvCargo], {
  stdio: 'inherit',
});
process.exit(r.status ?? 1);
