#!/usr/bin/env node
/* Launcher de Tauri desktop con gestión de CARGO_TARGET_DIR.
 * Pre-limpieza + watcher periódico + exec de `tauri dev`.
 * Resuelve que `npm run tauri:dev` escribía a C:\tmp\glory-target sin limpieza,
 * acumulando +7GB de artefactos Cargo. */

import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdirSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createConnection } from 'node:net';

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(__dirname, '..');
const desktopDir = resolve(projectRoot, 'clients', 'desktop');
const frameworkScripts = resolve(projectRoot, 'glory-rs', 'scripts');
const cargoTargetBase = process.env.CARGO_TARGET_DIR_BASE || (process.platform === 'win32' ? 'C:\\tmp\\glory-target' : '/tmp/glory-target');
const maxMb = process.env.GLORY_CARGO_TARGET_MAX_MB || '15000';
const intervalSeconds = process.env.GLORY_CARGO_CLEAN_INTERVAL_SECONDS || '120';
const isWin = process.platform === 'win32';
const backendPort = parseInt(process.env.KAMPLES_BACKEND_PORT || '3000', 10);

function cmdName(name) {
    if (!isWin) return name;
    if (name === 'npm') return 'npm.cmd';
    if (name === 'cargo') return 'cargo.exe';
    if (name === 'powershell') return 'powershell.exe';
    return name;
}

/* Esperar a que un puerto TCP esté disponible */
function waitForPort(port, timeoutMs = 120_000) {
    return new Promise((resolve, reject) => {
        const start = Date.now();
        const tryConnect = () => {
            const sock = createConnection({ host: '127.0.0.1', port });
            sock.once('connect', () => { sock.destroy(); resolve(); });
            sock.once('error', () => {
                sock.destroy();
                if (Date.now() - start > timeoutMs) {
                    reject(new Error(`Timeout esperando puerto ${port}`));
                } else {
                    setTimeout(tryConnect, 800);
                }
            });
        };
        tryConnect();
    });
}

/* Crear junctions en clients/desktop/public/ apuntando a frontend/public/
 * Necesario para que Vite sirva assets legacy (fonts, images, SVGs). */
function ensureDesktopPublicJunctions() {
    if (!isWin) return; // En Linux/Mac se usarían symlinks reales
    const desktopPublic = resolve(desktopDir, 'public');
    const frontendPublic = resolve(projectRoot, 'frontend', 'public');
    if (!existsSync(frontendPublic)) return;

    if (!existsSync(desktopPublic)) {
        mkdirSync(desktopPublic, { recursive: true });
    }

    const links = ['legacy-assets', 'auth', 'landing'];
    for (const name of links) {
        const src = resolve(frontendPublic, name);
        const dst = resolve(desktopPublic, name);
        if (existsSync(src) && !existsSync(dst)) {
            const r = spawnSync('cmd', ['/c', 'mklink', '/J', dst, src], { stdio: 'inherit' });
            if (r.status === 0) {
                console.log(`[launch-tauri] Junction: public/${name} → frontend/public/${name}`);
            } else {
                console.warn(`[launch-tauri] Junction fallo para ${name}`);
            }
        }
    }
}

/* Pre-limpieza: forzar antes de iniciar (no hay build activo) */
function runPreClean() {
    const script = resolve(frameworkScripts, 'clean-cargo-target.ps1');
    if (!isWin || !existsSync(script)) return;

    console.log(`[launch-tauri] Pre-limpieza de ${cargoTargetBase} (max ${maxMb} MB)...`);
    const r = spawnSync(cmdName('powershell'), [
        '-ExecutionPolicy', 'Bypass',
        '-File', script,
        '-TargetDirs', cargoTargetBase,
        '-MaxTotalMB', maxMb,
    ], { stdio: 'inherit', timeout: 60000 });

    if (r.status != null && r.status !== 0) {
        console.warn(`[launch-tauri] Pre-limpieza fallo (exit ${r.status})`);
    }
}

/* Watcher en background */
let watcherProc = null;
let backendProc = null;
function startWatcher() {
    const script = resolve(frameworkScripts, 'watch-cargo-target.ps1');
    if (!isWin || !existsSync(script)) return;

    console.log(`[launch-tauri] Watcher cada ${intervalSeconds}s (max ${maxMb} MB)`);
    watcherProc = spawn(cmdName('powershell'), [
        '-ExecutionPolicy', 'Bypass',
        '-File', script,
        '-TargetDirs', cargoTargetBase,
        '-MaxTotalMB', maxMb,
        '-IntervalSeconds', intervalSeconds,
    ], { stdio: 'ignore', detached: false });
}

function cleanup() {
    if (watcherProc && !watcherProc.killed) watcherProc.kill();
    if (backendProc && !backendProc.killed) backendProc.kill();
}

/* Ejecutar */
async function main() {
    ensureDesktopPublicJunctions();
    runPreClean();
    startWatcher();

    /* Arrancar backend Rust en background */
    console.log(`[launch-tauri] Iniciando backend Rust (puerto ${backendPort})...`);
    /* Programa y argumentos por separado: con la cadena entera y `shell: isWin`, en Linux
     * (shell:false) Node buscaba un ejecutable llamado literalmente "cargo run --bin
     * glory-backend" y moria con ENOENT. Desde que el portatil no lleva toolchain de Rust,
     * el unico sitio que compila el cliente es la 89, asi que el wrapper tiene que correr ahi. */
    backendProc = spawn(cmdName('cargo'), ['run', '--bin', 'glory-backend'], {
        cwd: projectRoot,
        stdio: 'inherit',
        env: { ...process.env },
    });

    backendProc.on('exit', (code) => {
        if (code != null && code !== 0) {
            console.error(`[launch-tauri] Backend Rust terminó con código ${code}`);
        }
    });

    /* Esperar a que el backend esté listo */
    try {
        await waitForPort(backendPort, 600_000); // 10 min — compilación completa puede ser larga
        console.log(`[launch-tauri] Backend listo en puerto ${backendPort}`);
    } catch (err) {
        console.error(`[launch-tauri] ${err.message}. Continuando de todas formas...`);
    }

    /* Lanzar Tauri desktop */
    const tauriProc = spawn(cmdName('npm'), ['run', 'tauri:dev'], {
        cwd: desktopDir,
        stdio: 'inherit',
        shell: isWin,
        env: { ...process.env },
    });

    tauriProc.on('exit', (code) => {
        cleanup();
        process.exit(code ?? 0);
    });

    process.on('SIGINT', () => { cleanup(); tauriProc.kill('SIGINT'); });
    process.on('SIGTERM', () => { cleanup(); tauriProc.kill('SIGTERM'); });
}

main();
