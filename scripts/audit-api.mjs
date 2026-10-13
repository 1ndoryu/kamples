#!/usr/bin/env node
/* [274A-11] Audit API: detecta endpoints que el frontend legacy invoca pero
 * el backend Rust no expone en `openapi.json`.
 *
 * Uso:
 *   node scripts/audit-api.mjs              # reporte humano + exit 1 si hay missing
 *   node scripts/audit-api.mjs --json       # JSON machine-readable
 *   node scripts/audit-api.mjs --vscode     # formato VS Code problemMatcher
 *
 * Lee `openapi.json` (generado por utoipa al compilar el backend) y
 * `frontend/src/legacy/services/*.ts`. Aplica las reglas de mapeo de
 * `frontend/src/bootstrap/wpJsonStub.ts` (PATH_MAP, /me/* -> /users/me/*,
 * etc) replicadas en `scripts/lib/apiResolver.mjs`.
 *
 * Existe para evitar el patron "frontend pega 404 -> usuario reporta -> portar
 * endpoint" (ver completados 274A-4..274A-10). Se debe correr antes de cerrar
 * sesion como diagnostico exhaustivo.
 *
 * Limitaciones actuales:
 * - No detecta calls con template literals dinamicos (`apiGet(`/foo/${id}`)`)
 *   porque el path no es estatico, pero la parte estatica anterior a `${}` se
 *   ignora con seguridad (preferimos falsos negativos a falsos positivos).
 * - No verifica auth/permisos, solo existencia de la ruta. */

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
    extractAllCalls,
    loadOpenapi,
    resolveCall,
} from './lib/apiResolver.mjs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, '..');

const args = new Set(process.argv.slice(2));
const FORMAT = args.has('--json') ? 'json' : args.has('--vscode') ? 'vscode' : 'human';

// Recursivo: los servicios viven en subcarpetas (services/<grupo>/), un listado plano no vería ninguno.
function listServiceFiles(dir = path.join(REPO_ROOT, 'frontend', 'src', 'legacy', 'services')) {
    if (!fs.existsSync(dir)) return [];
    return fs.readdirSync(dir, { withFileTypes: true }).flatMap(e => {
        const ruta = path.join(dir, e.name);
        if (e.isDirectory()) return listServiceFiles(ruta);
        return e.name.endsWith('.ts') && e.name !== 'apiCliente.ts' ? [ruta] : [];
    });
}

function relPath(absolute) {
    return path.relative(REPO_ROOT, absolute).replace(/\\/g, '/');
}

function main() {
    const { json: openapi, index } = loadOpenapi(REPO_ROOT);
    const files = listServiceFiles();

    const missing = [];
    const matched = [];
    let totalCalls = 0;

    for (const file of files) {
        const source = fs.readFileSync(file, 'utf8');
        const calls = extractAllCalls(source);
        for (const call of calls) {
            totalCalls += 1;
            const result = resolveCall(call, index);
            const entry = { file, ...call, ...result };
            if (result.matched) matched.push(entry);
            else missing.push(entry);
        }
    }

    if (FORMAT === 'json') {
        process.stdout.write(JSON.stringify({
            openapi_paths: Object.keys(openapi.paths || {}).length,
            total_calls: totalCalls,
            matched: matched.length,
            missing: missing.length,
            missing_details: missing.map(e => ({
                file: relPath(e.file),
                line: e.line,
                column: e.column,
                method: e.method,
                legacy_path: e.legacyPath,
                resolved_path: e.resolvedPath,
            })),
        }, null, 2) + '\n');
        process.exit(missing.length > 0 ? 1 : 0);
    }

    if (FORMAT === 'vscode') {
        for (const e of missing) {
            const f = relPath(e.file);
            process.stdout.write(`${f}:${e.line}:${e.column}: warning: ENDPOINT FALTANTE: ${e.method} ${e.resolvedPath}\n`);
        }
        process.exit(missing.length > 0 ? 1 : 0);
    }

    /* Human format */
    console.log(`[audit-api] OpenAPI paths: ${Object.keys(openapi.paths || {}).length}`);
    console.log(`[audit-api] Total calls escaneados: ${totalCalls}`);
    console.log(`[audit-api] Matched: ${matched.length}  Missing: ${missing.length}`);

    if (missing.length === 0) {
        console.log('\n[audit-api] OK: todos los calls del frontend tienen handler en el backend.');
        process.exit(0);
    }

    /* Agrupar missing por path resuelto + metodo para no mostrar duplicados. */
    const grouped = new Map();
    for (const e of missing) {
        const key = `${e.method} ${e.resolvedPath}`;
        if (!grouped.has(key)) grouped.set(key, []);
        grouped.get(key).push(e);
    }

    console.log(`\n[audit-api] Endpoints faltantes (${grouped.size} unicos):\n`);
    const sortedKeys = Array.from(grouped.keys()).sort();
    for (const key of sortedKeys) {
        const entries = grouped.get(key);
        console.log(`  ${key}`);
        for (const e of entries) {
            console.log(`      ${relPath(e.file)}:${e.line}:${e.column}  (legacy: ${e.legacyPath})`);
        }
    }
    console.log('');
    process.exit(1);
}

main();
