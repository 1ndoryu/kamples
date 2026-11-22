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
const argvCargo = process.argv.slice(2);
const config = taller.cargarConfig();
const proyecto = taller.detectarProyecto(process.cwd());

if (taller.modoEfectivo(argvCargo, config, proyecto) !== 'remoto') {
  /* El modo local ya no existe: se sale en claro en vez de fallar con un ENOENT. */
  console.error('[run-cargo] El portátil no compila Rust: quita --local y compila en el taller.');
  process.exit(1);
}

const r = spawnSync(process.execPath, [path.join(RCARGO_DIR, 'rcargo.mjs'), ...argvCargo], {
  stdio: 'inherit',
});
process.exit(r.status ?? 1);
