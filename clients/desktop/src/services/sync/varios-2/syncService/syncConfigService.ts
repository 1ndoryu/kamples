import { esDesktop } from '../../../varios-1/desktopService';
import { esSyncEnCurso } from '../../varios-1/syncGuards';
import { logSync } from '../../varios-1/syncLogger';
import { estado, guardarConfig, type SyncConfig } from '../syncState';

/* Configuración */

export async function elegirCarpetaSync(): Promise<string | null> {
    if (!esDesktop()) return null;

    try {
        const { open } = await import('@tauri-apps/plugin-dialog');
        const carpeta = await open({
            directory: true,
            multiple: false,
            title: 'Elegir carpeta de sincronización',
        });

        if (carpeta && typeof carpeta === 'string') {
            estado.config.carpetaLocal = carpeta;
            await guardarConfig();
            return carpeta;
        }
    } catch (err) {
        logSync.error('syncService', 'Error eligiendo carpeta', { error: err instanceof Error ? err.message : String(err) });
    }

    return null;
}

export async function toggleSincronizacion(activa: boolean): Promise<void> {
    estado.config.sincronizacionActiva = activa;
    await guardarConfig();
}

export function obtenerConfigSync(): SyncConfig {
    return { ...estado.config };
}

export function haySyncEnCurso(): boolean {
    return esSyncEnCurso();
}

export async function abrirCarpetaSync(): Promise<boolean> {
    if (!esDesktop()) return false;
    if (!estado.config.carpetaLocal) return false;

    try {
        const { invoke } = await import('@tauri-apps/api/core');
        await invoke('abrir_carpeta', { ruta: estado.config.carpetaLocal });
        return true;
    } catch (err) {
        logSync.error('syncService', 'Error abriendo carpeta local', { error: err instanceof Error ? err.message : String(err) });
        return false;
    }
}
