import { logSync } from '../../services/sync/varios-1/syncLogger';
import { toast } from '@app/stores/varios-4/toastStore';

/* Seleccionar un archivo en el explorador del sistema (resaltado en su carpeta) */
export async function abrirArchivoEnExplorador(rutaLocal: string): Promise<void> {
    try {
        const { invoke } = await import('@tauri-apps/api/core');
        await invoke('seleccionar_archivo', { ruta: rutaLocal });
    } catch {
        /* Silencioso: entorno no-Tauri o error de permisos */
    }
}

/* Abre la ventana independiente de configuración de sync via comando Rust */
export async function abrirVentanaConfig(): Promise<void> {
    try {
        const { invoke } = await import('@tauri-apps/api/core');
        await invoke('mostrar_ventana_config');
    } catch (err) {
        logSync.error('VentanaSincPanel', '[ConfigSync] Error abriendo ventana de configuracion:', err);
        toast.error('No se pudo abrir la configuración de sync');
    }
}
